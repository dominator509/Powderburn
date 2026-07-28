//! Simple metrics emission and collection for observability.
//!
//! Provides a free function `emit_metric` for one-shot metric output and a
//! `MetricsCollector` struct that accumulates named metrics for later
//! inspection (used by the determinism diff and benchmarking tools).
//!
//! Also provides a `MetricsRegistry` global singleton that holds atomic
//! counters, gauges, and histogram samples for 11 SPEC-007 metrics.
//! Every metric emits to stderr in SPEC-003 format:
//!     `metric: <name> <value>`

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

// ── Simple stdout emission (legacy, used by MetricsCollector) ───────

/// Emit a metric line to stdout in the format:
/// `metric: <name> <value> <unit>`
///
/// This is the primary observability hook. Downstream tools parse lines
/// starting with `metric:`.
pub fn emit_metric(name: &str, value: u64, unit: &str) {
    println!("metric: {} {} {}", name, value, unit);
}

/// Emit a metric line to stderr in SPEC-003 format:
/// `metric: <name> <value>`
///
/// Used by the MetricsRegistry for counters, gauges, and integer summaries.
pub fn emit_metric_stderr(name: &str, value: u64) {
    eprintln!("metric: {} {}", name, value);
}

// ── MetricsRegistry: global singleton with 11 SPEC-007 metrics ─────

/// A histogram that stores recent integer samples and can report summary stats.
#[derive(Debug)]
pub struct Histogram {
    samples: Mutex<Vec<u64>>,
    max_samples: usize,
}

impl Histogram {
    const fn new(max_samples: usize) -> Self {
        Self {
            samples: Mutex::new(Vec::new()),
            max_samples,
        }
    }

    fn lock_samples(&self) -> std::sync::MutexGuard<'_, Vec<u64>> {
        match self.samples.lock() {
            Ok(samples) => samples,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Record a sample in the histogram.
    pub fn record(&self, value: u64) {
        let mut samples = self.lock_samples();
        samples.push(value);
        if samples.len() > self.max_samples {
            samples.remove(0);
        }
    }

    /// Return min, max, saturating sum, and count of current samples.
    pub fn summary(&self) -> (u64, u64, u64, usize) {
        let samples = self.lock_samples();
        if samples.is_empty() {
            return (0, 0, 0, 0);
        }
        let mut min = u64::MAX;
        let mut max = u64::MIN;
        let mut sum = 0_u64;
        for &value in samples.iter() {
            min = min.min(value);
            max = max.max(value);
            sum = sum.saturating_add(value);
        }
        (min, max, sum, samples.len())
    }

    /// Return the number of samples.
    pub fn count(&self) -> usize {
        self.lock_samples().len()
    }

    /// Clear all samples.
    pub fn clear(&self) {
        self.lock_samples().clear();
    }
}

/// SPEC-007 metrics registry: global singleton with atomic counters,
/// gauges, and histograms for observability.
///
/// All metrics emit to stderr in real time via `emit_metric_stderr`.
#[derive(Debug)]
pub struct MetricsRegistry {
    /// `rng.draws.per_turn` — number of RNG draws since last reset.
    pub rng_draws: AtomicU64,
    /// `content.load.ms` — content load time in milliseconds.
    pub content_load_ms: AtomicU64,
    /// `save.write.ms` — save write time in milliseconds.
    pub save_write_ms: AtomicU64,
    /// `save.size.bytes` — save file size in bytes.
    pub save_size_bytes: AtomicU64,
    /// `smoke.volumes.live` — number of active smoke volumes.
    pub smoke_volumes_live: AtomicU64,
    /// `sim.actors.alive` — number of alive actors.
    pub sim_actors_alive: AtomicU64,
    /// `progression.xp.total` — total XP across all actors.
    pub progression_xp_total: AtomicU64,
    /// `sim.step.ms` — histogram of sim step cost in whole milliseconds.
    pub sim_step_ms: Histogram,
    /// `ai.turn.ms` — histogram of AI turn cost in whole milliseconds.
    pub ai_turn_ms: Histogram,
    /// `render.frame.ms` — histogram of frame render cost in whole milliseconds.
    pub render_frame_ms: Histogram,
    /// `sim.events.per_turn` — histogram of events generated per turn.
    pub sim_events_per_turn: Histogram,
}

impl MetricsRegistry {
    /// Create a new registry with all metrics zeroed.
    pub fn new() -> Self {
        Self {
            rng_draws: AtomicU64::new(0),
            content_load_ms: AtomicU64::new(0),
            save_write_ms: AtomicU64::new(0),
            save_size_bytes: AtomicU64::new(0),
            smoke_volumes_live: AtomicU64::new(0),
            sim_actors_alive: AtomicU64::new(0),
            progression_xp_total: AtomicU64::new(0),
            sim_step_ms: Histogram::new(500),
            ai_turn_ms: Histogram::new(500),
            render_frame_ms: Histogram::new(500),
            sim_events_per_turn: Histogram::new(500),
        }
    }

    /// Return a reference to the global registry, creating it on first call.
    pub fn global() -> &'static Self {
        static REGISTRY: OnceLock<MetricsRegistry> = OnceLock::new();
        REGISTRY.get_or_init(Self::new)
    }

