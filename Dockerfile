FROM rust:1.75-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY src ./src

ARG TARGETARCH
RUN case "${TARGETARCH}" in \
        amd64) cargo build --release --target x86_64-unknown-linux-musl ;; \
        arm64) cargo build --release --target aarch64-unknown-linux-musl ;; \
        *) cargo build --release ;; \
    esac

FROM alpine:3.19

RUN apk add --no-cache ca-certificates

WORKDIR /app

ARG TARGETARCH
COPY --from=builder /build/target/${TARGETARCH}-unknown-linux-musl/release/codex-usage /usr/local/bin/codex-usage

RUN adduser -D appuser && \
    chown -R appuser:appuser /app

USER appuser

ENTRYPOINT ["codex-usage"]
