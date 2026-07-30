//! Security response headers: a constant set applied to every response, plus
//! one listener-conditional header.
//!
//! Most of these headers are request-independent, so they live as a blanket
//! Tower layer over the whole router (SSR pages, static assets, and /readyz
//! alike). `Strict-Transport-Security` is the exception: it is only correct
//! on a listener that actually serves TLS, so it is threaded through as
//! per-listener state ([`Hsts`]) rather than emitted unconditionally. The
//! per-request `Content-Security-Policy` is deliberately *not* here: it is
//! coupled to Leptos's render nonce and is set during SSR in the `app` crate.

use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;

use crate::tls::TlsMode;

/// Whether responses from a listener carry `Strict-Transport-Security`.
///
/// Passed in per listener rather than decided here: the site listener sets it
/// from the resolved TLS intent, and the auxiliary plain-http health listener is
/// always handed [`Hsts::Off`], so "health never emits HSTS" holds by
/// construction instead of by anyone remembering the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hsts {
    /// Plain-http listener: emit nothing.
    Off,
    /// TLS listener: emit [`HSTS_VALUE`].
    On,
}

/// HSTS is a promise that this host is always https, so it must follow the
/// resolved TLS intent - never the presence of certificate files. The
/// conversion lives here, next to the type it produces, rather than as an
/// inline match at the call site in `main`.
impl From<&TlsMode> for Hsts {
    fn from(mode: &TlsMode) -> Self {
        match mode {
            TlsMode::Enabled { .. } => Hsts::On,
            TlsMode::Disabled => Hsts::Off,
        }
    }
}

/// Five minutes. Deliberately short while the production TLS termination point is
/// undecided: HSTS is client-side sticky and cannot be withdrawn from the server,
/// and orchestrated dev serves TLS, so a developer's own browser is the first
/// client to receive it. The ramp toward a year belongs to the deploy work, with
/// `includeSubDomains` and `preload` still deliberately absent.
const HSTS_VALUE: &str = "max-age=300";

/// Disable every browser feature the site does not use, so a future injected
/// script cannot reach for them either.
const PERMISSIONS_POLICY: &str = "geolocation=(), camera=(), microphone=(), \
usb=(), payment=(), accelerometer=(), gyroscope=(), magnetometer=(), \
autoplay=(), fullscreen=(), picture-in-picture=(), display-capture=()";

/// Add the constant security headers to the response, plus HSTS when the
/// listener this middleware is layered on serves TLS.
///
/// Wire it over the whole router via `axum::middleware::from_fn_with_state`.
pub async fn set_security_headers(
    State(hsts): State<Hsts>,
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    // Belt-and-suspenders alongside the CSP `frame-ancestors 'none'` directive,
    // for user agents that predate frame-ancestors.
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(PERMISSIONS_POLICY),
    );
    // The legacy XSS auditor is deprecated and removed from modern browsers (it
    // was itself a source of vulnerabilities); `0` turns it off entirely rather
    // than leaving any old implementation to its default-on behavior.
    headers.insert(header::X_XSS_PROTECTION, HeaderValue::from_static("0"));
    // Sever `window.opener` from cross-origin windows (anti-tabnabbing).
    headers.insert(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    // Stop other origins from embedding this site's resources.
    headers.insert(
        HeaderName::from_static("cross-origin-resource-policy"),
        HeaderValue::from_static("same-origin"),
    );

    if hsts == Hsts::On {
        headers.insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static(HSTS_VALUE),
        );
    }

    response
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// A swapped match arm in the `From` impl must turn this red - that is
    /// the entire reason the conversion was pulled out of `main` and into a
    /// unit-testable seam.
    #[test]
    fn enabled_tls_mode_converts_to_hsts_on() {
        let mode = TlsMode::Enabled {
            cert_path: PathBuf::from("cert.pem"),
            key_path: PathBuf::from("key.pem"),
        };
        assert_eq!(Hsts::from(&mode), Hsts::On);
    }

    #[test]
    fn disabled_tls_mode_converts_to_hsts_off() {
        assert_eq!(Hsts::from(&TlsMode::Disabled), Hsts::Off);
    }
}
