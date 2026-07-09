//! Runtime edge for the SSR server.
//!
//! This is the thin adapter to the tokio + hyper runtime: initialize telemetry,
//! read config, bind, serve, and drain on shutdown. Everything about *what the app
//! is* lives in the library ([`server::router`]) so a different runtime only needs a
//! different `main`.

use leptos::prelude::*;
use server::TelemetryConfig;

#[tokio::main]
async fn main() {
    // Initialize telemetry first so startup and every subsequent event is captured.
    // Held for the whole of `main` so the guard flushes on exit. A setup failure is
    // fatal by design — a misconfigured telemetry pipeline should surface loudly.
    let _telemetry = TelemetryConfig::from_env()
        .and_then(TelemetryConfig::install)
        .expect("telemetry should initialize (config comes from OTEL_* env)");

    let conf =
        get_configuration(None).expect("leptos configuration should resolve from LEPTOS_* env");
    let addr = conf.leptos_options.site_addr;
    let app = server::router(conf.leptos_options);

    tracing::info!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("site address should be bindable (LEPTOS_SITE_ADDR)");
    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server should run until shutdown");
}

/// Resolve when the process receives SIGINT (Ctrl+C) or SIGTERM, so in-flight
/// requests drain and the telemetry guard flushes before exit.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Ctrl+C handler should install");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM handler should install")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received, draining");
}
