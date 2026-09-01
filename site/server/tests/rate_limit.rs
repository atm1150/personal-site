//! Per-IP rate limiting on the site router: the full burst passes, requests
//! past it get 429 + retry-after, IPs are limited independently, and /readyz
//! is structurally exempt so orchestrator probes can never be throttled into
//! a false "down".

mod common;

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, Response, StatusCode};
use leptos::prelude::LeptosOptions;
use server::READYZ_PATH;
use server::limits::{BURST_SIZE, RequestLimits};
use server::security::Hsts;
use tower::ServiceExt;

/// Requests past the burst before the test gives up waiting for a 429. Keeps
/// the loop bounded while tolerating tokens replenished mid-test on a slow
/// machine (one token per 200ms; the loop sends far faster than that).
const EXTRA_ATTEMPTS: u32 = 20;

fn limited_router() -> Router {
    let options = LeptosOptions::builder().output_name("portfolio").build();
    server::router(options, Hsts::Off, None, RequestLimits::production())
}

async fn get_from(router: &Router, uri: &str, client: SocketAddr) -> Response<Body> {
    router
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .extension(ConnectInfo(client))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

fn client_a() -> SocketAddr {
    SocketAddr::from(([10, 0, 0, 1], 40001))
}

/// Drive `client` past its burst; returns the first 429 response.
async fn exhaust_burst(router: &Router, client: SocketAddr) -> Response<Body> {
    for n in 0..BURST_SIZE {
        let response = get_from(router, "/", client).await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "request {} of the burst should pass",
            n + 1
        );
    }
    for _ in 0..EXTRA_ATTEMPTS {
        let response = get_from(router, "/", client).await;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            return response;
        }
    }
    panic!("no 429 within {EXTRA_ATTEMPTS} requests past the burst");
}

#[tokio::test]
async fn the_full_burst_passes_for_one_client() {
    let router = limited_router();
    for n in 0..BURST_SIZE {
        let response = get_from(&router, "/", client_a()).await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "request {} of {BURST_SIZE} should pass",
            n + 1
        );
    }
}

#[tokio::test]
async fn past_the_burst_is_429_with_retry_after() {
    let router = limited_router();
    let response = exhaust_burst(&router, client_a()).await;

    let retry_after = response
        .headers()
        .get("retry-after")
        .expect("a 429 must tell the client when to retry")
        .to_str()
        .expect("retry-after should be ASCII");
    retry_after
        .parse::<u64>()
        .expect("retry-after should be whole seconds");
}

#[tokio::test]
async fn a_429_still_carries_the_constant_security_headers() {
    let router = limited_router();
    let response = exhaust_burst(&router, client_a()).await;

    assert!(
        response.headers().contains_key("x-content-type-options"),
        "the security-header layer must wrap rate-limit rejections"
    );
}

#[tokio::test]
async fn clients_are_limited_independently() {
    let router = limited_router();
    exhaust_burst(&router, client_a()).await;

    let other = SocketAddr::from(([10, 0, 0, 2], 40002));
    let response = get_from(&router, "/", other).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "one exhausted client must not affect another IP"
    );
}

#[tokio::test]
async fn readyz_is_exempt_even_for_an_exhausted_client() {
    let router = limited_router();
    exhaust_burst(&router, client_a()).await;

    let response = get_from(&router, READYZ_PATH, client_a()).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "health probes must never be rate limited"
    );
}

/// The site listener always provides the peer address (ConnectInfo); a
/// request without one means the server was wired wrong, and refusing loudly
/// beats limiting everyone as a single anonymous client.
#[tokio::test]
async fn a_request_without_a_peer_address_is_refused() {
    let router = limited_router();
    let response = router
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
