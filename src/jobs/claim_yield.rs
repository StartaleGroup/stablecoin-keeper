use crate::config::ChainConfig;
use crate::blockchain::BlockchainClient;
use crate::contracts::usdsc::USDSCContract;
use crate::retry::{execute_with_retry, RetryConfig};
use alloy::primitives::U256;
use anyhow::Result;
use std::str::FromStr;
use std::time::Duration;

pub struct ClaimYieldJob {
    config: ChainConfig,
    dry_run: bool,
}

impl ClaimYieldJob {
    pub fn new(config: ChainConfig, dry_run: bool) -> Self {
        Self { config, dry_run }
    }
    
    pub async fn execute(&self) -> Result<()> {
        println!("🔍 ClaimYield Job Starting...");
        println!("   Chain ID: {}", self.config.chain.chain_id);
        println!("   RPC URL: {}", self.config.chain.rpc_url);
        println!("   USDSC Address: {}", self.config.contracts.usdsc_address);
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
        
        // Create USDSC contract instance
        let usdsc_address = BlockchainClient::parse_address(&self.config.contracts.usdsc_address)?;
        let usdsc_contract = USDSCContract::new(Address::from_str(&self.config.contracts.usdsc_address)?, client.provider());
        
        // Check pending yield with retry
        let pending_yield = execute_with_retry(
            || {
                let contract = usdsc_contract.clone();
                Box::pin(async move {
                    contract.get_pending_yield().await
                })
            },
            &retry_config,
            "Get pending yield",
        ).await?;
        println!("💰 Pending yield: {}", pending_yield);
        
        // Check if yield is above threshold
        let min_threshold = U256::from_str(&self.config.thresholds.min_yield_threshold)?;
        
        if pending_yield >= min_threshold {
            println!("💰 Yield above threshold ({} >= {}), claiming...", pending_yield, min_threshold);
            
            if self.dry_run {
                println!("✅ DRY RUN: Would claim yield transaction");
                return Ok(());
            }
            
            // Execute claim transaction with retry
            let tx_hash = execute_with_retry(
                || {
                    let contract = usdsc_contract.clone();
                    Box::pin(async move {
                        contract.claim_yield().await
                    })
                },
                &retry_config,
                "Claim yield transaction",
            ).await?;
            println!("✅ Claim transaction sent: {:?}", tx_hash);
        } else {
            println!("⏳ Yield below threshold ({} < {}), skipping claim", pending_yield, min_threshold);
        }
        
        Ok(())
    }
}