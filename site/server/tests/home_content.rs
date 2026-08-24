//! The home page body as served: the real content lands in the SSR HTML, not
//! only after hydration, so a crawler and a no-JS visitor read the same page.

mod common;

use common::{body_string, get, test_router};
use server::security::Hsts;

async fn home_html() -> String {
    body_string(get(test_router(Hsts::Off), "/").await).await
}

#[tokio::test]
async fn home_leads_with_the_work_claim_as_its_only_h1() {
    let html = home_html().await;
    assert_eq!(html.matches("<h1>").count(), 1, "{html}");
    assert!(
        html.contains("<h1>I rebuild legacy stacks into modern systems</h1>"),
        "{html}"
    );
}

#[tokio::test]
async fn home_prose_carries_the_intro_paragraphs() {
    let html = home_html().await;
    assert!(html.contains(r#"class="prose""#), "{html}");
    for needle in [
        "I am a full stack coder who specializes in backend systems.",
        "My professional coding experience has been with private companies,",
        "Outside of my professional experience I enjoy designing and running my own homelab.",
        "If you are a recruiter or hiring manager,",
    ] {
        assert!(html.contains(needle), "missing {needle:?} in {html}");
    }
}

#[tokio::test]
async fn portrait_renders_with_alt_text_and_dimensions() {
    let html = home_html().await;
    // Matched attribute by attribute: the renderer picks its own order.
    for needle in [
        r#"src="/portrait.jpg""#,
        r#"alt="Andrew Miller, smiling in the woods""#,
        r#"width="1080""#,
        r#"height="1080""#,
        r#"class="portrait""#,
    ] {
        assert_eq!(html.matches(needle).count(), 1, "{needle} in {html}");
    }
}

#[tokio::test]
async fn resume_link_is_prose_not_a_bare_list_item() {
    let html = home_html().await;
    assert!(
        html.contains(r#"<a href="/resume">resume page</a>"#),
        "{html}"
    );
    assert!(
        !html.contains("Résumé"),
        "placeholder link survived: {html}"
    );
}
