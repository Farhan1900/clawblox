use nalgebra::{Translation3, UnitQuaternion};
use rapier3d::prelude::*;
use std::collections::{HashMap, HashSet};

use super::constants::physics as consts;
use super::lua::types::{CFrame, PartType, PhysicalProperties, Vector3};

mod assemblies;
mod characters;
mod parts;

pub use characters::{CharacterControllerState, GroundKinematicSupport};
use parts::build_collider_with_physical_properties;

/// Large enough for normal worlds, small enough to keep Rapier broad-phase keys
/// away from overflow and floating-point precision collapse.
pub const PHYSICS_WORLD_ENVELOPE: f32 = 50_000.0;
pub const PHYSICS_VELOCITY_ENVELOPE: f32 = 10_000.0;

fn finite_vec3(values: [f32; 3]) -> bool {
    values.iter().all(|v| v.is_finite())
}

fn within_abs_envelope(values: [f32; 3], envelope: f32) -> bool {
    values.iter().all(|v| v.abs() <= envelope)
}

pub fn transform_within_physics_envelope(position: [f32; 3]) -> bool {
    finite_vec3(position) && within_abs_envelope(position, PHYSICS_WORLD_ENVELOPE)
}

pub fn velocity_within_physics_envelope(velocity: [f32; 3]) -> bool {
    finite_vec3(velocity) && within_abs_envelope(velocity, PHYSICS_VELOCITY_ENVELOPE)
}

fn rotation_matrix_is_finite(rotation: &[[f32; 3]; 3]) -> bool {
    rotation.iter().flatten().all(|v| v.is_finite())
}

/// Converts a 3x3 rotation matrix to a UnitQuaternion (Shepperd's method).
pub fn rotation_matrix_to_quaternion(m: &[[f32; 3]; 3]) -> UnitQuaternion<f32> {
    // Roblox CFrames are rigid transforms: their 3x3 component is orientation,
    // not scale or shear. Normalize here before handing the matrix to Rapier so
    // a bad script/runtime edge cannot turn a part into a scaled physics frame.
    let basis = CFrame {
        position: Vector3::zero(),
        rotation: *m,
    }
    .orthonormalized();
    let r = basis.rotation;
    let rot = nalgebra::Matrix3::new(
        r[0][0], r[0][1], r[0][2], r[1][0], r[1][1], r[1][2], r[2][0], r[2][1], r[2][2],
    );
    let rotation = nalgebra::Rotation3::from_matrix_unchecked(rot);
    UnitQuaternion::from_rotation_matrix(&rotation)
}

/// Converts a UnitQuaternion back to a 3x3 rotation matrix.
pub fn quaternion_to_rotation_matrix(q: &UnitQuaternion<f32>) -> [[f32; 3]; 3] {
    let rot = q.to_rotation_matrix();
    let m = rot.matrix();
    [
        [m[(0, 0)], m[(0, 1)], m[(0, 2)]],
        [m[(1, 0)], m[(1, 1)], m[(1, 2)]],
        [m[(2, 0)], m[(2, 1)], m[(2, 2)]],
    ]
}

// Collision groups for Roblox-style physics behavior
// Characters collide with static geometry and each other.
// Note: rapier3d uses InteractionGroups (not CollisionGroups like bevy_rapier)
const GROUP_STATIC: Group = Group::GROUP_1; // Walls, floors, obstacles
const GROUP_CHARACTER: Group = Group::GROUP_2; // Player characters

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartInteractionState {
    pub can_collide: bool,
    pub can_touch: bool,
    pub can_query: bool,
}

impl PartInteractionState {
    fn collider_enabled(self) -> bool {
        self.can_collide || self.can_touch || self.can_query
    }
}

#[derive(Debug, Clone)]
pub struct PhysicsAssemblyPart {
    pub lua_id: u64,
    pub position: [f32; 3],
    pub rotation: [[f32; 3]; 3],
    pub size: [f32; 3],
    pub anchored: bool,
    pub can_collide: bool,
    pub can_touch: bool,
    pub can_query: bool,
    pub shape: PartType,
    pub physical_properties: Option<PhysicalProperties>,
    pub velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub position_dirty: bool,
    pub cframe_dirty: bool,
    pub velocity_dirty: bool,
    pub angular_velocity_dirty: bool,
}

#[derive(Debug, Clone)]
pub struct PhysicsAssemblySpec {
    pub root_id: u64,
    pub parts: Vec<PhysicsAssemblyPart>,
}

/// Wrapper around Rapier3D physics world for game physics simulation.
/// Syncs with Lua Workspace parts to simulate physics for non-anchored parts.
pub struct PhysicsWorld {
    pub gravity: Vector<Real>,
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: DefaultBroadPhase,
    pub narrow_phase: NarrowPhase,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub query_pipeline: QueryPipeline,

    /// Maps Lua instance ID to Rapier rigid body handle
    pub lua_to_body: HashMap<u64, RigidBodyHandle>,
    /// Maps Rapier rigid body handle to Lua instance ID (reverse lookup)
    pub body_to_lua: HashMap<RigidBodyHandle, u64>,
    /// Maps Lua instance ID to its PartType shape (for collider rebuilds)
    pub lua_to_shape: HashMap<u64, PartType>,
    /// Maps Lua instance ID to current collider physical properties.
    pub lua_to_physical_properties: HashMap<u64, Option<PhysicalProperties>>,
    /// Maps Lua instance ID to current interaction flags used to configure colliders.
    pub lua_to_interactions: HashMap<u64, PartInteractionState>,
    /// Character controllers for player movement
    pub character_controllers: HashMap<u64, CharacterControllerState>,
    /// Maps Rapier collider handle to Lua instance ID (for touch detection)
    pub collider_to_lua: HashMap<ColliderHandle, u64>,
    /// Maps Lua part IDs to their collider handles. Multiple Lua parts can
    /// share one rigid body when rigid welds form a Roblox-style assembly.
    pub lua_to_collider: HashMap<u64, ColliderHandle>,
    /// Body-local transform for each Lua part collider within its physical assembly.
    pub lua_to_local_frame: HashMap<u64, Isometry<Real>>,
    /// Maps a Rapier body to all Lua parts represented by its colliders.
    pub body_to_luas: HashMap<RigidBodyHandle, Vec<u64>>,
    /// Stable structural signatures for physical assemblies. Matching signatures
    /// allow us to preserve Rapier bodies so sleeping and warmstarting work.
    assembly_signatures: HashMap<u64, String>,
    /// Script-driven kinematic linear velocity sampled from per-tick target translation.
    pub kinematic_linear_velocities: HashMap<RigidBodyHandle, [f32; 3]>,
    /// Bodies queued for removal after the current physics step finishes.
    pending_body_removals: Vec<RigidBodyHandle>,
}

fn part_isometry(position: [f32; 3], rotation: &[[f32; 3]; 3]) -> Isometry<Real> {
    Isometry::from_parts(
        Translation3::new(position[0], position[1], position[2]),
        rotation_matrix_to_quaternion(rotation),
    )
}

