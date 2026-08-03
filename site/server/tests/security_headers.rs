//! Every response carries the constant security headers, and each document
//! response carries a `Content-Security-Policy` whose script nonce matches the
//! nonce Leptos stamped on the inline hydration `<script>`. A header/script
//! nonce mismatch would silently break hydration, so it is asserted directly.

mod common;

use std::collections::BTreeMap;

use axum::body::Body;
use axum::http::StatusCode;
use common::{body_string, test_router};
use server::READYZ_PATH;
use server::security::Hsts;

// HSTS is excluded: it is listener-conditional and has its own tests.
const CONSTANT_SECURITY_HEADERS: [&str; 7] = [
    "x-content-type-options",
    "referrer-policy",
    "x-frame-options",
    "permissions-policy",
    "x-xss-protection",
    "cross-origin-opener-policy",
    "cross-origin-resource-policy",
];

// Written out rather than imported from `server`: importing would only check
// that the constant equals itself.
const EXPECTED_REFERRER_POLICY: &str = "strict-origin-when-cross-origin";
const EXPECTED_PERMISSIONS_POLICY: &str = "geolocation=(), camera=(), microphone=(), \
usb=(), payment=(), accelerometer=(), gyroscope=(), magnetometer=(), \
autoplay=(), fullscreen=(), picture-in-picture=(), display-capture=()";

async fn get_with(hsts: Hsts, uri: &str) -> axum::http::Response<Body> {
    common::get(test_router(hsts), uri).await
}

async fn get(uri: &str) -> axum::http::Response<Body> {
    get_with(Hsts::Off, uri).await
}

/// Extract the value delimited by `open` .. `close`, starting the search at the
/// first `open`. Returns None if either delimiter is missing.
fn between<'a>(haystack: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = haystack.find(open)? + open.len();
    let rest = &haystack[start..];
    let end = rest.find(close)?;
    Some(&rest[..end])
}

#[tokio::test]
async fn document_csp_nonce_matches_the_hydration_script_nonce() {
    let response = get("/").await;

    let csp = response
        .headers()
        .get("content-security-policy")
        .expect("a document response must carry a CSP header")
        .to_str()
        .expect("CSP header is ASCII")
        .to_owned();

    let html = body_string(response).await;

    let header_nonce = between(&csp, "'nonce-", "'").expect("CSP script-src carries a nonce");
    // The first nonce="" attribute in the document is the hydration <script>.
    let script_nonce = between(&html, "nonce=\"", "\"").expect("hydration script carries a nonce");

    assert!(!header_nonce.is_empty(), "nonce must be non-empty");
    assert_eq!(
        header_nonce, script_nonce,
        "CSP header nonce must equal the hydration script nonce, else hydration is blocked"
    );
}

#[tokio::test]
async fn error_page_also_carries_a_csp_header() {
    let response = get("/this-route-does-not-exist").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(
        response.headers().contains_key("content-security-policy"),
        "the fallback error page must also carry a CSP header"
    );
}

#[tokio::test]
async fn every_response_carries_the_constant_security_headers() {
    for uri in ["/", "/this-route-does-not-exist", READYZ_PATH] {
        let headers = get(uri).await.headers().clone();

        assert_eq!(
            headers.get("x-content-type-options").map(|v| v.as_bytes()),
            Some(&b"nosniff"[..]),
            "{uri} must send X-Content-Type-Options: nosniff"
        );
        assert_eq!(
            headers.get("x-frame-options").map(|v| v.as_bytes()),
            Some(&b"DENY"[..]),
            "{uri} must send X-Frame-Options: DENY"
        );
        assert_eq!(
            headers.get("referrer-policy").map(|v| v.as_bytes()),
            Some(EXPECTED_REFERRER_POLICY.as_bytes()),
            "{uri} must send Referrer-Policy: {EXPECTED_REFERRER_POLICY}"
        );
        assert_eq!(
            headers.get("permissions-policy").map(|v| v.as_bytes()),
            Some(EXPECTED_PERMISSIONS_POLICY.as_bytes()),
            "{uri} must send the full Permissions-Policy denylist"
        );
        assert_eq!(
            headers.get("x-xss-protection").map(|v| v.as_bytes()),
            Some(&b"0"[..]),
            "{uri} must send X-XSS-Protection: 0 (disable the legacy auditor)"
        );
        assert_eq!(
            headers
                .get("cross-origin-opener-policy")
                .map(|v| v.as_bytes()),
            Some(&b"same-origin"[..]),
            "{uri} must send Cross-Origin-Opener-Policy: same-origin"
        );
        assert_eq!(
            headers
                .get("cross-origin-resource-policy")
                .map(|v| v.as_bytes()),
            Some(&b"same-origin"[..]),
            "{uri} must send Cross-Origin-Resource-Policy: same-origin"
        );
    }
}

#[tokio::test]
async fn hsts_is_emitted_only_when_the_listener_serves_tls() {
    for uri in ["/", "/this-route-does-not-exist"] {
        let on = get_with(Hsts::On, uri).await.headers().clone();
        assert_eq!(
            on.get("strict-transport-security").map(|v| v.as_bytes()),
            Some(&b"max-age=300"[..]),
            "{uri} must send HSTS when the listener serves TLS"
        );
    }
}

#[tokio::test]
async fn hsts_is_absent_on_plain_http_runs() {
    for uri in ["/", "/this-route-does-not-exist"] {
        let off = get_with(Hsts::Off, uri).await.headers().clone();
        assert!(
            !off.contains_key("strict-transport-security"),
            "{uri} must not send HSTS over plain http - the browser would refuse http on this host until it expires"
        );
    }
}

/// Just the constant headers, so per-response ones (content-type, CSP) don't
/// break the comparison.
fn constant_headers(response: &axum::http::Response<Body>) -> BTreeMap<&str, String> {
    CONSTANT_SECURITY_HEADERS
        .iter()
        .filter_map(|name| {
            let value = response.headers().get(*name)?;
            Some((
                *name,
                value.to_str().expect("policy headers are ASCII").to_owned(),
            ))
        })
        .collect()
}

async fn health_get(uri: &str) -> axum::http::Response<Body> {
    common::get(server::health_app(), uri).await
}

/// The health listener is a separate `Router`; nothing keeps its layer stack in
/// step with the site's. tls.rs checks it in isolation, which cannot detect the
/// two drifting apart.
#[tokio::test]
async fn health_listener_readyz_matches_the_site_listener_readyz() {
    let via_health = health_get(READYZ_PATH).await;
    let via_site = get(READYZ_PATH).await;

    assert_eq!(via_health.status(), StatusCode::OK);
    assert_eq!(via_site.status(), StatusCode::OK);
    assert_eq!(
        constant_headers(&via_health),
        constant_headers(&via_site),
        "both /readyz surfaces must carry an identical set of constant security headers"
    );
}

// No-HSTS on the health listener is covered by tls.rs over a real socket.
