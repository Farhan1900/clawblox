use nalgebra::UnitQuaternion;
use rapier3d::control::{
    CharacterAutostep, CharacterLength, EffectiveCharacterMovement, KinematicCharacterController,
};
use rapier3d::prelude::*;

use super::PhysicsWorld;
use crate::game::constants::humanoid as humanoid_consts;
use crate::game::constants::physics as consts;
use crate::game::lua::services::physics_service::CollisionGroupMask;

fn interaction_groups(collision_group: CollisionGroupMask) -> InteractionGroups {
    InteractionGroups::new(
        Group::from(collision_group.membership),
        Group::from(collision_group.filter),
    )
}

/// State for a character controller (player or NPC)
pub struct CharacterControllerState {
    pub controller: KinematicCharacterController,
    pub collider_handle: ColliderHandle,
    pub body_handle: RigidBodyHandle,
    pub vertical_velocity: f32,
    pub target_position: Option<[f32; 3]>,
    pub walk_speed: f32,
    pub jump_power: f32,
    pub jump_requested: bool,
    pub jump_buffer_remaining: f32,
    pub jump_cooldown_remaining: f32,
    /// Seconds elapsed since current MoveTo target was set.
    pub move_to_elapsed: f32,
    pub grounded: bool,
    pub horizontal_velocity: [f32; 2],
    pub collision_group: CollisionGroupMask,
}

#[derive(Debug, Clone, Copy)]
pub struct GroundKinematicSupport {
    pub velocity: [f32; 3],
    pub distance: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct DynamicSupportContact {
    pub lua_id: u64,
    pub point: [f32; 3],
    pub distance: f32,
}

impl PhysicsWorld {
    /// Adds a character controller for player movement
    /// Uses a capsule shape for smooth collisions with environment
    pub fn add_character(
        &mut self,
        lua_id: u64,
        position: [f32; 3],
        radius: f32,
        height: f32,
    ) -> RigidBodyHandle {
        // Create kinematic body for character
        let body = RigidBodyBuilder::kinematic_position_based()
            .translation(vector![position[0], position[1], position[2]])
            .build();
        let body_handle = self.rigid_body_set.insert(body);

        // Create capsule collider (half-height is the cylinder part, total height = 2*half_height + 2*radius)
        let half_height = (height - 2.0 * radius).max(0.0) / 2.0;
        let collider = ColliderBuilder::capsule_y(half_height, radius)
            .active_events(ActiveEvents::COLLISION_EVENTS)
            .active_collision_types(ActiveCollisionTypes::all())
            .collision_groups(InteractionGroups::all())
            .solver_groups(InteractionGroups::all())
            .build();
        let collider_handle =
            self.collider_set
                .insert_with_parent(collider, body_handle, &mut self.rigid_body_set);

        // Create character controller with consistent settings (same as move_character)
        let mut controller = KinematicCharacterController::default();
        controller.autostep = Some(CharacterAutostep {
            max_height: CharacterLength::Absolute(consts::AUTOSTEP_MAX_HEIGHT),
            min_width: CharacterLength::Absolute(consts::AUTOSTEP_MIN_WIDTH),
            include_dynamic_bodies: true,
        });
        controller.max_slope_climb_angle = 45.0_f32.to_radians();
        controller.min_slope_slide_angle = 30.0_f32.to_radians();
        controller.snap_to_ground = Some(CharacterLength::Absolute(consts::SNAP_TO_GROUND));

        let state = CharacterControllerState {
            controller,
            collider_handle,
            body_handle,
            vertical_velocity: 0.0,
            target_position: None,
            walk_speed: consts::WALK_SPEED,
            jump_power: humanoid_consts::DEFAULT_JUMP_POWER,
            jump_requested: false,
            jump_buffer_remaining: 0.0,
            jump_cooldown_remaining: 0.0,
            move_to_elapsed: 0.0,
            grounded: false,
            horizontal_velocity: [0.0, 0.0],
            collision_group: CollisionGroupMask::default(),
        };

        self.character_controllers.insert(lua_id, state);
        self.lua_to_body.insert(lua_id, body_handle);
        self.body_to_lua.insert(body_handle, lua_id);
        self.body_to_luas.insert(body_handle, vec![lua_id]);
        self.lua_to_collider.insert(lua_id, collider_handle);
        self.lua_to_local_frame.insert(lua_id, Isometry::identity());
        self.collider_to_lua.insert(collider_handle, lua_id);

        body_handle
    }

