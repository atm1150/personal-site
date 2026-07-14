//! Content-Security-Policy for document responses.
//!
//! [`content_security_policy`] builds the header value; the SSR-only
//! [`set_csp_header`] reads the per-request nonce Leptos already provides and
//! writes the header on the response, so the header and the rendered
//! `<script nonce>` always carry the same value.

/// Build the Content-Security-Policy header value for a document response.
///
/// `nonce` is the per-request value Leptos stamps on its inline hydration
/// `<script>`; emitting the same nonce here keeps the header and the rendered
/// markup in agreement (a mismatch silently blocks hydration).
///
/// `dev` widens `connect-src` to permit the `cargo leptos watch` hot-reload
/// WebSocket; production passes `false` for a same-origin-only policy.
pub fn content_security_policy(nonce: &str, dev: bool) -> String {
    // Dev only: cargo leptos watch opens a hot-reload WebSocket, which a strict
    // same-origin connect-src would block. Release stays same-origin only.
    let connect_src = if dev {
        "connect-src 'self' ws: wss:"
    } else {
        "connect-src 'self'"
    };

    [
        "default-src 'self'".to_owned(),
        // Fully strict: 'self' covers /pkg/portfolio.js, 'wasm-unsafe-eval' is
        // required for WebAssembly.instantiate under CSP3, and the nonce
        // authorizes Leptos's inline hydration <script>. No 'unsafe-inline'.
        format!("script-src 'self' 'wasm-unsafe-eval' 'nonce-{nonce}'"),
        // 'unsafe-inline' because singlestage's load-bearing ThemeProvider emits a
        // raw, un-nonced <style>, and a nonce in style-src makes browsers ignore
        // 'unsafe-inline'. Tighten to a nonce if user input or dynamic content is
        // added (the only conditions under which inline styles become exploitable).
        "style-src 'self' 'unsafe-inline'".to_owned(),
        "img-src 'self'".to_owned(),
        "font-src 'self'".to_owned(),
        connect_src.to_owned(),
        "object-src 'none'".to_owned(),
        "base-uri 'self'".to_owned(),
        "frame-ancestors 'none'".to_owned(),
        "form-action 'self'".to_owned(),
    ]
    .join("; ")
}

/// Set the `Content-Security-Policy` header on the current SSR response.
///
/// Reads the per-request nonce Leptos provides (the same one it stamps on the
/// inline hydration `<script>`) so the header authorizes exactly that script;
/// header and markup cannot drift because they share one source. Call once per
/// document render, from `App`. A no-op if no nonce or response is in context
/// (e.g. the `generate_route_list` pass), so it never panics off the SSR path.
#[cfg(feature = "ssr")]
pub fn set_csp_header() {
    use leptos::prelude::use_context;
    use leptos_axum::ResponseOptions;

    let (Some(nonce), Some(response)) =
        (leptos::nonce::use_nonce(), use_context::<ResponseOptions>())
    else {
        return;
    };

    let policy = content_security_policy(&nonce, cfg!(debug_assertions));
    if let Ok(value) = http::HeaderValue::from_str(&policy) {
        response.insert_header(http::header::CONTENT_SECURITY_POLICY, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract a single directive (e.g. "script-src") from a policy string.
    fn directive<'a>(policy: &'a str, name: &str) -> &'a str {
        policy
            .split(';')
            .map(str::trim)
            .find(|d| d.starts_with(name))
            .unwrap_or_else(|| panic!("policy should contain a {name} directive; got:\n{policy}"))
    }

    #[test]
    fn script_src_is_strict_with_nonce_and_no_unsafe_inline() {
        let policy = content_security_policy("abc123", false);
        let script_src = directive(&policy, "script-src");

        assert_eq!(
            script_src, "script-src 'self' 'wasm-unsafe-eval' 'nonce-abc123'",
            "script-src must allow only self, wasm instantiation, and the request nonce"
        );
        assert!(
            !script_src.contains("'unsafe-inline'"),
            "script-src must never allow 'unsafe-inline'; got: {script_src}"
        );
    }

    #[test]
    fn style_src_allows_unsafe_inline() {
        let policy = content_security_policy("abc123", false);
        let style_src = directive(&policy, "style-src");

        assert!(
            style_src.contains("'unsafe-inline'"),
            "style-src must allow inline styles for singlestage's ThemeProvider; got: {style_src}"
        );
    }

    #[test]
    fn dev_builds_permit_the_hot_reload_websocket() {
        let dev = content_security_policy("abc123", true);
        assert!(
            directive(&dev, "connect-src").contains("ws:"),
            "dev connect-src must allow the reload WebSocket; got:\n{dev}"
        );
    }

    #[test]
    fn release_builds_keep_connect_src_same_origin_only() {
        let release = content_security_policy("abc123", false);
        assert_eq!(
            directive(&release, "connect-src"),
            "connect-src 'self'",
            "release connect-src must be same-origin only (no ws:)"
        );
    }

    #[test]
    fn locks_down_framing_and_object_and_base() {
        let policy = content_security_policy("abc123", false);
        assert_eq!(directive(&policy, "default-src"), "default-src 'self'");
        assert_eq!(
            directive(&policy, "frame-ancestors"),
            "frame-ancestors 'none'"
        );
        assert_eq!(directive(&policy, "object-src"), "object-src 'none'");
        assert_eq!(directive(&policy, "base-uri"), "base-uri 'self'");
    }
}
