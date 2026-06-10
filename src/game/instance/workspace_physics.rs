use std::collections::{HashMap, HashSet};

use crate::game::lua::events::{track_yielded_threads, RBXScriptSignal};
use crate::game::lua::instance::{Instance, WeakInstanceRef};
use crate::game::physics::{
    rotation_matrix_to_quaternion, AlignKind, AlignMotorSpec, AlignTarget, PhysicsAssemblyPart,
    PhysicsAssemblySpec,
};

use super::GameInstance;

type PhysIsometry = rapier3d::prelude::Isometry<rapier3d::prelude::Real>;

fn cframe_to_isometry(cframe: &crate::game::lua::types::CFrame) -> PhysIsometry {
    PhysIsometry::from_parts(
        nalgebra::Translation3::new(cframe.position.x, cframe.position.y, cframe.position.z),
        rotation_matrix_to_quaternion(&cframe.rotation),
    )
}

fn attachment_local_isometry(attachment: &Instance) -> Option<PhysIsometry> {
    let data = attachment.data.lock().unwrap();
    data.attachment_data
        .as_ref()
        .map(|attachment| cframe_to_isometry(&attachment.cframe))
}

const RIGID_ALIGN_MAX_SPEED: f32 = 60.0;
const RIGID_ALIGN_MAX_ANGULAR_SPEED: f32 = 30.0;

/// Pair-wise touch lifecycle for one tick.
#[derive(Debug, Default, Clone)]
struct TouchTransitions {
    began: Vec<(u64, u64)>,
    ended: Vec<(u64, u64)>,
}

/// Compute touch begin/end transitions from current and previous overlap sets.
fn compute_touch_transitions(
    current: &HashSet<(u64, u64)>,
    previous: &HashSet<(u64, u64)>,
) -> TouchTransitions {
    let began = current
        .iter()
        .filter(|pair| !previous.contains(pair))
        .copied()
        .collect();

    let ended = previous
        .iter()
        .filter(|pair| !current.contains(pair))
        .copied()
        .collect();

    TouchTransitions { began, ended }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn test_compute_touch_transitions() {
        let previous = HashSet::from([(1, 2), (3, 4)]);
        let current = HashSet::from([(3, 4), (5, 6)]);

        let transitions = compute_touch_transitions(&current, &previous);
        assert_eq!(transitions.began, vec![(5, 6)]);
        assert_eq!(transitions.ended, vec![(1, 2)]);
    }
}

fn clamp_magnitude(mut value: [f32; 3], limit: f32) -> [f32; 3] {
    if limit.is_infinite() && limit.is_sign_positive() {
        return value;
    }
    if !limit.is_finite() || limit < 0.0 {
        return [0.0, 0.0, 0.0];
    }
    if limit == 0.0 {
        return [0.0, 0.0, 0.0];
    }
    let magnitude = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if magnitude > limit && magnitude > f32::EPSILON {
        let scale = limit / magnitude;
        value[0] *= scale;
        value[1] *= scale;
        value[2] *= scale;
    }
    value
}

fn rotation_error_vector(current: [[f32; 3]; 3], target: [[f32; 3]; 3]) -> [f32; 3] {
    let c0 = [current[0][0], current[1][0], current[2][0]];
    let c1 = [current[0][1], current[1][1], current[2][1]];
    let c2 = [current[0][2], current[1][2], current[2][2]];
    let t0 = [target[0][0], target[1][0], target[2][0]];
    let t1 = [target[0][1], target[1][1], target[2][1]];
    let t2 = [target[0][2], target[1][2], target[2][2]];
    let cross = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let x = cross(c0, t0);
    let y = cross(c1, t1);
    let z = cross(c2, t2);
    [
        (x[0] + y[0] + z[0]) * 0.5,
        (x[1] + y[1] + z[1]) * 0.5,
        (x[2] + y[2] + z[2]) * 0.5,
    ]
}

