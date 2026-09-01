//! TLS for the site listener: plain http by default, rustls when the env
//! contract declares TLS intent and provides a certificate. Intent is a
//! separate declaration from the certificate paths - it is never inferred
//! from their presence, so a deploy that forgets to mount its certificate
//! fails to start instead of quietly serving plain http. Also resolves the
//! auxiliary health listener address, which is part of the same contract -
//! it exists so orchestrator probes reach readiness without trusting that
//! certificate.
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
/// Env var declaring whether this run serves TLS. Required to be `on` before the
/// TLS pair is honored: intent is declared, never inferred from file presence.
pub const TLS_SWITCH_VAR: &str = "SITE_TLS";

/// Listener misconfiguration. Startup fails loudly on any of these - a server
/// that silently fell back to plain http when TLS was declared but
/// unreachable would look healthy while violating the operator's intent.
#[derive(Debug, Error)]
pub enum TlsError {
    /// `SITE_TLS` holds something other than `on` or `off`.
    #[error("{TLS_SWITCH_VAR} {0:?} is not valid (want \"on\" or \"off\")")]
    Switch(String),
    /// `SITE_TLS=on` but the certificate material is incomplete.
    #[error(
        "{TLS_SWITCH_VAR} is on but {missing} is not set; TLS was declared, so plain http is not an acceptable fallback"
    )]
    RequiredButMissing { missing: &'static str },
    /// TLS is off but certificate material was supplied anyway.
    #[error(
        "{set} is set but {TLS_SWITCH_VAR} is not on; set {TLS_SWITCH_VAR}=on to serve TLS or unset the certificate paths"
    )]
    DisabledButConfigured { set: &'static str },
    /// `HEALTH_ADDR` is present but not a parseable socket address.
    #[error("{HEALTH_ADDR_VAR} {0:?} is not a valid socket address (want e.g. 127.0.0.1:4002)")]
    HealthAddr(String),
}

/// Whether the site listener terminates TLS.
///
/// An explicit sum type (not `Option<(PathBuf, PathBuf)>`) so an inconsistent
/// combination of declared intent and cert paths is rejected at construction
/// and everywhere a `TlsMode` is in scope it is already validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsMode {
    /// TLS declared off (or left to default): bind plain http (dev default
    /// under `cargo leptos watch`).
    Disabled,
    /// TLS declared on with a full cert/key pair: terminate TLS with it.
    Enabled {
        cert_path: PathBuf,
        key_path: PathBuf,
    },
}

impl TlsMode {
    /// Resolve from declared intent plus the raw pair of env values. Absent and
    /// empty paths are the same (matching the telemetry config's treatment of
    /// `OTEL_*`). Intent is required: TLS is never inferred from file presence,
    /// so a declared-on run with missing material fails instead of downgrading.
    fn resolve(enabled: bool, cert: Option<&str>, key: Option<&str>) -> Result<Self, TlsError> {
        let cert = cert.filter(|s| !s.is_empty());
        let key = key.filter(|s| !s.is_empty());
        match (enabled, cert, key) {
            (true, Some(cert), Some(key)) => Ok(Self::Enabled {
                cert_path: cert.into(),
                key_path: key.into(),
            }),
            (true, None, Some(_)) => Err(TlsError::RequiredButMissing {
                missing: TLS_CERT_VAR,
            }),
            (true, Some(_), None) => Err(TlsError::RequiredButMissing {
                missing: TLS_KEY_VAR,
            }),
            (true, None, None) => Err(TlsError::RequiredButMissing {
                missing: "TLS_CERT_PATH and TLS_KEY_PATH",
            }),
            (false, None, None) => Ok(Self::Disabled),
            (false, Some(_), Some(_)) => Err(TlsError::DisabledButConfigured {
                set: "TLS_CERT_PATH and TLS_KEY_PATH",
            }),
            (false, Some(_), None) => Err(TlsError::DisabledButConfigured { set: TLS_CERT_VAR }),
            (false, None, Some(_)) => Err(TlsError::DisabledButConfigured { set: TLS_KEY_VAR }),
        }
    }
}

/// Where the TLS intent came from, for the startup provenance line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsSource {
    /// `SITE_TLS` was set explicitly.
    Declared,
    /// `SITE_TLS` was absent or empty; plain http by default.
    Defaulted,
}

