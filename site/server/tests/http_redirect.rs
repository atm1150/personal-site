//! The plain-http redirect listener's app: every request 301s to the public
//! base URL. These tests drive the Router directly (no sockets), matching
//! the sibling integration tests.

mod common;

use app::meta::PublicBaseUrl;
use axum::http::{StatusCode, header};
use common::{get, get_with_host};
use server::http_redirect::redirect_app;

fn app() -> axum::Router {
    redirect_app(PublicBaseUrl::new("https://atmiller.org").expect("valid base URL"))
}

#[tokio::test]
async fn root_redirects_to_base() {
    let response = get(app(), "/").await;
    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response.headers()[header::LOCATION],
        "https://atmiller.org/"
    );
}

#[tokio::test]
async fn path_and_query_are_preserved() {
    let response = get(app(), "/resume?from=card").await;
    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response.headers()[header::LOCATION],
        "https://atmiller.org/resume?from=card"
    );
}

#[tokio::test]
async fn every_host_gets_the_same_target() {
    // Unlike the www middleware, this listener never inspects Host: apex,
    // www, and a raw IP all land on the canonical origin in one hop.
    for host in ["atmiller.org", "www.atmiller.org", "203.0.113.7"] {
        let response = get_with_host(app(), "/x", host).await;
        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert_eq!(
            response.headers()[header::LOCATION],
            "https://atmiller.org/x",
            "host {host} should not change the target"
        );
    }
}

#[tokio::test]
async fn constant_security_headers_ride_the_redirect_without_hsts() {
    let response = get(app(), "/").await;
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
    assert_eq!(response.headers()[header::X_FRAME_OPTIONS], "DENY");
    assert!(
        !response
            .headers()
            .contains_key(header::STRICT_TRANSPORT_SECURITY),
        "HSTS must not be sent over plain http"
    );
}