impl GameInstance {
    /// Syncs Workspace.Gravity to physics world
    pub(super) fn sync_gravity(&mut self) {
        if let Some(runtime) = &self.lua_runtime {
            let gravity = runtime.workspace().data.lock().unwrap().gravity;
            self.physics.set_gravity(gravity);
        }
    }

    /// Applies Roblox-style attachment constraints before Lua state is synced to physics.
    pub(super) fn apply_attachment_constraints(&mut self, dt: f32) {
        let Some(runtime) = &self.lua_runtime else {
            return;
        };

        // Align constraints represented by a live motor joint this tick;
        // joints for everything else are swept afterward.
        let mut live_motors: HashSet<u64> = HashSet::new();

        for inst in runtime.workspace().get_descendants() {
            let (align_position, align_orientation) = {
                let data = inst.data.lock().unwrap();
                (
                    data.align_position_data.clone(),
                    data.align_orientation_data.clone(),
                )
            };

            if let Some(align) = align_position {
                let Some(attachment0_ref) = align.attachment0.as_ref().and_then(|w| w.upgrade())
                else {
                    continue;
                };
                let attachment0 = Instance::from_ref(attachment0_ref);
                let Some(parent_part) = attachment0.attachment_parent_part() else {
                    continue;
                };
                let Some(current_attachment_world) = attachment0.attachment_world_cframe() else {
                    continue;
                };
                let attachment1 = align
                    .attachment1
                    .as_ref()
                    .and_then(|w| w.upgrade())
                    .map(Instance::from_ref);

                // Non-rigid aligns are persistent motorized joints, solved
                // implicitly with the rest of the system: several constraints
                // on one body stay stable, and conflicting targets resolve to
                // a bounded compromise instead of pumping energy. Targets on
                // dynamic bodies are followed through the world anchor inside
                // upsert_align_motor (Roblox aligns are reactionless). An
                // upsert can only fail while a body is missing (e.g. the part
                // is not yet synced); the constraint simply waits a tick.
                if !align.rigidity_enabled {
                    let target = match &attachment1 {
                        Some(att1) => att1.attachment_parent_part().and_then(|part1| {
                            attachment_local_isometry(att1).map(|frame| AlignTarget::Part {
                                part_id: part1.id().0,
                                attachment_frame: frame,
                            })
                        }),
                        None => Some(AlignTarget::World(PhysIsometry::from_parts(
                            nalgebra::Translation3::new(
                                align.position.x,
                                align.position.y,
                                align.position.z,
                            ),
                            nalgebra::UnitQuaternion::identity(),
                        ))),
                    };
                    if let (Some(target), Some(frame0)) =
                        (target, attachment_local_isometry(&attachment0))
                    {
                        let spec = AlignMotorSpec {
                            align_id: inst.id().0,
                            kind: AlignKind::Position,
                            part_id: parent_part.id().0,
                            attachment_frame: frame0,
                            target,
                            responsiveness: align.responsiveness.max(0.0),
                            max_output: [align.max_force; 3],
                            max_velocity: align.max_velocity,
                        };
                        if self.physics.upsert_align_motor(&spec) {
                            live_motors.insert(spec.align_id);
                        }
                    }
                    continue;
                }

                // RigidityEnabled: a velocity-level hard tracker, not a
                // spring — kept as a direct velocity solve.
                let target_position = if let Some(attachment1) = &attachment1 {
                    let Some(target_world) = attachment1.attachment_world_cframe() else {
                        continue;
                    };
                    target_world.position
                } else {
                    align.position
                };

                let mass = self.physics.get_assembly_mass_for_part(parent_part.id().0);
                let current_velocity = self
                    .physics
                    .get_velocity_at_position_for_part(
                        parent_part.id().0,
                        current_attachment_world.position.to_array(),
                    )
                    .unwrap_or([0.0, 0.0, 0.0]);
                if let Some(mass) = mass {
                    if dt <= f32::EPSILON {
                        continue;
                    }
                    let error = [
                        target_position.x - current_attachment_world.position.x,
                        target_position.y - current_attachment_world.position.y,
                        target_position.z - current_attachment_world.position.z,
                    ];
                    let desired_velocity = clamp_magnitude(
                        [error[0] / dt, error[1] / dt, error[2] / dt],
                        RIGID_ALIGN_MAX_SPEED,
                    );
                    let force = [
                        (desired_velocity[0] - current_velocity[0]) * mass / dt,
                        (desired_velocity[1] - current_velocity[1]) * mass / dt,
                        (desired_velocity[2] - current_velocity[2]) * mass / dt,
                    ];
                    self.physics.add_force_at_position_to_part(
                        parent_part.id().0,
                        force,
                        current_attachment_world.position.to_array(),
                    );
                    continue;
                }
            }

            if let Some(align) = align_orientation {
                let Some(attachment0_ref) = align.attachment0.as_ref().and_then(|w| w.upgrade())
                else {
                    continue;
                };
                let attachment0 = Instance::from_ref(attachment0_ref);
                let Some(parent_part) = attachment0.attachment_parent_part() else {
                    continue;
                };
                let Some(current_attachment_world) = attachment0.attachment_world_cframe() else {
                    continue;
                };
                let attachment1 = align
                    .attachment1
                    .as_ref()
                    .and_then(|w| w.upgrade())
                    .map(Instance::from_ref);

                // Motor path (see the AlignPosition branch above). Without an
                // Attachment1 the constraint has no target and applies no
                // torque, matching the force path's behavior.
                if !align.rigidity_enabled {
                    if let Some(att1) = &attachment1 {
                        let target = att1.attachment_parent_part().and_then(|part1| {
                            attachment_local_isometry(att1).map(|frame| AlignTarget::Part {
                                part_id: part1.id().0,
                                attachment_frame: frame,
                            })
                        });
                        if let (Some(target), Some(frame0)) =
                            (target, attachment_local_isometry(&attachment0))
                        {
                            let spec = AlignMotorSpec {
                                align_id: inst.id().0,
                                kind: AlignKind::Orientation,
                                part_id: parent_part.id().0,
                                attachment_frame: frame0,
                                target,
                                responsiveness: align.responsiveness.max(0.0),
                                max_output: align.max_torque.to_array(),
                                max_velocity: 0.0,
                            };
                            if self.physics.upsert_align_motor(&spec) {
                                live_motors.insert(spec.align_id);
                            }
                        }
                    }
                    continue;
                }

                // RigidityEnabled: a velocity-level hard tracker, not a
                // spring — kept as a direct angular velocity solve.
                let target_rotation = if let Some(attachment1) = &attachment1 {
                    let Some(target_world) = attachment1.attachment_world_cframe() else {
                        continue;
                    };
                    target_world.rotation
                } else {
                    current_attachment_world.rotation
                };

                if self
                    .physics
                    .get_assembly_mass_for_part(parent_part.id().0)
                    .is_some()
                {
                    if dt <= f32::EPSILON {
                        continue;
                    }
                    let error =
                        rotation_error_vector(current_attachment_world.rotation, target_rotation);
                    let desired_angular_velocity = clamp_magnitude(
                        [error[0] / dt, error[1] / dt, error[2] / dt],
                        RIGID_ALIGN_MAX_ANGULAR_SPEED,
                    );
                    self.physics.set_assembly_angular_velocity_for_part(
                        parent_part.id().0,
                        desired_angular_velocity,
                    );
                    continue;
                }
            }
        }

        self.physics.retain_align_motors(&live_motors);
    }

