//! The telemetry error type. Its own file because it is shared across the module —
//! `config` produces [`Endpoint`](TelemetryError::Endpoint), `pipeline` produces
//! [`Exporter`](TelemetryError::Exporter) / [`Init`](TelemetryError::Init).

use thiserror::Error;

/// Errors from building or installing the telemetry pipeline.
#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("invalid OTLP endpoint: {0}")]
    Endpoint(String),
    #[error("failed to build OTLP exporter: {0}")]
    Exporter(String),
    #[error("failed to install tracing subscriber: {0}")]
    Init(String),
}