impl PhysicsWorld {
    fn body_is_within_physics_envelope(&self, handle: RigidBodyHandle) -> bool {
        let Some(body) = self.rigid_body_set.get(handle) else {
            return true;
        };
        let pos = body.translation();
        let linvel = body.linvel();
        let angvel = body.angvel();
        let position = [pos.x, pos.y, pos.z];
        let linear_velocity = [linvel.x, linvel.y, linvel.z];
        let angular_velocity = [angvel.x, angvel.y, angvel.z];

        transform_within_physics_envelope(position)
            && velocity_within_physics_envelope(linear_velocity)
            && velocity_within_physics_envelope(angular_velocity)
    }

    fn remove_body_mappings(&mut self, body_handle: RigidBodyHandle) {
        let root_id = self.body_to_lua.remove(&body_handle);
        if let Some(root_id) = root_id {
            self.assembly_signatures.remove(&root_id);
        }

        let lua_ids = self
            .body_to_luas
            .remove(&body_handle)
            .or_else(|| root_id.map(|id| vec![id]))
            .unwrap_or_default();
        for lua_id in lua_ids {
            self.lua_to_body.remove(&lua_id);
            self.lua_to_collider.remove(&lua_id);
            self.lua_to_local_frame.remove(&lua_id);
            self.lua_to_shape.remove(&lua_id);
            self.lua_to_physical_properties.remove(&lua_id);
            self.lua_to_interactions.remove(&lua_id);
        }

        self.kinematic_linear_velocities.remove(&body_handle);
        if let Some(body) = self.rigid_body_set.get(body_handle) {
            for &collider_handle in body.colliders() {
                self.collider_to_lua.remove(&collider_handle);
            }
        }
    }

    /// Removes bodies whose transforms or velocities have left the finite range
    /// Rapier can robustly simulate. Roblox similarly prevents fallen/far-origin
    /// parts from living forever; this keeps one bad dynamic body from halting
    /// the whole server.
    pub fn purge_invalid_bodies(&mut self) -> usize {
        let handles: Vec<_> = self
            .rigid_body_set
            .iter()
            .filter_map(|(handle, _)| {
                if self.body_is_within_physics_envelope(handle) {
                    None
                } else {
                    Some(handle)
                }
            })
            .collect();

        let removed = handles.len();
        for handle in handles {
            self.remove_body_mappings(handle);
            self.queue_body_removal(handle);
        }
        removed
    }

