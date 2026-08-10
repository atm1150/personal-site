//! Converts a request-time panic into a minimal, generic 500 page instead of
//! tearing down the connection with no response at all.
//!
//! The panic message is never sent to the client - only `tracing::error!`
//! sees it, so the OTLP pipeline can still capture it for diagnosis. This is
//! deliberately last-resort: handlers should return `Result`/`AppError`
//! wherever possible, and this layer only guards against the ones that don't.

use std::any::Any;

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use tower_http::catch_panic::CatchPanicLayer;

/// Minimal full document: no dynamic content, so there is nothing per-request
/// to accidentally leak. Mirrors the shape of the 404 `ErrorTemplate` page.
const BODY: &str = "<!doctype html>\
<html lang=\"en\">\
<head>\
<meta charset=\"utf-8\">\
<title>500 Internal Server Error</title>\
</head>\
<body>\
<div class=\"server-error-page\">\
<h1>500 Internal Server Error</h1>\
<p>Something went wrong on our end.</p>\
<p><a href=\"/\">Back to home</a></p>\
</div>\
</body>\
</html>";

/// Wrap the Leptos routes in this so a panicking handler produces the generic
/// 500 page above instead of dropping the connection.
pub fn catch_panic_layer() -> CatchPanicLayer<fn(Box<dyn Any + Send>) -> Response> {
    CatchPanicLayer::custom(respond_to_panic)
}

/// The real panic message stays server-side: logged here, never written into
/// the response body built below.
fn respond_to_panic(payload: Box<dyn Any + Send>) -> Response {
    let detail = if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    };
    tracing::error!(panic.detail = %detail, "request handler panicked");

    let mut response = Response::new(BODY.into());
    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response
}
