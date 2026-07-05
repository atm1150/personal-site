# portfolio

Personal site. Leptos 0.8 (Rust) in server-side-rendering mode behind Axum, hydrated in the browser via WebAssembly. Cargo workspace: `app` (pages and components), `frontend` (WASM entry point), `server` (Axum binary). Scaffolded from [leptos-rs/start-axum-workspace](https://github.com/leptos-rs/start-axum-workspace) (MIT).

## Build and run

Stable Rust with the `wasm32-unknown-unknown` target, plus [cargo-leptos](https://github.com/leptos-rs/cargo-leptos):

```sh
cargo leptos watch            # dev server on http://127.0.0.1:4000
cargo leptos build --release  # server binary + site bundle under target/
```

## License

© 2026 Andrew Miller. All rights reserved — published for reading as a portfolio, not for reuse. See [LICENSE](LICENSE); third-party components remain under their own terms.
