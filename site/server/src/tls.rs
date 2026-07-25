//! TLS for the site listener: plain http by default, rustls when the env
//! contract provides a certificate. Also resolves the auxiliary health
//! listener address, which is part of the same contract - it exists so
//! orchestrator probes reach readiness without trusting that certificate.
//!
//! Mirrors [`crate::telemetry`]'s config shape: pure `resolve` functions that
//! never touch the process environment (so tests need no env mutation), with
//! [`TlsConfig::from_env`] as the thin edge that reads the real variables.
//! The env contract is deliberately supplier-agnostic - locally the Aspire
//! AppHost injects the developer certificate's paths; in production the
//! container injects paths to a real certificate. Same binary, same branch.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use axum::Router;
use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use thiserror::Error;

/// Env var naming the PEM certificate (chain) file. Paired with [`TLS_KEY_VAR`].
pub const TLS_CERT_VAR: &str = "TLS_CERT_PATH";
/// Env var naming the PEM private-key file. Paired with [`TLS_CERT_VAR`].
pub const TLS_KEY_VAR: &str = "TLS_KEY_PATH";
/// Env var naming the socket address for the auxiliary plain-http health listener.
pub const HEALTH_ADDR_VAR: &str = "HEALTH_ADDR";

/// Listener misconfiguration. Startup fails loudly on any of these - a server
/// that silently fell back to plain http when TLS was half-configured would
/// look healthy while violating the operator's intent.
#[derive(Debug, Error)]
pub enum TlsError {
    /// Exactly one of the TLS pair is set.
    #[error("{set} is set but {missing} is not; set both to serve TLS or neither for plain http")]
    HalfConfigured {
        set: &'static str,
        missing: &'static str,
    },
    /// `HEALTH_ADDR` is present but not a parseable socket address.
    #[error("{HEALTH_ADDR_VAR} {0:?} is not a valid socket address (want e.g. 127.0.0.1:4002)")]
    HealthAddr(String),
}

/// Whether the site listener terminates TLS.
///
/// An explicit sum type (not `Option<(PathBuf, PathBuf)>`) so the half-set env
/// state is rejected at construction and everywhere a `TlsMode` is in scope it
/// is already validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsMode {
    /// No TLS vars set: bind plain http (dev default under `cargo leptos watch`).
    Disabled,
    /// Both TLS vars set: terminate TLS with this PEM cert/key pair.
    Enabled {
        cert_path: PathBuf,
        key_path: PathBuf,
    },
}

impl TlsMode {
    /// Resolve from the raw pair of env values. Absent and empty are the same
    /// (matching the telemetry config's treatment of `OTEL_*`); a half-set
    /// pair is a misconfiguration, not a fallback to plain http.
    fn resolve(cert: Option<&str>, key: Option<&str>) -> Result<Self, TlsError> {
        let cert = cert.filter(|s| !s.is_empty());
        let key = key.filter(|s| !s.is_empty());
        match (cert, key) {
            (None, None) => Ok(Self::Disabled),
            (Some(cert), Some(key)) => Ok(Self::Enabled {
                cert_path: cert.into(),
                key_path: key.into(),
            }),
            (Some(_), None) => Err(TlsError::HalfConfigured {
                set: TLS_CERT_VAR,
                missing: TLS_KEY_VAR,
            }),
            (None, Some(_)) => Err(TlsError::HalfConfigured {
                set: TLS_KEY_VAR,
                missing: TLS_CERT_VAR,
            }),
        }
    }
}

/// Resolved listener configuration - the "what", read from the environment
/// once with no side effects. The "how" is [`serve`] plus `main`'s wiring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsConfig {
    pub mode: TlsMode,
    /// Where the auxiliary plain-http `/readyz` listener binds, if anywhere.
    pub health_addr: Option<SocketAddr>,
}

impl TlsConfig {
    /// Resolve configuration from `TLS_CERT_PATH`/`TLS_KEY_PATH`/`HEALTH_ADDR`.
    pub fn from_env() -> Result<Self, TlsError> {
        let cert = std::env::var(TLS_CERT_VAR).ok();
        let key = std::env::var(TLS_KEY_VAR).ok();
        let health = std::env::var(HEALTH_ADDR_VAR).ok();
        Ok(Self {
            mode: TlsMode::resolve(cert.as_deref(), key.as_deref())?,
            health_addr: resolve_health_addr(health.as_deref())?,
        })
    }
}

