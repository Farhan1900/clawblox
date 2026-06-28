use std::collections::HashMap;
use std::env;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const DEFAULT_REPORT_EVERY_TICKS: u64 = 60;

#[derive(Debug, Clone, Copy)]
struct ProfileConfig {
    enabled: bool,
    report_every_ticks: u64,
}

impl ProfileConfig {
    fn from_env() -> Self {
        let explicit_ticks = env::var("CLAWBLOX_PROFILE_TICKS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0);
        let enabled = explicit_ticks.is_some()
            || env::var("CLAWBLOX_PROFILE")
                .map(|v| {
                    matches!(
                        v.as_str(),
                        "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
                    )
                })
                .unwrap_or(false);

        Self {
            enabled,
            report_every_ticks: explicit_ticks.unwrap_or(DEFAULT_REPORT_EVERY_TICKS),
        }
    }
}

fn config() -> &'static ProfileConfig {
    static CONFIG: OnceLock<ProfileConfig> = OnceLock::new();
    CONFIG.get_or_init(ProfileConfig::from_env)
}

#[derive(Default)]
struct StageTotals {
    total: Duration,
    interval_total: Duration,
    max: Duration,
    interval_max: Duration,
}

#[derive(Default)]
struct ProfileTotals {
    count: u64,
    interval_count: u64,
    total: Duration,
    interval_total: Duration,
    max_total: Duration,
    interval_max_total: Duration,
    stage_order: Vec<&'static str>,
    stages: HashMap<&'static str, StageTotals>,
}

#[derive(Default)]
struct CounterTotals {
    count: u64,
    interval_count: u64,
    metric_order: Vec<&'static str>,
    metrics: HashMap<&'static str, CounterMetricTotals>,
}

#[derive(Default)]
struct CounterMetricTotals {
    total: f64,
    interval_total: f64,
    max: f64,
    interval_max: f64,
}

fn totals() -> &'static Mutex<HashMap<&'static str, ProfileTotals>> {
    static TOTALS: OnceLock<Mutex<HashMap<&'static str, ProfileTotals>>> = OnceLock::new();
    TOTALS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn counter_totals() -> &'static Mutex<HashMap<&'static str, CounterTotals>> {
    static TOTALS: OnceLock<Mutex<HashMap<&'static str, CounterTotals>>> = OnceLock::new();
    TOTALS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

pub fn enabled() -> bool {
    config().enabled
}

/// Lightweight, env-gated wall-clock profiler for one repeated code path.
///
/// Set `CLAWBLOX_PROFILE=1` to print rolling averages every 60 frames, or
/// `CLAWBLOX_PROFILE_TICKS=N` to choose a different reporting interval.
pub struct ProfileFrame {
    label: &'static str,
    start: Instant,
    last_mark: Instant,
    stages: Vec<(&'static str, Duration)>,
}

pub fn record_values(label: &'static str, values: &[(&'static str, f64)]) {
    let cfg = config();
    if !cfg.enabled {
        return;
    }

    let mut all_totals = counter_totals().lock().unwrap();
    let entry = all_totals.entry(label).or_default();
    entry.count += 1;
    entry.interval_count += 1;

    for (metric, value) in values {
        if !entry.metrics.contains_key(metric) {
            entry.metric_order.push(metric);
        }
        let metric_entry = entry.metrics.entry(metric).or_default();
        metric_entry.total += *value;
        metric_entry.interval_total += *value;
        metric_entry.max = metric_entry.max.max(*value);
        metric_entry.interval_max = metric_entry.interval_max.max(*value);
    }

    if entry.count % cfg.report_every_ticks != 0 {
        return;
    }

    let count = entry.count as f64;
    let interval_count = entry.interval_count as f64;
    let mut parts = Vec::with_capacity(entry.metric_order.len());
    for metric in &entry.metric_order {
        if let Some(metric_total) = entry.metrics.get_mut(metric) {
            parts.push(format!(
                "{} interval_avg={:.3} interval_max={:.3} avg={:.3} max={:.3}",
                metric,
                metric_total.interval_total / interval_count,
                metric_total.interval_max,
                metric_total.total / count,
                metric_total.max
            ));
            metric_total.interval_total = 0.0;
            metric_total.interval_max = 0.0;
        }
    }

    eprintln!(
        "[ProfileCounters] {} frames={} values=[{}]",
        label,
        entry.count,
        parts.join(", ")
    );
    entry.interval_count = 0;
}

impl ProfileFrame {
    pub fn new(label: &'static str) -> Option<Self> {
        if !config().enabled {
            return None;
        }
        let now = Instant::now();
        Some(Self {
            label,
            start: now,
            last_mark: now,
            stages: Vec::new(),
        })
    }

    pub fn mark(&mut self, stage: &'static str) {
        let now = Instant::now();
        self.stages
            .push((stage, now.duration_since(self.last_mark)));
        self.last_mark = now;
    }

    pub fn finish(self) {
        let total = self.start.elapsed();
        let cfg = config();
        if !cfg.enabled {
            return;
        }

        let mut all_totals = totals().lock().unwrap();
        let entry = all_totals.entry(self.label).or_default();
        entry.count += 1;
        entry.interval_count += 1;
        entry.total += total;
        entry.interval_total += total;
        entry.max_total = entry.max_total.max(total);
        entry.interval_max_total = entry.interval_max_total.max(total);

        for (stage, elapsed) in self.stages {
            if !entry.stages.contains_key(stage) {
                entry.stage_order.push(stage);
            }
            let stage_entry = entry.stages.entry(stage).or_default();
            stage_entry.total += elapsed;
            stage_entry.interval_total += elapsed;
            stage_entry.max = stage_entry.max.max(elapsed);
            stage_entry.interval_max = stage_entry.interval_max.max(elapsed);
        }

        if entry.count % cfg.report_every_ticks != 0 {
            return;
        }

        let count = entry.count as f64;
        let interval_count = entry.interval_count as f64;
        let mut parts = Vec::with_capacity(entry.stage_order.len());
        for stage in &entry.stage_order {
            if let Some(stage_total) = entry.stages.get_mut(stage) {
                parts.push(format!(
                    "{} interval_avg={:.3} interval_max={:.3} avg={:.3} max={:.3}",
                    stage,
                    ms(stage_total.interval_total) / interval_count,
                    ms(stage_total.interval_max),
                    ms(stage_total.total) / count,
                    ms(stage_total.max)
                ));
                stage_total.interval_total = Duration::ZERO;
                stage_total.interval_max = Duration::ZERO;
            }
        }

        eprintln!(
            "[Profile] {} frames={} interval_total_avg={:.3}ms interval_total_max={:.3}ms total_avg={:.3}ms total_max={:.3}ms stages=[{}]",
            self.label,
            entry.count,
            ms(entry.interval_total) / interval_count,
            ms(entry.interval_max_total),
            ms(entry.total) / count,
            ms(entry.max_total),
            parts.join(", ")
        );
        entry.interval_count = 0;
        entry.interval_total = Duration::ZERO;
        entry.interval_max_total = Duration::ZERO;
    }
}
