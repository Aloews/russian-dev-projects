FROM rust:1.86-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/universal-service-hub /usr/local/bin/universal-service-hub
ENV HOST=0.0.0.0 PORT=8080
EXPOSE 8080
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/universal-service-hub"]
