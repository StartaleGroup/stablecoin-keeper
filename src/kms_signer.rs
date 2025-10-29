use alloy::signers::{aws::AwsSigner, Signer};
use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_kms::Client as KmsClient;

#[derive(Clone)]
pub struct KmsSigner {
    signer: AwsSigner,
}

impl KmsSigner {
    pub async fn new(key_id: String, region: String, chain_id: u64) -> Result<Self> {
        println!("🔐 Initializing AWS KMS signer...");

        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_config::Region::new(region.clone()))
            .load()
            .await;
        let kms_client = KmsClient::new(&config);

        let signer = AwsSigner::new(kms_client, key_id, Some(chain_id))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create AWS signer: {}", e))?;

        println!("✅ KMS signer initialized successfully");
        println!(
            "📍 Ethereum address: 0x{}",
            hex::encode(signer.address().as_slice())
        );

        Ok(Self { signer })
    }

    pub fn address(&self) -> alloy::primitives::Address {
        self.signer.address()
    }

    pub fn as_alloy_signer(&self) -> &AwsSigner {
        &self.signer
    }
}
