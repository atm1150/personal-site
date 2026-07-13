pub mod resume;

use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    StaticSegment,
    components::{Route, Router, Routes},
};
use resume::ResumePage;
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

    view! {
        <Stylesheet id="leptos" href="/pkg/portfolio.css"/>

        <Title text="atmil — portfolio"/>

        // ThemeProvider is load-bearing beyond theming: it injects singlestage's
        // compiled component CSS (a <style> tag) into the page. Components render
        // unstyled without it.
        <ThemeProvider>
            <Router>
                <main>
                    // TODO Need a fallback page eventually
                    <Routes fallback=|| "Page not found.".into_view()>
                        <Route path=StaticSegment("") view=HomePage/>
                        <Route path=StaticSegment("resume") view=ResumePage/>
                    </Routes>
                </main>
            </Router>
        </ThemeProvider>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    view! {
        <h1>"Portfolio"</h1>
        <p><a href="/resume">"Résumé"</a></p>
    }
}
