//! Telemetry pipeline — the "how": OTLP exporters, providers, the tracing-subscriber
//! layers, and installation. Consumes [`TelemetryConfig`] and produces the running
//! pipeline plus its [`TelemetryGuard`].

use std::path::Path;
use std::time::Duration;

use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::tonic_types::transport::{Certificate, ClientTlsConfig};
use opentelemetry_otlp::{LogExporter, SpanExporter, WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_semantic_conventions::attribute::{
    DEPLOYMENT_ENVIRONMENT_NAME, SERVICE_INSTANCE_ID, SERVICE_VERSION,
};
use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject as _;
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer};

use super::config::{OtlpEndpoint, TelemetryConfig, TelemetryMode};
use super::error::TelemetryError;
use super::guard::TelemetryGuard;

/// Instrumentation scope name stamped on spans this crate emits.
const SCOPE: &str = "portfolio";

/// Per-request export deadline for the OTLP exporters.
const EXPORT_TIMEOUT: Duration = Duration::from_secs(10);

/// The boxed, type-erased OTel layer (span + log pair behind one noise filter) for
/// a subscriber of type `S`. Erased to keep [`TelemetryConfig::layers`]'s signature
/// readable (and satisfy clippy's `type_complexity`).
pub type OtelLayer<S> = Box<dyn Layer<S> + Send + Sync>;

impl TelemetryConfig {
    /// Build providers and install the global subscriber (OTel span + log layers
    /// plus a stdout fmt layer), install the flush-on-panic hook, and return the
    /// guard. Hold the guard for the process lifetime so batch exporters flush.
    pub fn install(self) -> Result<TelemetryGuard, TelemetryError> {
        let mode_desc = self.mode_description();

        let (otel_layer, guard) = self.layers::<tracing_subscriber::Registry>()?;

        // Make the tracer provider global so `tracing-opentelemetry` and any direct
        // `opentelemetry` API usage resolve the same pipeline.
        if let Some(tracer_provider) = guard.tracer_provider() {
            opentelemetry::global::set_tracer_provider(tracer_provider.clone());
        }

        // `Option<Layer>` is a no-op when `None` (the disabled path), so one
        // composition covers both modes: OTel layer (if any) on the bare registry,
        // with fmt on top.
        tracing_subscriber::registry()
            .with(otel_layer)
            .with(tracing_subscriber::fmt::layer().with_filter(default_filter()))
            .try_init()
            .map_err(|e| TelemetryError::Init(e.to_string()))?;

        // Subscriber is live now — record the resolved mode.
        tracing::info!(telemetry = %mode_desc, "telemetry initialized");

        guard.install_panic_hook();
        Ok(guard)
    }

    /// Build providers and hand the OTel `tracing` layer back for a caller-owned
    /// subscriber, together with the owning guard. The layer is the span + log
    /// pair composed behind one noise filter; in
    /// [`Disabled`](TelemetryMode::Disabled) mode it is `None` (a no-op when
    /// passed to `.with(...)`) and the guard is inert.
    pub fn layers<S>(&self) -> Result<(Option<OtelLayer<S>>, TelemetryGuard), TelemetryError>
    where
        S: tracing::Subscriber + for<'a> LookupSpan<'a> + Send + Sync,
    {
        let endpoint = match &self.mode {
            TelemetryMode::Disabled => return Ok((None, TelemetryGuard::inert())),
            TelemetryMode::Exporting(endpoint) => endpoint,
        };

        let tls_config = resolve_tls_trust(
            endpoint.is_tls(),
            self.trusted_ca.as_deref(),
            cfg!(debug_assertions),
        )?
        .map(load_tls_config)
        .transpose()?;

        let resource = self.build_resource();
        let tracer_provider =
            build_tracer_provider(endpoint, resource.clone(), tls_config.clone())?;
        let logger_provider = build_logger_provider(endpoint, resource, tls_config)?;

        let otel_layer = compose_otel_layer(&tracer_provider, &logger_provider);

        Ok((
            Some(otel_layer),
            TelemetryGuard::new(tracer_provider, logger_provider),
        ))
    }

