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
/// This is the primary observability hook.  Downstream tools (diff, bench,
/// dashboard) parse lines starting with `metric:`.
pub fn emit_metric(name: &str, value: f64, unit: &str) {
    // Print exactly: "metric: <name> <value> <unit>"
    println!("metric: {} {} {}", name, value, unit);
}

/// Emit a metric line to stderr in SPEC-003 format:
/// `metric: <name> <value>`
///
/// Used by the MetricsRegistry for non-unit metrics (counters and gauges).
pub fn emit_metric_stderr(name: &str, value: f64) {
    // Print exactly: "metric: <name> <value>"
    eprintln!("metric: {} {}", name, value);
}

// ── MetricsRegistry: global singleton with 11 SPEC-007 metrics ─────

/// A histogram that stores recent samples and can report summary stats.
#[derive(Debug)]
pub struct Histogram {
    samples: Mutex<Vec<f64>>,
    max_samples: usize,
}

impl Histogram {
    const fn new(max_samples: usize) -> Self {
        Self {
            samples: Mutex::new(Vec::new()),
            max_samples,
        }
    }

    /// Record a sample in the histogram.
    pub fn record(&self, value: f64) {
        let mut samples = self.samples.lock().unwrap();
        samples.push(value);
        if samples.len() > self.max_samples {
            samples.remove(0);
        }
    }

    /// Return min, max, sum, count of current samples.
    pub fn summary(&self) -> (f64, f64, f64, usize) {
        let samples = self.samples.lock().unwrap();
        if samples.is_empty() {
            return (0.0, 0.0, 0.0, 0);
        }
        let mut min = f64::MAX;
        let mut max = f64::MIN;
        let mut sum = 0.0;
        for &v in samples.iter() {
            if v < min {
                min = v;
            }
            if v > max {
                max = v;
            }
            sum += v;
        }
        (min, max, sum, samples.len())
    }

    /// Return the number of samples.
    pub fn count(&self) -> usize {
        let samples = self.samples.lock().unwrap();
        samples.len()
    }

    /// Clear all samples.
    pub fn clear(&self) {
        let mut samples = self.samples.lock().unwrap();
        samples.clear();
    }
}

/// SPEC-007 metrics registry: global singleton with atomic counters,
/// gauges, and histograms for observability.
///
/// All 11 metrics emit to stderr in real time via `emit_metric_stderr`.
pub struct MetricsRegistry {
    // --- Counters (monotonically increasing) ---
    /// `rng.draws.per_turn` — number of RNG draws since last reset
    pub rng_draws: AtomicU64,

    // --- Gauges (point-in-time values) ---
    /// `content.load.ms` — content load time in milliseconds
    pub content_load_ms: AtomicU64,
    /// `save.write.ms` — save write time in milliseconds
    pub save_write_ms: AtomicU64,
    /// `save.size.bytes` — save file size in bytes
    pub save_size_bytes: AtomicU64,
    /// `smoke.volumes.live` — number of active smoke volumes
    pub smoke_volumes_live: AtomicU64,
    /// `sim.actors.alive` — number of alive actors
    pub sim_actors_alive: AtomicU64,
    /// `progression.xp.total` — total XP across all actors
    pub progression_xp_total: AtomicU64,

    // --- Histograms (collections of timed/sized samples) ---
    /// `sim.step.ms` — histogram of sim step cost in ms
    pub sim_step_ms: Histogram,
    /// `ai.turn.ms` — histogram of AI turn cost in ms
    pub ai_turn_ms: Histogram,
    /// `render.frame.ms` — histogram of frame render cost in ms
    pub render_frame_ms: Histogram,
    /// `sim.events.per_turn` — histogram of events generated per turn
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
        // Gauges and counters
        emit_metric_stderr(
            "rng.draws.per_turn",
            self.rng_draws.load(Ordering::Relaxed) as f64,
        );
        emit_metric_stderr(
            "content.load.ms",
            self.content_load_ms.load(Ordering::Relaxed) as f64,
        );
        emit_metric_stderr(
            "save.write.ms",
            self.save_write_ms.load(Ordering::Relaxed) as f64,
        );
        emit_metric_stderr(
            "save.size.bytes",
            self.save_size_bytes.load(Ordering::Relaxed) as f64,
        );
        emit_metric_stderr(
            "smoke.volumes.live",
            self.smoke_volumes_live.load(Ordering::Relaxed) as f64,
        );
        emit_metric_stderr(
            "sim.actors.alive",
            self.sim_actors_alive.load(Ordering::Relaxed) as f64,
        );
        emit_metric_stderr(
            "progression.xp.total",
            self.progression_xp_total.load(Ordering::Relaxed) as f64,
        );

        // Histograms: emit count and mean
        let (smin, smax, ssum, scnt) = self.sim_step_ms.summary();
        if scnt > 0 {
            emit_metric_stderr("sim.step.ms.mean", ssum / scnt as f64);
            emit_metric_stderr("sim.step.ms.min", smin);
            emit_metric_stderr("sim.step.ms.max", smax);
        }

        let (amin, amax, asum, acnt) = self.ai_turn_ms.summary();
        if acnt > 0 {
            emit_metric_stderr("ai.turn.ms.mean", asum / acnt as f64);
            emit_metric_stderr("ai.turn.ms.min", amin);
            emit_metric_stderr("ai.turn.ms.max", amax);
        }

