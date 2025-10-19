use stablecoin_backend::retry::{execute_with_retry, RetryConfig};
use stablecoin_backend::transaction_monitor::{TransactionStatus, TransactionReceipt};
use stablecoin_backend::config::ChainConfig;
use alloy::primitives::{B256, U256};
use anyhow::Result;
use std::str::FromStr;
use std::time::Duration;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn test_retry_logic_success() -> Result<()> {
    // Test retry logic with a failing operation that eventually succeeds
    let config = RetryConfig::new(
        3, // max_attempts
        Duration::from_millis(10), // base_delay
        Duration::from_secs(1), // max_delay
        2.0, // backoff_multiplier
    );
    
    let attempt_count = Arc::new(Mutex::new(0));
    let result = execute_with_retry(
        || {
            let count = attempt_count.clone();
            async move {
                let mut attempts = count.lock().unwrap();
                *attempts += 1;
                if *attempts < 2 {
                    Err::<String, anyhow::Error>(anyhow::anyhow!("Simulated failure"))
                } else {
                    Ok("Success".to_string())
                }
            }
        },
        &config,
        "Test operation",
    ).await?;
    
    assert_eq!(result, "Success");
    assert_eq!(*attempt_count.lock().unwrap(), 2);
    
    println!("✅ Retry logic success test passed");
    Ok(())
}

#[tokio::test]
async fn test_retry_logic_failure() -> Result<()> {
    // Test retry logic with an operation that always fails
    let config = RetryConfig::new(
        2, // max_attempts
        Duration::from_millis(10), // base_delay
        Duration::from_secs(1), // max_delay
        2.0, // backoff_multiplier
    );
    
    let result = execute_with_retry(
        || {
            async move {
                Err::<String, anyhow::Error>(anyhow::anyhow!("Always fails"))
            }
        },
        &config,
        "Failing operation",
    ).await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Always fails"));
    
    println!("✅ Retry logic failure test passed");
    Ok(())
}

#[tokio::test]
async fn test_transaction_status_enum() -> Result<()> {
    // Test that TransactionStatus enum works correctly
    let success = TransactionStatus::Success;
    let failed = TransactionStatus::Failed;
    let timeout = TransactionStatus::Timeout;
    
    // Test that all variants can be created
    assert!(matches!(success, TransactionStatus::Success));
    assert!(matches!(failed, TransactionStatus::Failed));
    assert!(matches!(timeout, TransactionStatus::Timeout));
    
    // Test equality
    assert_eq!(success, TransactionStatus::Success);
    assert_ne!(success, TransactionStatus::Failed);
    
    println!("✅ TransactionStatus enum test passed");
    Ok(())
}

#[tokio::test]
async fn test_transaction_receipt_creation() -> Result<()> {
    // Test that TransactionReceipt can be created
    let hash = B256::from([1u8; 32]);
    let block_number = 12345u64;
    let gas_used = U256::from(21000u64);
    let status = TransactionStatus::Success;
    
    let receipt = TransactionReceipt {
        hash,
        block_number,
        gas_used,
        status: status.clone(),
    };
    
    // Test that receipt was created correctly
    assert_eq!(receipt.hash, hash);
    assert_eq!(receipt.block_number, block_number);
    assert_eq!(receipt.gas_used, gas_used);
    assert_eq!(receipt.status, status);
    
    println!("✅ TransactionReceipt creation test passed");
    Ok(())
}

