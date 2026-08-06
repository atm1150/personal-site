//! robots.txt is served as a real static asset with the intended directives.

mod common;

use common::{body_string, get, test_router_serving_public};
use server::security::Hsts;

#[tokio::test]
async fn robots_txt_served_as_plain_text_with_expected_directives() {
    let response = get(test_router_serving_public(Hsts::Off), "/robots.txt").await;

    assert_eq!(response.status(), 200);
    let content_type = response
        .headers()
        .get("content-type")
        .expect("robots.txt response should carry a content-type")
        .to_str()
        .expect("content-type should be ASCII");
    assert!(
        content_type.starts_with("text/plain"),
        "robots.txt must serve as text/plain, got {content_type}"
    );

    let body = body_string(response).await;
    assert!(body.lines().any(|l| l.trim() == "User-agent: *"));
    assert!(body.lines().any(|l| l.trim() == "Disallow: /readyz"));
    assert!(body.lines().any(|l| l.trim() == "Allow: /"));
    // Exact-line check: a substring test would false-positive on "Disallow: /readyz".
    assert!(
        !body.lines().any(|l| l.trim() == "Disallow: /"),
        "robots.txt must never disallow the site root"
    );
}
