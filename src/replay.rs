use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct ReplayInfo {
    pub first_tick: u64,
    pub current_tick: u64,
    pub total_ticks: u64,
    pub tick_rate: u32,
    pub duration_ms: u64,
    pub speed: f32,
    pub paused: bool,
    pub skip_idle: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pause_at_tick: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReplayActiveRange {
    pub start_tick: u64,
    pub end_tick: u64,
}

pub struct ReplayPlayback {
    first_tick: u64,
    current_tick: u64,
    total_ticks: u64,
    tick_rate: u32,
    duration_ms: u64,
    speed: f32,
    paused: bool,
    skip_idle: bool,
    pause_at_tick: Option<u64>,
    fractional_ticks: f64,
    active_ranges: Vec<ReplayActiveRange>,
}

impl ReplayPlayback {
    pub fn new(
        first_tick: u64,
        total_ticks: u64,
        tick_rate: u32,
        duration_ms: u64,
        speed: f32,
        active_ranges: Vec<ReplayActiveRange>,
    ) -> Result<Self, String> {
        if tick_rate == 0 {
            return Err("recording tick_rate must be greater than zero".to_string());
        }
        if total_ticks < first_tick {
            return Err(format!(
                "recording total_ticks ({total_ticks}) is before first_tick ({first_tick})"
            ));
        }

        Ok(Self {
            first_tick,
            current_tick: first_tick,
            total_ticks,
            tick_rate,
            duration_ms,
            speed: speed.clamp(0.1, 100.0),
            paused: false,
            skip_idle: false,
            pause_at_tick: None,
            fractional_ticks: 0.0,
            active_ranges: normalize_active_ranges(active_ranges, first_tick, total_ticks),
        })
    }

    pub fn info(&self) -> ReplayInfo {
        ReplayInfo {
            first_tick: self.first_tick,
            current_tick: self.current_tick,
            total_ticks: self.total_ticks,
            tick_rate: self.tick_rate,
            duration_ms: self.duration_ms,
            speed: self.speed,
            paused: self.paused,
            skip_idle: self.skip_idle,
            pause_at_tick: self.pause_at_tick,
        }
    }

    pub fn current_tick(&self) -> u64 {
        self.current_tick
    }

    pub fn seek(&mut self, tick: u64) {
        self.current_tick = tick.clamp(self.first_tick, self.total_ticks);
        self.pause_at_tick = None;
        self.fractional_ticks = 0.0;
    }

    pub fn set_speed(&mut self, value: f32) {
        self.speed = value.clamp(0.1, 100.0);
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if paused {
            self.pause_at_tick = None;
        }
    }

    pub fn play(&mut self) {
        self.pause_at_tick = None;
        self.paused = false;
    }

    pub fn play_range(&mut self, start_tick: u64, end_tick: u64) {
        let start_tick = start_tick.clamp(self.first_tick, self.total_ticks);
        let end_tick = end_tick.clamp(start_tick, self.total_ticks);
        self.current_tick = start_tick;
        self.pause_at_tick = Some(end_tick);
        self.fractional_ticks = 0.0;
        self.paused = false;
        if self.current_tick >= end_tick {
            self.current_tick = end_tick;
            self.pause_at_tick = None;
            self.paused = true;
            return;
        }
        if self.skip_idle {
            self.skip_idle_gap();
            self.apply_pause_bound();
        }
    }

    pub fn set_skip_idle(&mut self, skip_idle: bool) {
        self.skip_idle = skip_idle;
        if self.skip_idle {
            self.skip_idle_gap();
        }
    }

    pub fn advance_frame(&mut self, frame_duration: Duration) {
        if self.paused || self.current_tick >= self.total_ticks {
            return;
        }

        if self.skip_idle {
            self.skip_idle_gap();
            if self.current_tick >= self.total_ticks {
                return;
            }
        }

        let ticks_per_frame =
            self.speed as f64 * frame_duration.as_secs_f64() * self.tick_rate as f64;
        self.fractional_ticks += ticks_per_frame;
        let advance = self.fractional_ticks.floor() as u64;
        self.fractional_ticks -= advance as f64;
        self.current_tick = (self.current_tick + advance).min(self.total_ticks);

        if self.skip_idle {
            self.skip_idle_gap();
        }
        self.apply_pause_bound();
    }

    fn apply_pause_bound(&mut self) {
        if let Some(pause_at_tick) = self.pause_at_tick {
            if self.current_tick >= pause_at_tick {
                self.current_tick = pause_at_tick;
                self.pause_at_tick = None;
                self.paused = true;
                self.fractional_ticks = 0.0;
            }
        }
    }

    fn skip_idle_gap(&mut self) {
        if self.current_tick >= self.total_ticks || self.is_active_tick(self.current_tick) {
            return;
        }
        self.fractional_ticks = 0.0;
        if let Some(next_tick) = self.next_active_tick_after(self.current_tick) {
            self.current_tick = next_tick;
        } else {
            self.current_tick = self.total_ticks;
        }
    }

    fn is_active_tick(&self, tick: u64) -> bool {
        let idx = self
            .active_ranges
            .partition_point(|range| range.end_tick < tick);
        self.active_ranges
            .get(idx)
            .map(|range| range.start_tick <= tick && tick <= range.end_tick)
            .unwrap_or(false)
    }

    fn next_active_tick_after(&self, tick: u64) -> Option<u64> {
        let idx = self
            .active_ranges
            .partition_point(|range| range.end_tick <= tick);
        self.active_ranges.get(idx).map(|range| range.start_tick)
    }
}

fn normalize_active_ranges(
    mut ranges: Vec<ReplayActiveRange>,
    first_tick: u64,
    total_ticks: u64,
) -> Vec<ReplayActiveRange> {
    ranges.sort_by_key(|range| (range.start_tick, range.end_tick));

    let mut normalized: Vec<ReplayActiveRange> = Vec::new();
    for range in ranges {
        let start_tick = range.start_tick.clamp(first_tick, total_ticks);
        let end_tick = range.end_tick.clamp(first_tick, total_ticks);
        if end_tick < start_tick {
            continue;
        }
        if let Some(prev) = normalized.last_mut() {
            if start_tick <= prev.end_tick.saturating_add(1) {
                prev.end_tick = prev.end_tick.max(end_tick);
                continue;
            }
        }
        normalized.push(ReplayActiveRange {
            start_tick,
            end_tick,
        });
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_idle_jumps_to_next_active_range() {
        let mut playback = ReplayPlayback::new(
            10,
            100,
            60,
            1000,
            1.0,
            vec![
                ReplayActiveRange {
                    start_tick: 20,
                    end_tick: 30,
                },
                ReplayActiveRange {
                    start_tick: 40,
                    end_tick: 50,
                },
            ],
        )
        .unwrap();

        playback.set_skip_idle(true);
        assert_eq!(playback.current_tick(), 20);
    }

    #[test]
    fn skip_idle_keeps_active_ranges_playing_linearly() {
        let mut playback = ReplayPlayback::new(
            10,
            100,
            60,
            1000,
            1.0,
            vec![ReplayActiveRange {
                start_tick: 20,
                end_tick: 40,
            }],
        )
        .unwrap();

        playback.seek(25);
        playback.set_skip_idle(true);
        playback.advance_frame(Duration::from_millis(100));

        assert_eq!(playback.current_tick(), 31);
    }

    #[test]
    fn skip_idle_skips_trailing_idle_time_to_end() {
        let mut playback = ReplayPlayback::new(
            10,
            100,
            60,
            1000,
            1.0,
            vec![ReplayActiveRange {
                start_tick: 20,
                end_tick: 30,
            }],
        )
        .unwrap();

        playback.seek(31);
        playback.set_skip_idle(true);

        assert_eq!(playback.current_tick(), 100);
    }
}
