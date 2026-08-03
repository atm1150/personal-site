//! Fixtures shared by the integration tests.
//!
//! Each test binary compiles this module separately and uses only part of it,
//! so unused items here are expected rather than dead code.
#![allow(dead_code)]

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
    server::router(options, hsts)
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
