# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.4] - 2026-05-04

### Fixed

- **`provider/http.rs`**: `HttpClient` now defers `reqwest::blocking::Client` construction to first HTTP call instead of `new()`. This fixes a panic that occurred when a provider was constructed inside a `#[tokio::test]` runtime — the blocking client's thread pool would be dropped when the runtime dropped, causing "cannot drop a runtime in a context where blocking is not allowed".
- **`provider/anilist.rs`**: Updated `post_with_retry` to use the new `HttpClient::client()` method instead of the removed `.inner` field.
- **`error.rs`**: Added `Error::Sync` variant to handle `Arc<Mutex>` poisoning in the lazy HTTP client.

### Added

- **`provider/http.rs`**: Regression tests confirming `HttpClient` construction is safe inside both `#[tokio::test]` and `#[test]` environments.
- **`Cargo.toml`**: Added `tokio` as a dev-dependency for the regression tests.
