SHELL := /usr/bin/env bash

IMAGE_TAG ?= animedb:dev
API_PORT ?= 8080
ANIMEDB_DATABASE_PATH ?= $(CURDIR)/data/animedb.sqlite
ANIMEDB_LISTEN_ADDR ?= 0.0.0.0:$(API_PORT)
ANIMEDB_REAL_MAX_PAGES ?= 1
ANIMEDB_REAL_PAGE_SIZE ?= 5

.PHONY: build test test-real docker-build docker-run debug-api crate-real clean

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
	docker build -t $(IMAGE_TAG) .

docker-run:
	mkdir -p "$(dir $(ANIMEDB_DATABASE_PATH))"
	docker run --rm \
		-p $(API_PORT):8080 \
		-e ANIMEDB_DATABASE_PATH=/data/animedb.sqlite \
		-v "$(dir $(ANIMEDB_DATABASE_PATH)):/data" \
		$(IMAGE_TAG)

debug-api:
	mkdir -p "$(dir $(ANIMEDB_DATABASE_PATH))"
	ANIMEDB_DATABASE_PATH="$(ANIMEDB_DATABASE_PATH)" \
	ANIMEDB_LISTEN_ADDR="$(ANIMEDB_LISTEN_ADDR)" \
	cargo run -p animedb-api

clean:
	cargo clean
