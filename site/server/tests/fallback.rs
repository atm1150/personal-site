//! An unmatched route is served by the Leptos SSR fallback, which renders the
//! `ErrorTemplate` and returns HTTP 404, not a 200 with not-found content
//! (the soft-404 that hurts crawlability).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use leptos::prelude::LeptosOptions;
use tower::ServiceExt; // brings `oneshot` onto the router

#[tokio::test]
async fn unmatched_route_renders_not_found_page_with_404() {
    // `output_name` is the only field without a default; site_root defaults to
    // ".", so the fallback's static-file probe misses and SSR rendering runs.
    let options = LeptosOptions::builder().output_name("portfolio").build();
    let app = server::router(options);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/this-route-does-not-exist")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "an unmatched route must return a hard 404"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should collect");
    let html = String::from_utf8(body.to_vec()).expect("body should be UTF-8");

    // Prove it was *our* ErrorTemplate that rendered, not just any 404 response.
    assert!(
        html.contains("not-found-page"),
        "response should render the ErrorTemplate root class; got:\n{html}"
    );
    assert!(
        html.contains("Back to home"),
        "response should render the not-found page content; got:\n{html}"
    );
}
