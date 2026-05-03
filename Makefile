SHELL := /usr/bin/env bash

IMAGE_TAG ?= ghcr.io/enrell/animedb-api:latest
API_PORT ?= 8080
ANIMEDB_DATABASE_PATH ?= $(CURDIR)/data/animedb.sqlite
ANIMEDB_LISTEN_ADDR ?= 0.0.0.0:$(API_PORT)
ANIMEDB_REAL_MAX_PAGES ?= 1
ANIMEDB_REAL_PAGE_SIZE ?= 5

.PHONY: build test test-real docker-build docker-run docker-push docker-login debug-api crate-real clean

build:
	cargo build

test:
	cargo test

crate-real:
	ANIMEDB_REAL_MAX_PAGES=$(ANIMEDB_REAL_MAX_PAGES) \
	ANIMEDB_REAL_PAGE_SIZE=$(ANIMEDB_REAL_PAGE_SIZE) \
	cargo run -p animedb --example real_pipeline

test-real:
	IMAGE_TAG=$(IMAGE_TAG) \
	API_PORT=$(API_PORT) \
	ANIMEDB_REAL_MAX_PAGES=$(ANIMEDB_REAL_MAX_PAGES) \
	ANIMEDB_REAL_PAGE_SIZE=$(ANIMEDB_REAL_PAGE_SIZE) \
	./scripts/test-real-pipeline.sh

docker-build:
	docker buildx build --platform linux/amd64,linux/arm64 -t $(IMAGE_TAG) .

docker-run:
	mkdir -p "$(dir $(ANIMEDB_DATABASE_PATH))"
	docker run --rm \
		-p $(API_PORT):8080 \
		-e ANIMEDB_DATABASE_PATH=/data/animedb.sqlite \
		-v "$(dir $(ANIMEDB_DATABASE_PATH)):/data" \
		$(IMAGE_TAG)

docker-build-native:
	docker build -t $(IMAGE_TAG) .

docker-login:
	echo "Login to GHCR:" && docker login ghcr.io

docker-push: docker-build
	docker push $(IMAGE_TAG)
	docker push $(IMAGE_TAG)-linux-amd64 || true
	docker push $(IMAGE_TAG)-linux-arm64 || true

debug-api:
	mkdir -p "$(dir $(ANIMEDB_DATABASE_PATH))"
	ANIMEDB_DATABASE_PATH="$(ANIMEDB_DATABASE_PATH)" \
	ANIMEDB_LISTEN_ADDR="$(ANIMEDB_LISTEN_ADDR)" \
	cargo run -p animedb-api

clean:
	cargo clean
