//! The resume page as served. The contact line's segments are glued to the
//! separators, so without explicit wrap points its longest run sets the whole
//! page's minimum width - wider than a phone.

mod common;

use common::{body_string, get, test_router};
use server::security::Hsts;

async fn resume_html() -> String {
    body_string(get(test_router(Hsts::Off), "/resume").await).await
}

fn offset_of_exactly_one(html: &str, needle: &str) -> usize {
    assert_eq!(
        html.matches(needle).count(),
        1,
        "{needle} should appear exactly once: {html}"
    );
    html.find(needle).expect("counted once above")
}

#[tokio::test]
async fn contact_line_offers_a_wrap_point_after_each_separator() {
    let html = resume_html().await;
    assert_eq!(
        html.matches(r#"<span class="sep">·</span><wbr>"#).count(),
        4,
        "{html}"
    );
}

#[tokio::test]
async fn contact_line_ends_with_the_site_address() {
    let html = resume_html().await;
    assert_eq!(
        html.matches(r#"<wbr><a href="https://atmiller.org">atmiller.org</a>"#)
            .count(),
        1,
        "{html}"
    );
}

#[tokio::test]
async fn projects_section_sits_between_skills_and_experience() {
    let html = resume_html().await;
    let skills = offset_of_exactly_one(&html, r#"id="skills""#);
    let projects = offset_of_exactly_one(&html, r#"id="projects""#);
    let experience = offset_of_exactly_one(&html, r#"id="experience""#);
    assert!(skills < projects && projects < experience, "{html}");
}

#[tokio::test]
async fn dns_challenge_name_cannot_wrap_at_its_hyphen() {
    let html = resume_html().await;
    assert_eq!(
        html.matches(r#"<span class="nowrap">DNS-01</span>"#)
            .count(),
        1,
        "{html}"
    );
}

/// The contact line and the footer each carry one of these links too, so the
/// count is taken inside the section alone.
#[tokio::test]
async fn projects_entry_links_the_live_site_and_the_repository() {
    let html = resume_html().await;
    let start = html.find(r#"id="projects""#).expect("projects section");
    let end = html.find(r#"id="experience""#).expect("experience section");
    let section = &html[start..end];
    for needle in [
        r#"href="https://atmiller.org""#,
        r#"href="https://github.com/atm1150/personal-site""#,
    ] {
        assert_eq!(
            section.matches(needle).count(),
            1,
            "{needle} in:\n{section}"
        );
    }
}
