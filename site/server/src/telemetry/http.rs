//! Producer-side HTTP telemetry: the tower middleware that turns each incoming
//! request into a `tracing` span (which the pipeline's span layer exports).
//!
//! This has no knowledge of which routes exist — the composition root decides what
//! the layer wraps. Route awareness comes only from the [`MatchedPath`] request
//! extension axum inserts after routing (`Router::layer` middleware runs
//! post-routing, so it is visible here).
//!
//! The response status is recorded onto the span in [`on_response`], not via
//! tower-http's default response event: that event is `DEBUG`-level and the OTel
//! layers floor at `info`, so it never exports. `tracing` only lets a span carry
//! fields declared at creation, so [`request_span`] declares the status fields as
//! [`Empty`](tracing::field::Empty) up front for `on_response` to fill.

use std::time::Duration;

use axum::extract::{MatchedPath, Request};
use axum::http::Version;
use axum::http::header::USER_AGENT;
use axum::response::Response;
use tower_http::classify::{ServerErrorsAsFailures, SharedClassifier};
use tower_http::trace::{DefaultOnRequest, TraceLayer};
use tracing::Span;

/// Cap on the recorded `user_agent.original` value. The header is attacker-supplied
/// and hyper accepts very large ones; without a cap, each span can pin ~hundreds of
/// KB in the batch queue while the collector is slow or down.
const MAX_USER_AGENT_LEN: usize = 256;

/// The request-tracing layer type, ready to drop into a router via `.layer(...)`.
/// Named (with plain fn-pointer span maker + response hook) so the type stays
/// writable; the trailing `OnBodyChunk`/`OnEos`/`OnFailure` params keep their defaults.
pub type HttpTraceLayer = TraceLayer<
    SharedClassifier<ServerErrorsAsFailures>,
    fn(&Request) -> Span,
    DefaultOnRequest,
    fn(&Response, Duration, &Span),
>;

/// Build the request-tracing layer.
pub fn trace_layer() -> HttpTraceLayer {
    TraceLayer::new_for_http()
        .make_span_with(request_span as fn(&Request) -> Span)
        .on_response(on_response as fn(&Response, Duration, &Span))
}

/// The span for one HTTP request. Named `<METHOD> <route template>` (or just
/// `<METHOD>` when no route matched) via tracing-opentelemetry's special `otel.name`
/// field, per the OTel HTTP semconv for server spans: the name must be
/// low-cardinality, and the raw path is attacker-chosen — naming spans after it
/// would let any URL scanner mint unbounded distinct operation names downstream.
/// The raw path is still carried by the `url.path` attribute. Attribute keys follow
/// the OTel HTTP semantic conventions; `http.response.status_code`/`otel.status_code`
/// are declared [`Empty`](tracing::field::Empty) and filled by [`on_response`].
fn request_span(request: &Request) -> Span {
    let method = request.method();
    let otel_name = match request.extensions().get::<MatchedPath>() {
        Some(route) => format!("{method} {}", route.as_str()),
        None => method.to_string(),
    };
    let span = tracing::info_span!(
        "http_request",
        otel.name = %otel_name,
        http.request.method = %method,
        url.path = %request.uri().path(),
        network.protocol.version = protocol_version(request.version()),
        user_agent.original = tracing::field::Empty,
        http.response.status_code = tracing::field::Empty,
        otel.status_code = tracing::field::Empty,
    );
    if let Some(user_agent) = request
        .headers()
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())
    {
        span.record(
            "user_agent.original",
            truncate(user_agent, MAX_USER_AGENT_LEN),
        );
    }
    span
}

/// Record the response status onto the request span. Sets `otel.status_code = ERROR`
/// on 5xx (the field tracing-opentelemetry maps to the OTel span Status); other
/// statuses leave the status Unset, per the HTTP semantic conventions for server spans.
fn on_response(response: &Response, _latency: Duration, span: &Span) {
    let status = response.status();
    span.record("http.response.status_code", status.as_u16());
    if status.is_server_error() {
        span.record("otel.status_code", "ERROR");
    }
}

/// Map the HTTP version to its `network.protocol.version` semconv value.
fn protocol_version(version: Version) -> &'static str {
    match version {
        Version::HTTP_09 => "0.9",
        Version::HTTP_10 => "1.0",
        Version::HTTP_11 => "1.1",
        Version::HTTP_2 => "2",
        Version::HTTP_3 => "3",
        _ => "unknown",
    }
}