/// Resolved listener configuration - the "what", read from the environment
/// once with no side effects. The "how" is [`serve`] plus `main`'s wiring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsConfig {
    pub mode: TlsMode,
    /// Where the auxiliary plain-http `/readyz` listener binds, if anywhere.
    pub health_addr: Option<SocketAddr>,
    /// Where the TLS intent came from, for the startup provenance line.
    pub source: TlsSource,
}

impl TlsConfig {
    /// Resolve configuration from `SITE_TLS`/`TLS_CERT_PATH`/`TLS_KEY_PATH`/`HEALTH_ADDR`.
    pub fn from_env() -> Result<Self, TlsError> {
        // Bound before borrowing: `std::env::var(..).ok().as_deref()` inline
        // would drop the temporary while the borrow is still live.
        let switch = std::env::var(TLS_SWITCH_VAR).ok();
        let cert = std::env::var(TLS_CERT_VAR).ok();
        let key = std::env::var(TLS_KEY_VAR).ok();
        let health = std::env::var(HEALTH_ADDR_VAR).ok();
        Self::resolve(
            switch.as_deref(),
            cert.as_deref(),
            key.as_deref(),
            health.as_deref(),
        )
    }

    /// The pure half of [`Self::from_env`], so tests need no env mutation.
    fn resolve(
        switch: Option<&str>,
        cert: Option<&str>,
        key: Option<&str>,
        health: Option<&str>,
    ) -> Result<Self, TlsError> {
        let (enabled, source) = resolve_switch(switch)?;
        Ok(Self {
            mode: TlsMode::resolve(enabled, cert, key)?,
            health_addr: resolve_health_addr(health)?,
            source,
        })
    }
}