    fn build_resource(&self) -> Resource {
        // `Resource::builder()` already merges the env detector (OTEL_SERVICE_NAME,
        // OTEL_RESOURCE_ATTRIBUTES) and the telemetry-SDK detector, so we only add
        // the identity attributes the environment doesn't carry.
        let mut builder = Resource::builder().with_attributes([
            KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
            KeyValue::new(SERVICE_INSTANCE_ID, uuid::Uuid::new_v4().to_string()),
        ]);
        if let Some(name) = &self.service_name {
            builder = builder.with_service_name(name.clone());
        }
        if let Some(env) = &self.deployment_environment {
            builder =
                builder.with_attributes([KeyValue::new(DEPLOYMENT_ENVIRONMENT_NAME, env.clone())]);
        }
        builder.build()
    }

    fn mode_description(&self) -> String {
        match &self.mode {
            TelemetryMode::Exporting(e) => format!("exporting to {}", e.as_str()),
            TelemetryMode::Disabled => "disabled (stdout only)".to_owned(),
        }
    }
}

/// Resolve the trust source for the export transport: plaintext needs none; https
/// needs the trusted-cert path, and only debug builds accept one (release has no
/// custom-trust story yet).
fn resolve_tls_trust(
    endpoint_is_tls: bool,
    trusted_ca: Option<&Path>,
    debug_build: bool,
) -> Result<Option<&Path>, TelemetryError> {
    match (endpoint_is_tls, debug_build, trusted_ca) {
        (false, _, _) => Ok(None),
        (true, true, Some(path)) => Ok(Some(path)),
        (true, true, None) => Err(TelemetryError::Tls(
            "https OTLP endpoint requires OTEL_EXPORTER_OTLP_CERTIFICATE (path to the \
             collector's trusted certificate)"
                .to_owned(),
        )),
        (true, false, _) => Err(TelemetryError::Tls(
            "https OTLP export is not supported in release builds; use a plaintext http \
             endpoint"
                .to_owned(),
        )),
    }
}

/// Build the exporter TLS config, validating up front that the file holds a PEM
/// certificate — tonic accepts arbitrary bytes here and would fail only at the
/// first export.
fn load_tls_config(path: &Path) -> Result<ClientTlsConfig, TelemetryError> {
    let pem = std::fs::read(path)
        .map_err(|e| TelemetryError::Tls(format!("cannot read {}: {e}", path.display())))?;
    CertificateDer::from_pem_slice(&pem).map_err(|e| {
        TelemetryError::Tls(format!(
            "{} is not a PEM certificate: {e:?}",
            path.display()
        ))
    })?;
    Ok(ClientTlsConfig::new().ca_certificate(Certificate::from_pem(pem)))
}

fn build_tracer_provider(
    endpoint: &OtlpEndpoint,
    resource: Resource,
    tls_config: Option<ClientTlsConfig>,
) -> Result<SdkTracerProvider, TelemetryError> {
    let mut builder = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint.as_str())
        .with_timeout(EXPORT_TIMEOUT);
    if let Some(tls) = tls_config {
        builder = builder.with_tls_config(tls);
    }
    let exporter = builder
        .build()
        .map_err(|e| TelemetryError::Exporter(e.to_string()))?;

    Ok(SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build())
}

fn build_logger_provider(
    endpoint: &OtlpEndpoint,
    resource: Resource,
    tls_config: Option<ClientTlsConfig>,
) -> Result<SdkLoggerProvider, TelemetryError> {
    let mut builder = LogExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint.as_str())
        .with_timeout(EXPORT_TIMEOUT);
    if let Some(tls) = tls_config {
        builder = builder.with_tls_config(tls);
    }
    let exporter = builder
        .build()
        .map_err(|e| TelemetryError::Exporter(e.to_string()))?;

    Ok(SdkLoggerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build())
}

/// fmt/console filter — honors `RUST_LOG`, else `info`.
fn default_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}

