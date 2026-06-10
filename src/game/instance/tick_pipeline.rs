use super::GameInstance;

/// Executes simulation phases for one tick.
/// Ordered to mirror Roblox-like semantics:
/// input -> sync -> character motion -> physics -> touch -> sync back.
pub(super) fn run_tick_phases(instance: &mut GameInstance, dt: f32) {
    // Sync Lua workspace gravity to physics.
    instance.sync_gravity();

    // Process agent inputs (fire InputReceived events).
    // Do this before syncing MoveTo targets so movement can apply in the same tick.
    // Also do this before Lua->physics sync so script writes (e.g. Part.CFrame)
    // are pushed to physics in the same frame.
    if let Some(runtime) = &instance.lua_runtime {
        if let Err(e) = runtime.process_agent_inputs() {
            instance.handle_lua_error("Failed to process agent inputs", &e);
            if instance.halted_error.is_some() {
                return;
            }
        }
    }

    // Sync new/changed Lua parts to physics (skip character-controlled parts).
    instance.sync_lua_to_physics(dt);

    // Rebuild physics constraints from Lua weld objects before the solver step.
    instance.sync_constraints_to_physics();

    // Apply one-frame BasePart physics commands queued by Lua methods such as
    // ApplyImpulse after assemblies exist and before the solver advances.
    instance.process_physics_commands();

    // Rapier user forces persist until reset. Roblox BodyMovers, VectorForce,
    // and water forces are evaluated continuously for the current step.
    instance.physics.reset_continuous_forces();

    // Apply attachment-driven constraints after Lua state has a physics body
    // and after force reset so they participate in the current solver step.
    instance.apply_attachment_constraints(dt);

    // Apply continuous legacy body movers after assemblies exist and before
    // the solver advances, matching Roblox's assembly-level mover semantics.
    instance.apply_body_movers();

    // Apply modern constraint actuators such as VectorForce through the same
    // continuous-force path used by water and legacy compatibility shims.
    instance.apply_modern_actuators();

    // Apply Terrain liquid-channel fluid forces before the solver advances.
    instance.apply_fluid_forces(dt);

    // Sync script control targets to physics character controllers.
    instance.sync_controller_targets();

    // Update query pipeline before character movement so move_shape sees current collisions.
    instance
        .physics
        .query_pipeline
        .update(&instance.physics.collider_set);

    // Update character controller movement.
    instance.update_character_movement(dt);

    // Step physics simulation.
    instance.physics.step(dt);

    // Detect touch overlaps and fire Touched/TouchEnded events.
    instance.fire_touch_events();

    // Sync physics results back to Lua (for Anchored=false parts and characters).
    instance.sync_physics_to_lua();

    // Remove bodies only after the solver has consumed all modified-body state.
    instance.physics.flush_pending_removals();
}
