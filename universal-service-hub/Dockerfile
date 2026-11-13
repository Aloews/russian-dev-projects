FROM rust:1.88-bookworm AS builder
WORKDIR /build
ENV CARGO_NET_RETRY=2 CARGO_HTTP_TIMEOUT=60 CARGO_PROFILE_RELEASE_OPT_LEVEL=1
COPY Cargo.toml ./
COPY src ./src
COPY rust-migrations ./rust-migrations
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /build/target/release/universal-service-hub /app/universal-service-hub
USER 65532:65532
ENV PORT=8000
EXPOSE 8000
ENTRYPOINT ["/app/universal-service-hub"]
