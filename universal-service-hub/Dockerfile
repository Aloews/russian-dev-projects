FROM rust:1.88-bookworm AS builder
WORKDIR /build
COPY Cargo.toml ./
COPY src ./src
COPY rust-migrations ./rust-migrations
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 ush \
    && useradd --system --uid 10001 --gid ush --home-dir /app ush
WORKDIR /app
COPY --from=builder /build/target/release/universal-service-hub /app/universal-service-hub
USER 10001:10001
ENV PORT=8000
EXPOSE 8000
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8000/healthz || exit 1
ENTRYPOINT ["/app/universal-service-hub"]
