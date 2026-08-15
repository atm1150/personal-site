//! The persistent layout shell: skip link, header, footer.
//!
//! Every route renders inside this shell, so the landmarks (`<header>`,
//! `<nav>`, `<main>`, `<footer>`) exist exactly once per page.

use leptos::prelude::*;
use leptos_router::components::A;

/// Anchor the skip link and the `<main>` landmark to the same id.
pub const MAIN_CONTENT_ID: &str = "main-content";

/// First focusable element on the page: lets keyboard and screen-reader users
/// jump past the header. Visually hidden until focused (see `main.css`).
///
/// `target="_self"` is load-bearing.
#[component]
pub fn SkipLink() -> impl IntoView {
    view! {
        <a class="skip-link" href=format!("#{MAIN_CONTENT_ID}") target="_self">
            "Skip to content"
        </a>
    }
}

/// Wordmark plus primary navigation.
#[component]
pub fn SiteHeader() -> impl IntoView {
    view! {
        <header class="site-header">
            <div class="shell-row">
                <a href="/" class="wordmark">
                    "Andrew Miller"
                </a>
                <nav class="site-nav" aria-label="Main">
                    <A href="/" exact=true>
                        "Home"
                    </A>
                    <A href="/resume">"Resume"</A>
                </nav>
            </div>
        </header>
    }
}

/// One line: attribution, identity links, and what the site is built with.
#[component]
pub fn SiteFooter() -> impl IntoView {
    view! {
        <footer class="site-footer">
            <div class="shell-row">
                <p>
                    "© 2026 Andrew Miller"
                    <span class="sep" aria-hidden="true">"·"</span>
                    // rel="me" marks these as the author's own profiles.
                    <a href="https://github.com/atm1150/" rel="me">"GitHub"</a>
                    <span class="sep" aria-hidden="true">"·"</span>
                    <a href="https://www.linkedin.com/in/atmiller1150" rel="me">"LinkedIn"</a>
                    <span class="sep" aria-hidden="true">"·"</span>
                    "Built with Leptos + Rust"
                </p>
            </div>
        </footer>
    }
}
