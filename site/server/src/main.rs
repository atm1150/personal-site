use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use leptos::prelude::*;
use leptos_axum::{generate_route_list, LeptosRoutes};
use app::*;
use leptos::logging::log;

/// The readiness path this server serves. The Aspire AppHost health-checks the same
/// path, sourcing it from `ready-path` in `[workspace.metadata.orchestrator]` in the
/// workspace Cargo.toml; `readyz_route_matches_workspace_metadata` below pins this
/// const to that declaration so the two sides cannot drift silently.
const READYZ_PATH: &str = "/readyz";

/// Readiness probe. Static 200 while the server is a single stateless process;
/// gains real dependency checks (e.g. database reachability) when those arrive.
async fn readyz() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok")
}

#[tokio::main]
async fn main() {

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    // Generate the list of routes in your Leptos App
    let routes = generate_route_list(App);

    let app = Router::new()
        .route(READYZ_PATH, get(readyz))
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    log!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the served readiness path to the one declared in the workspace Cargo.toml.
    ///
    /// The full agreement chain this closes: the axum route uses `READYZ_PATH`
    /// (compiler-checked), this test asserts `READYZ_PATH` equals the declared
    /// `ready-path` (test-checked), and the Aspire AppHost reads that declaration at
    /// startup to build its health probe (by construction). Together: the path Aspire
    /// probes is the path this server serves.
    #[test]
    fn readyz_route_matches_workspace_metadata() {
        let manifest: toml::Value = toml::from_str(include_str!("../../Cargo.toml"))
            .expect("workspace Cargo.toml should be valid TOML");

        let declared = manifest
            .get("workspace")
            .and_then(|v| v.get("metadata"))
            .and_then(|v| v.get("orchestrator"))
            .and_then(|v| v.get("ready-path"))
            .and_then(|v| v.as_str())
            .expect("[workspace.metadata.orchestrator] ready-path should be declared in site/Cargo.toml");

        assert_eq!(
            declared, READYZ_PATH,
            "ready-path in site/Cargo.toml must match the route this server registers"
        );
    }

    /// The contract the orchestrator's health probe relies on: 200 with a body.
    #[tokio::test]
    async fn readyz_returns_ok() {
        let (status, body) = readyz().await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "ok");
    }
}