    /// Syncs Lua parts to the physics world as Roblox-style rigid assemblies.
    ///
    /// Active rigid welds define connected components of parts. Each component is
    /// represented as one Rapier body with one collider per Lua part, which keeps
    /// per-part query/touch identity while matching Roblox's "assembly is one
    /// rigid body" physics model.
    pub(super) fn sync_lua_to_physics(&mut self, dt: f32) {
        let Some(runtime) = &self.lua_runtime else {
            return;
        };

        let physics_service = runtime.game().physics_service();
        let descendants = runtime.workspace().get_descendants();
        let mut part_instances: HashMap<u64, Instance> = HashMap::new();
        let mut adjacency: HashMap<u64, Vec<u64>> = HashMap::new();

        for part in &descendants {
            let data = part.data.lock().unwrap();
            let lua_id = data.id.0;
            if data.part_data.is_some() {
                part_instances.insert(lua_id, part.clone());
                adjacency.entry(lua_id).or_default();
            }
        }

        for instance in &descendants {
            let data = instance.data.lock().unwrap();
            let part_id = |part_ref: &Option<WeakInstanceRef>| {
                part_ref
                    .as_ref()
                    .and_then(|w| w.upgrade())
                    .map(|p| p.lock().unwrap().id.0)
            };
            let mut maybe_edge = |part0: Option<u64>, part1: Option<u64>| {
                let (Some(a), Some(b)) = (part0, part1) else {
                    return;
                };
                if a == b || !part_instances.contains_key(&a) || !part_instances.contains_key(&b) {
                    return;
                }
                adjacency.entry(a).or_default().push(b);
                adjacency.entry(b).or_default().push(a);
            };

            if let Some(weld) = &data.weld_data {
                if weld.enabled {
                    maybe_edge(part_id(&weld.part0), part_id(&weld.part1));
                }
            }

            if let Some(weld) = &data.weld_constraint_data {
                if weld.enabled {
                    maybe_edge(part_id(&weld.part0), part_id(&weld.part1));
                }
            }
        }

        let mut visited = HashSet::new();
        let mut assembly_specs = Vec::new();
        let mut ids: Vec<u64> = part_instances.keys().copied().collect();
        ids.sort_unstable();

        for start_id in ids {
            if !visited.insert(start_id) {
                continue;
            }

            let mut component = Vec::new();
            let mut stack = vec![start_id];
            while let Some(id) = stack.pop() {
                component.push(id);
                if let Some(neighbors) = adjacency.get(&id) {
                    for &neighbor in neighbors {
                        if visited.insert(neighbor) {
                            stack.push(neighbor);
                        }
                    }
                }
            }
            component.sort_unstable();

            if component.len() > 1 {
                let cframe_write = component
                    .iter()
                    .filter_map(|lua_id| {
                        let part = part_instances.get(lua_id)?;
                        let data = part.data.lock().unwrap();
                        let part_data = data.part_data.as_ref()?;
                        if !part_data.cframe_dirty {
                            return None;
                        }
                        Some((
                            *lua_id,
                            [
                                part_data.position.x,
                                part_data.position.y,
                                part_data.position.z,
                            ],
                            part_data.cframe.rotation,
                        ))
                    })
                    // Match existing sync semantics for multiple writes in one
                    // component: later sorted members win.
                    .last();

                if let Some((written_id, position, rotation)) = cframe_write {
                    if let Some(projected) = self
                        .physics
                        .project_assembly_cframe_write(written_id, position, &rotation)
                    {
                        for lua_id in &component {
                            let Some((position, rotation)) = projected.get(lua_id) else {
                                continue;
                            };
                            let Some(part) = part_instances.get(lua_id) else {
                                continue;
                            };
                            let mut data = part.data.lock().unwrap();
                            let Some(part_data) = data.part_data.as_mut() else {
                                continue;
                            };
                            part_data.position.x = position[0];
                            part_data.position.y = position[1];
                            part_data.position.z = position[2];
                            part_data.cframe.position = part_data.position;
                            part_data.cframe.rotation = *rotation;
                            part_data.position_dirty = true;
                            part_data.cframe_dirty = true;
                        }
                    }
                }
            }

            let mut specs = Vec::with_capacity(component.len());
            for lua_id in &component {
                let Some(part) = part_instances.get(lua_id) else {
                    continue;
                };
                let mut data = part.data.lock().unwrap();
                let Some(part_data) = data.part_data.as_mut() else {
                    continue;
                };

                // Character-controlled parts keep their dedicated controller body.
                if self.physics.has_character(*lua_id) {
                    self.physics.set_character_collision_group(
                        *lua_id,
                        physics_service.collision_group_mask_by_id(part_data.collision_group_id),
                    );
                    if part_data.position_dirty {
                        self.physics.set_character_position(
                            *lua_id,
                            [
                                part_data.position.x,
                                part_data.position.y,
                                part_data.position.z,
                            ],
                        );
                        part_data.position_dirty = false;
                    }
                    // Roblox parity: writing a character part's CFrame sets its
                    // facing. Propagate the authored yaw to the kinematic body;
                    // otherwise rotation writes on a character are silently dropped.
                    if part_data.cframe_dirty {
                        let r = &part_data.cframe.rotation;
                        let yaw = r[0][2].atan2(r[2][2]);
                        self.physics.set_character_yaw(*lua_id, yaw);
                        part_data.cframe_dirty = false;
                    }
                    continue;
                }

                specs.push(PhysicsAssemblyPart {
                    lua_id: *lua_id,
                    position: [
                        part_data.position.x,
                        part_data.position.y,
                        part_data.position.z,
                    ],
                    rotation: part_data.cframe.rotation,
                    size: [part_data.size.x, part_data.size.y, part_data.size.z],
                    anchored: part_data.anchored,
                    can_collide: part_data.can_collide,
                    can_touch: part_data.can_touch,
                    can_query: part_data.can_query,
                    collision_group: physics_service
                        .collision_group_mask_by_id(part_data.collision_group_id),
                    shape: part_data.shape,
                    physical_properties: Some(part_data.effective_physical_properties()),
                    velocity: [
                        part_data.velocity.x,
                        part_data.velocity.y,
                        part_data.velocity.z,
                    ],
                    angular_velocity: [
                        part_data.angular_velocity.x,
                        part_data.angular_velocity.y,
                        part_data.angular_velocity.z,
                    ],
                    position_dirty: part_data.position_dirty,
                    cframe_dirty: part_data.cframe_dirty,
                    velocity_dirty: part_data.velocity_dirty,
                    angular_velocity_dirty: part_data.angular_velocity_dirty,
                });
            }

            if specs.is_empty() {
                continue;
            }

            let root_id = specs
                .iter()
                .filter(|part| part.anchored)
                .map(|part| part.lua_id)
                .min()
                .unwrap_or_else(|| specs.iter().map(|part| part.lua_id).min().unwrap());
            assembly_specs.push(PhysicsAssemblySpec {
                root_id,
                parts: specs,
            });
        }

        self.physics.sync_assemblies(assembly_specs, dt);

        for part in part_instances.values() {
            let mut data = part.data.lock().unwrap();
            let lua_id = data.id.0;
            if let Some(part_data) = data.part_data.as_mut() {
                if self.physics.has_character(lua_id) {
                    continue;
                }
                part_data.size_dirty = false;
                part_data.anchored_dirty = false;
                part_data.can_collide_dirty = false;
                part_data.interaction_dirty = false;
                part_data.velocity_dirty = false;
                part_data.angular_velocity_dirty = false;
                part_data.shape_dirty = false;
                part_data.custom_physical_properties_dirty = false;
                part_data.position_dirty = false;
                part_data.cframe_dirty = false;
            }
        }
    }

