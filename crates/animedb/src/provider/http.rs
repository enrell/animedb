/// Shared HTTP infrastructure for all providers.
///
/// This module is intentionally free of any provider-specific domain
/// knowledge.  It contains only generic HTTP building blocks that every
/// provider can reuse.
use reqwest::blocking::Client;
use std::time::Duration;

use crate::error::Result;

// ---------------------------------------------------------------------------
// HttpClient — thin wrapper around reqwest with a base URL
// ---------------------------------------------------------------------------

/// A thin, reusable HTTP client bound to a base URL.
///
/// Providers hold one of these and call `.get(path)` / `.post(path)` to
/// build requests relative to their configured endpoint.
#[derive(Clone, Debug)]
pub struct HttpClient {
    pub inner: Client,
    pub base_url: String,
}

impl HttpClient {
    /// Constructs a client with the given timeout and base URL.
    pub fn new(timeout: Duration, base_url: impl Into<String>) -> Self {
        let inner = Client::builder()
            .timeout(timeout)
            .user_agent("animedb/0.1")
            .build()
            .expect("reqwest blocking client must build");

        Self {
            inner,
            base_url: base_url.into(),
        }
    }

    /// Standard 30-second timeout, empty base URL.
    pub fn standard() -> Self {
        Self::new(Duration::from_secs(30), "")
    }

    /// Returns a builder for a GET request to `{base_url}{path}`.
    pub fn get(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.inner.get(format!("{}{}", self.base_url, path))
    }

    /// Returns a builder for a POST request to `{base_url}{path}`.
    pub fn post(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.inner.post(format!("{}{}", self.base_url, path))
    }

    /// Override the base URL, returning a new `HttpClient`.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
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
