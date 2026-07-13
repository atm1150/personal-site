//! Portfolio SSR server library — the composition root.
//!
//! `main` (the runtime edge) reads config, sets up telemetry, and calls [`router`]
//! to assemble the application. Keeping composition here rather than in `main` means
//! porting to a different runtime only needs a new `main` (or a new main/lib pair) —
//! the app's shape is defined once, here.

pub mod telemetry;

pub use telemetry::{TelemetryConfig, TelemetryError, TelemetryGuard};

use app::*;
use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use leptos::prelude::*;
use leptos_axum::{LeptosRoutes, generate_route_list};

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

/// Assemble the application router.
///
/// The Leptos routes + fallback are traced via [`telemetry::trace_layer`]; `/readyz`
/// is mounted *outside* that layer, so the orchestrator's health poll produces no
/// spans — the exclusion is structural, telemetry has no knowledge of the path.
pub fn router(leptos_options: LeptosOptions) -> Router {
    let routes = generate_route_list(App);

    let traced = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options)
        .layer(telemetry::trace_layer());

    Router::new()
        .route(READYZ_PATH, get(readyz))
        .merge(traced)
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
}