    /// Syncs physics positions back to Lua for non-anchored parts and character controllers
    pub(super) fn sync_physics_to_lua(&mut self) {
        let Some(runtime) = &self.lua_runtime else {
            return;
        };

        let descendants = runtime.workspace().get_descendants();

        for part in descendants {
            let mut data = part.data.lock().unwrap();
            let lua_id = data.id.0;

            if let Some(part_data) = &mut data.part_data {
                // Check if this is a character-controlled part
                if self.physics.has_character(lua_id) {
                    if part_data.position_dirty {
                        continue;
                    }
                    if let Some(pos) = self.physics.get_character_position(lua_id) {
                        part_data.position.x = pos[0];
                        part_data.position.y = pos[1];
                        part_data.position.z = pos[2];
                        part_data.cframe.position = part_data.position;
                    }
                    if let Some(handle) = self.physics.get_handle(lua_id) {
                        if let Some(rot) = self.physics.get_rotation_matrix(handle) {
                            part_data.cframe.rotation = rot;
                        }
                    }
                    if let Some(vel) = self.physics.get_character_velocity(lua_id) {
                        part_data.velocity.x = vel[0];
                        part_data.velocity.y = vel[1];
                        part_data.velocity.z = vel[2];
                    }
                    part_data.angular_velocity.x = 0.0;
                    part_data.angular_velocity.y = 0.0;
                    part_data.angular_velocity.z = 0.0;
                } else if !part_data.anchored {
                    // If Lua wrote Position/CFrame this frame, keep that authoritative value
                    // until sync_lua_to_physics consumes it.
                    if part_data.position_dirty {
                        continue;
                    }
                    if let Some(pos) = self.physics.get_part_position(lua_id) {
                        part_data.position.x = pos[0];
                        part_data.position.y = pos[1];
                        part_data.position.z = pos[2];
                        part_data.cframe.position = part_data.position;
                    }

                    if let Some(rot) = self.physics.get_part_rotation_matrix(lua_id) {
                        part_data.cframe.rotation = rot;
                    }

                    if let Some(vel) = self.physics.get_part_velocity(lua_id) {
                        part_data.velocity.x = vel[0];
                        part_data.velocity.y = vel[1];
                        part_data.velocity.z = vel[2];
                    }

                    if let Some(handle) = self.physics.get_handle(lua_id) {
                        if let Some(ang_vel) = self.physics.get_angular_velocity(handle) {
                            part_data.angular_velocity.x = ang_vel[0];
                            part_data.angular_velocity.y = ang_vel[1];
                            part_data.angular_velocity.z = ang_vel[2];
                        }
                    }
                }
            }
        }
    }

