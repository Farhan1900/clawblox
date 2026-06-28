use super::*;
use crate::game::lua::animation::AnimationScheduler;
use crate::game::profiling::{record_values, ProfileFrame};

impl LuaRuntime {
    pub fn tick(&self, delta_time: f32) -> Result<()> {
        self.begin_frame(delta_time)?;
        self.end_frame(delta_time)
    }

    /// Runs start-of-frame Lua work before physics (pending coroutines + Stepped event).
    pub fn begin_frame(&self, delta_time: f32) -> Result<()> {
        self.discover_and_run_scripts()?;

        let now = self.runtime_clock.advance(delta_time as f64);

        // Process Debris timers before running frame logic.
        // This gives AddItem lifetimes deterministic frame-bound semantics.
        self.game.debris_service().process_expired(&self.lua, now)?;

        if let Some(scheduler) = self.lua.app_data_ref::<AnimationScheduler>() {
            scheduler.tick(&self.lua, delta_time)?;
        }

        // 1. Resume pending coroutines (callbacks that yielded on DataStore operations, etc.)
        self.resume_pending_coroutines()?;

        // 2. Fire Stepped as coroutines (Roblox pre-physics event)
        let stepped = self.game.run_service().stepped();
        let yielded_threads = stepped.fire_as_coroutines(
            &self.lua,
            MultiValue::from_iter([Value::Number(now), Value::Number(delta_time as f64)]),
        )?;

        // 3. Track any newly yielded coroutines for resumption on next tick
        self.track_yielded_threads(yielded_threads)?;

        Ok(())
    }

    /// Runs end-of-frame Lua work after physics (Heartbeat event).
    pub fn end_frame(&self, delta_time: f32) -> Result<()> {
        let mut profile = ProfileFrame::new("lua.end_frame");
        // Fire Heartbeat as coroutines (allows callbacks to yield)
        let heartbeat = self.game.run_service().heartbeat();
        let heartbeat_connections = heartbeat.connection_count();
        if let Some(profile) = profile.as_mut() {
            profile.mark("get_heartbeat");
        }
        let yielded_threads = heartbeat.fire_as_coroutines(
            &self.lua,
            MultiValue::from_iter([Value::Number(delta_time as f64)]),
        )?;
        let yielded_count = yielded_threads.len();
        if let Some(profile) = profile.as_mut() {
            profile.mark("fire_heartbeat");
        }

        // Track any newly yielded coroutines for resumption on next tick
        self.track_yielded_threads(yielded_threads)?;
        if let Some(profile) = profile.as_mut() {
            profile.mark("track_yielded");
        }
        record_values(
            "lua.end_frame",
            &[
                ("heartbeat_connections", heartbeat_connections as f64),
                ("yielded_threads", yielded_count as f64),
            ],
        );
        if let Some(profile) = profile {
            profile.finish();
        }

        Ok(())
    }
}
