//! An unmatched route is served by the Leptos SSR fallback, which renders the
//! `ErrorTemplate` and returns HTTP 404, not a 200 with not-found content
//! (the soft-404 that hurts crawlability).

mod common;

use axum::http::StatusCode;
use common::{body_string, test_router};
use server::security::Hsts;

#[tokio::test]
async fn unmatched_route_renders_not_found_page_with_404() {
    let response = common::get(test_router(Hsts::Off), "/this-route-does-not-exist").await;

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "an unmatched route must return a hard 404"
    );

    let html = body_string(response).await;

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
