//! SSR head metadata: what a crawler actually receives. These requests go
//! through the real router because leptos_meta only fills the head via
//! <MetaTags/> during a full SSR render - a component-level render would
//! pass while every scraper saw nothing.

mod common;

use common::{body_string, get, test_router_with_base};
use server::security::Hsts;

const BASE: &str = "https://example.test";

#[tokio::test]
async fn home_head_carries_full_meta() {
    let html = body_string(get(test_router_with_base(Hsts::Off, Some(BASE)), "/").await).await;

    assert!(
        html.contains("<title>Andrew Miller - I rebuild legacy stacks into modern systems</title>"),
        "{html}"
    );
    assert!(html.contains(
        r#"<meta name="description" content="Full stack developer specializing in backend systems. This site is my public code sample: built from scratch, self-hosted skills included, resume inside.""#
    ));
    assert!(html.contains(
        r#"<meta property="og:title" content="Andrew Miller - I rebuild legacy stacks into modern systems""#
    ));
    assert!(html.contains(
        r#"<meta property="og:description" content="Full stack developer specializing in backend systems. This site is my public code sample: built from scratch, self-hosted skills included, resume inside.""#
    ));
    assert!(html.contains(r#"<meta property="og:type" content="website""#));
    assert!(html.contains(r#"<meta property="og:url" content="https://example.test/""#));
    assert!(html.contains(r#"<link rel="canonical" href="https://example.test/""#));
}

#[tokio::test]
async fn resume_head_carries_full_meta() {
    let html =
        body_string(get(test_router_with_base(Hsts::Off, Some(BASE)), "/resume").await).await;

    assert!(
        html.contains("<title>Andrew Miller — Software Engineer</title>"),
        "{html}"
    );
    assert!(html.contains(
        r#"<meta name="description" content="Resume of Andrew Miller, software engineer in Seattle, WA.""#
    ));
    assert!(html.contains(r#"<meta property="og:url" content="https://example.test/resume""#));
    assert!(html.contains(r#"<link rel="canonical" href="https://example.test/resume""#));
}

#[tokio::test]
async fn no_base_url_omits_absolute_tags_and_still_serves() {
    let response = get(test_router_with_base(Hsts::Off, None), "/").await;
    assert_eq!(response.status(), 200);
    let html = body_string(response).await;

    assert!(!html.contains("og:url"));
    assert!(!html.contains("canonical"));
    // The rest of the metadata does not depend on the base URL.
    assert!(html.contains(
        r#"<meta property="og:title" content="Andrew Miller - I rebuild legacy stacks into modern systems""#
    ));
}

/// leptos_meta renders every Meta component it sees - nothing dedupes them.
/// A second registration of the same tag (an App-level "fallback", or the
/// shell-rendered og:url/canonical re-added through leptos_meta) would ship
/// duplicates that crawlers resolve unpredictably; exact counts catch that.
#[tokio::test]
async fn head_tags_appear_exactly_once() {
    let needles = [
        "<title>",
        r#"<meta name="description""#,
        r#"property="og:title""#,
        r#"property="og:description""#,
        r#"property="og:type""#,
        r#"property="og:url""#,
        r#"rel="canonical""#,
    ];
    for path in ["/", "/resume"] {
        let html = body_string(get(test_router_with_base(Hsts::Off, Some(BASE)), path).await).await;
        for needle in needles {
            assert_eq!(
                html.matches(needle).count(),
                1,
                "{path} should carry exactly one {needle}"
            );
        }
    }
}

#[tokio::test]
async fn not_found_page_omits_absolute_tags() {
    let response = get(
        test_router_with_base(Hsts::Off, Some(BASE)),
        "/no-such-page",
    )
    .await;
    assert_eq!(response.status(), 404);
    let html = body_string(response).await;

    assert!(!html.contains("og:url"));
    assert!(!html.contains("canonical"));
}
