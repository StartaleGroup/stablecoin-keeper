//! Integration Tests
//! 
//! Tests for component interaction, KMS integration, blockchain connectivity, and end-to-end workflows.
//! These tests verify that different components work together correctly.

use stablecoin_backend::blockchain::BlockchainClient;
use stablecoin_backend::config::ChainConfig;
use stablecoin_backend::retry::{execute_with_retry, RetryConfig};
use stablecoin_backend::transaction_monitor::TransactionMonitor;
use stablecoin_backend::jobs::{ClaimYieldJob, DistributeRewardsJob};
use stablecoin_backend::contracts::usdsc::USDSCContract;
use stablecoin_backend::contracts::reward_redistributor::RewardRedistributorContract;
use alloy::primitives::Address;
use anyhow::Result;
use std::str::FromStr;
use std::time::Duration;

#[tokio::test]
async fn test_kms_signer_integration() -> Result<()> {
    // Test that KMS signer is properly integrated with provider
    let test_rpc_url = "https://eth.llamarpc.com"; // Public RPC for testing
    let test_chain_id = 1u64;
    let test_kms_key_id = "test-kms-key-id";
    
    // Create a test config with KMS settings
    let config = create_test_config()?;
    
    // This should create a provider with integrated KMS signer
    let client = BlockchainClient::new(test_rpc_url, test_chain_id, test_kms_key_id, &config).await?;
    
    // Verify we can get the provider
    let provider = client.provider();
    
    // Test that the provider includes the signer by checking if we can get chain ID
    let chain_id = provider.get_chain_id().await?;
    assert_eq!(chain_id, test_chain_id);
    
    println!("✅ KMS signer integration test passed");
    Ok(())
}

#[tokio::test]
async fn test_retry_logic() -> Result<()> {
    // Test retry logic with a failing operation that eventually succeeds
    let config = RetryConfig::new(
        3, // max_attempts
        Duration::from_millis(10), // base_delay
        Duration::from_secs(1), // max_delay
        2.0, // backoff_multiplier
    );
    
    let attempt_count = std::sync::Arc::new(std::sync::Mutex::new(0));
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
    
    println!("✅ Retry logic test passed");
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
    
    let attempt_count = std::sync::Arc::new(std::sync::Mutex::new(0));
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
async fn test_transaction_monitor_creation() -> Result<()> {
    // Test transaction monitor creation and basic functionality
    let test_rpc_url = "https://eth.llamarpc.com";
    let test_chain_id = 1u64;
    let test_kms_key_id = "test-kms-key-id";
    
    let config = create_test_config()?;
    let client = BlockchainClient::new(test_rpc_url, test_chain_id, test_kms_key_id, &config).await?;
    let provider = client.provider();
    
    // Create transaction monitor
    let _monitor = TransactionMonitor::new(provider, Duration::from_secs(30), Duration::from_secs(1));
    
    // Test that monitor was created successfully
    println!("✅ Transaction monitor creation test passed");
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
async fn test_address_parsing() -> Result<()> {
    // Test address parsing functionality
    let valid_address = "0x1234567890123456789012345678901234567890";
    let parsed_address = BlockchainClient::parse_address(valid_address)?;
    
    assert_eq!(parsed_address, Address::from_str(valid_address)?);
    
    // Test invalid address
    let invalid_address = "invalid_address";
    let result = BlockchainClient::parse_address(invalid_address);
    assert!(result.is_err());
    
    println!("✅ Address parsing test passed");
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
async fn test_chain_id_validation() -> Result<()> {
    // Test chain ID validation
    let test_rpc_url = "https://eth.llamarpc.com";
    let expected_chain_id = 1u64;
    let test_kms_key_id = "test-kms-key-id";
    
    let config = create_test_config()?;
    let client = BlockchainClient::new(test_rpc_url, expected_chain_id, test_kms_key_id, &config).await?;
    
    // Test that we can get the chain ID
    let provider = client.provider();
    let chain_id = provider.get_chain_id().await?;
    assert_eq!(chain_id, expected_chain_id);
    
    println!("✅ Chain ID validation test passed");
    Ok(())
}

#[tokio::test]
async fn test_kms_validation() -> Result<()> {
    // Test KMS configuration validation
    let config = create_test_config()?;
    
    // Test that KMS configuration is present
    assert!(config.kms.is_some());
    let kms_config = config.kms.unwrap();
    assert_eq!(kms_config.key_id, "test-kms-key-id");
    assert_eq!(kms_config.region, Some("us-east-1".to_string()));
    
    println!("✅ KMS validation test passed");
    Ok(())
}

#[tokio::test]
async fn test_job_creation() -> Result<()> {
    // Test that jobs can be created with proper configuration
    let config = create_test_config()?;
    
    // Test ClaimYieldJob creation
    let _claim_job = ClaimYieldJob::new(config.clone(), true); // dry_run = true
    println!("✅ ClaimYieldJob created successfully");
    
    // Test DistributeRewardsJob creation
    let _distribute_job = DistributeRewardsJob::new(config, true); // dry_run = true
    println!("✅ DistributeRewardsJob created successfully");
    
    println!("✅ Job creation test passed");
    Ok(())
}

#[tokio::test]
async fn test_contract_instantiation() -> Result<()> {
    // Test that contract instances can be created
    let test_rpc_url = "https://eth.llamarpc.com";
    let test_chain_id = 1u64;
    let test_kms_key_id = "test-kms-key-id";
    
    let config = create_test_config()?;
    let client = BlockchainClient::new(test_rpc_url, test_chain_id, test_kms_key_id, &config).await?;
    let provider = client.provider();
    
    // Test USDSC contract creation
    let usdsc_address = Address::from_str("0x1234567890123456789012345678901234567890")?;
    let _usdsc_contract = USDSCContract::new(usdsc_address, provider.clone(), client.clone());
    
    // Test RewardRedistributor contract creation
    let redistributor_address = Address::from_str("0x0987654321098765432109876543210987654321")?;
    let _redistributor_contract = RewardRedistributorContract::new(redistributor_address, provider, client.clone());
    
    println!("✅ Contract instantiation test passed");
    Ok(())
}

// Helper functions for creating test configurations
fn create_test_config() -> Result<ChainConfig> {
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
    Ok(config)
}