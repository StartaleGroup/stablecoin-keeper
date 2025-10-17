use crate::config::ChainConfig;
use crate::blockchain::BlockchainClient;
use crate::contracts::reward_redistributor::RewardRedistributorContract;
use crate::retry::{execute_with_retry, RetryConfig};
use anyhow::Result;
use std::time::Duration;

pub struct DistributeRewardsJob {
    config: ChainConfig,
    dry_run: bool,
}

impl DistributeRewardsJob {
    pub fn new(config: ChainConfig, dry_run: bool) -> Self {
        Self { config, dry_run }
    }
    
    pub async fn execute(&self) -> Result<()> {
        println!("🔍 Distribute Rewards Job Starting...");
        println!("   Chain ID: {}", self.config.chain.chain_id);
        println!("   RPC URL: {}", self.config.chain.rpc_url);
        println!("   RewardRedistributor: {:?}", self.config.contracts.reward_redistributor_address);
        println!("   Dry Run: {}", self.dry_run);
        
        // Create retry configuration
        let retry_config = RetryConfig::new(
            self.config.retry.max_attempts,
            Duration::from_secs(self.config.retry.base_delay_seconds),
            Duration::from_secs(self.config.retry.max_delay_seconds),
            self.config.retry.backoff_multiplier,
        );

        // Connect to blockchain with retry
        let client = execute_with_retry(
            || {
                let rpc_url = self.config.chain.rpc_url.clone();
                let chain_id = self.config.chain.chain_id;
                let private_key = self.config.chain.private_key.clone();
                Box::pin(async move {
                    BlockchainClient::new(&rpc_url, chain_id, &private_key).await
                })
            },
            &retry_config,
            "Blockchain connection",
        ).await?;
        
        let block_number = client.get_block_number().await?;
        println!("📦 Current block: {}", block_number);
        
        if let Some(redistributor_addr) = &self.config.contracts.reward_redistributor_address {
            // Create RewardRedistributor contract instance
            let redistributor_address = BlockchainClient::parse_address(redistributor_addr)?;
            let redistributor_contract = RewardRedistributorContract::new(redistributor_address, client.provider());
            
            // Preview distribution with retry
            let preview = execute_with_retry(
                || {
                    let contract = redistributor_contract.clone();
                    Box::pin(async move {
                        contract.preview_distribute().await
                    })
                },
                &retry_config,
                "Preview distribution",
            ).await?;
            println!("📊 Distribution preview:");
            println!("   Could be minted: {}", preview.0);
            println!("   Fee to Startale: {}", preview.1);
            println!("   To Earn: {}", preview.2);
            println!("   To On: {}", preview.3);
            println!("   To Startale Extra: {}", preview.4);
            
            if self.dry_run {
                println!("✅ DRY RUN: Would call distribute() on RewardRedistributor");
                return Ok(());
            }
            
            // Execute distribute transaction with retry
            println!("🚀 Calling distribute() on RewardRedistributor...");
            let tx_hash = execute_with_retry(
                || {
                    let contract = redistributor_contract.clone();
                    Box::pin(async move {
                        contract.distribute().await
                    })
                },
                &retry_config,
                "Distribute transaction",
            ).await?;
            println!("✅ Distribute transaction sent: {:?}", tx_hash);
        } else {
            println!("⚠️ No RewardRedistributor address configured");
        }
        
        Ok(())
    }
}