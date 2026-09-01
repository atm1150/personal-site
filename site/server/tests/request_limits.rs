//! Request-shape bounds on the site router: body size and handler time.
//! Exercised through test-util routes (`/__test/echo` reads its body,
//! `/__test/hang` never completes) because no production route does either.

mod common;

use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, Response, StatusCode};
use leptos::prelude::LeptosOptions;
use server::limits::{BODY_LIMIT_BYTES, RequestLimits};
use server::security::Hsts;
use tower::ServiceExt;

fn router_with(limits: RequestLimits) -> Router {
    let options = LeptosOptions::builder().output_name("portfolio").build();
    server::router(options, Hsts::Off, None, limits)
}

async fn post_body(router: Router, uri: &str, body: Vec<u8>) -> Response<Body> {
    router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(uri)
                .body(Body::from(body))
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

#[tokio::test]
async fn a_body_at_the_limit_passes() {
    let response = post_body(
        router_with(RequestLimits::disabled()),
        "/__test/echo",
        vec![0u8; BODY_LIMIT_BYTES],
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn an_oversized_body_is_rejected() {
    let response = post_body(
        router_with(RequestLimits::disabled()),
        "/__test/echo",
        vec![0u8; BODY_LIMIT_BYTES + 1],
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn a_413_still_carries_the_constant_security_headers() {
    let response = post_body(
        router_with(RequestLimits::disabled()),
        "/__test/echo",
        vec![0u8; BODY_LIMIT_BYTES + 1],
    )
    .await;
    assert!(
        response.headers().contains_key("x-content-type-options"),
        "the security-header layer must wrap body-limit rejections"
    );
}

#[tokio::test]
async fn a_handler_stuck_past_the_timeout_answers_408() {
    let limits = RequestLimits {
        request_timeout: Duration::from_millis(100),
        ..RequestLimits::disabled()
    };
    let response = router_with(limits)
        .oneshot(
            Request::builder()
                .uri("/__test/hang")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");
    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
}