    /// Sets the target position for a character (for Goto action)
    pub fn set_character_target(&mut self, lua_id: u64, target: Option<[f32; 3]>) {
        if let Some(state) = self.character_controllers.get_mut(&lua_id) {
            state.target_position = target;
            state.move_to_elapsed = 0.0;
        }
    }

    /// Sets locomotion walk speed for a character controller.
    pub fn set_character_walk_speed(&mut self, lua_id: u64, walk_speed: f32) {
        if let Some(state) = self.character_controllers.get_mut(&lua_id) {
            state.walk_speed = walk_speed.max(0.0);
        }
    }

    /// Requests a jump using the provided jump power.
    pub fn request_character_jump(&mut self, lua_id: u64, jump_power: f32) {
        if let Some(state) = self.character_controllers.get_mut(&lua_id) {
            state.jump_power = jump_power.max(0.0);
            state.jump_requested = true;
            state.jump_buffer_remaining = humanoid_consts::JUMP_BUFFER_SECS;
        }
    }

    /// Advance jump buffer timer and clear stale buffered jumps.
    pub fn tick_character_jump_buffer(&mut self, lua_id: u64, dt: f32) {
        if let Some(state) = self.character_controllers.get_mut(&lua_id) {
            state.jump_cooldown_remaining = (state.jump_cooldown_remaining - dt).max(0.0);
            if state.jump_requested {
                state.jump_buffer_remaining = (state.jump_buffer_remaining - dt).max(0.0);
                if state.jump_buffer_remaining <= 0.0 {
                    state.jump_requested = false;
                }
            }
        }
    }

    /// Consumes one pending jump request, returning jump power if requested.
    pub fn try_consume_character_jump(
        &mut self,
        lua_id: u64,
        can_jump: bool,
        vertical_velocity: f32,
    ) -> Option<f32> {
        let state = self.character_controllers.get_mut(&lua_id)?;
        if !state.jump_requested || !can_jump {
            return None;
        }
        if state.jump_cooldown_remaining > 0.0 {
            return None;
        }
        // Prevent repeat impulse while already traveling upward.
        if vertical_velocity > 0.0 {
            return None;
        }
        state.jump_cooldown_remaining = 0.18;
        if state.jump_requested {
            state.jump_requested = false;
            state.jump_buffer_remaining = 0.0;
            Some(state.jump_power)
        } else {
            None
        }
    }

    /// Gets current locomotion walk speed for a character controller.
    pub fn get_character_walk_speed(&self, lua_id: u64) -> Option<f32> {
        self.character_controllers
            .get(&lua_id)
            .map(|s| s.walk_speed)
    }

    /// Teleports a character to a specific position (clears target + vertical velocity)
    pub fn set_character_position(&mut self, lua_id: u64, position: [f32; 3]) {
        if let Some(state) = self.character_controllers.get_mut(&lua_id) {
            if let Some(body) = self.rigid_body_set.get_mut(state.body_handle) {
                body.set_translation(vector![position[0], position[1], position[2]], true);
            }
            state.target_position = None;
            state.vertical_velocity = 0.0;
            state.jump_requested = false;
            state.jump_buffer_remaining = 0.0;
            state.jump_cooldown_remaining = 0.0;
            state.move_to_elapsed = 0.0;
            state.horizontal_velocity = [0.0, 0.0];
        }
    }

    /// Gets the current position of a character
    pub fn get_character_position(&self, lua_id: u64) -> Option<[f32; 3]> {
        let state = self.character_controllers.get(&lua_id)?;
        let body = self.rigid_body_set.get(state.body_handle)?;
        let pos = body.translation();
        Some([pos.x, pos.y, pos.z])
    }

    /// Gets current linear velocity of a character controller body.
    pub fn get_character_velocity(&self, lua_id: u64) -> Option<[f32; 3]> {
        let state = self.character_controllers.get(&lua_id)?;
        let body = self.rigid_body_set.get(state.body_handle)?;
        let vel = body.linvel();
        Some([vel.x, vel.y, vel.z])
    }

    /// Sets the facing yaw for a character controller body.
    pub fn set_character_yaw(&mut self, lua_id: u64, yaw: f32) -> bool {
        let Some(state) = self.character_controllers.get(&lua_id) else {
            return false;
        };
        let Some(body) = self.rigid_body_set.get_mut(state.body_handle) else {
            return false;
        };
        let rot = UnitQuaternion::from_euler_angles(0.0, yaw, 0.0);
        body.set_next_kinematic_rotation(rot);
        true
    }

