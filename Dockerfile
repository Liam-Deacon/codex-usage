ARG TARGETARCH=x86_64
ARG TARGETDIR=${TARGETARCH}-unknown-linux-musl

FROM rust:1.75-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --target ${TARGETDIR}

FROM alpine:3.19

RUN apk add --no-cache ca-certificates

WORKDIR /app

COPY --from=builder /build/target/${TARGETDIR}/release/codex-usage /usr/local/bin/codex-usage

RUN adduser -D appuser && \
    chown -R appuser:appuser /app

USER appuser

ENTRYPOINT ["codex-usage"]
