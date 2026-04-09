FROM rust:1.89-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build --release -p animedb-api

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/animedb-api /usr/local/bin/animedb-api

ENV ANIMEDB_DATABASE_PATH=/data/animedb.sqlite
ENV ANIMEDB_LISTEN_ADDR=0.0.0.0:8080

VOLUME ["/data"]
EXPOSE 8080

CMD ["animedb-api"]
