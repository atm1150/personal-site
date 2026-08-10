//! Runtime edge for the SSR server.
//!
//! This is the thin adapter to the tokio + hyper runtime: initialize telemetry,
//! read config, bind, serve, and drain on shutdown. Everything about *what the app
//! is* lives in the library ([`server::router`]) so a different runtime only needs a
//! different `main`.

use leptos::prelude::*;
use server::TelemetryConfig;
use server::security::Hsts;
use server::tls::{self, TlsConfig, TlsMode, TlsSource};

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

    // Resolved before the router is built: the router needs to know whether this
    // run serves TLS, and a misconfigured listener should fail before any work.
    let tls_config = TlsConfig::from_env()
        .expect("listener configuration should resolve from SITE_TLS/TLS_*_PATH/HEALTH_ADDR env");

    // One provenance line per resolved key: an override is never an error, but it
    // is always visible. "declared" vs "defaulted" is exactly the distinction the
    // env contract now carries and the old cert-presence inference could not.
    let (tls_state, tls_origin) = match (&tls_config.mode, tls_config.source) {
        (TlsMode::Enabled { .. }, _) => ("on", "env SITE_TLS"),
        (TlsMode::Disabled, TlsSource::Declared) => ("off", "env SITE_TLS"),
        (TlsMode::Disabled, TlsSource::Defaulted) => ("off", "default (SITE_TLS unset)"),
    };
    tracing::info!(tls = tls_state, source = tls_origin, "tls intent resolved");

    // Same contract as SITE_TLS: absent is a fallback in dev, a hard failure
    // under LEPTOS_ENV=PROD, and malformed is always fatal.
    let is_prod = matches!(conf.leptos_options.env, leptos::config::Env::PROD);
    let public_base_url = server::config::resolve_public_base_url(
        is_prod,
        std::env::var("PUBLIC_BASE_URL").ok().as_deref(),
    )
    .expect(
        "PUBLIC_BASE_URL should be a bare absolute http(s) URL (required when LEPTOS_ENV=PROD)",
    );
    match &public_base_url {
        Some(base) => {
            tracing::info!(
                base = base.as_str(),
                source = "env PUBLIC_BASE_URL",
                "public base url resolved"
            );
        }
        None => {
            tracing::info!(
                source = "default (PUBLIC_BASE_URL unset)",
                "public base url absent; og:url and canonical omitted"
            );
        }
    }
    match &public_base_url {
        Some(base) => tracing::info!(
            www_host = format!("www.{}", base.host()),
            source = "derived from PUBLIC_BASE_URL",
            "www redirect enabled"
        ),
        None => tracing::info!(
            source = "default (PUBLIC_BASE_URL unset)",
            "www redirect disabled"
        ),
    }

    let app = server::router(
        conf.leptos_options,
        Hsts::from(&tls_config.mode),
        public_base_url,
    );

    match tls_config.mode {
        TlsMode::Disabled => {
            tracing::info!("listening on http://{addr}");
            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .expect("site address should be bindable (LEPTOS_SITE_ADDR)");
            // The site socket is bound (connections queue from here on), so a
            // health 200 can no longer precede the site actually serving.
            spawn_health_listener(tls_config.health_addr).await;
            axum::serve(listener, app.into_make_service())
                .with_graceful_shutdown(shutdown_signal())
                .await
                .expect("server should run until shutdown");
        }
        TlsMode::Enabled {
            cert_path,
            key_path,
        } => {
            tracing::info!("listening on https://{addr}");
            // axum-server drives graceful shutdown through its Handle instead
            // of a future: relay the signal, allowing 10s for in-flight
            // requests to drain (the http path waits unbounded; here a bound
            // keeps a wedged TLS connection from stalling shutdown).
            let handle = axum_server::Handle::new();
            tokio::spawn({
                let handle = handle.clone();
                async move {
                    shutdown_signal().await;
                    handle.graceful_shutdown(Some(std::time::Duration::from_secs(10)));
                }
            });
            let server = tokio::spawn(tls::serve(app, addr, cert_path, key_path, handle.clone()));
            // The health listener starts only once `listening()` resolves with
            // an address: the cert has loaded and the site socket is bound, so
            // a health 200 always means the site is serving. On startup failure
            // it resolves None - skip health and join the server task, whose
            // error the expect below surfaces.
            if handle.listening().await.is_some() {
                spawn_health_listener(tls_config.health_addr).await;
            }
            server
                .await
                .expect("TLS server task should not panic")
                .expect("TLS server should run until shutdown");
        }
    }
}

/// Bind and spawn the auxiliary plain-http `/readyz` listener, if configured.
///
/// Callers invoke this only after the site listener is up, preserving the
/// invariant that a health 200 implies the site is serving. Spawned, not
/// joined: health polling must never outlive or block the site listener.
async fn spawn_health_listener(health_addr: Option<std::net::SocketAddr>) {
    let Some(health_addr) = health_addr else {
        return;
    };
    let health_listener = tokio::net::TcpListener::bind(&health_addr)
        .await
        .expect("health address should be bindable (HEALTH_ADDR)");
    tracing::info!("health listener on http://{health_addr}");
    tokio::spawn(async move {
        axum::serve(health_listener, server::health_app())
            .await
            .expect("health listener should serve");
    });
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
