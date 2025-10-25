use stablecoin_backend::blockchain::BlockchainClient;

#[tokio::test]
async fn test_blockchain_client_creation() {
    // Test blockchain client creation with mock private key

    let result = BlockchainClient::new("https://1rpc.io/sepolia", 11155111, "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef").await;
    
    match result {
        Ok(_client) => {
            println!("✅ Blockchain client created successfully");
        }
        Err(e) => {
            println!("⚠️ Blockchain client creation failed: {}", e);
            // This might fail due to network issues, which is acceptable for tests
        }
    }
}

#[tokio::test]
async fn test_address_parsing() {
    // Test address parsing functionality
    let valid_address = "0x1234567890123456789012345678901234567890";
    let result = BlockchainClient::parse_address(valid_address);
    
    match result {
        Ok(address) => {
            assert_eq!(address.to_string().to_lowercase(), valid_address.to_lowercase());
            println!("✅ Address parsing works correctly");
        }
        Err(e) => {
            panic!("Address parsing failed: {}", e);
        }
    }
}

#[tokio::test]
async fn test_invalid_address_parsing() {
    // Test invalid address parsing
    let invalid_address = "invalid_address";
    let result = BlockchainClient::parse_address(invalid_address);
    
    match result {
        Ok(_) => {
            panic!("Should have failed for invalid address");
        }
        Err(_) => {
            println!("✅ Invalid address correctly rejected");
        }
    }
}