    /// Updates the collision group used by the character controller collider and movement queries.
    pub fn set_character_collision_group(
        &mut self,
        lua_id: u64,
        collision_group: CollisionGroupMask,
    ) {
        let Some(state) = self.character_controllers.get_mut(&lua_id) else {
            return;
        };
        if state.collision_group == collision_group {
            return;
        }
        state.collision_group = collision_group;
        if let Some(collider) = self.collider_set.get_mut(state.collider_handle) {
            let groups = interaction_groups(collision_group);
            collider.set_collision_groups(groups);
            collider.set_solver_groups(groups);
        }
    }

    /// Gets the character controller state (for checking grounded, target, etc.)
    pub fn get_character_state(&self, lua_id: u64) -> Option<&CharacterControllerState> {
        self.character_controllers.get(&lua_id)
    }

    /// Gets mutable character controller state
    pub fn get_character_state_mut(
        &mut self,
        lua_id: u64,
    ) -> Option<&mut CharacterControllerState> {
        self.character_controllers.get_mut(&lua_id)
    }

    /// Removes a character controller
    pub fn remove_character(&mut self, lua_id: u64) -> bool {
        if let Some(state) = self.character_controllers.remove(&lua_id) {
            self.lua_to_body.remove(&lua_id);
            self.body_to_lua.remove(&state.body_handle);
            self.body_to_luas.remove(&state.body_handle);
            self.lua_to_collider.remove(&lua_id);
            self.lua_to_local_frame.remove(&lua_id);
            self.collider_to_lua.remove(&state.collider_handle);
            self.kinematic_linear_velocities.remove(&state.body_handle);
            self.queue_body_removal(state.body_handle);
            true
        } else {
            false
        }
    }

    /// Checks if a Lua instance has a character controller
    pub fn has_character(&self, lua_id: u64) -> bool {
        self.character_controllers.contains_key(&lua_id)
    }

    /// Returns the linear velocity of the kinematic body directly supporting this character.
    /// Used for Roblox-like moving platform carry behavior.
    pub fn get_ground_kinematic_velocity(
        &self,
        lua_id: u64,
        max_distance: f32,
    ) -> Option<[f32; 3]> {
        self.get_ground_kinematic_support(lua_id, max_distance)
            .map(|support| support.velocity)
    }

    /// Returns kinematic support data for the surface directly below this character.
    pub fn get_ground_kinematic_support(
        &self,
        lua_id: u64,
        max_distance: f32,
    ) -> Option<GroundKinematicSupport> {
        let state = self.character_controllers.get(&lua_id)?;
        let body = self.rigid_body_set.get(state.body_handle)?;
        let origin = body.translation();

        let ray = Ray::new(
            point![origin.x, origin.y, origin.z],
            vector![0.0, -1.0, 0.0],
        );

        let filter = QueryFilter::default()
            .exclude_rigid_body(state.body_handle)
            .exclude_sensors()
            .groups(interaction_groups(state.collision_group));

        let (hit_collider, toi) = self.query_pipeline.cast_ray(
            &self.rigid_body_set,
            &self.collider_set,
            &ray,
            max_distance,
            true,
            filter,
        )?;

        let collider = self.collider_set.get(hit_collider)?;
        let parent = collider.parent()?;
        let ground_body = self.rigid_body_set.get(parent)?;
        if !ground_body.is_kinematic() {
            return None;
        }

        let v = ground_body.linvel();
        let sampled_v = self
            .kinematic_linear_velocities
            .get(&parent)
            .copied()
            .unwrap_or([v.x, v.y, v.z]);
        Some(GroundKinematicSupport {
            velocity: sampled_v,
            distance: toi,
        })
    }

    /// Returns the dynamic rigid body directly supporting this character.
    pub fn get_ground_dynamic_support(
        &self,
        lua_id: u64,
        max_distance: f32,
    ) -> Option<DynamicSupportContact> {
        let state = self.character_controllers.get(&lua_id)?;
        let body = self.rigid_body_set.get(state.body_handle)?;
        let origin = body.translation();

        let ray = Ray::new(
            point![origin.x, origin.y, origin.z],
            vector![0.0, -1.0, 0.0],
        );

        let filter = QueryFilter::default()
            .exclude_rigid_body(state.body_handle)
            .exclude_sensors()
            .groups(interaction_groups(state.collision_group));

        let (hit_collider, toi) = self.query_pipeline.cast_ray(
            &self.rigid_body_set,
            &self.collider_set,
            &ray,
            max_distance,
            true,
            filter,
        )?;

        let collider = self.collider_set.get(hit_collider)?;
        let parent = collider.parent()?;
        let ground_body = self.rigid_body_set.get(parent)?;
        if !ground_body.is_dynamic() {
            return None;
        }
        let lua_id = self.body_to_lua.get(&parent).copied()?;
        Some(DynamicSupportContact {
            lua_id,
            point: [origin.x, origin.y - toi, origin.z],
            distance: toi,
        })
    }