#[tokio::test]
async fn test_config_loading() -> Result<()> {
    // Test that configuration can be loaded from TOML files
    let config_content = r#"
[chain]
chain_id = 1
rpc_url = "https://eth.llamarpc.com"
private_key = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"

[contracts]
usdsc_address = "0x1234567890123456789012345678901234567890"
recipient_address = "0x0987654321098765432109876543210987654321"

[thresholds]
min_yield_threshold = "1000000000000000000"

[retry]
max_attempts = 3
base_delay_seconds = 5
max_delay_seconds = 300
backoff_multiplier = 2.0

[monitoring]
transaction_timeout_seconds = 300
poll_interval_seconds = 5
timeout_block_number = 0
timeout_gas_used = "0"

[transaction]
value_wei = "0"
"#;
    
    // Write config to temporary file
    let temp_file = std::env::temp_dir().join("test_config.toml");
    std::fs::write(&temp_file, config_content)?;
    
    // Load config
    let config = ChainConfig::load(temp_file.to_str().unwrap())?;
    
    // Verify config was loaded correctly
    assert_eq!(config.chain.chain_id, 1);
    assert_eq!(config.chain.rpc_url, "https://eth.llamarpc.com");
    assert_eq!(config.contracts.usdsc_address, "0x1234567890123456789012345678901234567890");
    assert_eq!(config.thresholds.min_yield_threshold, "1000000000000000000");
    assert_eq!(config.retry.max_attempts, 3);
    
    // Clean up
    std::fs::remove_file(&temp_file)?;
    
    println!("✅ Config loading test passed");
    Ok(())
}

#[tokio::test]
async fn test_environment_variable_substitution() -> Result<()> {
    // Test environment variable substitution in config
    std::env::set_var("TEST_RPC_URL", "https://test.example.com");
    std::env::set_var("TEST_PRIVATE_KEY", "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890");
    
    let config_content = r#"
[chain]
chain_id = 1
rpc_url = "${TEST_RPC_URL}"
private_key = "${TEST_PRIVATE_KEY}"

[contracts]
usdsc_address = "0x1234567890123456789012345678901234567890"

[thresholds]
min_yield_threshold = "1000000000000000000"

[retry]
max_attempts = 3
base_delay_seconds = 5
max_delay_seconds = 300
backoff_multiplier = 2.0

[monitoring]
transaction_timeout_seconds = 300
poll_interval_seconds = 5
timeout_block_number = 0
timeout_gas_used = "0"

[transaction]
value_wei = "0"
"#;
    
    let temp_file = std::env::temp_dir().join("test_env_config.toml");
    std::fs::write(&temp_file, config_content)?;
    
    let config = ChainConfig::load(temp_file.to_str().unwrap())?;
    
    // Verify environment variables were substituted
    assert_eq!(config.chain.rpc_url, "https://test.example.com");
    assert_eq!(config.chain.private_key, "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890");
    
    // Clean up
    std::fs::remove_file(&temp_file)?;
    std::env::remove_var("TEST_RPC_URL");
    std::env::remove_var("TEST_PRIVATE_KEY");
    
    println!("✅ Environment variable substitution test passed");
    Ok(())
}

#[tokio::test]
async fn test_yield_threshold_logic() -> Result<()> {
    // Test yield threshold comparison logic
    let min_threshold = U256::from_str("1000000000000000000")?; // 1 USDC
    let pending_yield_low = U256::from_str("500000000000000000")?; // 0.5 USDC
    let pending_yield_high = U256::from_str("2000000000000000000")?; // 2 USDC
    
    // Test below threshold
    assert!(pending_yield_low < min_threshold);
    
    // Test above threshold
    assert!(pending_yield_high >= min_threshold);
    
    println!("✅ Yield threshold logic test passed");
    Ok(())
}