    fn queryable_collider_predicate(&self) -> impl Fn(ColliderHandle, &Collider) -> bool + '_ {
        move |handle, _collider| {
            self.collider_to_lua
                .get(&handle)
                .and_then(|lua_id| self.lua_to_interactions.get(lua_id))
                .map(|state| state.can_query)
                .unwrap_or(true)
        }
    }

    /// Creates a new physics world with default gravity
    pub fn new() -> Self {
        Self {
            gravity: vector![0.0, -consts::DEFAULT_GRAVITY, 0.0],
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            query_pipeline: QueryPipeline::new(),
            lua_to_body: HashMap::new(),
            body_to_lua: HashMap::new(),
            lua_to_shape: HashMap::new(),
            lua_to_physical_properties: HashMap::new(),
            lua_to_interactions: HashMap::new(),
            character_controllers: HashMap::new(),
            collider_to_lua: HashMap::new(),
            lua_to_collider: HashMap::new(),
            lua_to_local_frame: HashMap::new(),
            body_to_luas: HashMap::new(),
            assembly_signatures: HashMap::new(),
            kinematic_linear_velocities: HashMap::new(),
            pending_body_removals: Vec::new(),
        }
    }

    /// Sets the gravity for the physics world
    pub fn set_gravity(&mut self, gravity_y: f32) {
        self.gravity = vector![0.0, -gravity_y, 0.0];
    }

    /// Steps the physics simulation forward by dt seconds
    pub fn step(&mut self, dt: f32) {
        self.purge_invalid_bodies();
        self.integration_parameters.dt = dt;
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &(),
            &(),
        );
        self.purge_invalid_bodies();
    }

    /// Removes all impulse joints. Constraints are rebuilt from Lua every tick.
    pub fn clear_impulse_joints(&mut self) {
        let handles: Vec<_> = self
            .impulse_joint_set
            .iter()
            .map(|(handle, _)| handle)
            .collect();
        for handle in handles {
            self.impulse_joint_set.remove(handle, true);
        }
    }

    /// Inserts a fixed joint between two rigid bodies using body-local joint frames.
    pub fn insert_fixed_joint(
        &mut self,
        body1: RigidBodyHandle,
        body2: RigidBodyHandle,
        local_frame1: Isometry<Real>,
        local_frame2: Isometry<Real>,
    ) -> ImpulseJointHandle {
        let joint = FixedJointBuilder::new()
            .contacts_enabled(false)
            .local_frame1(local_frame1)
            .local_frame2(local_frame2);
        self.impulse_joint_set.insert(body1, body2, joint, true)
    }

    /// Adds a part to the physics world
    /// - Anchored parts become kinematic (position-based, no physics simulation)
    /// - Non-anchored parts become dynamic (affected by gravity and collisions)
    pub fn add_part(
        &mut self,
        lua_id: u64,
        position: [f32; 3],
        rotation: &[[f32; 3]; 3], // 3x3 rotation matrix
        size: [f32; 3],
        anchored: bool,
        can_collide: bool,
        can_touch: bool,
        can_query: bool,
        shape: PartType,
        physical_properties: Option<PhysicalProperties>,
    ) -> RigidBodyHandle {
        let quat = rotation_matrix_to_quaternion(rotation);

        // Create rigid body
        let body = if anchored {
            RigidBodyBuilder::kinematic_position_based()
        } else {
            RigidBodyBuilder::dynamic()
        }
        .translation(vector![position[0], position[1], position[2]])
        .rotation(quat.scaled_axis())
        .build();

        let handle = self.rigid_body_set.insert(body);

        let interaction_state = PartInteractionState {
            can_collide,
            can_touch,
            can_query,
        };

        // Create collider with correct shape
        let collider = build_collider_with_physical_properties(
            size,
            shape,
            interaction_state,
            physical_properties,
        );

        let collider_handle =
            self.collider_set
                .insert_with_parent(collider, handle, &mut self.rigid_body_set);

        // Store mappings
        self.lua_to_body.insert(lua_id, handle);
        self.body_to_lua.insert(handle, lua_id);
        self.body_to_luas.insert(handle, vec![lua_id]);
        self.lua_to_collider.insert(lua_id, collider_handle);
        self.lua_to_local_frame.insert(lua_id, Isometry::identity());
        self.lua_to_shape.insert(lua_id, shape);
        self.lua_to_physical_properties
            .insert(lua_id, physical_properties);
        self.lua_to_interactions.insert(lua_id, interaction_state);
        self.collider_to_lua.insert(collider_handle, lua_id);

        handle
    }

    /// Removes a part from the physics world
    pub fn remove_part(&mut self, lua_id: u64) -> bool {
        // If this lua_id is a character HRP, clean up the character controller
        // to avoid stale handles that would panic in rapier during step/move.
        if let Some(state) = self.character_controllers.remove(&lua_id) {
            self.lua_to_body.remove(&lua_id);
            self.body_to_lua.remove(&state.body_handle);
            self.body_to_luas.remove(&state.body_handle);
            self.lua_to_collider.remove(&lua_id);
            self.lua_to_local_frame.remove(&lua_id);
            self.collider_to_lua.remove(&state.collider_handle);
            self.kinematic_linear_velocities.remove(&state.body_handle);
            self.queue_body_removal(state.body_handle);
            return true;
        }

        if let Some(&handle) = self.lua_to_body.get(&lua_id) {
            if let Some(root_id) = self.body_to_lua.get(&handle).copied() {
                self.remove_assembly_by_root(root_id);
                return true;
            }
            self.lua_to_body.remove(&lua_id);
            self.lua_to_shape.remove(&lua_id);
            self.lua_to_physical_properties.remove(&lua_id);
            self.lua_to_interactions.remove(&lua_id);
            self.lua_to_collider.remove(&lua_id);
            self.lua_to_local_frame.remove(&lua_id);
            self.kinematic_linear_velocities.remove(&handle);
            // Remove collider->lua mappings before disabling and queuing the body.
            if let Some(body) = self.rigid_body_set.get(handle) {
                for &ch in body.colliders() {
                    self.collider_to_lua.remove(&ch);
                }
            }
            self.queue_body_removal(handle);
            true
        } else {
            false
        }
    }

    fn queue_body_removal(&mut self, handle: RigidBodyHandle) {
        if let Some(body) = self.rigid_body_set.get_mut(handle) {
            body.set_enabled(false);
            let colliders: Vec<_> = body.colliders().iter().copied().collect();
            for collider_handle in colliders {
                if let Some(collider) = self.collider_set.get_mut(collider_handle) {
                    collider.set_enabled(false);
                }
            }
        }

        if !self.pending_body_removals.contains(&handle) {
            self.pending_body_removals.push(handle);
        }
    }

    /// Finalize body removals that were deferred until after the physics step.
    pub fn flush_pending_removals(&mut self) {
        let pending = std::mem::take(&mut self.pending_body_removals);
        for handle in pending {
            let _ = self.rigid_body_set.remove(
                handle,
                &mut self.island_manager,
                &mut self.collider_set,
                &mut self.impulse_joint_set,
                &mut self.multibody_joint_set,
                true,
            );
        }
        self.island_manager
            .cleanup_removed_rigid_bodies(&mut self.rigid_body_set);
    }

    /// Updates the position of an anchored (kinematic) part
    pub fn set_kinematic_position_with_dt(
        &mut self,
        handle: RigidBodyHandle,
        position: [f32; 3],
        dt: f32,
    ) {
        if !transform_within_physics_envelope(position) {
            self.remove_body_mappings(handle);
            self.queue_body_removal(handle);
            return;
        }
        if let Some(body) = self.rigid_body_set.get_mut(handle) {
            if body.is_kinematic() {
                let cur = body.translation();
                let inv_dt = if dt > consts::EPSILON { 1.0 / dt } else { 0.0 };
                self.kinematic_linear_velocities.insert(
                    handle,
                    [
                        (position[0] - cur.x) * inv_dt,
                        (position[1] - cur.y) * inv_dt,
                        (position[2] - cur.z) * inv_dt,
                    ],
                );
                body.set_next_kinematic_translation(vector![position[0], position[1], position[2]]);
            }
        }
    }

    /// Updates the position of an anchored (kinematic) part using default tick dt.
    pub fn set_kinematic_position(&mut self, handle: RigidBodyHandle, position: [f32; 3]) {
        self.set_kinematic_position_with_dt(handle, position, consts::TIMESTEP);
    }

    /// Updates the rotation of an anchored (kinematic) part from a 3x3 rotation matrix
    pub fn set_kinematic_rotation(&mut self, handle: RigidBodyHandle, rotation: &[[f32; 3]; 3]) {
        if !rotation_matrix_is_finite(rotation) {
            self.remove_body_mappings(handle);
            self.queue_body_removal(handle);
            return;
        }
        if let Some(body) = self.rigid_body_set.get_mut(handle) {
            if body.is_kinematic() {
                let quat = rotation_matrix_to_quaternion(rotation);
                body.set_next_kinematic_rotation(quat);
            }
        }
    }

    /// Teleports a dynamic body to a world position.
    pub fn set_dynamic_position(&mut self, handle: RigidBodyHandle, position: [f32; 3]) {
        if !transform_within_physics_envelope(position) {
            self.remove_body_mappings(handle);
            self.queue_body_removal(handle);
            return;
        }
        if let Some(body) = self.rigid_body_set.get_mut(handle) {
            if body.is_dynamic() {
                body.set_translation(vector![position[0], position[1], position[2]], true);
            }
        }
    }

    /// Sets a dynamic body's world orientation from a 3x3 rotation matrix.
    pub fn set_dynamic_rotation(&mut self, handle: RigidBodyHandle, rotation: &[[f32; 3]; 3]) {
        if !rotation_matrix_is_finite(rotation) {
            self.remove_body_mappings(handle);
            self.queue_body_removal(handle);
            return;
        }
        if let Some(body) = self.rigid_body_set.get_mut(handle) {
            if body.is_dynamic() {
                let quat = rotation_matrix_to_quaternion(rotation);
                body.set_rotation(quat, true);
            }
        }
    }

    /// Sets the velocity of a dynamic part
    pub fn set_velocity(&mut self, handle: RigidBodyHandle, velocity: [f32; 3]) {
        if !velocity_within_physics_envelope(velocity) {
            self.remove_body_mappings(handle);
            self.queue_body_removal(handle);
            return;
        }
        if let Some(body) = self.rigid_body_set.get_mut(handle) {
            if body.is_dynamic() {
                body.set_linvel(vector![velocity[0], velocity[1], velocity[2]], true);
            }
        }
    }

    /// Sets the angular velocity of a dynamic part
    pub fn set_angular_velocity(&mut self, handle: RigidBodyHandle, angular_velocity: [f32; 3]) {
        if !velocity_within_physics_envelope(angular_velocity) {
            self.remove_body_mappings(handle);
            self.queue_body_removal(handle);
            return;
        }
        if let Some(body) = self.rigid_body_set.get_mut(handle) {
            if body.is_dynamic() {
                body.set_angvel(
                    vector![
                        angular_velocity[0],
                        angular_velocity[1],
                        angular_velocity[2]
                    ],
                    true,
                );
            }
        }
    }

    /// Gets the position of a rigid body
    pub fn get_position(&self, handle: RigidBodyHandle) -> Option<[f32; 3]> {
        self.rigid_body_set.get(handle).map(|body| {
            let pos = body.translation();
            [pos.x, pos.y, pos.z]
        })
    }

    /// Gets the rotation of a rigid body as quaternion [x, y, z, w]
    pub fn get_rotation(&self, handle: RigidBodyHandle) -> Option<[f32; 4]> {
        self.rigid_body_set.get(handle).map(|body| {
            let rot = body.rotation();
            [rot.i, rot.j, rot.k, rot.w]
        })
    }

    /// Gets the rotation of a rigid body as a 3x3 rotation matrix
    pub fn get_rotation_matrix(&self, handle: RigidBodyHandle) -> Option<[[f32; 3]; 3]> {
        self.rigid_body_set
            .get(handle)
            .map(|body| quaternion_to_rotation_matrix(body.rotation()))
    }

    fn get_part_world_isometry(&self, lua_id: u64) -> Option<Isometry<Real>> {
        let body_handle = *self.lua_to_body.get(&lua_id)?;
        let body = self.rigid_body_set.get(body_handle)?;
        let local = self
            .lua_to_local_frame
            .get(&lua_id)
            .copied()
            .unwrap_or_else(Isometry::identity);
        Some(*body.position() * local)
    }

    /// Projects a Roblox-style CFrame write to one member of a welded assembly
    /// across every Lua part represented by the same physical body.
    ///
    /// Setting BasePart.CFrame on a welded member moves the assembly so that the
    /// written member reaches the requested frame while all weld offsets are
    /// preserved. This helper returns the resulting world frames for every Lua
    /// part in the current physical assembly.
    pub fn project_assembly_cframe_write(
        &self,
        lua_id: u64,
        position: [f32; 3],
        rotation: &[[f32; 3]; 3],
    ) -> Option<HashMap<u64, ([f32; 3], [[f32; 3]; 3])>> {
        let body_handle = *self.lua_to_body.get(&lua_id)?;
        let local = self
            .lua_to_local_frame
            .get(&lua_id)
            .copied()
            .unwrap_or_else(Isometry::identity);
        let target_member_iso = part_isometry(position, rotation);
        let target_body_iso = target_member_iso * local.inverse();
        let lua_ids = self.body_to_luas.get(&body_handle)?;

        let mut out = HashMap::with_capacity(lua_ids.len());
        for part_id in lua_ids {
            let part_local = self
                .lua_to_local_frame
                .get(part_id)
                .copied()
                .unwrap_or_else(Isometry::identity);
            let part_iso = target_body_iso * part_local;
            let pos = part_iso.translation.vector;
            out.insert(
                *part_id,
                (
                    [pos.x, pos.y, pos.z],
                    quaternion_to_rotation_matrix(&part_iso.rotation),
                ),
            );
        }
        Some(out)
    }

    pub fn get_part_position(&self, lua_id: u64) -> Option<[f32; 3]> {
        self.get_part_world_isometry(lua_id).map(|iso| {
            let pos = iso.translation.vector;
            [pos.x, pos.y, pos.z]
        })
    }

    pub fn get_part_rotation_matrix(&self, lua_id: u64) -> Option<[[f32; 3]; 3]> {
        self.get_part_world_isometry(lua_id)
            .map(|iso| quaternion_to_rotation_matrix(&iso.rotation))
    }

    pub fn get_part_velocity(&self, lua_id: u64) -> Option<[f32; 3]> {
        let body_handle = *self.lua_to_body.get(&lua_id)?;
        let body = self.rigid_body_set.get(body_handle)?;
        let part_iso = self.get_part_world_isometry(lua_id)?;
        let point = point![
            part_iso.translation.vector.x,
            part_iso.translation.vector.y,
            part_iso.translation.vector.z
        ];
        let vel = body.velocity_at_point(&point);
        Some([vel.x, vel.y, vel.z])
    }

    /// Gets the velocity of a rigid body
    pub fn get_velocity(&self, handle: RigidBodyHandle) -> Option<[f32; 3]> {
        self.rigid_body_set.get(handle).map(|body| {
            let vel = body.linvel();
            [vel.x, vel.y, vel.z]
        })
    }

    /// Gets the angular velocity of a rigid body
    pub fn get_angular_velocity(&self, handle: RigidBodyHandle) -> Option<[f32; 3]> {
        self.rigid_body_set.get(handle).map(|body| {
            let vel = body.angvel();
            [vel.x, vel.y, vel.z]
        })
    }

    /// Checks if a Lua instance has a physics body
    pub fn has_part(&self, lua_id: u64) -> bool {
        self.lua_to_body.contains_key(&lua_id)
    }

    /// Gets all Lua part IDs that have physics bodies
    pub fn get_all_part_ids(&self) -> Vec<u64> {
        self.lua_to_body.keys().copied().collect()
    }

    /// Gets the handle for a Lua instance
    pub fn get_handle(&self, lua_id: u64) -> Option<RigidBodyHandle> {
        self.lua_to_body.get(&lua_id).copied()
    }

    pub fn dynamic_body_count(&self) -> usize {
        self.rigid_body_set
            .iter()
            .filter(|(_, body)| body.is_dynamic())
            .count()
    }

    pub fn awake_dynamic_body_count(&self) -> usize {
        self.rigid_body_set
            .iter()
            .filter(|(_, body)| body.is_dynamic() && !body.is_sleeping())
            .count()
    }

    pub fn is_dynamic_body(&self, handle: RigidBodyHandle) -> bool {
        self.rigid_body_set
            .get(handle)
            .map(|body| body.is_dynamic())
            .unwrap_or(false)
    }

    /// Updates whether a part is anchored or dynamic
    pub fn set_anchored(&mut self, lua_id: u64, anchored: bool) {
        if let Some(&handle) = self.lua_to_body.get(&lua_id) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                if anchored {
                    body.set_body_type(RigidBodyType::KinematicPositionBased, true);
                    self.kinematic_linear_velocities
                        .insert(handle, [0.0, 0.0, 0.0]);
                } else {
                    body.set_body_type(RigidBodyType::Dynamic, true);
                    self.kinematic_linear_velocities.remove(&handle);
                }
            }
        }
    }

    /// Checks if there is a clear line of sight between two positions
    /// Returns true if no obstacle blocks the view
    pub fn has_line_of_sight(
        &self,
        from: [f32; 3],
        to: [f32; 3],
        exclude_body: Option<RigidBodyHandle>,
        target_body: Option<RigidBodyHandle>,
    ) -> bool {
        let direction = vector![to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let max_dist = direction.magnitude();

        if max_dist < 0.001 {
            return true; // Same position
        }

        let normalized = direction / max_dist;
        let ray = Ray::new(point![from[0], from[1], from[2]], normalized);

        let can_query = self.queryable_collider_predicate();
        let filter = if let Some(body_handle) = exclude_body {
            QueryFilter::default().exclude_rigid_body(body_handle)
        } else {
            QueryFilter::default()
        }
        .predicate(&can_query);

        // Cast ray and check if we hit something before reaching the target.
        // For Roblox-style LOS we consider the target visible when the first hit
        // belongs to the target character body.
        if let Some((hit_collider, _hit_dist)) = self.query_pipeline.cast_ray(
            &self.rigid_body_set,
            &self.collider_set,
            &ray,
            max_dist,
            true, // solid
            filter,
        ) {
            if let Some(expected_body) = target_body {
                if let Some(collider) = self.collider_set.get(hit_collider) {
                    if collider.parent() == Some(expected_body) {
                        return true;
                    }
                }
            }
            false
        } else {
            true // No obstacle hit
        }
    }

    /// Detects all overlapping pairs of bodies in the physics world.
    /// Returns a set of normalized (min, max) Lua ID pairs that are currently overlapping.
    /// Uses `intersections_with_shape` which works for all body type combinations
    /// (including kinematic+kinematic which narrow_phase misses).
    pub fn detect_overlaps(&self) -> HashSet<(u64, u64)> {
        let mut overlaps = HashSet::new();

        for (&ch, &lua_id) in &self.collider_to_lua {
            let Some(collider) = self.collider_set.get(ch) else {
                continue;
            };
            let parent_body = collider.parent();
            let shape = collider.shape();
            let pos = collider.position();

            let filter = QueryFilter::default().exclude_collider(ch);

            self.query_pipeline.intersections_with_shape(
                &self.rigid_body_set,
                &self.collider_set,
                pos,
                shape,
                filter,
                |other_collider_handle| {
                    let Some(&other_lua_id) = self.collider_to_lua.get(&other_collider_handle)
                    else {
                        return true;
                    };
                    if other_lua_id == lua_id {
                        return true;
                    }
                    if let Some(other_collider) = self.collider_set.get(other_collider_handle) {
                        if other_collider.parent() == parent_body {
                            return true;
                        }
                    }
                    let pair = if lua_id < other_lua_id {
                        (lua_id, other_lua_id)
                    } else {
                        (other_lua_id, lua_id)
                    };
                    overlaps.insert(pair);
                    true
                },
            );
        }

        overlaps
    }

    /// Returns active contact/intersection pairs already computed by Rapier's
    /// narrow phase during the physics step. This is the Roblox-like hot path
    /// for Touched/TouchEnded: consume contact state produced by simulation
    /// instead of polling every collider against the whole world each frame.
    pub fn detect_narrow_phase_touches(&self) -> HashSet<(u64, u64)> {
        let mut touches = HashSet::new();

        let mut insert_pair = |collider_a, collider_b| {
            let Some(&lua_a) = self.collider_to_lua.get(&collider_a) else {
                return;
            };
            let Some(&lua_b) = self.collider_to_lua.get(&collider_b) else {
                return;
            };
            if lua_a == lua_b {
                return;
            }

            if let (Some(a), Some(b)) = (
                self.collider_set.get(collider_a),
                self.collider_set.get(collider_b),
            ) {
                if a.parent().is_some() && a.parent() == b.parent() {
                    return;
                }
            }

            if lua_a < lua_b {
                touches.insert((lua_a, lua_b));
            } else {
                touches.insert((lua_b, lua_a));
            }
        };

        for pair in self.narrow_phase.contact_pairs() {
            if pair.has_any_active_contact {
                insert_pair(pair.collider1, pair.collider2);
            }
        }

        for (collider_a, collider_b, intersecting) in self.narrow_phase.intersection_pairs() {
            if intersecting {
                insert_pair(collider_a, collider_b);
            }
        }

        touches
    }

    /// Casts a ray downward from a position to detect ground
    /// Returns (hit_distance, hit_y) if ground is found within max_distance
    pub fn raycast_down(
        &self,
        origin: [f32; 3],
        max_distance: f32,
        exclude_body: Option<RigidBodyHandle>,
    ) -> Option<(f32, f32)> {
        let ray = Ray::new(
            point![origin[0], origin[1], origin[2]],
            vector![0.0, -1.0, 0.0],
        );

        let can_query = self.queryable_collider_predicate();
        let filter = if let Some(body_handle) = exclude_body {
            QueryFilter::default().exclude_rigid_body(body_handle)
        } else {
            QueryFilter::default()
        }
        .predicate(&can_query);

        if let Some((_, hit)) = self.query_pipeline.cast_ray(
            &self.rigid_body_set,
            &self.collider_set,
            &ray,
            max_distance,
            true, // solid
            filter,
        ) {
            let hit_point = ray.point_at(hit);
            Some((hit, hit_point.y))
        } else {
            None
        }
    }

}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physics_world_creation() {
        let world = PhysicsWorld::new();
        assert_eq!(world.gravity.y, -consts::DEFAULT_GRAVITY);
    }

    #[test]
    fn test_add_anchored_part() {
        let mut world = PhysicsWorld::new();

        let handle = world.add_part(
            1,
            [0.0, 10.0, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [4.0, 1.0, 2.0],
            true, // anchored
            true, // can_collide
            true,
            true,
            PartType::Block,
            None,
        );

        assert!(world.has_part(1));
        let pos = world.get_position(handle).unwrap();
        assert_eq!(pos, [0.0, 10.0, 0.0]);
    }

    #[test]
    fn test_dynamic_part_falls() {
        let mut world = PhysicsWorld::new();

        let handle = world.add_part(
            1,
            [0.0, 10.0, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [1.0, 1.0, 1.0],
            false, // not anchored - should fall
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        let initial_pos = world.get_position(handle).unwrap();

        // Step physics a few times
        for _ in 0..10 {
            world.step(1.0 / 60.0);
        }

        let final_pos = world.get_position(handle).unwrap();

        // Y position should be lower due to gravity
        assert!(final_pos[1] < initial_pos[1]);
    }

    #[test]
    fn test_character_raycast_and_move() {
        let mut world = PhysicsWorld::new();

        // Add a floor
        world.add_part(
            1,
            [0.0, 0.0, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [100.0, 1.0, 100.0],
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        // Add character above floor
        let char_id = 100;
        world.add_character(char_id, [0.0, 6.0, 0.0], 1.0, 5.0);

        world.step(1.0 / 60.0);
        world.query_pipeline.update(&world.collider_set);

        // Test raycast finds ground
        let pos = world.get_character_position(char_id).unwrap();
        let state = world.get_character_state(char_id).unwrap();
        let hit = world.raycast_down(pos, 10.0, Some(state.body_handle));
        assert!(hit.is_some(), "Should detect floor");

        let (distance, ground_y) = hit.unwrap();
        // Character at Y=6, floor top at Y=0.5, distance = 5.5
        assert!(
            distance > 5.0 && distance < 6.0,
            "Distance to floor should be ~5.5, got {}",
            distance
        );
        assert!(
            (ground_y - 0.5).abs() < 0.1,
            "Ground Y should be ~0.5 (floor top), got {}",
            ground_y
        );

        // Test movement with controller translation (should not suppress horizontal)
        if let Some(state) = world.get_character_state_mut(char_id) {
            state.grounded = true;
        }
        let movement = world
            .move_character(char_id, [1.0, -0.05, 0.0], 1.0 / 60.0)
            .unwrap();
        world.step(1.0 / 60.0);

        let final_pos = world.get_character_position(char_id).unwrap();
        assert!(
            movement.translation.x.abs() > 0.0,
            "Horizontal movement should be applied"
        );
        assert!(final_pos[0] > 0.5, "Should have moved in X");
    }

    #[test]
    fn test_character_horizontal_movement_when_grounded() {
        let mut world = PhysicsWorld::new();

        world.add_part(
            1,
            [0.0, 0.0, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [100.0, 1.0, 100.0],
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        let char_id = 200;
        world.add_character(char_id, [0.0, 3.0, 0.0], 1.0, 5.0);
        if let Some(state) = world.get_character_state_mut(char_id) {
            state.grounded = true;
        }

        let movement = world
            .move_character(char_id, [1.0, -0.05, 0.0], 1.0 / 60.0)
            .unwrap();
        assert!(
            movement.translation.x.abs() > 0.0 || movement.translation.z.abs() > 0.0,
            "Horizontal movement should not be suppressed when grounded"
        );
    }

    #[test]
    fn test_characters_block_each_other() {
        let mut world = PhysicsWorld::new();

        world.add_part(
            1,
            [0.0, -0.5, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [100.0, 1.0, 100.0],
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        let char_a = 300;
        let char_b = 301;
        world.add_character(char_a, [0.0, 2.6, 0.0], 1.0, 5.0);
        world.add_character(char_b, [2.6, 2.6, 0.0], 1.0, 5.0);

        if let Some(state) = world.get_character_state_mut(char_a) {
            state.grounded = true;
        }
        if let Some(state) = world.get_character_state_mut(char_b) {
            state.grounded = true;
        }

        let dt = 1.0 / 60.0;
        world.step(dt);
        world.query_pipeline.update(&world.collider_set);

        let movement = world.move_character(char_a, [1.0, 0.0, 0.0], dt).unwrap();
        world.step(dt);

        let final_pos = world.get_character_position(char_a).unwrap();
        assert!(
            movement.translation.x < 0.55,
            "Character should be blocked by another character, got {:?}",
            movement.translation
        );
        assert!(
            final_pos[0] < 0.55,
            "Character should not move through another character, final_pos={:?}",
            final_pos
        );
    }

    #[test]
    fn test_character_ignores_sensor_colliders() {
        let mut world = PhysicsWorld::new();

        // Floor so the character can move while grounded.
        world.add_part(
            1,
            [0.0, -0.5, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [100.0, 1.0, 100.0],
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        // Non-collidable "trigger" directly in the movement path.
        world.add_part(
            2,
            [0.0, 1.0, -8.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [8.0, 2.0, 8.0],
            true,
            false, // sensor
            true,
            true,
            PartType::Block,
            None,
        );

        let char_id = 300;
        world.add_character(char_id, [0.0, 2.6, 8.0], 1.0, 5.0);

        let dt = 1.0 / 60.0;
        for _ in 0..120 {
            world.query_pipeline.update(&world.collider_set);
            world.move_character(char_id, [0.0, 0.0, -0.3], dt).unwrap();
            world.step(dt);
        }

        let final_pos = world.get_character_position(char_id).unwrap();
        assert!(
            final_pos[2] < -6.0,
            "Character should pass through CanCollide=false sensor. Final z={}",
            final_pos[2]
        );
    }

    #[test]
    fn test_get_ground_kinematic_velocity() {
        let mut world = PhysicsWorld::new();

        // Moving platform under the character.
        let platform = world.add_part(
            1,
            [0.0, 0.0, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [10.0, 1.0, 10.0],
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        let char_id = 301;
        world.add_character(char_id, [0.0, 2.6, 0.0], 1.0, 5.0);

        let dt = 1.0 / 60.0;
        world.step(dt);
        world.query_pipeline.update(&world.collider_set);

        // Move the kinematic platform upward one step.
        world.set_kinematic_position(platform, [0.0, 0.2, 0.0]);
        world.step(dt);
        world.query_pipeline.update(&world.collider_set);

        let v = world
            .get_ground_kinematic_velocity(char_id, 4.0)
            .expect("Expected supporting kinematic velocity");
        assert!(
            v[1] > 0.0,
            "Platform upward velocity should be positive, got {:?}",
            v
        );
    }

    #[test]
    fn test_get_character_contact_kinematic_velocity_for_rotating_body() {
        let mut world = PhysicsWorld::new();

        let spinner = world.add_part(
            1,
            [0.0, 2.0, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [16.0, 4.0, 3.0],
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        let char_id = 302;
        world.add_character(char_id, [0.0, 2.0, 2.0], 1.0, 5.0);

        let dt = 1.0 / 60.0;
        world.step(dt);
        world.query_pipeline.update(&world.collider_set);

        let theta = 0.5_f32;
        let c = theta.cos();
        let s = theta.sin();
        let rot_y = [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]];
        world.set_kinematic_rotation(spinner, &rot_y);
        world.step(dt);
        world.query_pipeline.update(&world.collider_set);

        let v = world
            .get_character_contact_kinematic_velocity(char_id)
            .expect("Expected contact velocity from rotating kinematic body");
        let horiz = (v[0] * v[0] + v[2] * v[2]).sqrt();
        assert!(
            horiz > 0.1,
            "Expected meaningful horizontal contact velocity, got {:?}",
            v
        );
    }

    #[test]
    fn test_character_movement_after_landing_on_thin_platform() {
        let mut world = PhysicsWorld::new();

        // Floor at Y=0 (like the game)
        world.add_part(
            1,
            [0.0, -1.0, 0.0], // center at Y=-1
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [100.0, 2.0, 100.0], // top at Y=0
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        // Thin platform at Y=0.1 (like base platform in tsunami game)
        world.add_part(
            2,
            [0.0, 0.1, 0.0], // center at Y=0.1
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [30.0, 0.2, 30.0], // top at Y=0.2
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        // Character spawns at Y=3 (above platform)
        let char_id = 100;
        world.add_character(char_id, [0.0, 3.0, 0.0], 1.0, 5.0);

        // Simulate falling (like the game does)
        let dt = 1.0 / 60.0;
        for _ in 0..30 {
            // About 0.5 seconds of falling
            world.query_pipeline.update(&world.collider_set);
            let gravity_movement: f32 = -196.2 * dt * dt; // Gravity acceleration
            world.move_character(char_id, [0.0, gravity_movement.max(-0.2), 0.0], dt);
            world.step(dt);
        }

        // Character should now be grounded
        let pos_after_landing = world.get_character_position(char_id).unwrap();
        println!("Position after landing: {:?}", pos_after_landing);

        // Try horizontal movement (like the game does)
        world.query_pipeline.update(&world.collider_set);
        let movement = world
            .move_character(char_id, [0.26, -0.02, 0.0], dt)
            .unwrap();

        println!("Movement result: {:?}", movement.translation);

        // This should NOT be blocked
        assert!(
            movement.translation.x.abs() > 0.1,
            "Horizontal movement should work after landing. Got: {:?}",
            movement.translation
        );
    }

    #[test]
    fn test_character_direction_reversal() {
        let mut world = PhysicsWorld::new();

        // Floor at Y=0 (top surface)
        world.add_part(
            1,
            [0.0, -1.0, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [200.0, 2.0, 200.0],
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        // Character with radius=1.0, height=5.0 (same as game)
        // Capsule: half_height=1.5, radius=1.0
        // Bottom of capsule is at Y = center - 1.5 - 1.0 = center - 2.5
        // For bottom to be at Y=0 (floor top), center should be at Y=2.5
        // Add small margin: Y=2.6
        let char_id = 100;
        world.add_character(char_id, [0.0, 2.6, 0.0], 1.0, 5.0);

        let dt = 1.0 / 60.0;

        // Just update the query pipeline, don't fall
        world.step(dt);
        world.query_pipeline.update(&world.collider_set);

        let pos_after_landing = world.get_character_position(char_id).unwrap();
        println!("After landing: {:?}", pos_after_landing);

        // Move +X for 20 frames
        // When grounded, desired_y should be 0 (game logic zeros it)
        println!("\n=== Moving +X ===");
        let mut grounded = false;
        for i in 0..20 {
            world.query_pipeline.update(&world.collider_set);
            // When grounded, zero vertical like the game does
            let desired_y = if grounded { 0.0 } else { -0.008 };
            let movement = world
                .move_character(char_id, [0.2, desired_y, 0.0], dt)
                .unwrap();
            grounded = movement.grounded;
            world.step(dt);
            if i < 5 || movement.translation.x.abs() < 0.01 {
                let pos = world.get_character_position(char_id).unwrap();
                println!(
                    "  Frame {}: pos={:.2} applied_x={:.3} grounded={}",
                    i, pos[0], movement.translation.x, movement.grounded
                );
            }
        }

        let pos_after_plus_x = world.get_character_position(char_id).unwrap();
        println!("After +X: {:?}", pos_after_plus_x);
        assert!(pos_after_plus_x[0] > 2.0, "+X movement should work");

        // Now reverse: move -X for 20 frames
        println!("\n=== Moving -X (reversal) ===");
        let mut stuck_count = 0;
        for i in 0..20 {
            world.query_pipeline.update(&world.collider_set);
            // When grounded, zero vertical like the game does
            let desired_y = if grounded { 0.0 } else { -0.008 };
            let movement = world
                .move_character(char_id, [-0.2, desired_y, 0.0], dt)
                .unwrap();
            grounded = movement.grounded;
            world.step(dt);
            let pos = world.get_character_position(char_id).unwrap();
            println!(
                "  Frame {}: pos={:.2} applied_x={:.3} grounded={}",
                i, pos[0], movement.translation.x, movement.grounded
            );
            if movement.translation.x.abs() < 0.01 {
                stuck_count += 1;
            }
        }

        let pos_after_minus_x = world.get_character_position(char_id).unwrap();
        println!("After -X: {:?}", pos_after_minus_x);

        // Should have moved back toward 0
        assert!(
            pos_after_minus_x[0] < pos_after_plus_x[0] - 1.0,
            "Direction reversal should work! pos_after_plus_x={} pos_after_minus_x={} stuck_frames={}",
            pos_after_plus_x[0], pos_after_minus_x[0], stuck_count
        );
    }

    #[test]
    fn test_character_autostep_over_small_obstacle() {
        let mut world = PhysicsWorld::new();

        // Floor
        world.add_part(
            1,
            [0.0, -0.5, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [100.0, 1.0, 100.0], // top at Y=0
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        // Small obstacle (0.3 studs tall, should be steppable with max_height=0.5)
        world.add_part(
            2,
            [5.0, 0.15, 0.0], // center at Y=0.15
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [1.0, 0.3, 4.0], // top at Y=0.3
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        // Character starts at X=0, grounded
        let char_id = 100;
        world.add_character(char_id, [0.0, 2.6, 0.0], 1.0, 5.0);

        let dt = 1.0 / 60.0;
        world.step(dt);
        world.query_pipeline.update(&world.collider_set);

        let start_pos = world.get_character_position(char_id).unwrap();
        println!("Start: {:?}", start_pos);

        // Move toward and over the obstacle
        for i in 0..60 {
            world.query_pipeline.update(&world.collider_set);
            let movement = world.move_character(char_id, [0.2, 0.0, 0.0], dt).unwrap();
            world.step(dt);

            let pos = world.get_character_position(char_id).unwrap();
            if i % 10 == 0 || (pos[0] > 4.0 && pos[0] < 6.0) {
                println!(
                    "Frame {}: pos=({:.2}, {:.2}, {:.2}) grounded={}",
                    i, pos[0], pos[1], pos[2], movement.grounded
                );
            }
        }

        let final_pos = world.get_character_position(char_id).unwrap();
        println!("Final: {:?}", final_pos);

        // Character should have moved past the obstacle (X > 6)
        assert!(
            final_pos[0] > 6.0,
            "Character should autostep over 0.3 stud obstacle. Final X={:.2}",
            final_pos[0]
        );
    }

    #[test]
    fn test_character_blocked_by_tall_obstacle() {
        let mut world = PhysicsWorld::new();

        // Floor
        world.add_part(
            1,
            [0.0, -0.5, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [100.0, 1.0, 100.0],
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        // Tall obstacle (1.0 stud tall, should block with max_height=0.5)
        world.add_part(
            2,
            [5.0, 0.5, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [1.0, 1.0, 4.0], // top at Y=1.0
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        let char_id = 100;
        world.add_character(char_id, [0.0, 2.6, 0.0], 1.0, 5.0);

        let dt = 1.0 / 60.0;
        world.step(dt);
        world.query_pipeline.update(&world.collider_set);

        // Move toward the obstacle
        for _ in 0..60 {
            world.query_pipeline.update(&world.collider_set);
            world.move_character(char_id, [0.2, 0.0, 0.0], dt);
            world.step(dt);
        }

        let final_pos = world.get_character_position(char_id).unwrap();
        println!("Final pos with tall obstacle: {:?}", final_pos);

        // Character should be blocked (X < 5, can't step over 1.0 stud obstacle)
        assert!(
            final_pos[0] < 5.0,
            "Character should be blocked by 1.0 stud obstacle. Final X={:.2}",
            final_pos[0]
        );
    }

    #[test]
    fn test_character_step_up_onto_platform() {
        // Mimics tsunami scenario: ground + raised platform
        let mut world = PhysicsWorld::new();

        // Ground (lower level) - extends UNDER the platform to avoid seam
        world.add_part(
            1,
            [0.0, -0.5, 0.0], // center
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [200.0, 1.0, 100.0], // top at Y=0, extends from X=-100 to X=100
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        // Raised platform (like tsunami base) - sits ON TOP of ground
        world.add_part(
            2,
            [50.0, 0.1, 0.0], // center at Y=0.1
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [100.0, 0.2, 100.0], // top at Y=0.2, bottom at Y=0, X from 0 to 100
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        // Character starts on ground (left side), will try to step up onto platform
        let char_id = 100;
        world.add_character(char_id, [-5.0, 2.6, 0.0], 1.0, 5.0);

        let dt = 1.0 / 60.0;
        world.step(dt);
        world.query_pipeline.update(&world.collider_set);

        let start_pos = world.get_character_position(char_id).unwrap();
        println!("Start (on ground): {:?}", start_pos);

        // Move +X toward and onto the platform
        for i in 0..40 {
            world.query_pipeline.update(&world.collider_set);
            let movement = world.move_character(char_id, [0.2, 0.0, 0.0], dt).unwrap();
            world.step(dt);

            let pos = world.get_character_position(char_id).unwrap();
            if i % 5 == 0 || (pos[0] > -2.0 && pos[0] < 5.0) {
                println!(
                    "Frame {}: pos=({:.2}, {:.2}, {:.2}) grounded={}",
                    i, pos[0], pos[1], pos[2], movement.grounded
                );
            }
        }

        let final_pos = world.get_character_position(char_id).unwrap();
        println!("Final: {:?}", final_pos);

        // Character should step up onto platform (X > 2, past the edge at X=0)
        assert!(
            final_pos[0] > 2.0,
            "Character should step UP onto 0.2 stud platform. Final X={:.2}",
            final_pos[0]
        );
    }

    /// Reproduces the crash from run 20260331T191258Z:
    /// When a character's HRP is removed via `remove_part()` (orphan cleanup),
    /// the body is destroyed in rapier but `character_controllers` still holds
    /// the stale handle. The next `move_character` or `step` then panics with
    /// "No element at index".
    #[test]
    fn test_remove_part_on_character_hrp_causes_stale_handle() {
        let mut world = PhysicsWorld::new();

        // Ground
        world.add_part(
            1,
            [0.0, -0.5, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [200.0, 1.0, 200.0],
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        // Add a character (like a player HRP)
        let char_id: u64 = 100;
        world.add_character(char_id, [0.0, 1.0, 0.0], 0.5, 2.0);
        assert!(world.has_character(char_id));

        // Simulate orphan cleanup: remove_part is called on the character's lua_id
        // This is what happens when sync_lua_to_physics finds the HRP gone from Lua
        let removed = world.remove_part(char_id);
        assert!(removed, "remove_part should find and remove the body");

        // BUG: character_controllers still has the entry with the stale handle
        assert!(
            !world.has_character(char_id),
            "character_controllers should be cleaned up when remove_part destroys a character body"
        );

        // If the assertion above is removed, the following would panic:
        // world.move_character(char_id, [1.0, 0.0, 0.0], 1.0 / 60.0);
        // world.step(1.0 / 60.0);
    }

    #[test]
    fn test_remove_part_after_same_frame_mutation_does_not_panic_on_step() {
        let mut world = PhysicsWorld::new();

        let lua_id = 1;
        let handle = world.add_part(
            lua_id,
            [0.0, 1.0, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [1.0, 1.0, 1.0],
            false,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        world.set_dynamic_position(handle, [1.0, 2.0, 3.0]);
        assert!(world.remove_part(lua_id));

        world.step(1.0 / 60.0);
        world.flush_pending_removals();

        assert!(!world.has_part(lua_id));
        assert!(world.get_handle(lua_id).is_none());
    }

    #[test]
    fn test_remove_character_after_same_frame_mutation_does_not_panic_on_step() {
        let mut world = PhysicsWorld::new();

        let lua_id = 100;
        world.add_character(lua_id, [0.0, 3.0, 0.0], 1.0, 5.0);
        world.set_character_position(lua_id, [2.0, 4.0, 0.0]);
        assert!(world.remove_character(lua_id));

        world.step(1.0 / 60.0);
        world.flush_pending_removals();

        assert!(!world.has_character(lua_id));
        assert!(world.get_handle(lua_id).is_none());
    }

    #[test]
    fn test_remove_part_after_same_frame_collider_rebuild_does_not_panic_on_step() {
        let mut world = PhysicsWorld::new();

        let lua_id = 7;
        world.add_part(
            lua_id,
            [0.0, 1.0, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [1.0, 1.0, 1.0],
            false,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        world.set_size(lua_id, [2.0, 2.0, 2.0]);
        world.set_shape(lua_id, PartType::Ball, [2.0, 2.0, 2.0]);
        assert!(world.remove_part(lua_id));

        world.step(1.0 / 60.0);
        world.flush_pending_removals();

        assert!(!world.has_part(lua_id));
        assert!(world.get_handle(lua_id).is_none());
    }

    #[test]
    fn test_scaled_rotation_matrix_is_normalized_before_physics() {
        let scaled_identity = [
            [0.111_069_754, 0.0, 0.0],
            [0.0, 0.111_069_77, 0.0],
            [0.0, 0.0, 0.111_069_78],
        ];

        let quat = rotation_matrix_to_quaternion(&scaled_identity);
        let rot = quaternion_to_rotation_matrix(&quat);

        assert!((rot[0][0] - 1.0).abs() < 1e-5);
        assert!((rot[1][1] - 1.0).abs() < 1e-5);
        assert!((rot[2][2] - 1.0).abs() < 1e-5);
        assert!(rot[0][1].abs() < 1e-5);
        assert!(rot[0][2].abs() < 1e-5);
        assert!(rot[1][0].abs() < 1e-5);
        assert!(rot[1][2].abs() < 1e-5);
        assert!(rot[2][0].abs() < 1e-5);
        assert!(rot[2][1].abs() < 1e-5);
    }

    #[test]
    fn test_purge_invalid_body_removes_far_dynamic_part_before_step() {
        let mut world = PhysicsWorld::new();
        let lua_id = 42;
        world.add_part(
            lua_id,
            [PHYSICS_WORLD_ENVELOPE * 2.0, 0.0, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [1.0, 1.0, 1.0],
            false,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        assert_eq!(world.purge_invalid_bodies(), 1);
        world.flush_pending_removals();
        assert!(world.get_handle(lua_id).is_none());
    }

    #[test]
    fn test_invalid_velocity_write_removes_dynamic_part() {
        let mut world = PhysicsWorld::new();
        let lua_id = 43;
        let handle = world.add_part(
            lua_id,
            [0.0, 1.0, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [1.0, 1.0, 1.0],
            false,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        world.set_velocity(handle, [f32::NAN, 0.0, 0.0]);
        world.flush_pending_removals();
        assert!(world.get_handle(lua_id).is_none());
    }

    #[test]
    fn test_inert_part_disables_collider_participation() {
        let mut world = PhysicsWorld::new();

        world.add_part(
            1,
            [0.0, 2.0, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [4.0, 4.0, 4.0],
            true,
            true,
            true,
            true,
            PartType::Block,
            None,
        );

        assert_eq!(world.collider_enabled_for_lua(1), Some(true));

        world.set_can_collide(1, false);
        assert_eq!(world.collider_enabled_for_lua(1), Some(true));

        world.set_can_touch(1, false);
        assert_eq!(world.collider_enabled_for_lua(1), Some(true));

        world.set_can_query(1, false);
        assert_eq!(world.collider_enabled_for_lua(1), Some(false));
    }
}
