use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_secs(5), // Default values - overridden by TOML config in production
            max_delay: Duration::from_secs(300), // Default values - overridden by TOML config in production
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    pub fn new(
        max_attempts: u32,
        base_delay: Duration,
        max_delay: Duration,
        backoff_multiplier: f64,
    ) -> Self {
        Self {
            max_attempts,
            base_delay,
            max_delay,
            backoff_multiplier,
        }
    }
}

pub async fn execute_with_retry<F, Fut, T, E>(
    operation: F,
    retry_config: &RetryConfig,
    operation_name: &str,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>> + Send,
    E: std::fmt::Display + Send + Sync + 'static,
{
    let mut attempt = 1;
    let mut last_error = None;

    while attempt <= retry_config.max_attempts {
        println!(
            "🔄 {} attempt {}/{}",
            operation_name, attempt, retry_config.max_attempts
        );

        match operation().await {
            Ok(result) => {
                println!("✅ {} succeeded on attempt {}", operation_name, attempt);
                return Ok(result);
            }
            Err(e) => {
                last_error = Some(e);
                println!(
                    "❌ {} failed on attempt {}: {}",
                    operation_name,
                    attempt,
                    last_error.as_ref().unwrap()
                );

                if attempt < retry_config.max_attempts {
                    let delay = calculate_delay(attempt, retry_config);
                    println!("⏳ Waiting {:?} before retry...", delay);
                    sleep(delay).await;
                }
            }
        }

        attempt += 1;
    }

    Err(anyhow::anyhow!(
        "{} failed after {} attempts. Last error: {}",
        operation_name,
        retry_config.max_attempts,
        last_error.unwrap()
    ))
}

fn calculate_delay(attempt: u32, config: &RetryConfig) -> Duration {
    let exponential_delay =
        config.base_delay.as_secs_f64() * config.backoff_multiplier.powi((attempt - 1) as i32);

    let delay_seconds = exponential_delay.min(config.max_delay.as_secs_f64());
    Duration::from_secs_f64(delay_seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_retry_success_on_first_attempt() {
        let config = RetryConfig::default();
        let call_count = AtomicU32::new(0);

        let result = execute_with_retry(
            || {
                let count = call_count.fetch_add(1, Ordering::SeqCst);
                async move {
                    if count == 0 {
                        Ok("success")
                    } else {
                        Err("unexpected call")
                    }
                }
            },
            &config,
            "test_operation",
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_success_on_second_attempt() {
        let config = RetryConfig::new(3, Duration::from_millis(10), Duration::from_secs(1), 2.0);
        let call_count = AtomicU32::new(0);

        let result = execute_with_retry(
            || {
                let count = call_count.fetch_add(1, Ordering::SeqCst);
                async move {
                    if count == 0 {
                        Err("first attempt fails")
                    } else {
                        Ok("success")
                    }
                }
            },
            &config,
            "test_operation",
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_failure_after_max_attempts() {
        let config = RetryConfig::new(2, Duration::from_millis(10), Duration::from_secs(1), 2.0);
        let call_count = AtomicU32::new(0);

        let result = execute_with_retry(
            || {
                call_count.fetch_add(1, Ordering::SeqCst);
                async move { Err::<&str, anyhow::Error>(anyhow::anyhow!("always fails")) }
            },
            &config,
            "test_operation",
        )
        .await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }
}