    /// Reset all metrics to zero / empty.
    pub fn reset_all(&self) {
        self.rng_draws.store(0, Ordering::Relaxed);
        self.content_load_ms.store(0, Ordering::Relaxed);
        self.save_write_ms.store(0, Ordering::Relaxed);
        self.save_size_bytes.store(0, Ordering::Relaxed);
        self.smoke_volumes_live.store(0, Ordering::Relaxed);
        self.sim_actors_alive.store(0, Ordering::Relaxed);
        self.progression_xp_total.store(0, Ordering::Relaxed);
        self.sim_step_ms.clear();
        self.ai_turn_ms.clear();
        self.render_frame_ms.clear();
        self.sim_events_per_turn.clear();
    }

    /// Emit all current metric values to stderr in SPEC-003 format.
    pub fn emit_all(&self) {
        emit_metric_stderr("rng.draws.per_turn", self.rng_draws.load(Ordering::Relaxed));
        emit_metric_stderr(
            "content.load.ms",
            self.content_load_ms.load(Ordering::Relaxed),
        );
        emit_metric_stderr("save.write.ms", self.save_write_ms.load(Ordering::Relaxed));
        emit_metric_stderr(
            "save.size.bytes",
            self.save_size_bytes.load(Ordering::Relaxed),
        );
        emit_metric_stderr(
            "smoke.volumes.live",
            self.smoke_volumes_live.load(Ordering::Relaxed),
        );
        emit_metric_stderr("sim.step.ms", self.sim_step_ms.summary().1);
        emit_metric_stderr("ai.turn.ms", self.ai_turn_ms.summary().1);
        emit_metric_stderr("render.frame.ms", self.render_frame_ms.summary().1);
        emit_metric_stderr("sim.events.per_turn", self.sim_events_per_turn.summary().1);
    }

    /// Record a sim step timing sample in whole milliseconds.
    pub fn record_sim_step(&self, ms: u64) {
        self.sim_step_ms.record(ms);
    }

    /// Record an AI turn timing sample in whole milliseconds.
    pub fn record_ai_turn(&self, ms: u64) {
        self.ai_turn_ms.record(ms);
    }

    /// Record a render frame timing sample in whole milliseconds.
    pub fn record_render_frame(&self, ms: u64) {
        self.render_frame_ms.record(ms);
    }

    /// Record number of events produced by a sim step.
    pub fn record_events_per_turn(&self, count: u64) {
        self.sim_events_per_turn.record(count);
    }

    /// Set the content load time gauge (ms).
    pub fn set_content_load_ms(&self, ms: u64) {
        self.content_load_ms.store(ms, Ordering::Relaxed);
    }

    /// Set the save write time gauge (ms).
    pub fn set_save_write_ms(&self, ms: u64) {
        self.save_write_ms.store(ms, Ordering::Relaxed);
    }

    /// Set the save file size gauge (bytes).
    pub fn set_save_size_bytes(&self, bytes: u64) {
        self.save_size_bytes.store(bytes, Ordering::Relaxed);
    }

    /// Set the active smoke volumes gauge.
    pub fn set_smoke_volumes_live(&self, count: u64) {
        self.smoke_volumes_live.store(count, Ordering::Relaxed);
    }

    /// Set the alive actors gauge.
    pub fn set_sim_actors_alive(&self, count: u64) {
        self.sim_actors_alive.store(count, Ordering::Relaxed);
    }

    /// Set the total XP gauge.
    pub fn set_progression_xp_total(&self, xp: u64) {
        self.progression_xp_total.store(xp, Ordering::Relaxed);
    }

    /// Increment the RNG draws counter by 1.
    pub fn increment_rng_draws(&self) {
        self.rng_draws.fetch_add(1, Ordering::Relaxed);
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Existing MetricsCollector (unchanged) ───────────────────────────

/// A named integer metric with value and unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metric {
    pub name: String,
    pub value: u64,
    pub unit: String,
}

/// Collects named metrics in insertion order.
///
/// Used in determinism-critical paths to record every numeric decision so
/// that differences between runs can be pinpointed with a simple diff.
#[derive(Debug, Clone)]
pub struct MetricsCollector {
    metrics: BTreeMap<String, Metric>,
}

impl MetricsCollector {
    /// Create a new empty collector.
    pub fn new() -> Self {
        Self {
            metrics: BTreeMap::new(),
        }
    }

    /// Record a metric. Existing names are overwritten (last-write-wins).
    pub fn record(&mut self, name: &str, value: u64, unit: &str) {
        let metric = Metric {
            name: name.to_string(),
            value,
            unit: unit.to_string(),
        };
        self.metrics.insert(name.to_string(), metric);
        emit_metric(name, value, unit);
    }

    /// Retrieve a recorded metric by name.
    pub fn get(&self, name: &str) -> Option<&Metric> {
        self.metrics.get(name)
    }

    /// Return all recorded metrics sorted by name.
    pub fn all(&self) -> Vec<&Metric> {
        self.metrics.values().collect()
    }

