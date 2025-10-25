use alloy::signers::{aws::AwsSigner, Signer};
use aws_config::BehaviorVersion;
use aws_sdk_kms::Client as KmsClient;
use anyhow::Result;

/// AWS KMS signer for secure transaction signing
#[derive(Clone)]
pub struct KmsSigner {
    signer: AwsSigner,
}

impl KmsSigner {
    /// Create a new KMS signer
    pub async fn new(key_id: String, region: String, chain_id: u64) -> Result<Self> {
        println!("🔐 Initializing AWS KMS signer...");
        println!("🔑 Key ID: {}", key_id);
        println!("🌍 Region: {}", region);
        println!("⛓️ Chain ID: {}", chain_id);
        
        // Set AWS region environment variable if not already set
        if std::env::var("AWS_REGION").is_err() {
            std::env::set_var("AWS_REGION", &region);
        }
        
        // Load AWS configuration
        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let kms_client = KmsClient::new(&config);
        
        // Create the Alloy AWS signer with the correct chain ID
        let signer = AwsSigner::new(kms_client, key_id, Some(chain_id)).await
            .map_err(|e| anyhow::anyhow!("Failed to create AWS signer: {}", e))?;
        
        println!("✅ KMS signer initialized successfully");
        println!("📍 Ethereum address: 0x{}", hex::encode(signer.address().as_slice()));
        
        Ok(Self { signer })
    }
    
    /// Get the Ethereum address
    pub fn address(&self) -> alloy::primitives::Address {
        self.signer.address()
    }
    
    
    /// Get the underlying Alloy AWS signer for use with providers
    pub fn as_alloy_signer(&self) -> &AwsSigner {
        &self.signer
    }
    
}