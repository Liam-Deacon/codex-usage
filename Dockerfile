# Build stage
FROM rust:1.75-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --target x86_64-unknown-linux-musl

# Runtime stage
FROM alpine:3.19

RUN apk add --no-cache ca-certificates

WORKDIR /app

COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/codex-usage /usr/local/bin/codex-usage

RUN adduser -D appuser && \
    chown -R appuser:appuser /app

USER appuser

ENTRYPOINT ["codex-usage"]