/// Truncate to at most `max` bytes without splitting a UTF-8 character.
/// (Header values that pass `to_str` are visible ASCII, but this helper does not
/// lean on that distant invariant.)
fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use axum::Router;
    use axum::body::Body;
    use axum::http::{HeaderValue, StatusCode};
    use axum::routing::get;
    use tower::ServiceExt as _;
    use tracing_subscriber::layer::SubscriberExt as _;

    use super::*;

    /// Captures every span's fields (from creation and later `record` calls) into
    /// one map per span, keyed by span id — enough to assert on what
    /// [`request_span`] / [`on_response`] emit without a real exporter.
    #[derive(Clone, Default)]
    struct SpanCapture {
        spans: Arc<Mutex<HashMap<u64, HashMap<String, String>>>>,
    }

    impl SpanCapture {
        /// Field maps of all captured spans (order unspecified; tests drive one
        /// request each, so there is exactly one).
        fn spans(&self) -> Vec<HashMap<String, String>> {
            self.spans.lock().unwrap().values().cloned().collect()
        }
    }

    struct FieldVisitor<'a>(&'a mut HashMap<String, String>);

    impl tracing::field::Visit for FieldVisitor<'_> {
        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            self.0.insert(field.name().to_owned(), value.to_owned());
        }

        fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
            self.0.insert(field.name().to_owned(), value.to_string());
        }

        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            self.0.insert(field.name().to_owned(), format!("{value:?}"));
        }
    }

    impl<S> tracing_subscriber::Layer<S> for SpanCapture
    where
        S: tracing::Subscriber,
    {
        fn on_new_span(
            &self,
            attrs: &tracing::span::Attributes<'_>,
            id: &tracing::span::Id,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            let mut fields = HashMap::new();
            attrs.record(&mut FieldVisitor(&mut fields));
            self.spans.lock().unwrap().insert(id.into_u64(), fields);
        }

        fn on_record(
            &self,
            id: &tracing::span::Id,
            values: &tracing::span::Record<'_>,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            let mut spans = self.spans.lock().unwrap();
            let fields = spans.entry(id.into_u64()).or_default();
            values.record(&mut FieldVisitor(fields));
        }
    }

    /// Send one request through a router wrapped in [`trace_layer`], returning the
    /// single captured request span's fields and the response status. The capture
    /// subscriber is thread-local (`set_default`), so parallel tests never interfere.
    async fn drive(request: Request) -> (HashMap<String, String>, StatusCode) {
        let capture = SpanCapture::default();
        let _guard =
            tracing::subscriber::set_default(tracing_subscriber::registry().with(capture.clone()));

        let app = Router::new()
            .route("/some/path", get(async || "ok"))
            .route("/users/{id}", get(async || "user"))
            .route("/boom", get(async || StatusCode::INTERNAL_SERVER_ERROR))
            .layer(trace_layer());
        let status = app.oneshot(request).await.expect("infallible").status();

        let mut spans = capture.spans();
        assert_eq!(spans.len(), 1, "each request produces exactly one span");
        (spans.pop().expect("asserted non-empty"), status)
    }

    fn get_request(uri: &str) -> Request {
        Request::builder()
            .uri(uri)
            .body(Body::empty())
            .expect("valid request")
    }

    #[tokio::test]
    async fn span_is_named_after_route_template_not_raw_path() {
        let (fields, status) = drive(get_request("/users/42")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fields["otel.name"], "GET /users/{id}");
        assert_eq!(fields["http.request.method"], "GET");
        assert_eq!(fields["url.path"], "/users/42");
    }

    #[tokio::test]
    async fn unmatched_request_span_is_named_method_only() {
        // No MatchedPath on the fallback path: scanner probes must all collapse
        // into one operation name instead of minting one per probed URL.
        let (fields, status) = drive(get_request("/wp-login.php")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(fields["otel.name"], "GET");
        assert_eq!(fields["url.path"], "/wp-login.php");
    }

    #[tokio::test]
    async fn user_agent_recorded_when_present() {
        let mut request = get_request("/some/path");
        request
            .headers_mut()
            .insert(USER_AGENT, HeaderValue::from_static("test-agent/1.0"));
        let (fields, _) = drive(request).await;
        assert_eq!(fields["user_agent.original"], "test-agent/1.0");
    }

    #[tokio::test]
    async fn user_agent_absent_when_header_missing() {
        let (fields, _) = drive(get_request("/some/path")).await;
        assert!(!fields.contains_key("user_agent.original"));
    }

    #[tokio::test]
    async fn user_agent_absent_when_header_is_not_utf8() {
        let mut request = get_request("/some/path");
        request.headers_mut().insert(
            USER_AGENT,
            HeaderValue::from_bytes(b"\xff\xfe").expect("obs-text bytes are a legal header value"),
        );
        let (fields, _) = drive(request).await;
        assert!(!fields.contains_key("user_agent.original"));
    }

    #[tokio::test]
    async fn user_agent_truncated_to_cap() {
        let long = "a".repeat(MAX_USER_AGENT_LEN * 4);
        let mut request = get_request("/some/path");
        request.headers_mut().insert(
            USER_AGENT,
            HeaderValue::from_str(&long).expect("ASCII header value"),
        );
        let (fields, _) = drive(request).await;
        assert_eq!(fields["user_agent.original"].len(), MAX_USER_AGENT_LEN);
    }

    #[tokio::test]
    async fn response_status_recorded_and_error_only_on_5xx() {
        let (fields, status) = drive(get_request("/boom")).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(fields["http.response.status_code"], "500");
        assert_eq!(fields["otel.status_code"], "ERROR");

        let (fields, _) = drive(get_request("/some/path")).await;
        assert_eq!(fields["http.response.status_code"], "200");
        assert!(!fields.contains_key("otel.status_code"));

        let (fields, _) = drive(get_request("/wp-login.php")).await;
        assert_eq!(fields["http.response.status_code"], "404");
        assert!(
            !fields.contains_key("otel.status_code"),
            "4xx is not a server error and leaves the OTel status Unset"
        );
    }

    #[test]
    fn protocol_version_maps_semconv_values() {
        assert_eq!(protocol_version(Version::HTTP_09), "0.9");
        assert_eq!(protocol_version(Version::HTTP_10), "1.0");
        assert_eq!(protocol_version(Version::HTTP_11), "1.1");
        assert_eq!(protocol_version(Version::HTTP_2), "2");
        assert_eq!(protocol_version(Version::HTTP_3), "3");
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("exact", 5), "exact");
        // 'é' is 2 bytes; a 3-byte cap may not split it.
        assert_eq!(truncate("aéé", 3), "aé");
        assert_eq!(truncate("aéé", 4), "aé");
    }
}
