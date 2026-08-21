//! Plain-http redirect listener config: when TLS terminates at the site
//! listener, port 80 would otherwise be dead; this listener answers every
//! plain-http request with a 301 to the public base URL. Same env contract
//! as HEALTH_ADDR: absent = no listener, present-but-broken = loud failure.

use std::net::SocketAddr;

use app::meta::PublicBaseUrl;
use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

use crate::security::{self, Hsts};
use crate::www_redirect::location_for;

/// Env var naming the socket address for the plain-http redirect listener.
pub const HTTP_REDIRECT_ADDR_VAR: &str = "HTTP_REDIRECT_ADDR";

#[derive(Debug, Error, PartialEq)]
pub enum HttpRedirectError {
    /// `HTTP_REDIRECT_ADDR` is present but not a parseable socket address.
    #[error(
        "{HTTP_REDIRECT_ADDR_VAR} {0:?} is not a valid socket address (want e.g. 0.0.0.0:8081)"
    )]
    Addr(String),
    /// A redirect listener with no target is a misconfiguration.
    #[error(
        "{HTTP_REDIRECT_ADDR_VAR} is set but PUBLIC_BASE_URL is not; the listener needs a redirect target"
    )]
    MissingBaseUrl,
}

/// Resolved redirect-listener configuration: where to bind, where to send.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRedirect {
    pub addr: SocketAddr,
    pub base: PublicBaseUrl,
}

/// Resolve `HTTP_REDIRECT_ADDR` against the already-resolved public base URL.
/// Absent/empty means no listener. Pure, like the sibling resolvers.
pub fn resolve(
    raw: Option<&str>,
    base: Option<&PublicBaseUrl>,
) -> Result<Option<HttpRedirect>, HttpRedirectError> {
    let Some(raw) = raw.filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let addr: SocketAddr = raw
        .parse()
        .map_err(|_| HttpRedirectError::Addr(raw.to_owned()))?;
    let base = base.ok_or(HttpRedirectError::MissingBaseUrl)?;
    Ok(Some(HttpRedirect {
        addr,
        base: base.clone(),
    }))
}

/// Build the redirect listener's complete app: every request 301s to the
/// public base URL with the original path and query, wearing the same constant
/// security headers every listener sends (HSTS off: plain http by design,
/// mirroring `health_app`).
pub fn redirect_app(base: PublicBaseUrl) -> Router {
    Router::new()
        .fallback(redirect_all)
        .with_state(base)
        .layer(from_fn_with_state(
            Hsts::Off,
            security::set_security_headers,
        ))
}

async fn redirect_all(State(base): State<PublicBaseUrl>, request: Request) -> Response {
    let mut response = StatusCode::MOVED_PERMANENTLY.into_response();
    response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&location_for(&base, request.uri()))
            .expect("a validated base URL and request URI compose into a valid header value"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> PublicBaseUrl {
        PublicBaseUrl::new("https://atmiller.org").expect("valid base URL")
    }

    #[test]
    fn absent_is_none() {
        assert_eq!(resolve(None, Some(&base())), Ok(None));
    }

    #[test]
    fn empty_is_none() {
        assert_eq!(resolve(Some(""), Some(&base())), Ok(None));
    }

    #[test]
    fn valid_addr_with_base_resolves() {
        let resolved = resolve(Some("0.0.0.0:8081"), Some(&base()))
            .expect("valid pair")
            .expect("present value should be Some");
        assert_eq!(
            resolved.addr,
            "0.0.0.0:8081"
                .parse::<SocketAddr>()
                .expect("literal parses")
        );
        assert_eq!(resolved.base, base());
    }

    #[test]
    fn malformed_addr_errors() {
        // A bare host with no port: the likeliest real misconfiguration.
        let err = resolve(Some("0.0.0.0"), Some(&base()))
            .expect_err("malformed address is a misconfiguration");
        assert!(
            matches!(err, HttpRedirectError::Addr(ref raw) if raw == "0.0.0.0"),
            "error should carry the rejected value, got: {err}"
        );
    }

    #[test]
    fn addr_without_base_errors() {
        // Set in an environment that never resolved a public identity: fail
        // at startup, not silently at request time.
        assert_eq!(
            resolve(Some("0.0.0.0:8081"), None),
            Err(HttpRedirectError::MissingBaseUrl)
        );
    }

    #[test]
    fn malformed_addr_reported_before_missing_base() {
        // Both wrong at once: the addr error wins so the operator fixes the
        // value they actually typed first.
        assert!(matches!(
            resolve(Some("nonsense"), None),
            Err(HttpRedirectError::Addr(_))
        ));
    }
}