    /// Detects touch overlaps and fires Touched / TouchEnded signals.
    /// Compares current-frame overlaps against previous-frame overlaps.
    ///
    /// Matches Roblox semantics:
    /// - Both parts must have CanTouch=true for events to fire on either
    /// - Touch state is tracked only when a part has a Touched/TouchEnded
    ///   listener, mirroring Roblox's TouchTransmitter/TouchInterest behavior
    /// - Two anchored (non-character) parts never fire Touched on each other
    /// - Touched fires once on first overlap, TouchEnded fires once when overlap ends
    pub(super) fn fire_touch_events(&mut self) {
        // Do all Lua work in a block to scope the runtime borrow,
        // collecting errors to handle afterward (split-borrow pattern).
        let mut errors: Vec<(&str, mlua::Error)> = Vec::new();
        let mut current = HashSet::new();

        if let Some(runtime) = &self.lua_runtime {
            let lua = runtime.lua();

            // Build lua_id -> Instance lookup from workspace descendants
            let descendants = runtime.workspace().get_descendants();
            let mut id_to_instance: HashMap<u64, Instance> = HashMap::new();
            for inst in &descendants {
                let data = inst.data.lock().unwrap();
                let lua_id = data.id.0;
                if data.part_data.is_some() {
                    id_to_instance.insert(lua_id, inst.clone());
                }
            }

            #[derive(Clone)]
            struct TouchPartInfo {
                anchored_non_character: bool,
                can_touch: bool,
                collision_group_id: u8,
                touched: RBXScriptSignal,
                touch_ended: RBXScriptSignal,
            }

            let part_info = |lua_id: u64| -> Option<TouchPartInfo> {
                let instance = id_to_instance.get(&lua_id)?.clone();
                let data = instance.data.lock().unwrap();
                let part = data.part_data.as_ref()?;
                Some(TouchPartInfo {
                    anchored_non_character: part.anchored && !self.physics.has_character(lua_id),
                    can_touch: part.can_touch,
                    collision_group_id: part.collision_group_id,
                    touched: part.touched.clone(),
                    touch_ended: part.touch_ended.clone(),
                })
            };

            let pair_can_touch = |a: &TouchPartInfo, b: &TouchPartInfo| -> bool {
                if !a.can_touch || !b.can_touch {
                    return false;
                }
                if a.anchored_non_character && b.anchored_non_character {
                    return false;
                }
                if !runtime
                    .game()
                    .physics_service()
                    .groups_collidable_ids(a.collision_group_id, b.collision_group_id)
                {
                    return false;
                }

                // Roblox creates internal TouchInterest/TouchTransmitter state
                // only for parts with touch listeners. Avoid tracking inert
                // construction-block pairs that cannot fire script callbacks.
                a.touched.connection_count() > 0
                    || b.touched.connection_count() > 0
                    || a.touch_ended.connection_count() > 0
                    || b.touch_ended.connection_count() > 0
            };

            // Helper: check if a lua_id is an anchored non-character part
            let is_anchored_non_character = |lua_id: u64| -> bool {
                if self.physics.has_character(lua_id) {
                    return false;
                }
                id_to_instance
                    .get(&lua_id)
                    .and_then(|inst| {
                        let data = inst.data.lock().unwrap();
                        data.part_data.as_ref().map(|p| p.anchored)
                    })
                    .unwrap_or(false)
            };

            for (a, b) in self.physics.detect_narrow_phase_touches() {
                let (Some(info_a), Some(info_b)) = (part_info(a), part_info(b)) else {
                    continue;
                };
                if pair_can_touch(&info_a, &info_b) {
                    current.insert((a, b));
                }
            }
            let transitions = compute_touch_transitions(&current, &self.prev_touches);

            // Helper: fire a signal on one part with the other as argument
            let mut fire_signal = |signal: &RBXScriptSignal, other: &Instance| match lua
                .create_userdata(other.clone())
            {
                Ok(ud) => {
                    match signal.fire_as_coroutines(
                        lua,
                        mlua::MultiValue::from_iter([mlua::Value::UserData(ud)]),
                    ) {
                        Ok(threads) => {
                            if let Err(e) = track_yielded_threads(lua, threads) {
                                errors.push(("Touch track threads", e));
                            }
                        }
                        Err(e) => errors.push(("Touch event", e)),
                    }
                }
                Err(e) => errors.push(("Touch userdata", e)),
            };

            // Began touches
            for (a, b) in transitions.began.iter().copied() {
                // Roblox: two anchored (non-character) parts never fire Touched
                if is_anchored_non_character(a) && is_anchored_non_character(b) {
                    continue;
                }

                let (inst_a, inst_b) = match (
                    id_to_instance.get(&a).cloned(),
                    id_to_instance.get(&b).cloned(),
                ) {
                    (Some(a), Some(b)) => (a, b),
                    _ => continue,
                };

                // Roblox: both parts must have CanTouch=true for events to fire
                let (a_can_touch, a_group, a_signal) = {
                    let data = inst_a.data.lock().unwrap();
                    match data.part_data.as_ref() {
                        Some(p) => (p.can_touch, p.collision_group_id, p.touched.clone()),
                        None => continue,
                    }
                };
                let (b_can_touch, b_group, b_signal) = {
                    let data = inst_b.data.lock().unwrap();
                    match data.part_data.as_ref() {
                        Some(p) => (p.can_touch, p.collision_group_id, p.touched.clone()),
                        None => continue,
                    }
                };

                if !a_can_touch || !b_can_touch {
                    continue;
                }
                if !runtime
                    .game()
                    .physics_service()
                    .groups_collidable_ids(a_group, b_group)
                {
                    continue;
                }

                // Fire Touched on A with B as argument, and B with A
                fire_signal(&a_signal, &inst_b);
                fire_signal(&b_signal, &inst_a);
            }

            // Ended touches
            for (a, b) in transitions.ended.iter().copied() {
                let (inst_a, inst_b) = match (
                    id_to_instance.get(&a).cloned(),
                    id_to_instance.get(&b).cloned(),
                ) {
                    (Some(a), Some(b)) => (a, b),
                    _ => continue,
                };

                let (a_can_touch, a_group, a_signal) = {
                    let data = inst_a.data.lock().unwrap();
                    match data.part_data.as_ref() {
                        Some(p) => (p.can_touch, p.collision_group_id, p.touch_ended.clone()),
                        None => continue,
                    }
                };
                let (b_can_touch, b_group, b_signal) = {
                    let data = inst_b.data.lock().unwrap();
                    match data.part_data.as_ref() {
                        Some(p) => (p.can_touch, p.collision_group_id, p.touch_ended.clone()),
                        None => continue,
                    }
                };

                if !a_can_touch || !b_can_touch {
                    continue;
                }
                if !runtime
                    .game()
                    .physics_service()
                    .groups_collidable_ids(a_group, b_group)
                {
                    continue;
                }

                fire_signal(&a_signal, &inst_b);
                fire_signal(&b_signal, &inst_a);
            }
        }

        // Handle collected errors after runtime borrow is released
        for (context, err) in errors {
            self.handle_lua_error(context, &err);
        }

        self.prev_touches = current;
    }