/// Compose the OTel layer pair behind the shared export filter: spans (tower-http /
/// `#[instrument]` tracing spans -> OTel spans) and events (tracing events -> OTel
/// log records). Free-standing so the log-bridge test can drive the exact production
/// composition against in-memory providers.
fn compose_otel_layer<S>(
    tracer_provider: &SdkTracerProvider,
    logger_provider: &SdkLoggerProvider,
) -> OtelLayer<S>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a> + Send + Sync,
{
    let span_layer = tracing_opentelemetry::layer::<S>().with_tracer(tracer_provider.tracer(SCOPE));
    let log_layer = OpenTelemetryTracingBridge::new(logger_provider);
    span_layer
        .and_then(log_layer)
        .with_filter(otel_filter())
        .boxed()
}

/// OTLP layer filter — turns on info and up
fn otel_filter() -> Targets {
    Targets::new().with_default(LevelFilter::INFO)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use opentelemetry::Key;
    use opentelemetry_semantic_conventions::attribute::SERVICE_NAME;
    use tracing::Level;

    use super::*;

    #[test]
    fn disabled_mode_yields_no_layers_and_inert_guard() {
        let config = TelemetryConfig {
            mode: TelemetryMode::Disabled,
            service_name: Some("portfolio-test".to_owned()),
            deployment_environment: None,
            trusted_ca: None,
        };
        let (layer, _guard) = config
            .layers::<tracing_subscriber::Registry>()
            .expect("disabled mode builds without an exporter");
        assert!(layer.is_none(), "disabled mode emits no OTel layer");
    }

    #[test]
    fn resolve_tls_trust_plaintext_ignores_cert() {
        let ca = PathBuf::from("/certs/dev.pem");
        for debug_build in [true, false] {
            for trusted_ca in [Some(ca.as_path()), None] {
                let trust = resolve_tls_trust(false, trusted_ca, debug_build)
                    .expect("plaintext endpoints never error");
                assert!(trust.is_none(), "plaintext export never configures TLS");
            }
        }
    }

    #[test]
    fn resolve_tls_trust_debug_https_with_cert_uses_it() {
        let ca = PathBuf::from("/certs/dev.pem");
        let trust = resolve_tls_trust(true, Some(ca.as_path()), true)
            .expect("debug https with a trusted cert resolves");
        assert_eq!(trust, Some(ca.as_path()));
    }

    #[test]
    fn resolve_tls_trust_debug_https_without_cert_errors_naming_var() {
        match resolve_tls_trust(true, None, true) {
            Err(TelemetryError::Tls(msg)) => assert!(
                msg.contains("OTEL_EXPORTER_OTLP_CERTIFICATE"),
                "error must name the missing variable: {msg}"
            ),
            other => panic!("expected a Tls error, got {other:?}"),
        }
    }

    #[test]
    fn resolve_tls_trust_release_https_errors() {
        // Even with a cert configured: release builds have no custom-trust story yet.
        match resolve_tls_trust(true, Some(Path::new("/certs/prod.pem")), false) {
            Err(TelemetryError::Tls(msg)) => {
                assert!(
                    msg.contains("release"),
                    "error must name the build mode: {msg}"
                );
            }
            other => panic!("expected a Tls error, got {other:?}"),
        }
    }

    #[test]
    fn exporting_https_without_cert_is_rejected() {
        // Test binaries compile with debug_assertions, so this exercises the
        // debug-build row of the trust matrix end to end through layers().
        let config = TelemetryConfig {
            mode: TelemetryMode::Exporting(
                OtlpEndpoint::parse("https://collector:4317").expect("valid https endpoint"),
            ),
            service_name: None,
            deployment_environment: None,
            trusted_ca: None,
        };
        // A match, not `expect_err`: the Ok payload (boxed layer + guard) is not
        // `Debug`, so `expect_err`'s `T: Debug` bound would not hold.
        match config.layers::<tracing_subscriber::Registry>() {
            Err(TelemetryError::Tls(_)) => {}
            Err(other) => panic!("expected a Tls error, got {other:?}"),
            Ok(_) => panic!("https endpoint without a trusted cert must be rejected"),
        }
    }

    // A tokio test because the tonic exporter build requires a live runtime (its
    // hyper-util executor panics outside one), even though nothing is awaited.
    #[tokio::test]
    async fn exporting_https_with_cert_builds_layer() -> Result<(), Box<dyn std::error::Error>> {
        let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()])?;
        let dir = tempfile::tempdir()?;
        let ca_path = dir.path().join("ca.pem");
        std::fs::write(&ca_path, certified.cert.pem())?;

        // Deliberately dead endpoint: the batch exporters connect lazily, so nothing
        // must ever leak to a live collector from a test.
        let config = TelemetryConfig {
            mode: TelemetryMode::Exporting(
                OtlpEndpoint::parse("https://127.0.0.1:1").expect("valid https endpoint"),
            ),
            service_name: Some("portfolio-test".to_owned()),
            deployment_environment: None,
            trusted_ca: Some(ca_path),
        };
        let (layer, _guard) = config
            .layers::<tracing_subscriber::Registry>()
            .expect("https endpoint with a trusted cert builds exporters");
        assert!(layer.is_some(), "exporting mode emits the composed layer");
        Ok(())
    }

    #[test]
    fn exporting_https_with_missing_cert_file_errors_with_path() {
        let config = TelemetryConfig {
            mode: TelemetryMode::Exporting(
                OtlpEndpoint::parse("https://collector:4317").expect("valid https endpoint"),
            ),
            service_name: None,
            deployment_environment: None,
            trusted_ca: Some(PathBuf::from("/nonexistent/dev-cert.pem")),
        };
        match config.layers::<tracing_subscriber::Registry>() {
            Err(TelemetryError::Tls(msg)) => assert!(
                msg.contains("/nonexistent/dev-cert.pem"),
                "error must name the unreadable path: {msg}"
            ),
            Err(other) => panic!("expected a Tls error, got {other:?}"),
            Ok(_) => panic!("a missing cert file must be rejected"),
        }
    }

    #[test]
    fn exporting_https_with_garbage_cert_file_errors_with_path() -> Result<(), std::io::Error> {
        let dir = tempfile::tempdir()?;
        let ca_path = dir.path().join("not-a-cert.pem");
        std::fs::write(&ca_path, "this is not a certificate")?;

        let config = TelemetryConfig {
            mode: TelemetryMode::Exporting(
                OtlpEndpoint::parse("https://collector:4317").expect("valid https endpoint"),
            ),
            service_name: None,
            deployment_environment: None,
            trusted_ca: Some(ca_path.clone()),
        };
        match config.layers::<tracing_subscriber::Registry>() {
            Err(TelemetryError::Tls(msg)) => assert!(
                msg.contains(ca_path.to_str().expect("utf-8 temp path")),
                "error must name the invalid file: {msg}"
            ),
            Err(other) => panic!("expected a Tls error, got {other:?}"),
            Ok(_) => panic!("a garbage cert file must be rejected"),
        }
        Ok(())
    }

    // A tokio test because the tonic exporter build requires a live runtime (its
    // hyper-util executor panics outside one), even though nothing is awaited.
    #[tokio::test]
    async fn exporting_http_endpoint_yields_otel_layer() {
        // Builds the real tonic exporters; no collector need be reachable since the
        // batch exporters connect lazily (hence the deliberately dead endpoint —
        // nothing must ever leak to a live collector from a test).
        let config = TelemetryConfig {
            mode: TelemetryMode::Exporting(
                OtlpEndpoint::parse("http://127.0.0.1:1").expect("valid http endpoint"),
            ),
            service_name: Some("portfolio-test".to_owned()),
            deployment_environment: None,
            trusted_ca: None,
        };
        let (layer, _guard) = config
            .layers::<tracing_subscriber::Registry>()
            .expect("http endpoint builds exporters");
        assert!(
            layer.is_some(),
            "exporting mode emits the composed span + log layer"
        );
    }

    #[test]
    fn otel_filter_suppresses_below_info_for_all_targets() {
        // The INFO floor is what keeps the exporter's own client stack (hyper/tonic
        // trace+debug chatter) from flooding the collector.
        let filter = otel_filter();
        for target in ["server::telemetry", "hyper", "tonic", "hyper::proto::h1"] {
            assert!(
                !filter.would_enable(target, &Level::DEBUG),
                "{target} debug must not export"
            );
            assert!(
                !filter.would_enable(target, &Level::TRACE),
                "{target} trace must not export"
            );
        }
    }

    #[test]
    fn otel_filter_passes_info_and_above_for_all_targets() {
        // Client-stack info+ is deliberately allowed (bounded — e.g. one warn per
        // failed export attempt); the flood the filter exists for was trace/debug
        // volume, which the INFO floor already suppresses.
        let filter = otel_filter();
        for target in ["server::telemetry", "hyper"] {
            assert!(filter.would_enable(target, &Level::INFO));
            assert!(filter.would_enable(target, &Level::ERROR));
        }
    }

    /// The log half of the pipeline, end to end through the production composition:
    /// tracing events cross the bridge into OTel log records at info and above, and
    /// below-info events — whatever their target — never reach the exporter.
    #[test]
    fn log_bridge_exports_info_and_above_only() {
        use opentelemetry::logs::Severity;
        use opentelemetry_sdk::logs::InMemoryLogExporter;

        let log_exporter = InMemoryLogExporter::default();
        // Simple (synchronous) exporter: each record exports as it is emitted, so
        // no batch thread or flush timing is involved.
        let logger_provider = SdkLoggerProvider::builder()
            .with_simple_exporter(log_exporter.clone())
            .build();
        // The span side of the composed layer is unused here.
        let tracer_provider = SdkTracerProvider::builder().build();

        let layer =
            compose_otel_layer::<tracing_subscriber::Registry>(&tracer_provider, &logger_provider);
        let _guard = tracing::subscriber::set_default(tracing_subscriber::registry().with(layer));

        tracing::trace!("trace event");
        tracing::debug!("debug event");
        tracing::info!("info event");
        tracing::warn!("warn event");
        tracing::error!("error event");
        // The observed collector-flood shape: client-stack chatter below info.
        tracing::debug!(target: "hyper", "client chatter");

        let severities: Vec<_> = log_exporter
            .get_emitted_logs()
            .expect("in-memory exporter is readable")
            .iter()
            .map(|log| log.record.severity_number())
            .collect();
        assert_eq!(
            severities,
            [
                Some(Severity::Info),
                Some(Severity::Warn),
                Some(Severity::Error)
            ],
            "exactly info/warn/error cross the bridge; trace/debug (any target) do not"
        );
    }

    #[test]
    fn resource_carries_configured_identity() {
        let config = TelemetryConfig {
            mode: TelemetryMode::Disabled,
            service_name: Some("portfolio-test".to_owned()),
            deployment_environment: Some("testing".to_owned()),
            trusted_ca: None,
        };
        let resource = config.build_resource();
        assert_eq!(
            resource.get(&Key::from_static_str(SERVICE_NAME)),
            Some("portfolio-test".into())
        );
        assert_eq!(
            resource.get(&Key::from_static_str(DEPLOYMENT_ENVIRONMENT_NAME)),
            Some("testing".into())
        );
        assert_eq!(
            resource.get(&Key::from_static_str(SERVICE_VERSION)),
            Some(env!("CARGO_PKG_VERSION").into())
        );
        // service.instance.id is a fresh UUID per boot — present, but its value is
        // deliberately not asserted.
        assert!(
            resource
                .get(&Key::from_static_str(SERVICE_INSTANCE_ID))
                .is_some()
        );
    }

    #[test]
    fn resource_omits_deployment_environment_when_unconfigured() {
        let config = TelemetryConfig {
            mode: TelemetryMode::Disabled,
            service_name: None,
            deployment_environment: None,
            trusted_ca: None,
        };
        let resource = config.build_resource();
        assert_eq!(
            resource.get(&Key::from_static_str(DEPLOYMENT_ENVIRONMENT_NAME)),
            None
        );
    }

    // `install()` is deliberately untested: it installs the process-global
    // subscriber and panic hook, which can happen only once per test binary.
}
