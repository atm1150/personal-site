//! Every response carries the constant security headers, and each document
//! response carries a `Content-Security-Policy` whose script nonce matches the
//! nonce Leptos stamped on the inline hydration `<script>`. A header/script
//! nonce mismatch would silently break hydration, so it is asserted directly.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use leptos::prelude::LeptosOptions;
use tower::ServiceExt; // brings `oneshot` onto the router

/// site_root defaults to ".", so the fallback's static-file probe misses and
/// SSR runs; output_name is the only field without a default.
fn test_router() -> axum::Router {
    let options = LeptosOptions::builder().output_name("portfolio").build();
    server::router(options)
}

async fn get(uri: &str) -> axum::http::Response<Body> {
    test_router()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

async fn body_string(response: axum::http::Response<Body>) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should collect");
    String::from_utf8(bytes.to_vec()).expect("body should be UTF-8")
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
    for uri in ["/", "/this-route-does-not-exist"] {
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
        assert!(
            headers.contains_key("referrer-policy"),
            "{uri} must send a Referrer-Policy"
        );
        assert!(
            headers.contains_key("permissions-policy"),
            "{uri} must send a Permissions-Policy"
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
