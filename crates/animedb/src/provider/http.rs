/// Shared HTTP infrastructure for all providers.
///
/// This module is intentionally free of any provider-specific domain
/// knowledge.  It contains only generic HTTP building blocks that every
/// provider can reuse.
use reqwest::blocking::Client;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// HttpClient — thin wrapper around reqwest with a base URL
// ---------------------------------------------------------------------------

/// A thin, reusable HTTP client bound to a base URL.
///
/// Providers hold one of these and call `.get(path)` / `.post(path)` to
/// build requests relative to their configured endpoint.
///
/// Construction is lazy: the inner `reqwest::blocking::Client` is built on
/// first HTTP call.  This avoids panicking when a provider is constructed
/// inside a `#[tokio::test]` runtime (blocking client must not outlive the
/// runtime it was created in).
#[derive(Clone, Debug)]
pub struct HttpClient {
    inner: Arc<Mutex<Option<Client>>>,
    pub base_url: String,
    timeout: Duration,
    pub proxy: Option<String>,
}

impl HttpClient {
    /// Constructs a client with the given timeout and base URL.
    ///
    /// The underlying `reqwest::blocking::Client` is built lazily on first
    /// HTTP call to avoid panicking when a provider is constructed inside a
    /// `#[tokio::test]` runtime.
    pub fn new(timeout: Duration, base_url: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            base_url: base_url.into(),
            timeout,
            proxy: None,
        }
    }

    /// Standard 30-second timeout, empty base URL.
    pub fn standard() -> Self {
        Self::new(Duration::from_secs(30), "")
    }

    /// Returns the shared `Client`, initializing it lazily on first call.
    pub fn client(&self) -> Result<Client> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| Error::Sync("poisoned".into()))?;
        if guard.is_none() {
            let mut builder = Client::builder()
                .timeout(self.timeout)
                .user_agent("animedb/0.1");

            if let Some(proxy_url) = &self.proxy {
                builder = builder.proxy(reqwest::Proxy::all(proxy_url).map_err(Error::Http)?);
            }

            let client = builder.build().map_err(Error::Http)?;
            *guard = Some(client);
        }
        Ok(guard.as_ref().unwrap().clone())
    }

    /// Returns a builder for a GET request to `{base_url}{path}`.
    pub fn get(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.client()
            .expect("HttpClient must have a valid client")
            .get(format!("{}{}", self.base_url, path))
    }

    /// Returns a builder for a POST request to `{base_url}{path}`.
    pub fn post(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.client()
            .expect("HttpClient must have a valid client")
            .post(format!("{}{}", self.base_url, path))
    }

    /// Override the base URL, returning a new `HttpClient`.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Sets the HTTP proxy, returning a new `HttpClient`.
    pub fn with_proxy(mut self, proxy_url: impl Into<String>) -> Self {
        self.proxy = Some(proxy_url.into());
        self
    }
}

// ---------------------------------------------------------------------------
// Retry — generic exponential back-off for rate-limited requests
// ---------------------------------------------------------------------------

/// Executes `request_fn`, retrying on HTTP 429 with exponential back-off.
///
/// `max_retries` is the maximum number of additional attempts after the first
/// failure.  The `Retry-After` header is respected when present.
pub fn with_retry<F>(
    max_retries: u32,
    initial_delay: Duration,
    mut request_fn: F,
) -> Result<reqwest::blocking::Response>
where
    F: FnMut() -> Result<reqwest::blocking::Response>,
{
    let mut delay = initial_delay;

    for attempt in 0..=max_retries {
        let response = request_fn()?;

        if response.status() != reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Ok(response.error_for_status()?);
        }

        // On the last attempt stop retrying and surface the error.
        if attempt == max_retries {
            return Ok(response.error_for_status()?);
        }

        // Honor the server's Retry-After header when present.
        if let Some(retry_after) = response.headers().get(reqwest::header::RETRY_AFTER)
            && let Ok(secs) = retry_after.to_str().unwrap_or("").parse::<u64>()
        {
            delay = Duration::from_secs(secs + 1);
        }

        std::thread::sleep(delay);
        delay *= 2;
    }

    unreachable!("loop always returns inside")
}

// ---------------------------------------------------------------------------
// Pagination helpers
// ---------------------------------------------------------------------------

/// Clamps `page_size` to the range `[1, max]`.
#[inline]
pub fn clamp_page_size(page_size: usize, max: usize) -> usize {
    page_size.clamp(1, max)
}

/// Converts a 1-based cursor page into a 0-based offset for providers that
/// use offset-based pagination (e.g. Kitsu's `page[offset]`).
#[inline]
pub fn page_to_offset(cursor_page: usize, page_size: usize) -> usize {
    cursor_page.saturating_sub(1) * page_size
}

// ---------------------------------------------------------------------------
// Regression tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod regression_tests {
    use super::HttpClient;

    /// Regression test: constructing `HttpClient` inside a `#[tokio::test]`
    /// runtime must not panic.
    ///
    /// Prior to the lazy initialization fix, `HttpClient::new()` eagerly
    /// constructed a `reqwest::blocking::Client`.  Dropping that client inside
    /// a `tokio::runtime::CurrentThread` (used by `#[tokio::test]`) caused a
    /// panic: "cannot drop runtime in a context where blocking is not allowed".
    #[tokio::test]
    async fn http_client_construction_inside_tokio_test_does_not_panic() {
        // This must not panic — the client is created lazily on first HTTP call,
        // so construction inside a tokio runtime is safe.
        let client = HttpClient::new(std::time::Duration::from_secs(30), "https://example.com");
        assert_eq!(client.base_url, "https://example.com");
        // No panic — even though we never make an HTTP call in this test,
        // the important thing is that construction itself is safe.
    }

    /// Same regression test but using `#[test]` (standard threadpool) to
    /// confirm both environments work.
    #[test]
    fn http_client_construction_in_std_test_does_not_panic() {
        let client = HttpClient::new(std::time::Duration::from_secs(30), "https://example.com");
        assert_eq!(client.base_url, "https://example.com");
    }
}
