//! A panic inside a request handler must not tear down the connection or leak
//! internal detail: the client gets a minimal generic 500 page, and the real
//! panic message is captured only through `tracing`, never in the response.
//!
//! Holding a std `MutexGuard` across `.await` is generally a deadlock risk on
//! a multi-threaded runtime; every test here uses the `#[tokio::test]`
//! default (current-thread), so there is no other task on this thread to
//! block on the lock, and holding it across the await is exactly the point -
//! see [`serialize_panic_route_access`].
#![allow(clippy::await_holding_lock)]

mod common;

use std::sync::{Arc, Mutex, PoisonError};

use axum::http::StatusCode;
use common::{body_string, test_router};
use server::TEST_PANIC_MARKER;
use server::security::Hsts;

const PANIC_ROUTE: &str = "/__test/panic";

/// All four tests below drive the same panic route, whose panic-response
/// handler shares one `tracing` callsite. A callsite's first hit caches its
/// interest computed from the registry of *registered* dispatchers - empty
/// until the capturing test constructs its subscriber, and an empty registry
/// caches Interest::never. Registration does rebuild every known callsite,
/// so serial orderings self-heal; the flake is concurrent: a first-hitting
/// test computes `never` against a pre-registration snapshot and can store
/// it after the rebuild already ran, so the capturing test's event is
/// skipped. Serializing access removes the overlap.
static PANIC_ROUTE_ACCESS: Mutex<()> = Mutex::new(());

fn serialize_panic_route_access() -> std::sync::MutexGuard<'static, ()> {
    PANIC_ROUTE_ACCESS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

#[tokio::test]
async fn panic_returns_generic_500_page() {
    let _serialize = serialize_panic_route_access();
    let response = common::get(test_router(Hsts::Off), PANIC_ROUTE).await;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let content_type = response
        .headers()
        .get("content-type")
        .expect("a 500 response should carry a content-type")
        .to_str()
        .expect("content-type header is ASCII")
        .to_owned();
    assert!(
        content_type.starts_with("text/html"),
        "got content-type: {content_type}"
    );

    let html = body_string(response).await;
    assert!(html.contains("server-error-page"), "got body:\n{html}");
    assert!(html.contains("Back to home"), "got body:\n{html}");
}

/// The point of this whole issue: the panic message and any source location
/// must never reach the client, only the generic page.
#[tokio::test]
async fn panic_detail_never_reaches_the_response() {
    let _serialize = serialize_panic_route_access();
    let response = common::get(test_router(Hsts::Off), PANIC_ROUTE).await;
    let html = body_string(response).await;

    assert!(!html.contains(TEST_PANIC_MARKER), "got body:\n{html}");
    assert!(!html.contains("panic"), "got body:\n{html}");
    assert!(!html.contains(".rs"), "got body:\n{html}");
}

#[tokio::test]
async fn panic_response_carries_security_headers() {
    let _serialize = serialize_panic_route_access();
    let response = common::get(test_router(Hsts::Off), PANIC_ROUTE).await;
    let headers = response.headers().clone();

    assert_eq!(
        headers.get("x-content-type-options").map(|v| v.as_bytes()),
        Some(&b"nosniff"[..])
    );
    assert_eq!(
        headers.get("x-frame-options").map(|v| v.as_bytes()),
        Some(&b"DENY"[..])
    );
    assert_eq!(
        headers.get("referrer-policy").map(|v| v.as_bytes()),
        Some(&b"strict-origin-when-cross-origin"[..])
    );
    assert_eq!(
        headers
            .get("cross-origin-opener-policy")
            .map(|v| v.as_bytes()),
        Some(&b"same-origin"[..])
    );
    assert_eq!(
        headers
            .get("cross-origin-resource-policy")
            .map(|v| v.as_bytes()),
        Some(&b"same-origin"[..])
    );
}

/// A `MakeWriter` that appends into a shared buffer instead of stdout, so the
/// test can inspect what got logged without scraping process output.
#[derive(Clone, Default)]
struct SharedBuf(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for SharedBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("capture lock should not be poisoned")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedBuf {
    type Writer = SharedBuf;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Proves the panic detail is captured internally (for the OTLP pipeline)
/// even though `panic_detail_never_reaches_the_response` proves it never
/// reaches the client. Current-thread runtime (the `#[tokio::test]` default)
/// keeps the handler on the same thread as the thread-local subscriber guard.
#[tokio::test]
async fn panic_detail_is_captured_internally() {
    let _serialize = serialize_panic_route_access();

    let buf = SharedBuf::default();
    let subscriber = tracing_subscriber::fmt().with_writer(buf.clone()).finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let _ = common::get(test_router(Hsts::Off), PANIC_ROUTE).await;

    let captured = buf
        .0
        .lock()
        .expect("capture lock should not be poisoned")
        .clone();
    let text = String::from_utf8_lossy(&captured);
    assert!(
        text.contains(TEST_PANIC_MARKER),
        "captured log did not contain the panic marker:\n{text}"
    );
}
