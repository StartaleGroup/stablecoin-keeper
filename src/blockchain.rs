use crate::kms_signer::KmsSigner;
use alloy::network::Ethereum;
use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder};
use anyhow::Result;
use std::str::FromStr;
use std::sync::Arc;
use url::Url;

#[derive(Clone)]
pub struct BlockchainClient {
    provider: Arc<dyn Provider<Ethereum>>,
}

impl BlockchainClient {
    pub async fn new(
        rpc_url: &str,
        expected_chain_id: u64,
        kms_key_id: &str,
        chain_config: &crate::config::ChainConfig,
    ) -> Result<Self> {
        println!("🔗 Connecting to RPC: {}", rpc_url);

        let url = Url::parse(rpc_url)?;

        let aws_region = chain_config.kms.as_ref()
            .and_then(|kms| kms.region.as_deref())
            .ok_or_else(|| anyhow::anyhow!("KMS region not configured. Set AWS_REGION environment variable or configure region in config file"))?;
        let kms_signer = KmsSigner::new(
            kms_key_id.to_string(),
            aws_region.to_string(),
            expected_chain_id,
        )
        .await?;
        let kms_address = kms_signer.address();

        let provider = ProviderBuilder::new()
            .wallet(kms_signer.as_alloy_signer().clone())
            .connect_http(url);

        let chain_id = provider.get_chain_id().await?;
        if chain_id != expected_chain_id {
            return Err(anyhow::anyhow!(
                "Chain ID mismatch: expected {}, got {}",
                expected_chain_id,
                chain_id
            ));
        }

        println!("✅ Connected to chain {}", expected_chain_id);
        println!("🔐 KMS Wallet address: {}", kms_address);

        Ok(Self {
            provider: Arc::new(provider),
        })
    }

    pub fn provider(&self) -> Arc<dyn Provider<Ethereum>> {
        self.provider.clone()
    }

    pub async fn get_block_number(&self) -> Result<u64> {
        let block_number = self.provider.get_block_number().await?;
        Ok(block_number)
    }

    pub fn parse_address(addr: &str) -> Result<Address> {
        Address::from_str(addr).map_err(|e| anyhow::anyhow!("Invalid address {}: {}", addr, e))
    }

    pub async fn send_transaction(
        &self,
        tx: alloy::rpc::types::TransactionRequest,
    ) -> Result<alloy::primitives::B256> {
        println!("📤 Sending transaction...");
        let pending = self.provider.send_transaction(tx).await?;
        let tx_hash = *pending.tx_hash();
        println!("✅ Transaction sent: {:?}", tx_hash);
        Ok(tx_hash)
    }
}
