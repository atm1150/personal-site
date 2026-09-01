//! The env-driven listener surface: TLS termination on the site listener and
//! the auxiliary plain-http health listener. These bind real sockets - the
//! TLS test performs a genuine rustls handshake against a throwaway rcgen
//! certificate, which `tower::oneshot` cannot exercise.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use axum_server::Handle;
use common::test_router;
use server::READYZ_PATH;
use server::security::Hsts;

/// A throwaway self-signed cert for `localhost`, written to PEM files the way
/// the real env contract delivers them.
struct ThrowawayCert {
    /// Held for its Drop: deleting the dir would invalidate the paths.
    _dir: tempfile::TempDir,
    cert_path: std::path::PathBuf,
    key_path: std::path::PathBuf,
    cert_pem: String,
}

fn throwaway_cert() -> Result<ThrowawayCert, Box<dyn std::error::Error>> {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()])?;
    let cert_pem = certified.cert.pem();
    let key_pem = certified.signing_key.serialize_pem();

    let dir = tempfile::tempdir()?;
    let cert_path = dir.path().join("cert.pem");
    let key_path = dir.path().join("key.pem");
    std::fs::write(&cert_path, &cert_pem)?;
    std::fs::write(&key_path, &key_pem)?;

    Ok(ThrowawayCert {
        _dir: dir,
        cert_path,
        key_path,
        cert_pem,
    })
}

#[tokio::test]
async fn tls_listener_serves_readyz_over_https() -> Result<(), Box<dyn std::error::Error>> {
    let cert = throwaway_cert()?;

    let handle = Handle::new();
    let bind_addr: SocketAddr = "127.0.0.1:0".parse()?;
    let server_task = tokio::spawn({
        let handle = handle.clone();
        let cert_path = cert.cert_path.clone();
        let key_path = cert.key_path.clone();
        async move {
            server::tls::serve(
                test_router(Hsts::On),
                bind_addr,
                cert_path,
                key_path,
                handle,
                &server::limits::ConnectionLimits::production(),
            )
            .await
        }
    });

    // Timeout so a listener that never binds fails the test instead of hanging it.
    let addr = tokio::time::timeout(Duration::from_secs(5), handle.listening())
        .await?
        .expect("TLS listener should bind");

    // Trust exactly the throwaway cert; force `localhost` (the cert's SAN) to
    // resolve to the ephemeral port instead of touching real DNS.
    let client = reqwest::Client::builder()
        .add_root_certificate(reqwest::Certificate::from_pem(cert.cert_pem.as_bytes())?)
        .resolve("localhost", addr)
        .build()?;

    let response = client
        .get(format!("https://localhost:{}{READYZ_PATH}", addr.port()))
        .send()
        .await?;
    assert_eq!(response.status(), 200, "readyz over TLS should be served");
    assert_eq!(response.text().await?, "ok");

    handle.shutdown();
    server_task.await??;
    Ok(())
}

/// tls::serve with unparseable PEM contents must return an error - never bind,
/// never fall back to plain http. (The env-shape half of fail-closed lives in
/// the tls module's unit tests; this covers bad file *contents*.)
#[tokio::test]
async fn tls_serve_fails_closed_on_malformed_pem() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let cert_path = dir.path().join("cert.pem");
    let key_path = dir.path().join("key.pem");
    std::fs::write(&cert_path, "not a certificate")?;
    std::fs::write(&key_path, "not a key")?;

    let result = server::tls::serve(
        test_router(Hsts::Off),
        "127.0.0.1:0".parse::<SocketAddr>()?,
        cert_path,
        key_path,
        Handle::new(),
        &server::limits::ConnectionLimits::production(),
    )
    .await;

    assert!(
        result.is_err(),
        "malformed PEM must fail closed instead of serving"
    );
    Ok(())
}

/// Bind `server::health_app()` - the exact app `main` serves on the auxiliary
/// listener - on an ephemeral port and return the bound address.
async fn spawn_health_app() -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        axum::serve(listener, server::health_app())
            .await
            .expect("health listener should serve");
    });
    Ok(addr)
}

#[tokio::test]
async fn health_app_serves_readyz_over_plain_http() -> Result<(), Box<dyn std::error::Error>> {
    let addr = spawn_health_app().await?;

    let response = reqwest::get(format!("http://{addr}{READYZ_PATH}")).await?;
    assert_eq!(response.status(), 200, "readyz should be served");
    assert_eq!(response.text().await?, "ok");
    Ok(())
}

#[tokio::test]
async fn health_app_serves_nothing_else() -> Result<(), Box<dyn std::error::Error>> {
    let addr = spawn_health_app().await?;

    let response = reqwest::get(format!("http://{addr}/")).await?;
    assert_eq!(
        response.status(),
        404,
        "the health listener must not expose the site over plain http"
    );
    Ok(())
}

#[tokio::test]
async fn health_app_carries_security_headers() -> Result<(), Box<dyn std::error::Error>> {
    let addr = spawn_health_app().await?;

    let response = reqwest::get(format!("http://{addr}{READYZ_PATH}")).await?;
    assert_eq!(
        response
            .headers()
            .get("x-content-type-options")
            .and_then(|v| v.to_str().ok()),
        Some("nosniff"),
        "the auxiliary listener must carry the same constant security headers as the main router"
    );
    Ok(())
}

#[tokio::test]
async fn hsts_reaches_the_client_over_a_real_tls_handshake()
-> Result<(), Box<dyn std::error::Error>> {
    let cert = throwaway_cert()?;

    let handle = Handle::new();
    let bind_addr: SocketAddr = "127.0.0.1:0".parse()?;
    let server_task = tokio::spawn({
        let handle = handle.clone();
        let cert_path = cert.cert_path.clone();
        let key_path = cert.key_path.clone();
        async move {
            server::tls::serve(
                test_router(Hsts::On),
                bind_addr,
                cert_path,
                key_path,
                handle,
                &server::limits::ConnectionLimits::production(),
            )
            .await
        }
    });

    let addr = tokio::time::timeout(Duration::from_secs(5), handle.listening())
        .await?
        .expect("TLS listener should bind");

    let client = reqwest::Client::builder()
        .add_root_certificate(reqwest::Certificate::from_pem(cert.cert_pem.as_bytes())?)
        .resolve("localhost", addr)
        .build()?;

    let response = client
        .get(format!("https://localhost:{}{READYZ_PATH}", addr.port()))
        .send()
        .await?;
    assert_eq!(
        response
            .headers()
            .get("strict-transport-security")
            .and_then(|v| v.to_str().ok()),
        Some("max-age=300"),
        "a real https response must carry HSTS"
    );

    handle.shutdown();
    server_task.await??;
    Ok(())
}

#[tokio::test]
async fn health_listener_never_carries_hsts() -> Result<(), Box<dyn std::error::Error>> {
    // The health listener is plain http even on TLS runs; HSTS there would tell
    // the client to use https on a port that does not speak it.
    let addr = spawn_health_app().await?;

    let response = reqwest::get(format!("http://{addr}{READYZ_PATH}")).await?;
    assert!(
        !response.headers().contains_key("strict-transport-security"),
        "the auxiliary plain-http listener must never send HSTS"
    );
    Ok(())
}
