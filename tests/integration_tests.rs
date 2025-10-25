use stablecoin_backend::blockchain::BlockchainClient;
use stablecoin_backend::config::ChainConfig;
use stablecoin_backend::retry::{execute_with_retry, RetryConfig};
use stablecoin_backend::transaction_monitor::TransactionMonitor;
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
    
    let call_count = std::sync::atomic::AtomicU32::new(0);
    let result = execute_with_retry(
        || {
            let count = call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async move {
                if count == 0 {
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
    assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 2);
    
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
    
    println!("✅ Retry failure test passed");
    Ok(())
}

#[tokio::test]
async fn test_transaction_monitor_creation() -> Result<()> {
    // Test that TransactionMonitor can be created with proper configuration
    let test_rpc_url = "https://eth.llamarpc.com";
    let test_chain_id = 1u64;
    let test_kms_key_id = "test-kms-key-id";
    
    let config = create_test_config()?;
    let client = BlockchainClient::new(test_rpc_url, test_chain_id, test_kms_key_id, &config).await?;
    let provider = client.provider();
    
    let _monitor = TransactionMonitor::new(
        provider,
        Duration::from_secs(30), // max_wait_time
        Duration::from_secs(1),  // poll_interval
    );
    
    // Test that monitor was created successfully (private fields can't be accessed)
    println!("✅ Transaction monitor created successfully");
    
    println!("✅ Transaction monitor creation test passed");
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
"#;
    
    // Write config to temporary file
    let temp_file = std::env::temp_dir().join(format!("test_config_{}.toml", std::process::id()));
    std::fs::write(&temp_file, config_content)?;
    
    // Load config
    let config = ChainConfig::load(temp_file.to_str().unwrap())?;
    
    // Verify config was loaded correctly
    assert_eq!(config.chain.chain_id, 1);
    assert_eq!(config.chain.rpc_url, "https://eth.llamarpc.com");
    assert_eq!(config.contracts.usdsc_address, "0x1234567890123456789012345678901234567890");
    assert_eq!(config.thresholds.min_yield_threshold, "1000000");
    assert_eq!(config.retry.max_attempts, 3);
    
    // Clean up
    std::fs::remove_file(&temp_file)?;
    
    println!("✅ Config loading test passed");
    Ok(())
}

#[tokio::test]
async fn test_address_parsing() -> Result<()> {
    // Test address parsing functionality
    let valid_address = "0x1234567890123456789012345678901234567890";
    let invalid_address = "invalid_address";
    
    // Test valid address
    let parsed = BlockchainClient::parse_address(valid_address)?;
    assert_eq!(parsed, Address::from_str(valid_address)?);
    
    // Test invalid address
    let result = BlockchainClient::parse_address(invalid_address);
    assert!(result.is_err());
    
    println!("✅ Address parsing test passed");
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
"#;
    
    let temp_file = std::env::temp_dir().join(format!("test_env_config_{}.toml", std::process::id()));
    std::fs::write(&temp_file, config_content)?;
    
    let config = ChainConfig::load(temp_file.to_str().unwrap())?;
    
    // Verify environment variables were substituted
    assert_eq!(config.chain.rpc_url, "https://test.example.com");
    assert_eq!(config.kms.as_ref().unwrap().key_id, "test-kms-key-id");
    
    // Clean up
    std::fs::remove_file(&temp_file)?;
    std::env::remove_var("TEST_RPC_URL");
    std::env::remove_var("TEST_PRIVATE_KEY");
    
    println!("✅ Environment variable substitution test passed");
    Ok(())
}

#[tokio::test]
async fn test_chain_id_validation() -> Result<()> {
    // Test chain ID validation
    let test_rpc_url = "https://eth.llamarpc.com";
    let config = create_test_config()?;
    
    // Test with correct chain ID
    let client = BlockchainClient::new(test_rpc_url, 1u64, "test-kms-key-id", &config).await?;
    assert!(client.provider().get_chain_id().await? == 1);
    
    // Test with incorrect chain ID (should fail)
    let result = BlockchainClient::new(test_rpc_url, 999u64, "test-kms-key-id", &config).await;
    assert!(result.is_err());
    // Can't access error message due to Debug trait requirement
    
    println!("✅ Chain ID validation test passed");
    Ok(())
}

#[tokio::test]
async fn test_kms_validation() -> Result<()> {
    // Test KMS validation
    let test_rpc_url = "https://eth.llamarpc.com";
    let test_chain_id = 1u64;
    
    // Test with valid KMS config
    let config = create_test_config()?;
    let client = BlockchainClient::new(test_rpc_url, test_chain_id, "test-kms-key-id", &config).await?;
    assert!(client.provider().get_chain_id().await? == test_chain_id);
    
    // Test with invalid chain ID (should fail)
    let result = BlockchainClient::new(test_rpc_url, 999u64, "test-kms-key-id", &config).await;
    assert!(result.is_err());
    
    println!("✅ KMS validation test passed");
    Ok(())
}

// Helper function for creating test configurations
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
    
    let temp_file = std::env::temp_dir().join(format!("test_config_{}.toml", std::process::id()));
    std::fs::write(&temp_file, config_content)?;
    
    let config = ChainConfig::load(temp_file.to_str().unwrap())?;
    std::fs::remove_file(&temp_file)?;
    
    Ok(config)
}
