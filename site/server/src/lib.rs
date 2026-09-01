//! Portfolio SSR server library — the composition root.
//!
//! `main` (the runtime edge) reads config, sets up telemetry, and calls [`router`]
//! to assemble the application. Keeping composition here rather than in `main` means
//! porting to a different runtime only needs a new `main` (or a new main/lib pair) —
//! the app's shape is defined once, here.

pub mod config;
pub mod http_redirect;
pub mod internal_error;
pub mod limits;
pub mod security;
pub mod telemetry;
pub mod tls;
pub mod www_redirect;

pub use telemetry::{TelemetryConfig, TelemetryError, TelemetryGuard};

use app::*;
use axum::Router;
use axum::http::StatusCode;
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use leptos::prelude::*;
use leptos_axum::{LeptosRoutes, generate_route_list};
use limits::RequestLimits;
use security::Hsts;
use tower_governor::GovernorLayer;
use www_redirect::{WwwRedirect, redirect_www};

/// The readiness path this server serves. The Aspire AppHost health-checks the same
/// path, sourcing it from `ready-path` in `[workspace.metadata.orchestrator]` in the
/// workspace Cargo.toml; `readyz_route_matches_workspace_metadata` below pins this
/// const to that declaration so the two sides cannot drift silently.
pub const READYZ_PATH: &str = "/readyz";

/// Readiness probe. Static 200 while the server is a single stateless process;
/// gains real dependency checks (e.g. database reachability) when those arrive.
async fn readyz() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok")
}

/// Distinctive panic payload the `/__test/panic` route raises, so integration
/// tests can prove it never reaches a response body while still seeing it in
/// captured logs. Integration tests cannot see `#[cfg(test)]` items in this
/// crate, hence the separate `test-util` feature instead.
#[cfg(feature = "test-util")]
pub const TEST_PANIC_MARKER: &str = "deliberate-test-panic-detail-marker";

/// Router for the auxiliary plain-http health listener: `/readyz` and nothing else.
///
/// Bound (by `main`, when `HEALTH_ADDR` is set) in addition to the site listener
/// so orchestrator probes reach readiness without trusting the TLS certificate.
/// Serving nothing else keeps the unauthenticated plain-http surface minimal.
pub fn health_router() -> Router {
    Router::new().route(READYZ_PATH, get(readyz))
}

/// The auxiliary health listener's complete app: [`health_router`] wrapped in
/// the same constant security headers the main router carries, so the two
/// `/readyz` surfaces respond identically however they are reached.
pub fn health_app() -> Router {
    // Always Hsts::Off: this listener is plain http by design, and internal
    // plumbing that is never browsed.
    health_router().layer(from_fn_with_state(
        Hsts::Off,
        security::set_security_headers,
    ))
}

/// Assemble the application router.
///
/// The Leptos routes + fallback are traced via [`telemetry::trace_layer`] and
/// wrapped in [`internal_error::catch_panic_layer`], so a panicking handler
/// still produces a traced response instead of dropping the connection;
/// `/readyz` is mounted *outside* both layers, so the orchestrator's health
/// poll produces no spans — the exclusion is structural, telemetry has no
/// knowledge of the path.
pub fn router(
    leptos_options: LeptosOptions,
    hsts: Hsts,
    public_base_url: Option<app::meta::PublicBaseUrl>,
    limits: RequestLimits,
) -> Router {
    let routes = generate_route_list(App);

    // Derived here, before `public_base_url` moves into the context closure.
    let www_redirect = WwwRedirect::derive(public_base_url.as_ref());

    let traced = Router::new()
        .leptos_routes_with_context(
            &leptos_options,
            routes,
            {
                let public_base_url = public_base_url.clone();
                move || {
                    // Provided only when configured: shell and pages read
                    // use_context, and absence is the documented degrade path.
                    if let Some(base) = public_base_url.clone() {
                        provide_context(base);
                    }
                }
            },
            {
                let leptos_options = leptos_options.clone();
                move || shell(leptos_options.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    // Added before the layers below so the test routes are wrapped by them -
    // a route added after `.layer(...)` would not be.
    #[cfg(feature = "test-util")]
    let traced = {
        async fn test_panic() {
            panic!("{}", TEST_PANIC_MARKER);
        }
        // echo reads its body (no production route does), hang never returns:
        // the observable surfaces for the body-limit and timeout tests.
        async fn test_echo(body: axum::body::Bytes) -> String {
            body.len().to_string()
        }
        async fn test_hang() {
            std::future::pending::<()>().await;
        }
        traced
            .route("/__test/panic", get(test_panic))
            .route("/__test/echo", axum::routing::post(test_echo))
            .route("/__test/hang", get(test_hang))
    };

    // catch_panic is innermost (added first) so a panicking handler still
    // produces an ordinary response; body-limit and timeout sit above it so
    // trace (outermost of this group, added last) observes their synthesized
    // 413/408 too, and the responder's error event fires inside the request
    // span.
    let traced = traced
        .layer(internal_error::catch_panic_layer())
        .layer(tower_http::limit::RequestBodyLimitLayer::new(
            limits::BODY_LIMIT_BYTES,
        ))
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            limits.request_timeout,
        ))
        .layer(telemetry::trace_layer());

    // Outside the trace layer: a rejected flood must not buy a span per
    // request. /readyz is merged below the layer, keeping probes exempt.
    let traced = match limits.governor {
        Some(config) => traced.layer(GovernorLayer::new(config)),
        None => traced,
    };

    // The www-redirect layer is innermost (added first), the constant
    // security headers outermost, so a 301 still carries the constant
    // headers. Both wrap the whole router: SSR pages, static assets, and
    // /readyz. The per-request CSP is set separately during SSR render.
    // A redirected www request never reaches `traced`, so it produces no
    // spans.
    health_router()
        .merge(traced)
        .layer(from_fn_with_state(www_redirect, redirect_www))
        .layer(from_fn_with_state(hsts, security::set_security_headers))
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