    /// Clear all recorded metrics.
    pub fn clear(&mut self) {
        self.metrics.clear();
    }

    /// Number of recorded metrics.
    pub fn len(&self) -> usize {
        self.metrics.len()
    }

    /// Returns `true` if the collector has no metrics.
    pub fn is_empty(&self) -> bool {
        self.metrics.is_empty()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_metric_output() {
        let output = std::panic::catch_unwind(|| {
            emit_metric("test_metric", 42, "ms");
        });
        assert!(output.is_ok());
    }

    #[test]
    fn test_registry_global() {
        let first = MetricsRegistry::global() as *const MetricsRegistry;
        let second = MetricsRegistry::global() as *const MetricsRegistry;
        assert_eq!(first, second);
    }

    #[test]
    fn test_registry_increment_rng() {
        let r = MetricsRegistry::new();
        r.increment_rng_draws();
        r.increment_rng_draws();
        r.increment_rng_draws();
        assert_eq!(r.rng_draws.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn test_registry_gauges() {
        let r = MetricsRegistry::new();
        r.set_content_load_ms(150);
        r.set_save_write_ms(42);
        r.set_save_size_bytes(16384);
        r.set_smoke_volumes_live(12);
        r.set_sim_actors_alive(8);
        r.set_progression_xp_total(2500);

        assert_eq!(r.content_load_ms.load(Ordering::Relaxed), 150);
        assert_eq!(r.save_write_ms.load(Ordering::Relaxed), 42);
        assert_eq!(r.save_size_bytes.load(Ordering::Relaxed), 16384);
        assert_eq!(r.smoke_volumes_live.load(Ordering::Relaxed), 12);
        assert_eq!(r.sim_actors_alive.load(Ordering::Relaxed), 8);
        assert_eq!(r.progression_xp_total.load(Ordering::Relaxed), 2500);
    }

    #[test]
    fn test_registry_histograms() {
        let registry = MetricsRegistry::new();
        registry.record_sim_step(1);
        registry.record_sim_step(2);
        registry.record_sim_step(3);
        let (min, max, sum, count) = registry.sim_step_ms.summary();
        assert_eq!((min, max, sum, count), (1, 3, 6, 3));
    }

    #[test]
    fn test_registry_emit_all() {
        let registry = MetricsRegistry::new();
        registry.set_content_load_ms(100);
        registry.set_sim_actors_alive(6);
        registry.increment_rng_draws();
        registry.record_sim_step(2);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            registry.emit_all();
        }));
        assert!(result.is_ok());
    }

    #[test]
    fn test_collector_record_and_get() {
        let mut collector = MetricsCollector::new();
        assert!(collector.is_empty());
        assert_eq!(collector.len(), 0);

        collector.record("turn_time", 15, "ms");
        assert_eq!(collector.len(), 1);
        assert!(!collector.is_empty());

        let Some(metric) = collector.get("turn_time") else {
            panic!("metric should exist");
        };
        assert_eq!(metric.name, "turn_time");
        assert_eq!(metric.value, 15);
        assert_eq!(metric.unit, "ms");
    }

    #[test]
    fn test_collector_overwrite() {
        let mut collector = MetricsCollector::new();
        collector.record("fps", 60, "fps");
        collector.record("fps", 30, "fps");

        let Some(metric) = collector.get("fps") else {
            panic!("metric should exist");
        };
        assert_eq!(metric.value, 30);
    }

    #[test]
    fn test_collector_all() {
        let mut collector = MetricsCollector::new();
        collector.record("a", 1, "ms");
        collector.record("b", 2, "s");
        collector.record("c", 3, "hz");

        let all = collector.all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].name, "a");
        assert_eq!(all[1].name, "b");
        assert_eq!(all[2].name, "c");
    }

    #[test]
    fn test_collector_get_nonexistent() {
        let c = MetricsCollector::new();
        assert!(c.get("nonexistent").is_none());
    }

    #[test]
    fn test_collector_clear() {
        let mut collector = MetricsCollector::new();
        collector.record("x", 1, "m");
        collector.record("y", 2, "m");
        assert_eq!(collector.len(), 2);

        collector.clear();
        assert_eq!(collector.len(), 0);
        assert!(collector.is_empty());
    }

    #[test]
    fn test_collector_default() {
        let c = MetricsCollector::default();
        assert!(c.is_empty());
    }

    #[test]
    fn test_metric_debug_and_clone() {
        let metric = Metric {
            name: "test".to_string(),
            value: 2,
            unit: "s".to_string(),
        };
        let cloned = metric.clone();
        assert_eq!(metric, cloned);

        let debug_string = format!("{:?}", metric);
        assert!(debug_string.contains("test"));
        assert!(debug_string.contains('2'));
        assert!(debug_string.contains('s'));
    }

    #[test]
    fn test_emit_metric_called_from_collector() {
        let mut collector = MetricsCollector::new();

        let output = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            collector.record("collector_test", 100, "percent");
        }));
        assert!(output.is_ok());

        let Some(metric) = collector.get("collector_test") else {
            panic!("metric should exist");
        };
        assert_eq!(metric.value, 100);
        assert_eq!(metric.unit, "percent");
    }
}
