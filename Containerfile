# Production image for the site server. Build with podman or docker from the
# repository root:
#
#   podman build -t portfolio .
#
# The builder stage mirrors CI (.forgejo/workflows/ci.yml): bump the Rust image
# pin and the cargo-leptos version here and there together.
FROM docker.io/library/rust:1.96-bookworm AS builder
WORKDIR /app

RUN rustup target add wasm32-unknown-unknown
RUN curl -L --proto '=https' --tlsv1.2 -sSf \
        https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash \
    && cargo binstall -y --locked cargo-leptos@0.3.6

COPY site ./site
WORKDIR /app/site
RUN cargo leptos build --release

# Runtime: distroless keeps only glibc/libgcc and CA certificates - no shell,
# no package manager. The server links rustls for TLS, so no OpenSSL is
# needed. The :nonroot tag runs the process as an unprivileged user.
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app

# Each value below is used twice and nothing else enforces agreement between
# the uses, so each is defined once: SITE_PORT feeds the default bind address
# and EXPOSE; SITE_ROOT is the asset COPY destination and what the server is
# told about it.
ARG SITE_PORT=8080
ARG SITE_ROOT=/app/site

# The COPY sources are cargo-leptos's standard output layout, kept hardcoded
# until something needs them parameterized (the Aspire publish path would be
# that consumer, when it arrives).
COPY --from=builder /app/site/target/release/server /app/server
COPY --from=builder /app/site/target/site ${SITE_ROOT}

# The runtime env contract; override LEPTOS_SITE_ADDR to bind elsewhere, and
# set TLS_CERT_PATH/TLS_KEY_PATH (mounted PEM pair) plus HEALTH_ADDR to serve
# TLS with the auxiliary plain-http /readyz listener.
ENV LEPTOS_SITE_ROOT=${SITE_ROOT} \
    LEPTOS_SITE_ADDR=0.0.0.0:${SITE_PORT}
EXPOSE ${SITE_PORT}

ENTRYPOINT ["/app/server"]
