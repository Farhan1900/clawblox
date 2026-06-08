use std::collections::{HashMap, HashSet};

use rapier3d::prelude::*;

use super::{
    build_collider_with_physical_properties, finite_vec3, part_isometry, rotation_matrix_is_finite,
    transform_within_physics_envelope, velocity_within_physics_envelope, PartInteractionState,
    PhysicsAssemblyPart, PhysicsAssemblySpec, PhysicsWorld,
};
use crate::game::constants::physics as consts;

fn part_within_physics_envelope(part: &PhysicsAssemblyPart) -> bool {
    transform_within_physics_envelope(part.position)
        && finite_vec3(part.size)
        && part.size.iter().all(|v| *v >= 0.0)
        && rotation_matrix_is_finite(&part.rotation)
        && velocity_within_physics_envelope(part.velocity)
        && velocity_within_physics_envelope(part.angular_velocity)
}

fn q5(value: f32) -> i64 {
    let quantized = (value * 100_000.0).round() as i64;
    if quantized == 0 {
        0
    } else {
        quantized
    }
}

fn fmt_vec3(out: &mut String, values: [f32; 3]) {
    out.push_str(&format!(
        "{},{},{}",
        q5(values[0]),
        q5(values[1]),
        q5(values[2])
    ));
}

fn fmt_iso(out: &mut String, iso: &Isometry<Real>) {
    let t = iso.translation.vector;
    let q = iso.rotation;
    out.push_str(&format!(
        "{},{},{}|{},{},{},{}",
        q5(t.x),
        q5(t.y),
        q5(t.z),
        q5(q.i),
        q5(q.j),
        q5(q.k),
        q5(q.w)
    ));
}

fn assembly_signature(
    root_iso: &Isometry<Real>,
    anchored: bool,
    parts: &[PhysicsAssemblyPart],
    existing_local_frames: &HashMap<u64, Isometry<Real>>,
) -> String {
    let mut out = format!("anchored={};", anchored);
    let inv_root = root_iso.inverse();
    let assembly_cframe_write = parts.iter().any(|part| part.cframe_dirty);
    let mut sorted = parts.to_vec();
    sorted.sort_by_key(|p| p.lua_id);
    for part in &sorted {
        // Roblox weld parity: setting Position on a welded part recalculates
        // that part's assembly-local offset, but setting CFrame moves the whole
        // assembly through the existing offsets. If any assembly member had a
        // CFrame write this frame, preserve every stored local frame; otherwise
        // moving the root part would make every other part look like it changed
        // weld offset relative to the new root transform.
        let local = if assembly_cframe_write {
            existing_local_frames
                .get(&part.lua_id)
                .copied()
                .unwrap_or_else(|| inv_root * part_isometry(part.position, &part.rotation))
        } else {
            inv_root * part_isometry(part.position, &part.rotation)
        };
        out.push_str(&format!("id={};shape={:?};", part.lua_id, part.shape));
        out.push_str("size=");
        fmt_vec3(&mut out, part.size);
        out.push_str(";local=");
        fmt_iso(&mut out, &local);
        out.push(';');
        if let Some(props) = part.physical_properties {
            out.push_str(&format!(
                "props={},{},{},{},{};",
                q5(props.density),
                q5(props.friction),
                q5(props.elasticity),
                q5(props.friction_weight),
                q5(props.elasticity_weight)
            ));
        }
    }
    out
}

impl PhysicsWorld {
    pub fn apply_impulse_to_part(&mut self, lua_id: u64, impulse: [f32; 3]) {
        if !velocity_within_physics_envelope(impulse) {
            return;
        }
        let Some(&handle) = self.lua_to_body.get(&lua_id) else {
            return;
        };
        if let Some(body) = self.rigid_body_set.get_mut(handle) {
            if body.is_dynamic() {
                body.apply_impulse(vector![impulse[0], impulse[1], impulse[2]], true);
            }
        }
    }

    pub fn apply_impulse_at_position_to_part(
        &mut self,
        lua_id: u64,
        impulse: [f32; 3],
        position: [f32; 3],
    ) {
        if !velocity_within_physics_envelope(impulse)
            || !transform_within_physics_envelope(position)
        {
            return;
        }
        let Some(&handle) = self.lua_to_body.get(&lua_id) else {
            return;
        };
        if let Some(body) = self.rigid_body_set.get_mut(handle) {
            if body.is_dynamic() {
                body.apply_impulse_at_point(
                    vector![impulse[0], impulse[1], impulse[2]],
                    point![position[0], position[1], position[2]],
                    true,
                );
            }
        }
    }

