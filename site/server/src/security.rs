//! Constant security response headers, applied to every response.
//!
//! These headers are request-independent, so they live as a blanket Tower layer
//! over the whole router (SSR pages, static assets, and /readyz alike). The
//! per-request `Content-Security-Policy` is deliberately *not* here: it is
//! coupled to Leptos's render nonce and is set during SSR in the `app` crate.

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;

/// Disable every browser feature the site does not use, so a future injected
/// script cannot reach for them either.
const PERMISSIONS_POLICY: &str = "geolocation=(), camera=(), microphone=(), \
usb=(), payment=(), accelerometer=(), gyroscope=(), magnetometer=(), \
autoplay=(), fullscreen=(), picture-in-picture=(), display-capture=()";

/// Add the constant security headers to the response.
///
/// Wire it over the whole router via `axum::middleware::from_fn`.
pub async fn set_security_headers(request: Request, next: Next) -> Response {
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

    response
}
