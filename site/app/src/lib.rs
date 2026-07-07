use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

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

        <Router>
            <main>
                // TODO Need a fallback page eventually
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    let count = RwSignal::new(0);
    let on_click = move |_| *count.write() += 1;

    view! {
        <h1>"Portfolio"</h1>
        <p>"Placeholder page: server-rendered by Leptos + Axum, hydrated by WebAssembly."</p>
        <button on:click=on_click>"Hydration check: " {count}</button>
    }
}