    pub fn apply_angular_impulse_to_part(&mut self, lua_id: u64, impulse: [f32; 3]) {
        if !velocity_within_physics_envelope(impulse) {
            return;
        }
        let Some(&handle) = self.lua_to_body.get(&lua_id) else {
            return;
        };
        if let Some(body) = self.rigid_body_set.get_mut(handle) {
            if body.is_dynamic() {
                body.apply_torque_impulse(vector![impulse[0], impulse[1], impulse[2]], true);
            }
        }
    }

    /// Syncs Roblox-style rigid assemblies into Rapier.
    ///
    /// Rigid weld graphs are represented as one Rapier body with one collider per
    /// Lua part. This matches Roblox's "assembly is one rigid body" physics
    /// model and avoids per-frame internal fixed-joint solves, wakeups, and sleep
    /// timer resets. The body is preserved while the assembly signature is stable.
    pub fn sync_assemblies(&mut self, specs: Vec<PhysicsAssemblySpec>, dt: f32) {
        let mut active_lua_ids = HashSet::new();
        let mut active_roots = HashSet::new();

        for spec in specs {
            if spec.parts.is_empty() {
                continue;
            }

            active_roots.insert(spec.root_id);
            for part in &spec.parts {
                active_lua_ids.insert(part.lua_id);
            }

            if !spec.parts.iter().all(part_within_physics_envelope) {
                self.remove_assembly_by_root(spec.root_id);
                continue;
            }

            let root_part = spec
                .parts
                .iter()
                .find(|part| part.lua_id == spec.root_id)
                .unwrap_or(&spec.parts[0]);
            let root_iso = part_isometry(root_part.position, &root_part.rotation);
            let anchored = spec.parts.iter().any(|part| part.anchored);
            let signature =
                assembly_signature(&root_iso, anchored, &spec.parts, &self.lua_to_local_frame);
            let existing_signature = self.assembly_signatures.get(&spec.root_id);

            if existing_signature != Some(&signature) {
                self.rebuild_assembly(&spec, root_iso, anchored, signature);
                continue;
            }

            self.update_existing_assembly(&spec, dt);
        }

        let orphaned_roots: Vec<u64> = self
            .assembly_signatures
            .keys()
            .copied()
            .filter(|root_id| !active_roots.contains(root_id))
            .collect();
        for root_id in orphaned_roots {
            self.remove_assembly_by_root(root_id);
        }

        let orphaned_parts: Vec<u64> = self
            .lua_to_body
            .keys()
            .copied()
            .filter(|lua_id| {
                !active_lua_ids.contains(lua_id) && !self.character_controllers.contains_key(lua_id)
            })
            .collect();
        for lua_id in orphaned_parts {
            self.remove_part(lua_id);
        }
    }

