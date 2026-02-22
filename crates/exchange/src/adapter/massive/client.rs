use super::{MassiveConfig, MassiveError, MassiveResult};
use reqwest::{Client, ClientBuilder};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Rate limiter for Massive API requests
#[derive(Debug)]
pub struct RateLimiter {
    /// Maximum requests per minute
    limit_per_minute: u32,
    /// Request timestamps in the current window
    requests: Vec<Instant>,
    /// Window start time
    window_start: Instant,
}

impl RateLimiter {
    pub fn new(limit_per_minute: u32) -> Self {
        Self {
            limit_per_minute,
            requests: Vec::new(),
            window_start: Instant::now(),
        }
    }

    /// Wait if necessary to respect rate limit
    pub async fn acquire(&mut self) -> MassiveResult<()> {
        let now = Instant::now();
        let window_duration = Duration::from_secs(60);

        // Reset window if it's been more than a minute
        if now.duration_since(self.window_start) >= window_duration {
            self.requests.clear();
            self.window_start = now;
        }

        // Remove requests older than 1 minute
        self.requests
            .retain(|&req_time| now.duration_since(req_time) < window_duration);

        // Check if we've hit the limit
        if self.requests.len() >= self.limit_per_minute as usize {
            // Calculate wait time until oldest request expires
            if let Some(&oldest) = self.requests.first() {
                let elapsed = now.duration_since(oldest);
                if elapsed < window_duration {
                    let wait_duration = window_duration - elapsed + Duration::from_millis(100);

                    log::warn!(
                        "Rate limit reached ({}/min), waiting {:?}",
                        self.limit_per_minute,
                        wait_duration
                    );

                    tokio::time::sleep(wait_duration).await;

                    // Reset after waiting
                    self.requests.clear();
                    self.window_start = Instant::now();
                }
            }
        }

        // Record this request
        self.requests.push(Instant::now());

        Ok(())
    }

    /// Get current request count in window
    pub fn current_count(&self) -> usize {
        let now = Instant::now();
        self.requests
            .iter()
            .filter(|&&req_time| now.duration_since(req_time) < Duration::from_secs(60))
            .count()
    }
}

/// HTTP client wrapper with rate limiting and retry logic
pub struct MassiveClient {
    client: Client,
    config: MassiveConfig,
    rate_limiter: Arc<Mutex<RateLimiter>>,
}

impl MassiveClient {
    /// Create a new Massive client
    pub fn new(config: MassiveConfig) -> MassiveResult<Self> {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()?;

        let rate_limiter = Arc::new(Mutex::new(RateLimiter::new(config.rate_limit_per_minute)));

        Ok(Self {
            client,
            config,
            rate_limiter,
        })
    }

    /// Make a GET request with rate limiting and retries
    pub async fn get(&self, url: &str) -> MassiveResult<reqwest::Response> {
        let mut attempts = 0;
        let max_attempts = self.config.max_retries + 1;

        loop {
            attempts += 1;

            // Wait for rate limiter
            self.rate_limiter.lock().await.acquire().await?;

            // Log request
            log::debug!("GET {} (attempt {}/{})", url, attempts, max_attempts);

            // Make request
            let result = self
                .client
                .get(url)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .send()
                .await;

            match result {
                Ok(response) => {
                    let status = response.status();

                    // Check for rate limit response
                    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        if attempts < max_attempts {
                            let retry_after = response
                                .headers()
                                .get("Retry-After")
                                .and_then(|v| v.to_str().ok())
                                .and_then(|v| v.parse::<u64>().ok())
                                .unwrap_or(self.config.retry_delay_ms);

                            log::warn!("Rate limited, waiting {}ms", retry_after);
                            tokio::time::sleep(Duration::from_millis(retry_after)).await;
                            continue;
                        } else {
                            return Err(MassiveError::RateLimit(
                                "Max retries exceeded due to rate limiting".to_string(),
                            ));
                        }
                    }

                    // Check for authentication errors
                    if status == reqwest::StatusCode::UNAUTHORIZED
                        || status == reqwest::StatusCode::FORBIDDEN
                    {
                        return Err(MassiveError::Auth(format!(
                            "Authentication failed with status {}",
                            status
                        )));
                    }

                    // Check for server errors (5xx)
                    if status.is_server_error() {
                        if attempts < max_attempts {
                            log::warn!(
                                "Server error {}, retrying in {}ms",
                                status,
                                self.config.retry_delay_ms
                            );
                            tokio::time::sleep(Duration::from_millis(self.config.retry_delay_ms))
                                .await;
                            continue;
                        } else {
                            return Err(MassiveError::Api(format!("Server error: {}", status)));
                        }
                    }

                    // Check for client errors (4xx)
                    if status.is_client_error() && status != reqwest::StatusCode::NOT_FOUND {
                        let error_body = response.text().await.unwrap_or_default();
                        return Err(MassiveError::Api(format!(
                            "Client error {}: {}",
                            status, error_body
                        )));
                    }

                    // Success or 404
                    return Ok(response);
                }
                Err(e) => {
                    // Check if error is retriable
                    let is_retriable = e.is_timeout() || e.is_connect() || e.is_request();

                    if is_retriable && attempts < max_attempts {
                        log::warn!(
                            "Request failed: {}, retrying in {}ms",
                            e,
                            self.config.retry_delay_ms
                        );
                        tokio::time::sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
                        continue;
                    } else {
                        return Err(MassiveError::Http(e));
                    }
                }
            }
        }
    }

    /// Get current rate limit status
    pub async fn rate_limit_status(&self) -> (usize, u32) {
        let limiter = self.rate_limiter.lock().await;
        (limiter.current_count(), self.config.rate_limit_per_minute)
    }
}

/// Create a new Massive HTTP client
pub fn create_client(config: MassiveConfig) -> MassiveResult<MassiveClient> {
    config.validate()?;
    MassiveClient::new(config)
}

/// Validate API key format — delegates to the shared implementation in `util`.
pub fn validate_api_key(api_key: &str) -> bool {
    crate::util::validate_api_key(api_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_api_key() {
        assert!(!validate_api_key(""));
        assert!(!validate_api_key("short"));
        assert!(validate_api_key("valid_key_123456"));
    }

    #[test]
    fn test_rate_limiter_creation() {
        let limiter = RateLimiter::new(5);
        assert_eq!(limiter.current_count(), 0);
    }

    #[tokio::test]
    async fn test_rate_limiter_acquire() {
        let mut limiter = RateLimiter::new(5);

        // Should allow up to 5 requests
        for _ in 0..5 {
            assert!(limiter.acquire().await.is_ok());
        }

        assert_eq!(limiter.current_count(), 5);
    }

    #[test]
    fn test_client_creation() {
        let config = MassiveConfig::new("test_api_key_123".to_string());
        let client = create_client(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_creation_invalid_key() {
        let config = MassiveConfig::new("short".to_string());
        let client = create_client(config);
        assert!(client.is_err());
    }
}
