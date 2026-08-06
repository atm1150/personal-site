//! Fixtures shared by the integration tests.
//!
//! Each test binary compiles this module separately and uses only part of it,
//! so unused items here are expected rather than dead code.
#![allow(dead_code)]

use app::meta::PublicBaseUrl;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response};
use leptos::prelude::LeptosOptions;
use server::security::Hsts;
use tower::ServiceExt; // brings `oneshot` onto the router

/// site_root defaults to ".", so the fallback's static-file probe misses and
/// SSR runs; output_name is the only field without a default.
pub fn test_router(hsts: Hsts) -> Router {
    let options = LeptosOptions::builder().output_name("portfolio").build();
    server::router(options, hsts, None)
}

/// test_router plus a validated public base URL, for the SSR metadata tests.
pub fn test_router_with_base(hsts: Hsts, base: Option<&str>) -> Router {
    let options = LeptosOptions::builder().output_name("portfolio").build();
    let base = base.map(|b| PublicBaseUrl::new(b).expect("test base URL should be valid"));
    server::router(options, hsts, base)
}

/// Router whose site_root points at the source public/ dir, so the static-file
/// fallback serves real assets (robots.txt). Test cwd is site/server, hence "../public".
pub fn test_router_serving_public(hsts: Hsts) -> Router {
    let options = LeptosOptions::builder()
        .output_name("portfolio")
        .site_root("../public")
        .build();
    server::router(options, hsts, None)
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

pub async fn body_string(response: Response<Body>) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should collect");
    String::from_utf8(bytes.to_vec()).expect("body should be UTF-8")
}
