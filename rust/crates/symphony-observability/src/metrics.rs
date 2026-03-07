//! Metrics collection and export for Symphony.
//!
//! This module provides operational metrics using the `metrics` crate
//! with Prometheus export capability.

use metrics_exporter_prometheus::PrometheusBuilder;
use std::net::SocketAddr;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MetricsInitError {
    #[error("failed to install Prometheus recorder: {0}")]
    Recorder(String),
}

/// Initialize the metrics system with Prometheus exporter.
///
/// # Example
///
/// ```rust,no_run
/// use symphony_observability::init_metrics;
///
/// let _ = init_metrics("127.0.0.1:9000".parse().unwrap());
/// ```
pub fn init_metrics(bind_addr: SocketAddr) -> Result<(), MetricsInitError> {
    PrometheusBuilder::new()
        .with_http_listener(bind_addr)
        .install()
        .map_err(|error| MetricsInitError::Recorder(error.to_string()))
}

/// Initialize metrics without HTTP exporter (for testing).
#[cfg(test)]
fn init_metrics_noop() -> Result<(), MetricsInitError> {
    // Use noop recorder for tests
    metrics::set_global_recorder(metrics::NoopRecorder)
        .map_err(|error| MetricsInitError::Recorder(error.to_string()))
}

/// Record a counter metric.
#[macro_export]
macro_rules! counter {
    ($name:expr, $value:expr) => {
        metrics::counter!($name).increment($value)
    };
}

/// Record a gauge metric.
#[macro_export]
macro_rules! gauge {
    ($name:expr, $value:expr) => {
        metrics::gauge!($name).set($value)
    };
}

/// Record a histogram metric.
#[macro_export]
macro_rules! histogram {
    ($name:expr, $value:expr) => {
        metrics::histogram!($name).record($value)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_init_noop_does_not_panic() {
        let _ = init_metrics_noop();
        metrics::counter!("test_counter").increment(1);
        metrics::gauge!("test_gauge").set(42.0);
        metrics::histogram!("test_histogram").record(0.5);
    }
}
