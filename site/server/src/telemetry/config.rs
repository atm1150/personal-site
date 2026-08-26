//! Telemetry configuration — the "what", pure and free of OTel-SDK deps.
//!
//! Resolved from the environment once with no side effects, so it can be inspected
//! before (or without) installing anything. The "how" (building providers and
//! installing the subscriber) lives in [`super::pipeline`].

use std::path::PathBuf;

use super::error::TelemetryError;

/// URL scheme prefixes shared by [`OtlpEndpoint::parse`] (which accepts them) and
/// [`OtlpEndpoint::is_tls`] (which distinguishes them) — one spelling for both.
const HTTP_SCHEME: &str = "http://";
const HTTPS_SCHEME: &str = "https://";

/// A validated OTLP collector endpoint.
///
/// Captures the http/https distinction ([`is_tls`](Self::is_tls)): https export
/// needs a trusted certificate ([`TelemetryConfig::trusted_ca`]), plaintext does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpEndpoint(String);

impl OtlpEndpoint {
    /// Parse a raw endpoint string. Requires an `http`/`https` URL with something
    /// after the scheme — a bare `"http://"` is a misconfiguration, and catching it
    /// here keeps the failure in the validating type instead of deep in the
    /// exporter builder. (Host/port shape beyond that is still the builder's call.)
    pub fn parse(raw: &str) -> Result<Self, TelemetryError> {
        let rest = raw
            .strip_prefix(HTTP_SCHEME)
            .or_else(|| raw.strip_prefix(HTTPS_SCHEME));
        match rest {
            Some(remainder) if !remainder.is_empty() => Ok(Self(raw.to_owned())),
            _ => Err(TelemetryError::Endpoint(raw.to_owned())),
        }
    }

    /// The endpoint as passed to the exporter builder.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the endpoint uses TLS (`https`).
    pub fn is_tls(&self) -> bool {
        self.0.starts_with(HTTPS_SCHEME)
    }
}

/// Whether telemetry exports over OTLP or degrades to stdout-only logging.
///
/// Modeled as an explicit sum type (not `Option` + scattered `is_some()`) so the
/// export decision is resolved once and inspectable by callers.
#[derive(Debug, Clone)]
pub enum TelemetryMode {
    /// Export traces + logs to the given OTLP endpoint.
    Exporting(OtlpEndpoint),
    /// No OTLP endpoint configured; stdout fmt logging only.
    Disabled,
}

impl TelemetryMode {
    /// Resolve the mode from the raw `OTEL_EXPORTER_OTLP_ENDPOINT` value.
    ///
    /// Absent or empty means "not orchestrated" → [`Disabled`](Self::Disabled).
    /// A present-but-malformed endpoint is a misconfiguration and errors, so
    /// startup fails loudly rather than silently running without telemetry.
    fn resolve(endpoint_var: Option<&str>) -> Result<Self, TelemetryError> {
        match endpoint_var {
            None => Ok(Self::Disabled),
            Some("") => Ok(Self::Disabled),
            Some(raw) => OtlpEndpoint::parse(raw).map(Self::Exporting),
        }
    }
}

/// Resolved telemetry configuration — the "what", separate from the "how"
/// ([`install`](TelemetryConfig::install)). Built from the environment once, with no
/// side effects, so callers can inspect it before (or without) installing anything.
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    pub mode: TelemetryMode,
    pub service_name: Option<String>,
    pub deployment_environment: Option<String>,
    /// PEM certificate trusted to verify an https collector (`OTEL_EXPORTER_OTLP_CERTIFICATE`).
    pub trusted_ca: Option<PathBuf>,
}

impl TelemetryConfig {
    /// Resolve configuration from the standard OTEL environment variables.
    /// Errors only if `OTEL_EXPORTER_OTLP_ENDPOINT` is set but malformed.
    pub fn from_env() -> Result<Self, TelemetryError> {
        let mode =
            TelemetryMode::resolve(std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok().as_deref())?;
        Ok(Self {
            mode,
            service_name: non_empty(std::env::var("OTEL_SERVICE_NAME").ok()),
            deployment_environment: non_empty(std::env::var("DEPLOYMENT_ENVIRONMENT").ok()),
            trusted_ca: non_empty(std::env::var("OTEL_EXPORTER_OTLP_CERTIFICATE").ok())
                .map(PathBuf::from),
        })
    }

    /// Whether this config will export over OTLP.
    pub fn is_exporting(&self) -> bool {
        matches!(self.mode, TelemetryMode::Exporting(_))
    }
}

/// `Some("")` and `None` both collapse to `None`.
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_empty() {
        assert!(OtlpEndpoint::parse("").is_err());
    }

    #[test]
    fn parse_rejects_non_http_scheme() {
        assert!(OtlpEndpoint::parse("ftp://collector:4317").is_err());
        assert!(OtlpEndpoint::parse("collector:4317").is_err());
    }

    #[test]
    fn parse_rejects_scheme_only() {
        assert!(OtlpEndpoint::parse("http://").is_err());
        assert!(OtlpEndpoint::parse("https://").is_err());
    }

    #[test]
    fn parse_accepts_http_and_is_not_tls() {
        let ep = OtlpEndpoint::parse("http://localhost:4317").expect("valid http endpoint");
        assert_eq!(ep.as_str(), "http://localhost:4317");
        assert!(!ep.is_tls());
    }

    #[test]
    fn parse_accepts_https_and_is_tls() {
        let ep = OtlpEndpoint::parse("https://localhost:4317").expect("valid https endpoint");
        assert!(ep.is_tls());
    }

    #[test]
    fn resolve_absent_endpoint_is_disabled() {
        assert!(matches!(
            TelemetryMode::resolve(None).expect("absent is not an error"),
            TelemetryMode::Disabled
        ));
    }

    #[test]
    fn resolve_empty_endpoint_is_disabled() {
        assert!(matches!(
            TelemetryMode::resolve(Some("")).expect("empty is not an error"),
            TelemetryMode::Disabled
        ));
    }

    #[test]
    fn resolve_malformed_endpoint_errors() {
        assert!(TelemetryMode::resolve(Some("not-a-url")).is_err());
    }

    #[test]
    fn resolve_valid_endpoint_is_exporting() {
        match TelemetryMode::resolve(Some("http://localhost:4317")).expect("valid endpoint") {
            TelemetryMode::Exporting(ep) => assert_eq!(ep.as_str(), "http://localhost:4317"),
            TelemetryMode::Disabled => panic!("expected Exporting"),
        }
    }
}
