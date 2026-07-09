//! OpenTelemetry wiring for the SSR server.
//!
//! Exports traces and structured logs over OTLP/gRPC to whatever collector
//! `OTEL_EXPORTER_OTLP_ENDPOINT` points at — in local dev that's the .NET Aspire
//! dashboard, which injects that variable (plus `OTEL_SERVICE_NAME` /
//! `OTEL_RESOURCE_ATTRIBUTES`) into this process. When the variable is absent,
//! telemetry degrades to plain stdout logging with no exporter and no
//! connection attempts.
//!
//! Layout (each file one concern):
//! - [`config`] — the "what": [`TelemetryConfig`] + value types, resolved from env. Pure.
//! - [`pipeline`] — the "how": OTLP exporters, providers, subscriber layers, install.
//! - [`guard`] — [`TelemetryGuard`]: owns the providers, flushes on shutdown/panic.
//! - [`http`] — [`trace_layer`]: the request-span tower middleware.
//! - [`error`] — [`TelemetryError`], shared across the module.
//!
//! Adding metrics later is additive and touches no public signature: enable the
//! `metrics` feature on the `opentelemetry*` crates, build an `SdkMeterProvider`
//! with a periodic `MetricExporter` in the exporting branch plus
//! `global::set_meter_provider(..)`, and add a `meter_provider` field to
//! [`TelemetryGuard`]. Metrics record via the meter API, not a subscriber layer.

mod config;
mod error;
mod guard;
mod http;
mod pipeline;

pub use config::{OtlpEndpoint, TelemetryConfig, TelemetryMode};
pub use error::TelemetryError;
pub use guard::TelemetryGuard;
pub use http::{HttpTraceLayer, trace_layer};
pub use pipeline::OtelLayer;
