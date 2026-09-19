//! The "How it's built" article as served: every heading, the list, and the
//! one external link land in the SSR HTML.

mod common;

use common::{body_string, get, test_router};
use server::security::Hsts;

async fn article_html() -> String {
    body_string(get(test_router(Hsts::Off), "/how-its-built").await).await
}

#[tokio::test]
async fn article_title_is_the_only_h1() {
    let html = article_html().await;
    assert_eq!(html.matches("<h1>").count(), 1, "{html}");
    assert!(html.contains("<h1>How it's built</h1>"), "{html}");
}

#[tokio::test]
async fn every_section_heading_is_served() {
    // cargo-leptos debug builds add <!> hydration markers inside two-root components.
    let html = article_html().await.replace("<!>", "");
    let h2s = [
        "Why Rust",
        "Why Leptos",
        "Server rendering and hydration",
        "Visual design",
        "Accessibility",
    ];
    let h3s = [
        "Static typing all the way to the browser",
        "Blazor, the familiar choice",
        "Dioxus, the up-and-comer",
        "The resume page",
    ];
    for heading in h2s {
        assert!(
            html.contains(&format!("<h2>{heading}</h2>")),
            "missing h2 {heading:?} in {html}"
        );
    }
    for heading in h3s {
        assert!(
            html.contains(&format!("<h3>{heading}</h3>")),
            "missing h3 {heading:?} in {html}"
        );
    }
    assert_eq!(html.matches("<h2>").count(), h2s.len(), "{html}");
    assert_eq!(html.matches("<h3>").count(), h3s.len(), "{html}");
}

#[tokio::test]
async fn advantages_list_has_three_items() {
    let html = article_html().await;
    assert_eq!(html.matches("<li>").count(), 3, "{html}");
    assert!(
        html.contains(
            "<li>By having an initial server render, the time to first paint is minimized.</li>"
        ),
        "{html}"
    );
}

#[tokio::test]
async fn blazor_vdom_reference_is_a_link() {
    let html = article_html().await;
    assert_eq!(
        html.matches(
            r#"<a href="https://github.com/dotnet/aspnetcore/discussions/48890">a VDOM style of rendering</a>"#
        )
        .count(),
        1,
        "{html}"
    );
}

#[tokio::test]
async fn repo_is_linked_from_intro_and_resume_section() {
    let html = article_html().await;
    assert_eq!(
        html.matches(r#"<a href="https://github.com/atm1150/personal-site">source code</a>"#)
            .count(),
        2,
        "{html}"
    );
}

#[tokio::test]
async fn article_body_is_prose() {
    let html = article_html().await;
    assert_eq!(
        html.matches(r#"<article class="prose">"#).count(),
        1,
        "{html}"
    );
}
