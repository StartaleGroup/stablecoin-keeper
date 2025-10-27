//! Unit Tests
//! 
//! Tests for individual functions, parsing logic, retry mechanisms, and data structures.
//! These tests verify isolated functionality without external dependencies.

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
    
    let attempt_count = Arc::new(Mutex::new(0));
    let result = execute_with_retry(
        || {
            let count = attempt_count.clone();
            async move {
                let mut attempts = count.lock().unwrap();
                *attempts += 1;
                Err::<String, anyhow::Error>(anyhow::anyhow!("Always fails"))
            }
        },
        &config,
        "Test operation",
    ).await;
    
    assert!(result.is_err());
    assert_eq!(*attempt_count.lock().unwrap(), 2);
    
    println!("✅ Retry logic failure test passed");
    Ok(())
}

#[tokio::test]
async fn test_transaction_status_enum() -> Result<()> {
    // Test TransactionStatus enum variants
    let success = TransactionStatus::Success;
    let failed = TransactionStatus::Failed;
    let timeout = TransactionStatus::Timeout;
    
    // Test that we can create and match on the enum
    match success {
        TransactionStatus::Success => println!("✅ Success status works"),
        _ => panic!("Expected Success status"),
    }
    
    match failed {
        TransactionStatus::Failed => println!("✅ Failed status works"),
        _ => panic!("Expected Failed status"),
    }
    
    match timeout {
        TransactionStatus::Timeout => println!("✅ Timeout status works"),
        _ => panic!("Expected Timeout status"),
    }
    
    println!("✅ Transaction status enum test passed");
    Ok(())
}

#[tokio::test]
async fn test_transaction_receipt_creation() -> Result<()> {
    // Test TransactionReceipt creation and field access
    let hash = B256::from([1u8; 32]);
    let receipt = TransactionReceipt {
        hash,
        block_number: 12345,
        gas_used: U256::from(21000),
        status: TransactionStatus::Success,
    };
    
    assert_eq!(receipt.hash, hash);
    assert_eq!(receipt.block_number, 12345);
    assert_eq!(receipt.gas_used, U256::from(21000));
    assert_eq!(receipt.status, TransactionStatus::Success);
    
    println!("✅ Transaction receipt creation test passed");
    Ok(())
}

#[tokio::test]
async fn test_config_loading() -> Result<()> {
    // Test configuration loading from TOML content
    let config_content = r#"
[chain]
chain_id = 1
rpc_url = "https://eth.llamarpc.com"

[contracts]
usdsc_address = "0x1234567890123456789012345678901234567890"
recipient_address = "0x0987654321098765432109876543210987654321"

[thresholds]
min_yield_threshold = "1000000"

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

[kms]
key_id = "test-kms-key-id"
region = "us-east-1"
"#;
    
    let temp_file = std::env::temp_dir().join(format!("test_config_{}_{}.toml", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::write(&temp_file, config_content)?;
    let config = ChainConfig::load(temp_file.to_str().unwrap())?;
    std::fs::remove_file(&temp_file)?;
    
    assert_eq!(config.chain.chain_id, 1);
    assert_eq!(config.chain.rpc_url, "https://eth.llamarpc.com");
    assert_eq!(config.contracts.usdsc_address, "0x1234567890123456789012345678901234567890");
    assert_eq!(config.thresholds.min_yield_threshold, "1000000");
    assert_eq!(config.retry.max_attempts, 3);
    assert!(config.kms.is_some());
    
    println!("✅ Config loading test passed");
    Ok(())
}

#[tokio::test]
async fn test_environment_variable_substitution() -> Result<()> {
    // Test environment variable substitution in config
    std::env::set_var("TEST_RPC_URL", "https://test.example.com");
    std::env::set_var("TEST_CHAIN_ID", "42");
    
    let config_content = r#"
[chain]
chain_id = 42
rpc_url = "${TEST_RPC_URL}"

[contracts]
usdsc_address = "0x1234567890123456789012345678901234567890"
recipient_address = "0x0987654321098765432109876543210987654321"

[thresholds]
min_yield_threshold = "1000000"

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

[kms]
key_id = "test-kms-key-id"
region = "us-east-1"
"#;
    
    let temp_file = std::env::temp_dir().join(format!("test_config_{}_{}.toml", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::write(&temp_file, config_content)?;
    let config = ChainConfig::load(temp_file.to_str().unwrap())?;
    std::fs::remove_file(&temp_file)?;
    
    assert_eq!(config.chain.chain_id, 42);
    assert_eq!(config.chain.rpc_url, "https://test.example.com");
    
    // Clean up environment variables
    std::env::remove_var("TEST_RPC_URL");
    std::env::remove_var("TEST_CHAIN_ID");
    
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
    let valid_config = RetryConfig::new(
        3,
        Duration::from_secs(1),
        Duration::from_secs(60),
        2.0,
    );
    
    // Test that valid config is created successfully
    assert_eq!(valid_config.max_attempts, 3);
    assert_eq!(valid_config.base_delay, Duration::from_secs(1));
    assert_eq!(valid_config.max_delay, Duration::from_secs(60));
    assert_eq!(valid_config.backoff_multiplier, 2.0);
    
    println!("✅ Retry configuration validation test passed");
    Ok(())
}

#[tokio::test]
async fn test_chain_specific_configuration() -> Result<()> {
    // Test chain-specific configuration loading
    let ethereum_config_content = r#"
[chain]
chain_id = 1
rpc_url = "https://eth.llamarpc.com"

[contracts]
usdsc_address = "0x1234567890123456789012345678901234567890"
recipient_address = "0x0987654321098765432109876543210987654321"

[thresholds]
min_yield_threshold = "1000000"

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

[kms]
key_id = "test-kms-key-id"
region = "us-east-1"
"#;
    
    let temp_file = std::env::temp_dir().join(format!("test_config_{}_{}.toml", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::write(&temp_file, ethereum_config_content)?;
    let config = ChainConfig::load(temp_file.to_str().unwrap())?;
    std::fs::remove_file(&temp_file)?;
    
    assert_eq!(config.chain.chain_id, 1);
    assert_eq!(config.chain.rpc_url, "https://eth.llamarpc.com");
    
    println!("✅ Chain-specific configuration test passed");
    Ok(())
}