#[tokio::test]
async fn test_retry_configuration_validation() -> Result<()> {
    // Test retry configuration validation
    let config_content = r#"
[chain]
chain_id = 1
rpc_url = "https://eth.llamarpc.com"
private_key = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"

[contracts]
usdsc_address = "0x1234567890123456789012345678901234567890"

[thresholds]
min_yield_threshold = "1000000000000000000"

[retry]
max_attempts = 3
base_delay_seconds = 5
max_delay_seconds = 300
backoff_multiplier = 2.0

[monitoring]
transaction_timeout_seconds = 300
poll_interval_seconds = 5
timeout_block_number = 0
timeout_gas_used = "0"

[transaction]
value_wei = "0"
"#;
    
    let temp_file = std::env::temp_dir().join("test_retry_config.toml");
    std::fs::write(&temp_file, config_content)?;
    
    let config = ChainConfig::load(temp_file.to_str().unwrap())?;
    
    // Test that retry settings are reasonable
    assert!(config.retry.max_attempts >= 1);
    assert!(config.retry.max_attempts <= 10); // Reasonable upper bound
    assert!(config.retry.base_delay_seconds >= 1);
    assert!(config.retry.max_delay_seconds >= config.retry.base_delay_seconds);
    assert!(config.retry.backoff_multiplier >= 1.0);
    assert!(config.retry.backoff_multiplier <= 5.0); // Reasonable upper bound
    
    // Clean up
    std::fs::remove_file(&temp_file)?;
    
    println!("✅ Retry configuration validation test passed");
    Ok(())
}

#[tokio::test]
async fn test_chain_specific_configuration() -> Result<()> {
    // Test that different chain configurations work
    let ethereum_config = create_ethereum_test_config()?;
    let soneium_config = create_soneium_test_config()?;
    
    // Test Ethereum config
    assert_eq!(ethereum_config.chain.chain_id, 1);
    assert!(ethereum_config.chain.rpc_url.contains("eth"));
    
    // Test Soneium config
    assert_eq!(soneium_config.chain.chain_id, 1946);
    assert!(soneium_config.chain.rpc_url.contains("soneium"));
    assert!(soneium_config.contracts.reward_redistributor_address.is_some());
    
    println!("✅ Chain-specific configuration test passed");
    Ok(())
}

// Helper functions for creating test configurations
fn create_ethereum_test_config() -> Result<ChainConfig> {
    let config_content = r#"
[chain]
chain_id = 1
rpc_url = "https://eth.llamarpc.com"
private_key = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"

[contracts]
usdsc_address = "0x1234567890123456789012345678901234567890"
recipient_address = "0x0987654321098765432109876543210987654321"

[thresholds]
min_yield_threshold = "1000000000000000000"

[retry]
max_attempts = 3
base_delay_seconds = 5
max_delay_seconds = 300
backoff_multiplier = 2.0

[monitoring]
transaction_timeout_seconds = 300
poll_interval_seconds = 5
timeout_block_number = 0
timeout_gas_used = "0"

[transaction]
value_wei = "0"
"#;
    
    let temp_file = std::env::temp_dir().join("ethereum_test_config.toml");
    std::fs::write(&temp_file, config_content)?;
    
    let config = ChainConfig::load(temp_file.to_str().unwrap())?;
    std::fs::remove_file(&temp_file)?;
    
    Ok(config)
}

fn create_soneium_test_config() -> Result<ChainConfig> {
    let config_content = r#"
[chain]
chain_id = 1946
rpc_url = "https://rpc.soneium.org"
private_key = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"

[contracts]
usdsc_address = "0x1111111111111111111111111111111111111111"
reward_redistributor_address = "0x2222222222222222222222222222222222222222"
earn_vault_address = "0x3333333333333333333333333333333333333333"
susdsc_vault_address = "0x4444444444444444444444444444444444444444"

[thresholds]
min_yield_threshold = "1000000000000000000"

[retry]
max_attempts = 3
base_delay_seconds = 5
max_delay_seconds = 300
backoff_multiplier = 2.0

[monitoring]
transaction_timeout_seconds = 300
poll_interval_seconds = 5
timeout_block_number = 0
timeout_gas_used = "0"

[transaction]
value_wei = "0"
"#;
    
    let temp_file = std::env::temp_dir().join("soneium_test_config.toml");
    std::fs::write(&temp_file, config_content)?;
    
    let config = ChainConfig::load(temp_file.to_str().unwrap())?;
    std::fs::remove_file(&temp_file)?;
    
    Ok(config)
}