    /// Returns the strongest horizontal contact velocity from overlapping kinematic bodies.
    /// This captures side pushes from moving/rotating anchored obstacles (e.g. spinners).
    pub fn get_character_contact_kinematic_velocity(&self, lua_id: u64) -> Option<[f32; 3]> {
        let state = self.character_controllers.get(&lua_id)?;
        let body = self.rigid_body_set.get(state.body_handle)?;
        let collider = self.collider_set.get(state.collider_handle)?;
        let shape = collider.shape();
        let pos = body.position();
        let center = pos.translation.vector;

        let filter = QueryFilter::default()
            .exclude_rigid_body(state.body_handle)
            .exclude_sensors()
            .groups(interaction_groups(state.collision_group));

        let mut best: Option<[f32; 3]> = None;
        let mut best_horiz_speed_sq = 0.0_f32;

        self.query_pipeline.intersections_with_shape(
            &self.rigid_body_set,
            &self.collider_set,
            pos,
            shape,
            filter,
            |other_collider_handle| {
                let Some(other_collider) = self.collider_set.get(other_collider_handle) else {
                    return true;
                };
                let Some(parent) = other_collider.parent() else {
                    return true;
                };
                let Some(other_body) = self.rigid_body_set.get(parent) else {
                    return true;
                };
                if !other_body.is_kinematic() {
                    return true;
                }

                let point_velocity =
                    other_body.velocity_at_point(&point![center.x, center.y, center.z]);
                let horiz_speed_sq =
                    point_velocity.x * point_velocity.x + point_velocity.z * point_velocity.z;
                if horiz_speed_sq > best_horiz_speed_sq {
                    best_horiz_speed_sq = horiz_speed_sq;
                    best = Some([point_velocity.x, point_velocity.y, point_velocity.z]);
                }
                true
            },
        );

        best.filter(|v| (v[0] * v[0] + v[2] * v[2]) > consts::EPSILON * consts::EPSILON)
    }

    /// Moves a character using the kinematic controller for full 3D translation.
    pub fn move_character(
        &mut self,
        lua_id: u64,
        desired_translation: [f32; 3],
        dt: f32,
    ) -> Option<EffectiveCharacterMovement> {
        let state = self.character_controllers.get(&lua_id)?;
        let body_handle = state.body_handle;
        let collider_handle = state.collider_handle;
        let collision_group = state.collision_group;

        let body = self.rigid_body_set.get(body_handle)?;
        let collider = self.collider_set.get(collider_handle)?;
        let shape = collider.shape();

        // Use full position (translation + rotation) like physics-world
        let current_pos = *body.position();

        // Create fresh controller each step (like physics-world)
        let controller = KinematicCharacterController {
            // Larger offset prevents getting stuck when sliding against surfaces
            offset: CharacterLength::Absolute(0.05),
            autostep: Some(CharacterAutostep {
                max_height: CharacterLength::Absolute(consts::AUTOSTEP_MAX_HEIGHT),
                min_width: CharacterLength::Absolute(consts::AUTOSTEP_MIN_WIDTH),
                include_dynamic_bodies: true,
            }),
            max_slope_climb_angle: 45.0_f32.to_radians(),
            min_slope_slide_angle: 30.0_f32.to_radians(),
            snap_to_ground: Some(CharacterLength::Absolute(consts::SNAP_TO_GROUND)),
            ..Default::default()
        };

        let desired = vector![
            desired_translation[0],
            desired_translation[1],
            desired_translation[2]
        ];
        let filter = QueryFilter::default()
            .exclude_rigid_body(body_handle)
            .exclude_sensors()
            .groups(interaction_groups(collision_group));

        let movement = controller.move_shape(
            dt,
            &self.rigid_body_set,
            &self.collider_set,
            &self.query_pipeline,
            shape,
            &current_pos,
            desired,
            filter,
            |_collision| {},
        );

        // Use set_next_kinematic_translation like physics-world
        // This schedules the movement for the next physics step
        let new_pos = current_pos.translation.vector + movement.translation;

        let body = self.rigid_body_set.get_mut(body_handle)?;
        body.set_next_kinematic_translation(new_pos);

        if let Some(state) = self.character_controllers.get_mut(&lua_id) {
            state.grounded = movement.grounded;
        }
        Some(movement)
    }
}
