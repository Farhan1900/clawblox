use std::any::Any;
use std::backtrace::Backtrace;
use std::thread;
use std::time::{Duration, Instant};

use crate::game::{cleanup_empty_instances, profiling::ProfileFrame, GameManager};

use super::tick;

const CLEANUP_INTERVAL_TICKS: u64 = 60;

pub(super) fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(msg) = payload.downcast_ref::<&str>() {
        (*msg).to_string()
    } else if let Some(msg) = payload.downcast_ref::<String>() {
        msg.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

pub(super) fn log_panic(component: &str, context: &str, payload: &(dyn Any + Send)) {
    let message = panic_payload_message(payload);
    let backtrace = Backtrace::force_capture();
    eprintln!(
        "[FATAL][{}] {} panic=\"{}\"\n{}",
        component, context, message, backtrace
    );
}

impl GameManager {
    pub fn run(self) {
        let tick_duration = Duration::from_millis(1000 / self.tick_rate);
        let mut tick_counter: u64 = 0;

        loop {
            let start = Instant::now();
            let mut profile = ProfileFrame::new("manager.loop");

            let tick_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                tick::tick_instances(&self.state);
            }));
            if let Some(profile) = profile.as_mut() {
                profile.mark("tick_instances");
            }
            if let Err(payload) = tick_result {
                log_panic("manager_loop", "tick_instances", &*payload);
            }

            tick_counter += 1;
            if tick_counter % CLEANUP_INTERVAL_TICKS == 0 && !self.state.disable_gc {
                let destroyed = cleanup_empty_instances(&self.state);
                if destroyed > 0 {
                    eprintln!("[Cleanup] Destroyed {} empty instances", destroyed);
                }
            }
            if let Some(profile) = profile.as_mut() {
                profile.mark("cleanup");
            }

            let elapsed = start.elapsed();
            if let Some(profile) = profile {
                profile.finish();
            }
            if elapsed < tick_duration {
                thread::sleep(tick_duration - elapsed);
            }
        }
    }
}
