# Stage 1: build
FROM rust:1.89-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build --release -p animedb-api && \
    cp target/release/animedb-api /usr/local/bin/animedb-api

# Stage 2: runtime
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /usr/local/bin/animedb-api /usr/local/bin/animedb-api

ENV ANIMEDB_DATABASE_PATH=/data/animedb.sqlite
ENV ANIMEDB_LISTEN_ADDR=0.0.0.0:8080

VOLUME ["/data"]
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD wget -q --spider http://localhost:8080/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/animedb-api"]
CMD []