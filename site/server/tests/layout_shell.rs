//! The layout shell as a crawler and a screen reader receive it: landmarks,
//! skip link, language, and the theme tokens. These go through the real router
//! because the shell only exists in a full SSR render - a component-level test
//! would pass while the served document was missing the head-injected theme.

mod common;

use common::{body_string, get, test_router};
use server::security::Hsts;

/// Home, resume, the article, and the 404 fallback all render inside the same shell.
const EVERY_PAGE: [&str; 4] = ["/", "/resume", "/how-its-built", "/no-such-page"];

async fn html_for(path: &str) -> String {
    body_string(get(test_router(Hsts::Off), path).await).await
}

#[tokio::test]
async fn every_page_carries_the_shell_landmarks_exactly_once() {
    for path in EVERY_PAGE {
        let html = html_for(path).await;
        // Matched attribute by attribute: the renderer chooses its own
        // attribute order, which is not something these tests should pin.
        for needle in [
            r#"class="site-header""#,
            r#"class="site-footer""#,
            r#"class="site-nav""#,
            r#"aria-label="Main""#,
            r#"id="main-content""#,
        ] {
            assert_eq!(
                html.matches(needle).count(),
                1,
                "{path} should carry exactly one {needle}"
            );
        }
    }
}

/// The skip link is only useful if it precedes the header in source order and
/// points at the element the `<main>` landmark actually carries.
#[tokio::test]
async fn skip_link_precedes_the_header_and_targets_main() {
    for path in EVERY_PAGE {
        let html = html_for(path).await;
        let skip = html
            .find(r##"href="#main-content""##)
            .unwrap_or_else(|| panic!("{path} should carry a skip link"));
        let header = html
            .find(r#"class="site-header""#)
            .expect("header should render");
        assert!(
            skip < header,
            "{path}: skip link must come before the header"
        );
        // No tabindex on the target: the fragment jump moves the point Tab
        // resumes from, so main does not need to be focusable.
        assert!(html.contains(r#"id="main-content""#), "{html}");
        assert!(!html.contains("tabindex"), "{html}");
        // Without target, leptos_router intercepts the jump and Tab resumes at
        // the header instead of inside main. Verified by keyboard.
        assert!(html.contains(r#"target="_self""#), "{path}: {html}");
    }
}

#[tokio::test]
async fn html_element_declares_english_exactly_once() {
    for path in EVERY_PAGE {
        let html = html_for(path).await;
        assert_eq!(
            html.matches(r#"lang="en""#).count(),
            1,
            "{path} should carry exactly one lang: {html}"
        );
    }
}

/// The palette reaches the browser as custom properties in the provider's
/// unlayered `<style id="theme">`. Auto mode must emit both a light `:root`
/// block and a dark one inside a prefers-color-scheme query - that media query
/// IS the theme switch, so its absence would mean a light-only site.
#[tokio::test]
async fn theme_emits_light_and_dark_token_blocks() {
    let html = html_for("/").await;

    assert!(html.contains("--site-link"), "site tokens missing: {html}");
    assert!(html.contains("--site-band"));
    assert!(html.contains("@media (prefers-color-scheme: dark)"));

    // Slate light background and its dark counterpart, from theme.rs.
    assert!(html.contains("--background: oklch(0.98 0.003 255)"));
    assert!(html.contains("--background: oklch(0.15 0.01 260)"));
}

/// Nav marks the active route for assistive technology; the stylesheet leans
/// on the same attribute, so styling never carries the meaning alone.
///
/// Exactly one element may claim it: the wordmark also links to `/`, and when
/// it was a router `<A>` the home page announced "current page" twice.
#[tokio::test]
async fn nav_marks_the_current_page_exactly_once() {
    let home = html_for("/").await;
    assert_eq!(home.matches(r#"aria-current="page""#).count(), 1, "{home}");

    let article = html_for("/how-its-built").await;
    assert_eq!(
        article.matches(r#"aria-current="page""#).count(),
        1,
        "{article}"
    );

    // An unmatched route is nobody's current page.
    let missing = html_for("/no-such-page").await;
    assert!(!missing.contains(r#"aria-current="page""#));
}

/// The 404 is a real page in the shell, not a bare centered message.
#[tokio::test]
async fn not_found_page_renders_inside_the_shell() {
    let response = get(test_router(Hsts::Off), "/no-such-page").await;
    assert_eq!(response.status(), 404);
    let html = body_string(response).await;

    assert!(html.contains(r#"class="not-found-page prose""#), "{html}");
    assert!(html.contains(r#"class="site-footer""#));
}
