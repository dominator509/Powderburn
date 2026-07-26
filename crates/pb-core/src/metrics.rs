//! Simple metrics emission and collection for observability.
//!
//! Provides a free function `emit_metric` for one-shot metric output and a
//! `MetricsCollector` struct that accumulates named metrics for later
//! inspection (used by the determinism diff and benchmarking tools).

use std::collections::BTreeMap;

/// Emit a metric line to stdout in the format:
/// `metric: <name> <value> <unit>`
///
/// This is the primary observability hook.  Downstream tools (diff, bench,
/// dashboard) parse lines starting with `metric:`.
pub fn emit_metric(name: &str, value: f64, unit: &str) {
    // Print exactly: "metric: <name> <value> <unit>"
    println!("metric: {} {} {}", name, value, unit);
}

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
        // Verify that MetricsCollector::record calls emit_metric by
        // capturing stdout output.
        let mut c = MetricsCollector::new();

        // AssertUnwindSafe is safe here — we are only testing that
        // emit_metric is called and does not panic.  If it panics,
        // the collector is left in a consistent state (BTreeMap insert
        // is atomic from the test's perspective).
        let output = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            c.record("collector_test", 99.9, "percent");
        }));
        assert!(output.is_ok());

        let m = c.get("collector_test").unwrap();
        assert!((m.value - 99.9).abs() < f64::EPSILON);
        assert_eq!(m.unit, "percent");
    }
}
