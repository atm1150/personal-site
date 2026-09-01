//! Fixtures shared by the integration tests.
//!
//! Each test binary compiles this module separately and uses only part of it,
//! so unused items here are expected rather than dead code.
#![allow(dead_code)]

use app::meta::PublicBaseUrl;
use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Request, Response, header};
use leptos::prelude::LeptosOptions;
use server::security::Hsts;
use tower::ServiceExt; // brings `oneshot` onto the router

/// site_root defaults to ".", so the fallback's static-file probe misses and
/// SSR runs; output_name is the only field without a default.
pub fn test_router(hsts: Hsts) -> Router {
    test_router_with_base(hsts, None)
}

/// test_router plus a validated public base URL - for the SSR metadata and
/// `www` redirect tests.
pub fn test_router_with_base(hsts: Hsts, base: Option<&str>) -> Router {
    let options = LeptosOptions::builder().output_name("portfolio").build();
    let base = base.map(|b| PublicBaseUrl::new(b).expect("test base URL should be valid"));
    server::router(
        options,
        hsts,
        base,
        server::limits::RequestLimits::disabled(),
    )
}

/// Router whose site_root points at the source public/ dir, so the static-file
/// fallback serves real assets (robots.txt). Test cwd is site/server, hence "../public".
pub fn test_router_serving_public(hsts: Hsts) -> Router {
    let options = LeptosOptions::builder()
        .output_name("portfolio")
        .site_root("../public")
        .build();
    server::router(
        options,
        hsts,
        None,
        server::limits::RequestLimits::disabled(),
    )
}

/// Send a GET through any router without binding a socket.
pub async fn get(router: Router, uri: &str) -> Response<Body> {
    router
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

/// Send a GET carrying an explicit `Host` header (the plain [`get`] helper
/// sends none), for the redirect tests that key off it.
pub async fn get_with_host(router: Router, uri: &str, host: &str) -> Response<Body> {
    router
        .oneshot(
            Request::builder()
                .uri(uri)
                .header(header::HOST, host)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

/// Send a GET whose request-target is absolute-form (`http://host/path`) and
/// carries no `Host` header at all - the shape an HTTP/2 request takes
/// (`:authority` instead of `Host`), exercising the URI-authority fallback
/// in `redirect_www`'s host extraction.
pub async fn get_absolute_form(router: Router, absolute_uri: &str) -> Response<Body> {
    router
        .oneshot(
            Request::builder()
                .uri(absolute_uri)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

/// Send a GET whose `Host` header is opaque bytes that fail UTF-8 validation
/// (still a legal `HeaderValue` - only control bytes and DEL are
/// disallowed), to prove host extraction degrades to pass-through instead of
/// panicking on it.
pub async fn get_with_invalid_host_bytes(
    router: Router,
    uri: &str,
    host_bytes: &[u8],
) -> Response<Body> {
    router
        .oneshot(
            Request::builder()
                .uri(uri)
                .header(
                    header::HOST,
                    HeaderValue::from_bytes(host_bytes)
                        .expect("bytes should be a legal (if non-UTF-8) header value"),
                )
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

pub async fn body_string(response: Response<Body>) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should collect");
    String::from_utf8(bytes.to_vec()).expect("body should be UTF-8")
}