    fn rebuild_assembly(
        &mut self,
        spec: &PhysicsAssemblySpec,
        root_iso: Isometry<Real>,
        anchored: bool,
        signature: String,
    ) {
        let mut old_roots: Vec<u64> = spec
            .parts
            .iter()
            .filter_map(|part| {
                let body = self.lua_to_body.get(&part.lua_id)?;
                self.body_to_lua.get(body).copied()
            })
            .collect();
        old_roots.sort_unstable();
        old_roots.dedup();

        // A weld can merge multiple previous assemblies, and the Roblox assembly
        // root can change when a lower-id part joins. Remove every old body
        // before inserting the replacement body; otherwise later orphan-root
        // cleanup can follow an old root id to the new body and delete it.
        for old_root in old_roots {
            self.remove_assembly_by_root(old_root);
        }

        let body = if anchored {
            RigidBodyBuilder::kinematic_position_based()
        } else {
            RigidBodyBuilder::dynamic()
        }
        .position(root_iso)
        .build();
        let body_handle = self.rigid_body_set.insert(body);

        let inv_root = root_iso.inverse();
        let mut lua_ids = Vec::with_capacity(spec.parts.len());
        let mut initial_velocity = None;
        let mut initial_angular_velocity = None;

        for part in &spec.parts {
            let interaction_state = PartInteractionState {
                can_collide: part.can_collide,
                can_touch: part.can_touch,
                can_query: part.can_query,
                collision_group: part.collision_group,
            };
            let local_frame = inv_root * part_isometry(part.position, &part.rotation);
            let mut collider = build_collider_with_physical_properties(
                part.size,
                part.shape,
                interaction_state,
                part.physical_properties,
            );
            collider.set_position(local_frame);
            let collider_handle = self.collider_set.insert_with_parent(
                collider,
                body_handle,
                &mut self.rigid_body_set,
            );

            self.lua_to_body.insert(part.lua_id, body_handle);
            self.lua_to_collider.insert(part.lua_id, collider_handle);
            self.lua_to_local_frame.insert(part.lua_id, local_frame);
            self.lua_to_shape.insert(part.lua_id, part.shape);
            self.lua_to_physical_properties
                .insert(part.lua_id, part.physical_properties);
            self.lua_to_interactions
                .insert(part.lua_id, interaction_state);
            self.collider_to_lua.insert(collider_handle, part.lua_id);
            lua_ids.push(part.lua_id);

            if initial_velocity.is_none() && (part.velocity_dirty || part.lua_id == spec.root_id) {
                initial_velocity = Some(part.velocity);
            }
            if initial_angular_velocity.is_none()
                && (part.angular_velocity_dirty || part.lua_id == spec.root_id)
            {
                initial_angular_velocity = Some(part.angular_velocity);
            }
        }

        self.body_to_lua.insert(body_handle, spec.root_id);
        self.body_to_luas.insert(body_handle, lua_ids);
        self.assembly_signatures.insert(spec.root_id, signature);

        if !anchored {
            if let Some(velocity) = initial_velocity {
                self.set_velocity(body_handle, velocity);
            }
            if let Some(angular_velocity) = initial_angular_velocity {
                self.set_angular_velocity(body_handle, angular_velocity);
            }
        } else {
            self.kinematic_linear_velocities
                .insert(body_handle, [0.0, 0.0, 0.0]);
        }
    }

    fn update_existing_assembly(&mut self, spec: &PhysicsAssemblySpec, dt: f32) {
        let Some(&body_handle) = self.lua_to_body.get(&spec.root_id) else {
            return;
        };

        for part in &spec.parts {
            let interaction_state = PartInteractionState {
                can_collide: part.can_collide,
                can_touch: part.can_touch,
                can_query: part.can_query,
                collision_group: part.collision_group,
            };
            if self.lua_to_interactions.get(&part.lua_id).copied() != Some(interaction_state) {
                self.set_interaction_state(part.lua_id, interaction_state);
            }

            if part.position_dirty {
                if let Some(local_frame) = self.lua_to_local_frame.get(&part.lua_id).copied() {
                    let target_body_iso =
                        part_isometry(part.position, &part.rotation) * local_frame.inverse();
                    if let Some(body) = self.rigid_body_set.get(body_handle) {
                        if body.is_kinematic() {
                            let cur = body.translation();
                            let target = target_body_iso.translation.vector;
                            let inv_dt = if dt > consts::EPSILON { 1.0 / dt } else { 0.0 };
                            self.kinematic_linear_velocities.insert(
                                body_handle,
                                [
                                    (target.x - cur.x) * inv_dt,
                                    (target.y - cur.y) * inv_dt,
                                    (target.z - cur.z) * inv_dt,
                                ],
                            );
                        }
                    }
                    if let Some(body) = self.rigid_body_set.get_mut(body_handle) {
                        if body.is_kinematic() {
                            body.set_next_kinematic_position(target_body_iso);
                        } else if body.is_dynamic() {
                            body.set_position(target_body_iso, true);
                        }
                    }
                }
            }
            if part.velocity_dirty {
                self.set_velocity(body_handle, part.velocity);
            }
            if part.angular_velocity_dirty {
                self.set_angular_velocity(body_handle, part.angular_velocity);
            }
        }
    }

    pub(super) fn remove_assembly_by_root(&mut self, root_id: u64) {
        let Some(body_handle) = self.lua_to_body.get(&root_id).copied() else {
            self.assembly_signatures.remove(&root_id);
            return;
        };

        self.remove_body_mappings(body_handle);
        self.queue_body_removal(body_handle);
    }
}
