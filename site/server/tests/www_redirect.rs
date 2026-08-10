//! `www.<host>` requests get a 301 to `PUBLIC_BASE_URL`, carrying the
//! original path, query, and the constant security headers; every other
//! `Host` (the apex itself, localhost, LAN IPs) passes through untouched.

mod common;

use axum::http::StatusCode;
use common::{
    get_absolute_form, get_with_host, get_with_invalid_host_bytes, test_router_with_base,
};
use server::security::Hsts;

const HTTPS_BASE: &str = "https://atmiller.org";

#[tokio::test]
async fn www_host_redirects_to_the_apex_with_path_and_query_preserved() {
    let response = get_with_host(
        test_router_with_base(Hsts::Off, Some(HTTPS_BASE)),
        "/some/path?q=1",
        "www.atmiller.org",
    )
    .await;

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response
            .headers()
            .get("location")
            .expect("redirect must carry a Location header"),
        "https://atmiller.org/some/path?q=1"
    );
}

#[tokio::test]
async fn www_host_at_root_redirects_to_the_apex_root() {
    let response = get_with_host(
        test_router_with_base(Hsts::Off, Some(HTTPS_BASE)),
        "/",
        "www.atmiller.org",
    )
    .await;

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response.headers().get("location").expect("Location header"),
        "https://atmiller.org/"
    );
}

#[tokio::test]
async fn www_host_match_is_case_insensitive() {
    let response = get_with_host(
        test_router_with_base(Hsts::Off, Some(HTTPS_BASE)),
        "/",
        "WWW.ATMILLER.ORG",
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "Host matching must be ASCII-case-insensitive"
    );
}

#[tokio::test]
async fn www_host_with_a_port_still_redirects() {
    let response = get_with_host(
        test_router_with_base(Hsts::Off, Some(HTTPS_BASE)),
        "/",
        "www.atmiller.org:8080",
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "the :port suffix must be stripped before comparison"
    );
    assert_eq!(
        response.headers().get("location").expect("Location header"),
        "https://atmiller.org/",
        "the Location target must be the base URL, not the port the request carried"
    );
}

/// The base URL's own port is not stripped: `join()` carries it through, and
/// the `www` match still keys on the host alone (the request's `Host` header
/// here carries no port of its own).
#[tokio::test]
async fn base_url_with_a_port_carries_it_into_the_location() {
    let response = get_with_host(
        test_router_with_base(Hsts::Off, Some("https://atmiller.org:8443")),
        "/some/path",
        "www.atmiller.org",
    )
    .await;

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response.headers().get("location").expect("Location header"),
        "https://atmiller.org:8443/some/path",
        "join() must carry the base URL's port through to the Location"
    );
}

#[tokio::test]
async fn apex_host_is_not_redirected() {
    let response = get_with_host(
        test_router_with_base(Hsts::Off, Some(HTTPS_BASE)),
        "/",
        "atmiller.org",
    )
    .await;

    assert_ne!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "the apex host must be served, not redirected"
    );
}

#[tokio::test]
async fn localhost_is_not_redirected() {
    let response = get_with_host(
        test_router_with_base(Hsts::Off, Some(HTTPS_BASE)),
        "/",
        "localhost",
    )
    .await;

    assert_ne!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "an unrelated Host must be served, not redirected"
    );
}

/// `WwwRedirect::Off` (no `PUBLIC_BASE_URL`) is the redirect-disabled state;
/// this is that case exercised through the router rather than the unit-level
/// `derive` tests in the server's `www_redirect` module.
#[tokio::test]
async fn no_base_url_never_redirects_even_a_www_host() {
    let response = get_with_host(
        test_router_with_base(Hsts::Off, None),
        "/",
        "www.atmiller.org",
    )
    .await;

    assert_ne!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "with no PUBLIC_BASE_URL, the redirect must be disabled regardless of Host"
    );
}

#[tokio::test]
async fn the_redirect_response_carries_the_constant_security_headers() {
    let response = get_with_host(
        test_router_with_base(Hsts::Off, Some(HTTPS_BASE)),
        "/",
        "www.atmiller.org",
    )
    .await;

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response
            .headers()
            .get("x-content-type-options")
            .map(|v| v.as_bytes()),
        Some(&b"nosniff"[..]),
        "the 301 response must also carry the constant security headers \
         (the redirect layer must sit inside the security-headers layer)"
    );
}

#[tokio::test]
async fn http_scheme_base_url_redirects_to_an_http_location() {
    let response = get_with_host(
        test_router_with_base(Hsts::Off, Some("http://atmiller.org")),
        "/some/path",
        "www.atmiller.org",
    )
    .await;

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response.headers().get("location").expect("Location header"),
        "http://atmiller.org/some/path"
    );
}

/// The asterisk-form request-target (`OPTIONS * HTTP/1.1`) yields
/// `path_and_query() == Some("*")`, not a real path; the redirect must fall
/// back to "/" rather than compose `https://atmiller.org*`.
#[tokio::test]
async fn asterisk_form_request_target_falls_back_to_the_root_path() {
    let response = get_with_host(
        test_router_with_base(Hsts::Off, Some(HTTPS_BASE)),
        "*",
        "www.atmiller.org",
    )
    .await;

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response.headers().get("location").expect("Location header"),
        "https://atmiller.org/",
        "asterisk-form must not leak into the Location path"
    );
}

/// HTTP/2 requests may carry `:authority` instead of a `Host` header; the
/// middleware must fall back to the URI's authority to find the same match.
#[tokio::test]
async fn no_host_header_falls_back_to_the_uri_authority() {
    let response = get_absolute_form(
        test_router_with_base(Hsts::Off, Some(HTTPS_BASE)),
        "http://www.atmiller.org/some/path?q=1",
    )
    .await;

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response.headers().get("location").expect("Location header"),
        "https://atmiller.org/some/path?q=1"
    );
}

#[tokio::test]
async fn hosts_that_merely_contain_the_www_host_as_a_substring_are_not_redirected() {
    for host in ["www.atmiller.org.evil.com", "xwww.atmiller.org"] {
        let response = get_with_host(
            test_router_with_base(Hsts::Off, Some(HTTPS_BASE)),
            "/",
            host,
        )
        .await;

        assert_ne!(
            response.status(),
            StatusCode::MOVED_PERMANENTLY,
            "Host {host:?} must be an exact match, not a suffix/prefix match"
        );
    }
}

#[tokio::test]
async fn no_host_header_and_no_uri_authority_passes_through_without_panicking() {
    // The plain relative-form request `common::get` sends carries neither a
    // Host header nor a URI authority - the case both extraction attempts
    // in `request_host` come up empty.
    let response = common::get(test_router_with_base(Hsts::Off, Some(HTTPS_BASE)), "/").await;

    assert_ne!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "with no host information at all, the request must simply pass through"
    );
}

#[tokio::test]
async fn apex_host_with_a_port_is_not_redirected() {
    let response = get_with_host(
        test_router_with_base(Hsts::Off, Some(HTTPS_BASE)),
        "/",
        "atmiller.org:8080",
    )
    .await;

    assert_ne!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "the apex (even with a port) must be served, not redirected"
    );
}

#[tokio::test]
async fn host_header_with_invalid_utf8_bytes_does_not_panic() {
    let response = get_with_invalid_host_bytes(
        test_router_with_base(Hsts::Off, Some(HTTPS_BASE)),
        "/",
        &[0xFF],
    )
    .await;

    assert_ne!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "a Host header that fails UTF-8 validation must degrade to pass-through, not redirect"
    );
}
