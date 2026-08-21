//! `www.<public host>` -> `PUBLIC_BASE_URL` redirect: derived state + middleware.
//!
//! TLS terminates at this server, so the binary serves the redirect itself.
//! The target is derived from [`PublicBaseUrl`] rather than declared as a
//! second value, so it can never disagree with the site's public identity.
//! Mirrors `crate::security`'s [`crate::security::Hsts`] pattern for
//! per-listener state.

use app::meta::PublicBaseUrl;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// Whether the listener this middleware is layered on redirects
/// `www.<host>` to the public base URL. Per-listener state, like
/// [`crate::security::Hsts`].
#[derive(Debug, Clone, PartialEq)]
pub enum WwwRedirect {
    /// No public base URL declared: every `Host` is served as-is.
    Off,
    /// Requests for `www.<base's host>` get a 301 to `<base><path>`.
    On {
        base: PublicBaseUrl,
        /// Precomputed `www.<host>` comparison target, built once instead of
        /// per request.
        www_host: String,
    },
}

impl WwwRedirect {
    /// Derive from the already-validated public base URL. `None` -> `Off`
    /// (dev, or any run without a public identity configured).
    pub fn derive(base: Option<&PublicBaseUrl>) -> WwwRedirect {
        let Some(base) = base else {
            return WwwRedirect::Off;
        };
        let www_host = format!("www.{}", base.host());
        WwwRedirect::On {
            base: base.clone(),
            www_host,
        }
    }
}

/// Redirect `www.<host>` to the public base URL with a 301, carrying the
/// original path and query. Every other `Host` (the apex itself, localhost,
/// LAN IPs, anything unset) passes through untouched.
///
/// Layer this *inside* (added before) `set_security_headers`, so the 301
/// response it produces still picks up the constant security headers on the
/// way back out.
pub async fn redirect_www(
    State(www_redirect): State<WwwRedirect>,
    request: Request,
    next: Next,
) -> Response {
    let WwwRedirect::On { base, www_host } = &www_redirect else {
        return next.run(request).await;
    };

    let Some(host) = request_host(&request) else {
        return next.run(request).await;
    };
    let host = strip_port(host);

    if !host.eq_ignore_ascii_case(www_host) {
        return next.run(request).await;
    }

    let location = location_for(base, request.uri());

    let mut response = StatusCode::MOVED_PERMANENTLY.into_response();
    response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&location)
            .expect("a validated base URL and request URI compose into a valid header value"),
    );
    response
}

/// Compose a redirect target: the public base URL joined with the request's
/// path and query. Falls back to "/" for the asterisk-form request-target
/// (`OPTIONS * HTTP/1.1`), which would otherwise compose a malformed target.
pub(crate) fn location_for(base: &PublicBaseUrl, uri: &axum::http::Uri) -> String {
    let path_and_query = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .filter(|pq| pq.starts_with('/'))
        .unwrap_or("/");
    base.join(path_and_query)
}

/// The request's host: the `Host` header first, falling back to the URI's
/// authority (HTTP/2 requests may carry `:authority` instead of a `Host`
/// header). `None` when neither is present.
fn request_host(request: &Request) -> Option<&str> {
    request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .or_else(|| {
            request
                .uri()
                .authority()
                .map(|authority| authority.as_str())
        })
}

/// Strip an optional `:port` suffix so `www.atmiller.org:8080` still compares
/// equal to `www.atmiller.org`.
///
/// A bracketed IPv6 literal (`[::1]:8080`) would mis-split on its internal
/// colons, but that is inert here: the result can never start with `www.`,
/// so no such host can ever be the redirect target being compared against.
fn strip_port(host: &str) -> &str {
    host.rsplit_once(':').map_or(host, |(host, _port)| host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_with_a_base_url_produces_on_with_the_precomputed_www_host() {
        let base = PublicBaseUrl::new("https://atmiller.org").expect("valid base URL");
        assert_eq!(
            WwwRedirect::derive(Some(&base)),
            WwwRedirect::On {
                base,
                www_host: "www.atmiller.org".to_owned(),
            }
        );
    }

    #[test]
    fn derive_with_no_base_url_is_off() {
        assert_eq!(WwwRedirect::derive(None), WwwRedirect::Off);
    }

    #[test]
    fn derive_precomputes_the_www_host_from_the_base_urls_host_alone() {
        // The base URL's port must not leak into www_host: incoming hosts
        // are compared only after strip_port has removed theirs.
        let base = PublicBaseUrl::new("https://atmiller.org:8443").expect("valid base URL");
        let WwwRedirect::On { www_host, .. } = WwwRedirect::derive(Some(&base)) else {
            panic!("Some(base) must derive On");
        };
        assert_eq!(www_host, "www.atmiller.org");
    }
}
