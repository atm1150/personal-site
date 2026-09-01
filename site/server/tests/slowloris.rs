//! Connection-level (slowloris) bounds, exercised over real sockets.

use std::net::SocketAddr;
use std::time::Duration;

use axum::{Router, routing::get};
use server::limits::{ConnectionLimits, apply_connection_bounds};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const TEST_HEADER_TIMEOUT: Duration = Duration::from_millis(100);
const CLOSE_MARGIN: Duration = Duration::from_secs(3);

async fn spawn_bounded_listener() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral");
    let addr = listener.local_addr().expect("addr");
    let app = Router::new().route("/", get(|| async { "ok" }));
    // http1_only skips the untimed version sniff, so the header-read timeout
    // also covers a silent connection.
    let mut server = axum_server::Server::<SocketAddr>::from_listener(listener).http1_only();
    apply_connection_bounds(
        &mut server,
        &ConnectionLimits::with_header_read_timeout(TEST_HEADER_TIMEOUT),
    );
    tokio::spawn(async move {
        let _ = server.serve(app.into_make_service()).await;
    });
    tokio::time::sleep(Duration::from_millis(20)).await; // let the accept loop start
    addr
}

/// Resolves when the server closes the socket; panics if it stays open past CLOSE_MARGIN.
async fn assert_closed_by_server(mut stream: TcpStream) {
    let mut buf = [0u8; 64];
    match tokio::time::timeout(CLOSE_MARGIN, stream.read(&mut buf)).await {
        Ok(Ok(0)) | Ok(Err(_)) => {}
        Ok(Ok(_n)) => match tokio::time::timeout(CLOSE_MARGIN, stream.read(&mut buf)).await {
            Ok(Ok(0)) | Ok(Err(_)) => {}
            _ => panic!("server did not close the dribbling connection"),
        },
        Err(_) => panic!("server never closed the dribbling connection within the margin"),
    }
}

#[tokio::test]
async fn http1_partial_header_is_closed() {
    let addr = spawn_bounded_listener().await;
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: x\r\n")
        .await
        .expect("write partial");
    assert_closed_by_server(stream).await;
}

#[tokio::test]
async fn a_silent_connection_is_closed() {
    let addr = spawn_bounded_listener().await;
    let stream = TcpStream::connect(addr).await.expect("connect");
    assert_closed_by_server(stream).await;
}
