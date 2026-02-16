FROM rust:1-alpine

RUN apk add --no-cache musl-dev

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --bin codex-usage

FROM alpine:3.19

RUN apk add --no-cache ca-certificates

COPY --from=0 /app/target/release/codex-usage /usr/local/bin/codex-usage

RUN adduser -D appuser && \
    chown -R appuser:appuser /usr/local/bin/codex-usage

USER appuser

ENTRYPOINT ["codex-usage"]