/// Resolve the raw `SITE_TLS` value. Absent/empty means plain http; any value
/// other than `on`/`off` is a misconfiguration, not a silent default.
fn resolve_switch(raw: Option<&str>) -> Result<(bool, TlsSource), TlsError> {
    match raw.filter(|s| !s.is_empty()) {
        None => Ok((false, TlsSource::Defaulted)),
        Some(raw) if raw.eq_ignore_ascii_case("on") => Ok((true, TlsSource::Declared)),
        Some(raw) if raw.eq_ignore_ascii_case("off") => Ok((false, TlsSource::Declared)),
        Some(raw) => Err(TlsError::Switch(raw.to_owned())),
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
/// `bounds` carries the connection-level bounds applied to this listener: the
/// http1 header-read (slowloris) timeout and the http2 keep-alive bound.
/// Resolves when the server has fully shut down.
pub async fn serve(
    app: Router,
    addr: SocketAddr,
    cert_path: impl AsRef<Path>,
    key_path: impl AsRef<Path>,
    handle: Handle<SocketAddr>,
    bounds: &crate::limits::ConnectionLimits,
) -> std::io::Result<()> {
    let config = RustlsConfig::from_pem_file(cert_path, key_path).await?;
    let mut server = axum_server::bind_rustls(addr, config).handle(handle);
    crate::limits::apply_connection_bounds(&mut server, bounds);
    server
        // with_connect_info: the rate limiter keys on the peer address.
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- switch parsing ---

    #[test]
    fn switch_absent_defaults_to_off() {
        assert_eq!(
            resolve_switch(None).expect("absent is not an error"),
            (false, TlsSource::Defaulted)
        );
    }

    #[test]
    fn switch_empty_defaults_to_off() {
        // Empty equals absent, matching how the cert/key pair treats empty values.
        assert_eq!(
            resolve_switch(Some("")).expect("empty is not an error"),
            (false, TlsSource::Defaulted)
        );
    }

    #[test]
    fn switch_on_is_declared() {
        assert_eq!(
            resolve_switch(Some("on")).expect("on is valid"),
            (true, TlsSource::Declared)
        );
    }

    #[test]
    fn switch_off_is_declared() {
        assert_eq!(
            resolve_switch(Some("off")).expect("off is valid"),
            (false, TlsSource::Declared)
        );
    }

    #[test]
    fn switch_is_case_insensitive() {
        assert_eq!(
            resolve_switch(Some("ON")).expect("ON is valid"),
            (true, TlsSource::Declared)
        );
        // Both arms, not just "on": a case-sensitive `raw == "off"` comparison
        // would pass every other test here while turning SITE_TLS=OFF into a
        // startup failure.
        assert_eq!(
            resolve_switch(Some("OFF")).expect("OFF is valid"),
            (false, TlsSource::Declared)
        );
    }

    #[test]
    fn switch_rejects_anything_else() {
        // "true" is the likeliest wrong guess at the vocabulary; a silently
        // ignored value here would serve plain http while the operator believes
        // TLS is on, which is the exact failure this issue exists to remove.
        let err = resolve_switch(Some("true")).expect_err("only on/off are accepted");
        assert!(
            matches!(err, TlsError::Switch(ref raw) if raw == "true"),
            "error should carry the rejected value, got: {err}"
        );
    }

    // --- mode resolution, now driven by intent ---

    #[test]
    fn tls_off_without_paths_is_disabled() {
        assert_eq!(
            TlsMode::resolve(false, None, None).expect("off with no paths is valid"),
            TlsMode::Disabled
        );
    }

    #[test]
    fn tls_on_with_full_pair_is_enabled_with_the_given_paths() {
        let mode = TlsMode::resolve(true, Some("/certs/site.pem"), Some("/certs/site.key"))
            .expect("on with a full pair is valid");
        assert_eq!(
            mode,
            TlsMode::Enabled {
                cert_path: PathBuf::from("/certs/site.pem"),
                key_path: PathBuf::from("/certs/site.key"),
            }
        );
    }

    #[test]
    fn tls_on_without_key_errors() {
        let err = TlsMode::resolve(true, Some("/certs/site.pem"), None)
            .expect_err("declared TLS must not fall back to plain http");
        assert!(
            matches!(err, TlsError::RequiredButMissing { missing } if missing == TLS_KEY_VAR),
            "error should name the missing variable, got: {err}"
        );
    }

    #[test]
    fn tls_on_without_cert_errors() {
        let err = TlsMode::resolve(true, None, Some("/certs/site.key"))
            .expect_err("declared TLS must not fall back to plain http");
        assert!(
            matches!(err, TlsError::RequiredButMissing { missing } if missing == TLS_CERT_VAR),
            "error should name the missing variable, got: {err}"
        );
    }

    #[test]
    fn tls_on_without_either_names_both() {
        // The deploy-forgot-to-mount-the-certs case: previously this booted and
        // served plain http with a green /readyz.
        let err = TlsMode::resolve(true, None, None)
            .expect_err("declared TLS with no materials must fail closed");
        assert!(
            matches!(err, TlsError::RequiredButMissing { missing } if missing.contains(TLS_CERT_VAR) && missing.contains(TLS_KEY_VAR)),
            "error should name both missing variables, got: {err}"
        );
    }

    #[test]
    fn tls_on_with_empty_paths_errors() {
        // Empty equals absent, so this is the missing-materials case.
        assert!(TlsMode::resolve(true, Some(""), Some("")).is_err());
    }

    #[test]
    fn tls_off_with_paths_errors() {
        // Certs mounted but intent never flipped: a contradiction, not a
        // preference. Silently ignoring the certs would hide a deploy mistake.
        let err = TlsMode::resolve(false, Some("/certs/site.pem"), None)
            .expect_err("off with certs present is a contradiction");
        assert!(
            matches!(err, TlsError::DisabledButConfigured { set } if set == TLS_CERT_VAR),
            "error should name the offending variable, got: {err}"
        );
    }

    #[test]
    fn tls_off_with_both_paths_names_both() {
        // The likeliest real occurrence of this error: a deploy mounts the full
        // certificate pair but never flips the intent switch. Mirrors how the
        // on-side names both missing variables.
        let err = TlsMode::resolve(false, Some("/certs/site.pem"), Some("/certs/site.key"))
            .expect_err("off with certs present is a contradiction");
        assert!(
            matches!(err, TlsError::DisabledButConfigured { set } if set.contains(TLS_CERT_VAR) && set.contains(TLS_KEY_VAR)),
            "error should name both offending variables, got: {err}"
        );
    }

    // --- provenance ---

    #[test]
    fn config_carries_the_intent_source_for_the_provenance_line() {
        // The startup line must be able to distinguish "operator said off" from
        // "nobody said anything", which is the ambiguity this issue removes.
        let declared = TlsConfig::resolve(Some("off"), None, None, None)
            .expect("off with no material is valid");
        assert_eq!(declared.source, TlsSource::Declared);
        assert_eq!(declared.mode, TlsMode::Disabled);

        let defaulted = TlsConfig::resolve(None, None, None, None).expect("absent is valid");
        assert_eq!(defaulted.source, TlsSource::Defaulted);
        assert_eq!(defaulted.mode, TlsMode::Disabled);
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
