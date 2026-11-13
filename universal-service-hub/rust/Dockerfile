FROM rust:1.85-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.toml
COPY src src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/universal-service-hub /usr/local/bin/universal-service-hub
ENV PORT=8000
EXPOSE 8000
USER 10001:10001
ENTRYPOINT ["universal-service-hub"]
