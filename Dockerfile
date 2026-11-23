# cloth-web backend image (plan §1.4). CI builds this on every push to main
# and pushes it to GHCR tagged with the commit SHA; inibaby just `docker
# pull`s — the NUC never compiles Rust.
#
# No database is needed at build time: all SQL is runtime `sqlx::query`,
# never build-time query macros.

# --- build ---
FROM rust:1.100-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

# --- run ---
FROM debian:trixie-slim
# ca-certificates for the rustls connections; wget is the compose
# healthcheck's only tool (`wget -qO- http://127.0.0.1:3000/healthz`).
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/backend /usr/local/bin/backend
# The binary only needs env + network (images go to Supabase Storage,
# nothing is written to disk), so unprivileged is fine.
USER nobody
EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/backend"]
