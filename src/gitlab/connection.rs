//! Defines a connection to gitlab
use core::time::Duration;

use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};

/// Caps the maximum retry delay at 64x the base delay
const MAX_BACKOFF_MULTIPLIER: u32 = 64;

/// Infos needed to connect to gitlab
#[derive(Clone)]
pub struct Connection {
    /// Hostname
    pub hostname: String,
    /// [`reqwest`] client, wrapped with a retry middleware
    pub http_client: ClientWithMiddleware,
    /// Authentication token
    pub token: String,
}

impl Connection {
    /// Creates a new [`Connection`]
    ///
    /// The HTTP client retries transient failures (timeouts, connection errors
    /// and `408`/`429`/`5xx` responses) with capped exponential backoff, up to
    /// `max_retries` times starting from the `retry_backoff` base delay.
    /// Setting `max_retries` to `0` disables retrying.
    ///
    /// The set of retried failures is defined by [`reqwest_retry`]'s default
    /// retryable strategy; see
    /// <https://docs.rs/reqwest-retry/0.9.1/src/reqwest_retry/retryable_strategy.rs.html#106>.
    pub fn new(
        hostname: String,
        token: String,
        accept_invalid_certs: bool,
        max_retries: u32,
        retry_backoff: Duration,
    ) -> Result<Self, reqwest::Error> {
        let inner_client = reqwest::ClientBuilder::new()
            .tls_danger_accept_invalid_certs(accept_invalid_certs)
            .build()?;

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(
                retry_backoff,
                retry_backoff.saturating_mul(MAX_BACKOFF_MULTIPLIER),
            )
            .build_with_max_retries(max_retries);

        let http_client = reqwest_middleware::ClientBuilder::new(inner_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Ok(Self {
            hostname,
            http_client,
            token,
        })
    }
}
