//! Configuration types for model providers.
//!
//! This module contains configuration structures for HTTP clients
//! and retry behavior.

/// Shared HTTP client configuration.
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    /// Request timeout in seconds.
    pub timeout_secs: Option<u64>,
    /// Maximum number of retries.
    pub max_retries: u32,
    /// User agent string.
    pub user_agent: Option<String>,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout_secs: Some(120),
            max_retries: 3,
            user_agent: None,
        }
    }
}

impl HttpClientConfig {
    /// Build a reqwest client with this configuration.
    ///
    /// # Panics
    ///
    /// Panics if the client cannot be built.
    #[must_use]
    pub fn build_client(&self) -> reqwest::Client {
        #[allow(unused_mut)]
        let mut builder = reqwest::Client::builder();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(timeout) = self.timeout_secs {
            builder = builder.timeout(std::time::Duration::from_secs(timeout));
        }

        if let Some(ref user_agent) = self.user_agent {
            builder = builder.user_agent(user_agent);
        }

        builder.build().expect("Failed to build HTTP client")
    }
}

/// Configuration for retrying failed requests.
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_attempts: u32,
    /// Initial delay between retries in milliseconds.
    pub initial_delay_ms: u64,
    /// Exponential backoff multiplier.
    pub backoff_multiplier: f64,
    /// Whether to add jitter to retry delays.
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 1000,
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// Calculate delay for a given attempt number (0-indexed).
    #[must_use]
    #[allow(
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn delay_for_attempt(&self, attempt: u32) -> std::time::Duration {
        let base_delay =
            self.initial_delay_ms as f64 * self.backoff_multiplier.powi(attempt as i32);
        let delay_ms = if self.jitter {
            // Add up to 25% jitter
            let jitter = base_delay * 0.25 * rand_factor();
            base_delay + jitter
        } else {
            base_delay
        };
        std::time::Duration::from_millis(delay_ms as u64)
    }
}

/// Generate a pseudo-random factor between 0.0 and 1.0.
fn rand_factor() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    f64::from(nanos % 1000) / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_client_config_default() {
        let config = HttpClientConfig::default();
        assert_eq!(config.timeout_secs, Some(120));
        assert_eq!(config.max_retries, 3);
        assert!(config.user_agent.is_none());
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.initial_delay_ms, 1000);
        assert_eq!(config.backoff_multiplier, 2.0);
        assert!(config.jitter);
    }

    #[test]
    fn test_retry_config_delay_without_jitter() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_delay_ms: 1000,
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let delay0 = config.delay_for_attempt(0);
        let delay1 = config.delay_for_attempt(1);
        let delay2 = config.delay_for_attempt(2);

        assert_eq!(delay0.as_millis(), 1000);
        assert_eq!(delay1.as_millis(), 2000);
        assert_eq!(delay2.as_millis(), 4000);
    }
}
