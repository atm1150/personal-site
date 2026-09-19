//! The persistent layout shell: skip link, header, footer.
//!
//! Every route renders inside this shell, so the landmarks (`<header>`,
//! `<nav>`, `<main>`, `<footer>`) exist exactly once per page.

use crate::errors::PageErrored;
use leptos::prelude::*;
use leptos_router::hooks::use_location;

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

/// True when `pathname` is the page this `href` names (trailing slash ignored).
pub(crate) fn is_current(pathname: &str, href: &str) -> bool {
    pathname.trim_end_matches('/') == href.trim_end_matches('/')
}

/// target="_self" when `href` is the page showing the error: a router nav to
/// the current page is a no-op.
fn full_load_target(href: &'static str) -> impl Fn() -> Option<&'static str> {
    let errored = use_context::<PageErrored>();
    let pathname = use_location().pathname;
    move || {
        (errored.is_some_and(|flag| flag.0.get()) && is_current(&pathname.get(), href))
            .then_some("_self")
    }
}

#[component]
fn NavLink(href: &'static str, children: Children) -> impl IntoView {
    let pathname = use_location().pathname;
    let here = move || is_current(&pathname.get(), href);
    view! {
        <a href=href aria-current=move || here().then_some("page") target=full_load_target(href)>
            {children()}
        </a>
    }
}

/// Wordmark plus primary navigation.
#[component]
pub fn SiteHeader() -> impl IntoView {
    view! {
        <header class="site-header">
            <div class="shell-row">
                // No aria-current on the wordmark: it also points home, and only
                // one element may claim the current page.
                <a href="/" class="wordmark" target=full_load_target("/")>
                    "Andrew Miller"
                </a>
                <nav class="site-nav" aria-label="Main">
                    <NavLink href="/">"Home"</NavLink>
                    <NavLink href="/resume">"Resume"</NavLink>
                    <NavLink href="/how-its-built">"How it's built"</NavLink>
                </nav>
            </div>
        </header>
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use crate::errors::PageErrored;
    use leptos_router::components::Router;
    use leptos_router::location::RequestUrl;

    fn render_header(errored: bool, path: &'static str) -> String {
        let owner = Owner::new();
        owner.with(move || {
            provide_context(RequestUrl::new(path));
            provide_context(PageErrored(RwSignal::new(errored)));
            view! {
                <Router>
                    <SiteHeader/>
                </Router>
            }
            .to_html()
        })
    }

    #[test]
    fn only_links_to_the_errored_page_become_full_loads() {
        let home = render_header(true, "/");
        assert_eq!(
            home.matches(r#"target="_self""#).count(),
            2,
            "wordmark and nav Home should carry target=\"_self\"; got:\n{home}"
        );
        // Re-rendering the nav for the error state must keep aria-current.
        assert!(home.contains(r#"aria-current="page""#), "got:\n{home}");

        let resume = render_header(true, "/resume");
        assert_eq!(
            resume.matches(r#"target="_self""#).count(),
            1,
            "only nav Resume should carry target=\"_self\"; got:\n{resume}"
        );

        let article = render_header(true, "/how-its-built");
        assert_eq!(
            article.matches(r#"target="_self""#).count(),
            1,
            "only nav How it's built should carry target=\"_self\"; got:\n{article}"
        );
    }

    fn render_footer(errored: bool, path: &'static str) -> String {
        let owner = Owner::new();
        owner.with(move || {
            provide_context(RequestUrl::new(path));
            provide_context(PageErrored(RwSignal::new(errored)));
            view! {
                <Router>
                    <SiteFooter/>
                </Router>
            }
            .to_html()
        })
    }

    #[test]
    fn footer_article_link_becomes_a_full_load_only_when_erroring_there() {
        let there = render_footer(true, "/how-its-built");
        assert!(there.contains(r#"href="/how-its-built""#), "got:\n{there}");
        assert_eq!(
            there.matches(r#"target="_self""#).count(),
            1,
            "got:\n{there}"
        );

        let elsewhere = render_footer(true, "/");
        assert_eq!(
            elsewhere.matches(r#"target="_self""#).count(),
            0,
            "got:\n{elsewhere}"
        );

        let healthy = render_footer(false, "/how-its-built");
        assert_eq!(
            healthy.matches(r#"target="_self""#).count(),
            0,
            "got:\n{healthy}"
        );
    }

    #[test]
    fn header_links_stay_router_links_normally() {
        let html = render_header(false, "/");

        assert_eq!(
            html.matches(r#"target="_self""#).count(),
            0,
            "no header link should carry a target outside the error state; got:\n{html}"
        );
        assert!(html.contains(r#"aria-current="page""#), "got:\n{html}");
    }

    #[test]
    fn is_current_ignores_trailing_slashes() {
        assert!(is_current("/resume/", "/resume"));
        assert!(is_current("/resume", "/resume"));
        assert!(is_current("/", "/"));
    }

    #[test]
    fn is_current_rejects_other_routes() {
        assert!(!is_current("/resume", "/"));
        assert!(!is_current("/", "/resume"));
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
                    <a href="/how-its-built" target=full_load_target("/how-its-built")>
                        "Built with Leptos + Rust"
                    </a>
                </p>
            </div>
        </footer>
    }
}
