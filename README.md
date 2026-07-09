# portfolio

Personal site. Leptos 0.8 (Rust) in server-side-rendering mode behind Axum, hydrated in the browser via WebAssembly. The Cargo workspace lives in `site/`: `app` (pages and components), `frontend` (WASM entry point), `server` (Axum binary). Scaffolded from [leptos-rs/start-axum-workspace](https://github.com/leptos-rs/start-axum-workspace) (MIT).

## Build and run

Stable Rust with the `wasm32-unknown-unknown` target, plus [cargo-leptos](https://github.com/leptos-rs/cargo-leptos):

```sh
cd site
cargo leptos watch            # dev server on http://127.0.0.1:4000
cargo leptos build --release  # server binary + site bundle under site/target/
```

## Telemetry

When `OTEL_EXPORTER_OTLP_ENDPOINT` is set, the server exports traces and logs over OTLP/gRPC (the standard `OTEL_SERVICE_NAME` / `OTEL_RESOURCE_ATTRIBUTES` variables are honored); when it is absent, telemetry degrades to stdout logging only.

The exporter is built without TLS support: the endpoint must be a plain `http://` URL, so the collector must run co-located with the server or be reachable over a trusted network. `https://` endpoints are rejected at startup until a `tls-*` feature of `opentelemetry-otlp` is enabled in `site/server/Cargo.toml`.

## License

© 2026 Andrew Miller. All rights reserved — published for reading as a portfolio, not for reuse. See [LICENSE](LICENSE); third-party components remain under their own terms.
