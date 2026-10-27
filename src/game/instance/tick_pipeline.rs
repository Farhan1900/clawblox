use super::GameInstance;
use crate::game::profiling::ProfileFrame;

/// Executes simulation phases for one tick.
/// Ordered to mirror Roblox-like semantics:
/// input -> sync -> character motion -> physics -> touch -> sync back.
pub(super) fn run_tick_phases(instance: &mut GameInstance, dt: f32) {
    let mut profile = ProfileFrame::new("instance.tick_pipeline");
    // Sync Lua workspace gravity to physics.
    instance.sync_gravity();
    if let Some(profile) = profile.as_mut() {
        profile.mark("sync_gravity");
    }

    // Process agent inputs (fire InputReceived events).
    // Do this before syncing MoveTo targets so movement can apply in the same tick.
    // Also do this before Lua->physics sync so script writes (e.g. Part.CFrame)
    // are pushed to physics in the same frame.
    if let Some(runtime) = &instance.lua_runtime {
        if let Err(e) = runtime.process_agent_inputs() {
            instance.handle_lua_error("Failed to process agent inputs", &e);
            if instance.halted_error.is_some() {
                if let Some(profile) = profile {
                    profile.finish();
                }
                return;
            }
        }
    }
    if let Some(profile) = profile.as_mut() {
        profile.mark("agent_inputs");
    }

    // Sync new/changed Lua parts to physics (skip character-controlled parts).
    instance.sync_lua_to_physics(dt);
    if let Some(profile) = profile.as_mut() {
        profile.mark("lua_to_physics");
    }

    // Rebuild physics constraints from Lua weld objects before the solver step.
    instance.sync_constraints_to_physics();
    if let Some(profile) = profile.as_mut() {
        profile.mark("constraints");
    }

    // Apply one-frame BasePart physics commands queued by Lua methods such as
    // ApplyImpulse after assemblies exist and before the solver advances.
    instance.process_physics_commands();
    if let Some(profile) = profile.as_mut() {
        profile.mark("physics_commands");
    }

    // Rapier user forces persist until reset. Roblox BodyMovers, VectorForce,
    // and water forces are evaluated continuously for the current step.
    instance.physics.reset_continuous_forces();
    if let Some(profile) = profile.as_mut() {
        profile.mark("reset_forces");
    }

    // Apply attachment-driven constraints after Lua state has a physics body
    // and after force reset so they participate in the current solver step.
    instance.apply_attachment_constraints(dt);
    if let Some(profile) = profile.as_mut() {
        profile.mark("attachment_constraints");
    }

    // Apply continuous legacy body movers after assemblies exist and before
    // the solver advances, matching Roblox's assembly-level mover semantics.
    instance.apply_body_movers();
    if let Some(profile) = profile.as_mut() {
        profile.mark("body_movers");
    }

    // Apply modern constraint actuators such as VectorForce through the same
    // continuous-force path used by water and legacy compatibility shims.
    instance.apply_modern_actuators();
    if let Some(profile) = profile.as_mut() {
        profile.mark("modern_actuators");
    }

    // Apply Terrain liquid-channel fluid forces before the solver advances.
    instance.apply_fluid_forces(dt);
    if let Some(profile) = profile.as_mut() {
        profile.mark("fluid_forces");
    }

    // Sync script control targets to physics character controllers.
    instance.sync_controller_targets();
    if let Some(profile) = profile.as_mut() {
        profile.mark("controller_targets");
    }

    // Update query pipeline before character movement so move_shape sees current collisions.
    instance
        .physics
        .query_pipeline
        .update(&instance.physics.collider_set);
    if let Some(profile) = profile.as_mut() {
        profile.mark("query_pipeline");
    }

    // Bound the speed dynamic bodies can gain from character contact this
    // step (kinematic capsules otherwise pump trapped bodies without limit).
    let character_contact_speeds = if instance.player_object_physics()
        && std::env::var_os("CLAWBLOX_DISABLE_WEDGE_RECOVERY").is_none()
    {
        Some(instance.physics.snapshot_character_contact_speeds())
    } else {
        None
    };
    if let Some(profile) = profile.as_mut() {
        profile.mark("contact_speed_snapshot");
    }

    // Update character controller movement.
    instance.update_character_movement(dt);
    if let Some(profile) = profile.as_mut() {
        profile.mark("character_movement");
    }

    // Step physics simulation.
    trace_state(instance, "pre");
    instance.physics.step(dt);
    trace_state(instance, "post");
    if let Some(profile) = profile.as_mut() {
        profile.mark("physics_step");
    }

    if let Some(pre_step_speeds) = &character_contact_speeds {
        instance.physics.clamp_character_contact_speed_gains(
            pre_step_speeds,
            crate::game::constants::physics::CHARACTER_CONTACT_SPEED_GAIN_CAP,
        );
    }
    if let Some(profile) = profile.as_mut() {
        profile.mark("contact_speed_clamp");
    }

    // Detect touch overlaps and fire Touched/TouchEnded events.
    instance.fire_touch_events();
    if let Some(profile) = profile.as_mut() {
        profile.mark("touch_events");
    }

    // Sync physics results back to Lua (for Anchored=false parts and characters).
    instance.sync_physics_to_lua();
    if let Some(profile) = profile.as_mut() {
        profile.mark("physics_to_lua");
    }

    // Remove bodies only after the solver has consumed all modified-body state.
    instance.physics.flush_pending_removals();
    if let Some(profile) = profile.as_mut() {
        profile.mark("flush_removals");
    }
    if let Some(profile) = profile {
        profile.finish();
    }
}

/// Debug aid for determinism: with CLAWBLOX_STATE_TRACE=<file>, append every body's exact state (bit patterns)
/// in Rapier handle order before and after each physics step, so two runs can be diffed tick by tick.
fn trace_state(instance: &GameInstance, phase: &str) {
    let Some(path) = std::env::var_os("CLAWBLOX_STATE_TRACE") else {
        return;
    };
    use std::io::Write;
    let mut out = String::new();
    for (handle, body) in instance.physics.rigid_body_set.iter() {
        let lua = instance.physics.body_to_lua.get(&handle).copied().unwrap_or(0);
        let p = body.position();
        let (v, w) = (body.linvel(), body.angvel());
        let (uf, ut) = (body.user_force(), body.user_torque());
        let bits: Vec<String> = [
            uf.x, uf.y, uf.z, ut.x, ut.y, ut.z,
            p.translation.x, p.translation.y, p.translation.z,
            p.rotation.i, p.rotation.j, p.rotation.k, p.rotation.w,
            v.x, v.y, v.z, w.x, w.y, w.z,
        ]
        .iter()
        .map(|f| format!("{:08x}", f.to_bits()))
        .collect();
        out.push_str(&format!(
            "{} {} h{} lua{} sleep{} {}\n",
            instance.tick, phase, handle.into_raw_parts().0, lua, body.is_sleeping() as u8, bits.join(" ")
        ));
    }
    if phase == "pre" {
        for (ch, collider) in instance.physics.collider_set.iter() {
            let lua = instance.physics.collider_to_lua.get(&ch).copied().unwrap_or(0);
            let parent = collider.parent().map(|h| h.into_raw_parts().0).unwrap_or(u32::MAX);
            out.push_str(&format!("{} col c{} lua{} parent{}\n", instance.tick, ch.into_raw_parts().0, lua, parent));
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(out.as_bytes());
    }
}
