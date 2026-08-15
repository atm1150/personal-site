pub mod csp;
pub mod error_template;
pub mod errors;
pub mod meta;
pub mod resume;
pub mod shell;
pub mod theme;

use leptos::prelude::*;
use leptos_meta::{Html, MetaTags, Stylesheet, provide_meta_context};
use leptos_router::{
    StaticSegment,
    components::{Route, Router, Routes},
};
use resume::ResumePage;
use shell::{MAIN_CONTENT_ID, SiteFooter, SiteHeader, SkipLink};
use singlestage::ThemeProvider;

/// This shell function is used to generate the placeholder html that provides the base index.html
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                // AutoReload emits its hot-reload script only when the LEPTOS_WATCH
                // env var is set at runtime (cargo leptos watch sets it). disable_watch
                // in release builds is belt-and-suspenders: even a stray LEPTOS_WATCH in
                // a production environment cannot re-enable the reload script.
                <AutoReload options=options.clone() disable_watch=!cfg!(debug_assertions)/>
                <HydrationScripts options/>
                <MetaTags/>
                <link rel="icon" href="/favicon.ico" sizes="32x32"/>
                <link rel="icon" href="/favicon.svg" type="image/svg+xml"/>
                <link rel="apple-touch-icon" href="/apple-touch-icon.png"/>
                // Server-only plain elements, outside the leptos_meta hydration cursor.
                // The 404 fallback renders without this context, so error pages omit them.
                {use_context::<crate::meta::PublicBaseUrl>()
                    .zip(use_context::<http::request::Parts>())
                    .map(|(base, parts)| {
                        let url = base.join(parts.uri.path());
                        // property isn't in leptos's typed <meta> attribute set
                        // (only charset/content/http_equiv/name are). Built via
                        // the raw element function rather than view! (whose
                        // output type drops the typed content() method once
                        // wrapped) - the same construction leptos_meta's own
                        // Meta component uses for this attribute. Builder-call
                        // order is render order, so property precedes content.
                        let og_url = leptos::html::meta().attr("property", "og:url").content(url.clone());
                        view! {
                            {og_url}
                            <link rel="canonical" href=url/>
                        }
                    })}
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    // Emit the CSP header on every document response (home, resume, and the
    // fallback error page all render inside App). SSR only; no-op on the client.
    #[cfg(feature = "ssr")]
    crate::csp::set_csp_header();

    view! {
        <Stylesheet id="leptos" href="/pkg/portfolio.css"/>
        // leptos_meta manages <html> during hydration and drops the attribute
        // set in the SSR shell, so lang is re-asserted here (axe: html-has-lang).
        <Html attr:lang="en"/>

        // ThemeProvider is load-bearing beyond theming: it injects singlestage's
        // compiled component CSS (a <style> tag) into the page. Components render
        // unstyled without it. The theme argument carries the site palette; in
        // the default auto mode it emits light at :root and dark inside a
        // prefers-color-scheme media query.
        <ThemeProvider theme=theme::SLATE_BRONZE>
            <Router>
                <SkipLink/>
                <SiteHeader/>
                <main id=MAIN_CONTENT_ID>
                    <Routes fallback=|| {
                        let mut errors = Errors::default();
                        errors.insert_with_default_key(crate::errors::AppError::NotFound);
                        view! { <error_template::ErrorTemplate errors/> }.into_view()
                    }>
                        <Route path=StaticSegment("") view=HomePage/>
                        <Route path=StaticSegment("resume") view=ResumePage/>
                    </Routes>
                </main>
                <SiteFooter/>
            </Router>
        </ThemeProvider>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    let meta = crate::meta::PageMeta {
        title: crate::meta::MetaTitle::new("atmil — portfolio")
            .expect("home title is a valid MetaTitle"),
        description: crate::meta::MetaDescription::new(
            "Personal website for displaying a code portfolio and expressing my thoughts",
        )
        .expect("home description is a valid MetaDescription"),
        image: None,
    };
    view! {
        <crate::meta::PageMetaTags meta/>
        <div class="prose">
            <h1>"Portfolio"</h1>
            <p><a href="/resume">"Résumé"</a></p>
        </div>
    }
}