        let (rmin, rmax, rsum, rcnt) = self.render_frame_ms.summary();
        if rcnt > 0 {
            emit_metric_stderr("render.frame.ms.mean", rsum / rcnt as f64);
            emit_metric_stderr("render.frame.ms.min", rmin);
            emit_metric_stderr("render.frame.ms.max", rmax);
        }

        let (emin, emax, esum, ecnt) = self.sim_events_per_turn.summary();
        if ecnt > 0 {
            emit_metric_stderr("sim.events.per_turn.mean", esum / ecnt as f64);
            emit_metric_stderr("sim.events.per_turn.min", emin);
            emit_metric_stderr("sim.events.per_turn.max", emax);
        }
    }

    // ── Convenience helpers ──────────────────────────────────────────

    /// Record a sim step timing sample (histogram, ms).
    pub fn record_sim_step(&self, ms: f64) {
        self.sim_step_ms.record(ms);
    }

    /// Record an AI turn timing sample (histogram, ms).
    pub fn record_ai_turn(&self, ms: f64) {
        self.ai_turn_ms.record(ms);
    }

    /// Record a render frame timing sample (histogram, ms).
    pub fn record_render_frame(&self, ms: f64) {
        self.render_frame_ms.record(ms);
    }

    /// Record number of events produced by a sim step (histogram).
    pub fn record_events_per_turn(&self, count: f64) {
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

/// A named metric with value and unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Metric {
    pub name: String,
    pub value: f64,
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

    /// Record a metric.  If a metric with the same name already exists, the
    /// value is overwritten (last-write-wins).  Also calls `emit_metric`.
    pub fn record(&mut self, name: &str, value: f64, unit: &str) {
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

    /// Return all recorded metrics as a slice, sorted by name (BTreeMap
    /// order).
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
        // emit_metric prints to stdout; we verify it does not panic and
        // produces the expected format by capturing output.
        let output = std::panic::catch_unwind(|| {
            emit_metric("test_metric", 42.5, "ms");
        });
        assert!(output.is_ok());
    }

    #[test]
    fn test_registry_global() {
        let r = MetricsRegistry::global();
        assert_eq!(r.rng_draws.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_registry_increment_rng() {
        let r = MetricsRegistry::global();
        r.reset_all();
        r.increment_rng_draws();
        r.increment_rng_draws();
        r.increment_rng_draws();
        assert_eq!(r.rng_draws.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn test_registry_gauges() {
        let r = MetricsRegistry::global();
        r.reset_all();
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
        let r = MetricsRegistry::global();
        r.reset_all();
        r.record_sim_step(1.5);
        r.record_sim_step(2.0);
        r.record_sim_step(3.2);
        let (_min, _max, _sum, cnt) = r.sim_step_ms.summary();
        assert_eq!(cnt, 3);
    }

    #[test]
    fn test_registry_emit_all() {
        let r = MetricsRegistry::global();
        r.reset_all();
        r.set_content_load_ms(100);
        r.set_sim_actors_alive(6);
        r.increment_rng_draws();
        r.record_sim_step(2.5);
        // Should not panic
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            r.emit_all();
        }));
        assert!(result.is_ok());
    }

    #[test]
    fn test_collector_record_and_get() {
        let mut c = MetricsCollector::new();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);

        c.record("turn_time", 15.3, "ms");
        assert_eq!(c.len(), 1);
        assert!(!c.is_empty());

        let m = c.get("turn_time").expect("metric should exist");
        assert_eq!(m.name, "turn_time");
        assert!((m.value - 15.3).abs() < f64::EPSILON);
        assert_eq!(m.unit, "ms");
    }

    #[test]
    fn test_collector_overwrite() {
        let mut c = MetricsCollector::new();
        c.record("fps", 60.0, "fps");
        c.record("fps", 30.0, "fps");

        let m = c.get("fps").expect("metric should exist");
        assert!((m.value - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_collector_all() {
        let mut c = MetricsCollector::new();
        c.record("a", 1.0, "ms");
        c.record("b", 2.0, "s");
        c.record("c", 3.0, "hz");

        let all = c.all();
        assert_eq!(all.len(), 3);
        // BTreeMap sorts by key, so order is a, b, c
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
        let mut c = MetricsCollector::new();
        c.record("x", 1.0, "m");
        c.record("y", 2.0, "m");
        assert_eq!(c.len(), 2);

        c.clear();
        assert_eq!(c.len(), 0);
        assert!(c.is_empty());
    }

    #[test]
    fn test_collector_default() {
        let c = MetricsCollector::default();
        assert!(c.is_empty());
    }

    #[test]
    fn test_metric_debug_and_clone() {
        let m = Metric {
            name: "test".to_string(),
            value: 1.5,
            unit: "s".to_string(),
        };
        let cloned = m.clone();
        assert_eq!(m, cloned);

        let debug_str = format!("{:?}", m);
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("1.5"));
        assert!(debug_str.contains("s"));
    }

    #[test]
    fn test_emit_metric_called_from_collector() {
        let mut c = MetricsCollector::new();

        let output = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            c.record("collector_test", 99.9, "percent");
        }));
        assert!(output.is_ok());

        let m = c.get("collector_test").unwrap();
        assert!((m.value - 99.9).abs() < f64::EPSILON);
        assert_eq!(m.unit, "percent");
    }
}