    /// Rebuild Roblox-style weld constraints into Rapier fixed joints.
    pub(super) fn sync_constraints_to_physics(&mut self) {
        let Some(runtime) = &self.lua_runtime else {
            return;
        };

        let descendants = runtime.workspace().get_descendants();
        self.physics.clear_weld_joints();

        let cframe_to_isometry = |cframe: crate::game::lua::types::CFrame| {
            rapier3d::prelude::Isometry::from_parts(
                nalgebra::Translation3::new(
                    cframe.position.x,
                    cframe.position.y,
                    cframe.position.z,
                ),
                rotation_matrix_to_quaternion(&cframe.rotation),
            )
        };

        for instance in &descendants {
            let data = instance.data.lock().unwrap();

            if let Some(weld_data) = &data.weld_data {
                if !weld_data.enabled {
                    continue;
                }

                // Get Part0 and Part1 references
                let part0_ref = weld_data.part0.as_ref().and_then(|w| w.upgrade());
                let part1_ref = weld_data.part1.as_ref().and_then(|w| w.upgrade());

                if let (Some(part0_ref), Some(part1_ref)) = (part0_ref, part1_ref) {
                    let part0_id = part0_ref.lock().unwrap().id.0;
                    let part1_id = part1_ref.lock().unwrap().id.0;
                    let Some(handle0) = self.physics.get_handle(part0_id) else {
                        continue;
                    };
                    let Some(handle1) = self.physics.get_handle(part1_id) else {
                        continue;
                    };
                    if handle0 == handle1 {
                        continue;
                    }
                    if !self.physics.is_dynamic_body(handle0)
                        && !self.physics.is_dynamic_body(handle1)
                    {
                        continue;
                    }
                    self.physics.insert_fixed_joint(
                        handle0,
                        handle1,
                        cframe_to_isometry(weld_data.c0),
                        cframe_to_isometry(weld_data.c1),
                    );
                }
            }

            if let Some(wc_data) = &data.weld_constraint_data {
                if !wc_data.enabled {
                    continue;
                }

                let part0_ref = wc_data.part0.as_ref().and_then(|w| w.upgrade());
                let part1_ref = wc_data.part1.as_ref().and_then(|w| w.upgrade());

                if let (Some(part0_ref), Some(part1_ref), Some(relative_cframe)) =
                    (part0_ref, part1_ref, wc_data.relative_cframe)
                {
                    let part0_id = part0_ref.lock().unwrap().id.0;
                    let part1_id = part1_ref.lock().unwrap().id.0;
                    let Some(handle0) = self.physics.get_handle(part0_id) else {
                        continue;
                    };
                    let Some(handle1) = self.physics.get_handle(part1_id) else {
                        continue;
                    };
                    if handle0 == handle1 {
                        continue;
                    }
                    if !self.physics.is_dynamic_body(handle0)
                        && !self.physics.is_dynamic_body(handle1)
                    {
                        continue;
                    }
                    self.physics.insert_fixed_joint(
                        handle0,
                        handle1,
                        cframe_to_isometry(relative_cframe),
                        rapier3d::prelude::Isometry::identity(),
                    );
                }
            }
        }
    }
}
