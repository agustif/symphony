//! Distributed tracing and telemetry for Symphony.
//!
//! This module provides OpenTelemetry integration for distributed tracing
//! using OTLP (OpenTelemetry Protocol) export.

use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;
use thiserror::Error;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use url::Url;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TelemetryInitError {
    #[error("failed to create OTLP exporter: {0}")]
    Exporter(String),
    #[error("failed to install tracing subscriber: {0}")]
    Subscriber(String),
}

/// Initialize telemetry with OpenTelemetry OTLP exporter.
///
/// # Example
///
/// ```rust,no_run
/// use symphony_observability::init_telemetry;
///
/// let _ = init_telemetry("symphony", "http://localhost:4317");
/// ```
pub fn init_telemetry(service_name: &str, otlp_endpoint: &str) -> Result<(), TelemetryInitError> {
    Url::parse(otlp_endpoint).map_err(|error| TelemetryInitError::Exporter(error.to_string()))?;

    // Create OTLP exporter using HTTP
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(otlp_endpoint)
        .with_timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|error| TelemetryInitError::Exporter(error.to_string()))?;

    // Create tracer provider with resource
    let resource = opentelemetry_sdk::Resource::builder_empty()
        .with_attribute(opentelemetry::KeyValue::new(
            "service.name",
            service_name.to_string(),
        ))
        .build();

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    // Get tracer with static name
    let tracer = provider.tracer(service_name.to_owned());

    // Create tracing layer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // Initialize subscriber
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(telemetry)
        .with(tracing_subscriber::fmt::layer())
        .try_init()
        .map_err(|error| TelemetryInitError::Subscriber(error.to_string()))?;

    // Keep provider alive
    opentelemetry::global::set_tracer_provider(provider);
    Ok(())
}

/// Initialize telemetry for testing (console only).
#[cfg(test)]
fn init_telemetry_noop() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();
}

/// Create a span for tracking operations.
#[macro_export]
macro_rules! operation_span {
    ($name:expr) => {
        tracing::info_span!($name)
    };
    ($name:expr, $($key:expr => $value:expr),+) => {
        tracing::info_span!($name, $($key = %$value),+)
    };
}

#[cfg(test)]
mod tests {
    use super::{TelemetryInitError, init_telemetry, init_telemetry_noop};

    #[test]
    fn telemetry_init_noop_does_not_panic() {
        init_telemetry_noop();
    }

    #[test]
    fn telemetry_init_reports_invalid_endpoint_as_error() {
        let result = init_telemetry("symphony-test", "http://127.0.0.1:invalid");
        assert!(matches!(
            result,
            Err(TelemetryInitError::Exporter(_) | TelemetryInitError::Subscriber(_))
        ));
    }
}
