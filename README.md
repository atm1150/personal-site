# portfolio

Personal site. Leptos 0.8 (Rust) in server-side-rendering mode behind Axum, hydrated in the browser via WebAssembly. The Cargo workspace lives in `site/`: `app` (pages and components), `frontend` (WASM entry point), `server` (Axum binary). Scaffolded from [leptos-rs/start-axum-workspace](https://github.com/leptos-rs/start-axum-workspace) (MIT).

## Build and run

Stable Rust with the `wasm32-unknown-unknown` target, plus [cargo-leptos](https://github.com/leptos-rs/cargo-leptos):

```sh
cd site
cargo leptos watch            # dev server on http://127.0.0.1:4000
cargo leptos build --release  # server binary + site bundle under site/target/
```

## TLS

The server binds plain http unless `TLS_CERT_PATH` and `TLS_KEY_PATH` are both set, in which case it terminates TLS itself (rustls) from that PEM certificate/key pair; setting exactly one of them is a startup error. When `HEALTH_ADDR` is set, the same process additionally binds a plain-http listener at that address serving only `/readyz`, so readiness can be probed without trusting the site certificate. The port for that listener is declared as `health-port` in `[workspace.metadata.orchestrator]` in `site/Cargo.toml`.

The Aspire AppHost (`Portfolio.Orchestrator/`) applies this contract in run mode: it serves the site at `https://localhost:4000` using the ASP.NET Core developer certificate and probes readiness on the plain-http health port. Trust the certificate once per machine with `aspire certs trust` (launching the AppHost with `dotnet run` does not auto-trust it the way `aspire run` does). Set `SITE_TLS=off` in the AppHost's environment to restore plain-http serving. Standalone `cargo leptos watch` is unaffected and serves http.

## Telemetry

When `OTEL_EXPORTER_OTLP_ENDPOINT` is set, the server exports traces and logs over OTLP/gRPC (the standard `OTEL_SERVICE_NAME` / `OTEL_RESOURCE_ATTRIBUTES` variables are honored); when it is absent, telemetry degrades to stdout logging only.

The exporter is built without TLS support: the endpoint must be a plain `http://` URL, so the collector must run co-located with the server or be reachable over a trusted network. `https://` endpoints are rejected at startup until a `tls-*` feature of `opentelemetry-otlp` is enabled in `site/server/Cargo.toml`.

## License

© 2026 Andrew Miller. All rights reserved — published for reading as a portfolio, not for reuse. See [LICENSE](LICENSE); third-party components remain under their own terms.
