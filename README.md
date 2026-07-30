# portfolio

Personal site. Leptos 0.8 (Rust) in server-side-rendering mode behind Axum, hydrated in the browser via WebAssembly. The Cargo workspace lives in `site/`: `app` (pages and components), `frontend` (WASM entry point), `server` (Axum binary). Scaffolded from [leptos-rs/start-axum-workspace](https://github.com/leptos-rs/start-axum-workspace) (MIT).

## Build and run

Stable Rust with the `wasm32-unknown-unknown` target, plus [cargo-leptos](https://github.com/leptos-rs/cargo-leptos):

```sh
cd site
cargo leptos watch            # dev server on http://127.0.0.1:4000
cargo leptos build --release  # server binary + site bundle under site/target/
```

## Container

Multi-stage [Containerfile](Containerfile), buildable with podman or docker from the repository root. The builder pins the same Rust image and cargo-leptos version as CI; the runtime image is distroless, contains only the server binary and the compiled site assets, and runs as a non-root user.

```sh
podman build -t portfolio .
podman run --rm -p 8080:8080 portfolio   # http://127.0.0.1:8080
```

The server binds `LEPTOS_SITE_ADDR` (image default `0.0.0.0:8080`). To serve TLS, set `SITE_TLS=on` and mount a PEM certificate/key pair, pointing the TLS variables at it; `HEALTH_ADDR` optionally binds an auxiliary plain-http listener serving only `/readyz`, so orchestrator probes need not trust the certificate:

```sh
podman run --rm -p 8443:8443 -p 8081:8081 \
  -v ./certs:/certs:ro \
  -e LEPTOS_SITE_ADDR=0.0.0.0:8443 \
  -e SITE_TLS=on \
  -e TLS_CERT_PATH=/certs/site.pem \
  -e TLS_KEY_PATH=/certs/site.key \
  -e HEALTH_ADDR=0.0.0.0:8081 \
  portfolio
```
## TLS

`SITE_TLS=on` serves TLS, terminating it itself (rustls) from the `TLS_CERT_PATH`/`TLS_KEY_PATH` PEM certificate/key pair; both variables are required, and missing either is a startup error. `SITE_TLS=off`, or the variable being unset, serves plain http; supplying either certificate path anyway is a startup error. Serving TLS therefore requires `SITE_TLS=on` in addition to the certificate paths; setting only the certificate paths no longer serves TLS on its own. The image declares `SITE_TLS=off` by default. When `HEALTH_ADDR` is set, the same process additionally binds a plain-http listener at that address serving only `/readyz`, so readiness can be probed without trusting the site certificate. The port for that listener is declared as `health-port` in `[workspace.metadata.orchestrator]` in `site/Cargo.toml`.

The Aspire AppHost (`Portfolio.Orchestrator/`) applies this contract in run mode: it serves the site at `https://localhost:4000` using the ASP.NET Core developer certificate and probes readiness on the plain-http health port. Trust the certificate once per machine with `aspire certs trust` (launching the AppHost with `dotnet run` does not auto-trust it the way `aspire run` does). `SITE_TLS` is part of the server's own contract, not only the AppHost's; set it to `off` in the AppHost's environment to restore plain-http serving. Standalone `cargo leptos watch` is unaffected and serves http.

## Telemetry

When `OTEL_EXPORTER_OTLP_ENDPOINT` is set, the server exports traces and logs over OTLP/gRPC (the standard `OTEL_SERVICE_NAME` / `OTEL_RESOURCE_ATTRIBUTES` variables are honored); when it is absent, telemetry degrades to stdout logging only.

The exporter is built without TLS support: the endpoint must be a plain `http://` URL, so the collector must run co-located with the server or be reachable over a trusted network. `https://` endpoints are rejected at startup until a `tls-*` feature of `opentelemetry-otlp` is enabled in `site/server/Cargo.toml`.

## License

© 2026 Andrew Miller. All rights reserved — published for reading as a portfolio, not for reuse. See [LICENSE](LICENSE); third-party components remain under their own terms.