/// Resolve the raw `HEALTH_ADDR` value. Absent/empty means no health listener.
fn resolve_health_addr(raw: Option<&str>) -> Result<Option<SocketAddr>, TlsError> {
    match raw.filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(raw) => raw
            .parse()
            .map(Some)
            .map_err(|_| TlsError::HealthAddr(raw.to_owned())),
    }
}

/// Serve `app` over TLS at `addr`, reading the PEM pair from disk.
///
/// `handle` is the axum-server control handle: `main` uses it for graceful
/// shutdown, tests use [`Handle::listening`] to learn the ephemeral bound port.
/// Resolves when the server has fully shut down.
pub async fn serve(
    app: Router,
    addr: SocketAddr,
    cert_path: impl AsRef<Path>,
    key_path: impl AsRef<Path>,
    handle: Handle<SocketAddr>,
) -> std::io::Result<()> {
    let config = RustlsConfig::from_pem_file(cert_path, key_path).await?;
    axum_server::bind_rustls(addr, config)
        .handle(handle)
        .serve(app.into_make_service())
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tls_absent_pair_is_disabled() {
        assert_eq!(
            TlsMode::resolve(None, None).expect("absent pair is not an error"),
            TlsMode::Disabled
        );
    }

    #[test]
    fn tls_empty_pair_is_disabled() {
        assert_eq!(
            TlsMode::resolve(Some(""), Some("")).expect("empty pair is not an error"),
            TlsMode::Disabled
        );
    }

    #[test]
    fn tls_full_pair_is_enabled_with_the_given_paths() {
        let mode = TlsMode::resolve(Some("/certs/site.pem"), Some("/certs/site.key"))
            .expect("full pair is valid");
        assert_eq!(
            mode,
            TlsMode::Enabled {
                cert_path: PathBuf::from("/certs/site.pem"),
                key_path: PathBuf::from("/certs/site.key"),
            }
        );
    }

    #[test]
    fn tls_cert_without_key_errors() {
        let err = TlsMode::resolve(Some("/certs/site.pem"), None)
            .expect_err("half-configured TLS must not silently fall back to http");
        assert!(
            matches!(
                err,
                TlsError::HalfConfigured { set, missing }
                    if set == TLS_CERT_VAR && missing == TLS_KEY_VAR
            ),
            "error should name the offending pair, got: {err}"
        );
    }

    #[test]
    fn tls_key_without_cert_errors() {
        let err = TlsMode::resolve(None, Some("/certs/site.key"))
            .expect_err("half-configured TLS must not silently fall back to http");
        assert!(
            matches!(
                err,
                TlsError::HalfConfigured { set, missing }
                    if set == TLS_KEY_VAR && missing == TLS_CERT_VAR
            ),
            "error should name the offending pair, got: {err}"
        );
    }

    #[test]
    fn tls_empty_cert_with_key_errors() {
        // Empty equals absent, so this is the half-configured case, not Enabled.
        assert!(TlsMode::resolve(Some(""), Some("/certs/site.key")).is_err());
    }

    #[test]
    fn health_absent_is_none() {
        assert_eq!(
            resolve_health_addr(None).expect("absent is not an error"),
            None
        );
    }

    #[test]
    fn health_empty_is_none() {
        assert_eq!(
            resolve_health_addr(Some("")).expect("empty is not an error"),
            None
        );
    }

    #[test]
    fn health_valid_addr_parses() {
        assert_eq!(
            resolve_health_addr(Some("127.0.0.1:4002")).expect("valid addr"),
            Some("127.0.0.1:4002".parse().expect("literal parses"))
        );
    }

    #[test]
    fn health_malformed_addr_errors() {
        // A bare host with no port: the likeliest real misconfiguration, and
        // still malformed since a SocketAddr requires `host:port`.
        let err = resolve_health_addr(Some("127.0.0.1"))
            .expect_err("malformed HEALTH_ADDR is a misconfiguration");
        assert!(
            matches!(err, TlsError::HealthAddr(ref raw) if raw == "127.0.0.1"),
            "error should carry the rejected value, got: {err}"
        );
    }
}
