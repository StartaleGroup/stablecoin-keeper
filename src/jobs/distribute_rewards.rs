use crate::config::ChainConfig;
use crate::blockchain::BlockchainClient;
use crate::contracts::reward_redistributor::RewardRedistributorContract;
use crate::contracts::usdsc::USDSCContract;
use crate::retry::{execute_with_retry, RetryConfig};
use crate::transaction_monitor::{TransactionMonitor, TransactionStatus};
use alloy::primitives::{Address, U256};
use anyhow::Result;
use std::str::FromStr;
use std::sync::Arc;
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
        
        let retry_config = RetryConfig::new(
            self.config.retry.max_attempts,
            Duration::from_secs(self.config.retry.base_delay_seconds),
            Duration::from_secs(self.config.retry.max_delay_seconds),
            self.config.retry.backoff_multiplier,
        );

        // KMS signing is required
        let kms_config = self.config.kms.as_ref()
            .ok_or_else(|| anyhow::anyhow!("KMS configuration is required. Please configure KMS settings in your config file or via CLI."))?;
            
        println!("🔐 Using KMS signing with key: {}", kms_config.key_id);
        let client = execute_with_retry(
            || {
                let rpc_url = self.config.chain.rpc_url.clone();
                let chain_id = self.config.chain.chain_id;
                let key_id = kms_config.key_id.clone();
                async move {
                    BlockchainClient::new(&rpc_url, chain_id, &key_id, &self.config).await
                }
            },
            &retry_config,
            "Blockchain connection (KMS)",
        ).await?;
        
        let block_number = client.get_block_number().await?;
        println!("📦 Current block: {}", block_number);
        
        // First check USDSC yield (reusing logic from claim_yield.rs)
        let usdsc_contract = USDSCContract::new(Address::from_str(&self.config.contracts.usdsc_address)?, client.provider(), Arc::new(client.clone()));
        
        // Check pending yield (no retry for lightweight read operations)
        let pending_yield = usdsc_contract.get_pending_yield().await?;
        println!("💰 Pending yield: {}", pending_yield);
        
        // Check if yield is above threshold
        let min_threshold = U256::from_str(&self.config.thresholds.min_yield_threshold)?;
        
        if pending_yield < min_threshold {
            println!("⏳ Yield below threshold ({} < {}), skipping distribution", pending_yield, min_threshold);
            return Ok(());
        }
        
        println!("💰 Yield above threshold ({} >= {}), proceeding with distribution...", pending_yield, min_threshold);
        
        if let Some(redistributor_addr) = &self.config.contracts.reward_redistributor_address {
            // Create RewardRedistributor contract instance
            let redistributor_address = BlockchainClient::parse_address(redistributor_addr)?;
            let redistributor_contract = RewardRedistributorContract::new(redistributor_address, client.provider(), Arc::new(client.clone()));
            
            // Preview distribution (no retry for lightweight read operations)
            let preview = redistributor_contract.preview_distribute().await?;
            println!("📊 Distribution preview:");
            println!("   Could be minted: {}", preview.0);
            println!("   Fee to Startale: {}", preview.1);
            println!("   To Earn: {}", preview.2);
            println!("   To sUSDSC: {}", preview.3);
            println!("   To Startale Treasury: {}", preview.4);
            
            if self.dry_run {
                println!("✅ DRY RUN: Would call distribute() on RewardRedistributor");
                return Ok(());
            }
            
            // Execute distribute transaction with retry
            println!("🚀 Calling distribute() on RewardRedistributor...");
            let tx_hash = execute_with_retry(
                || {
                    let contract = redistributor_contract.clone();
                    let value_wei = self.config.transaction.value_wei.clone();
                    async move {
                        contract.distribute(&value_wei).await
                    }
                },
                &retry_config,
                "Distribute transaction",
            ).await?;
            println!("✅ Distribute transaction sent: {:?}", tx_hash);
            
            // Monitor transaction until confirmation
            let timeout_gas_used = U256::from_str(&self.config.monitoring.timeout_gas_used)?;
            let monitor = TransactionMonitor::new_with_timeout_values(
                client.provider(),
                Duration::from_secs(self.config.monitoring.transaction_timeout_seconds),
                Duration::from_secs(self.config.monitoring.poll_interval_seconds),
                self.config.monitoring.timeout_block_number,
                timeout_gas_used,
            );
            
            let receipt = monitor.monitor_transaction(tx_hash).await?;
            match receipt.status {
                TransactionStatus::Success => {
                    println!("🎉 Distribute transaction confirmed in block {}", receipt.block_number);
                    println!("⛽ Gas used: {}", receipt.gas_used);
                }
                TransactionStatus::Failed => {
                    println!("❌ Distribute transaction failed");
                    return Err(anyhow::anyhow!("Transaction failed"));
                }
                TransactionStatus::Timeout => {
                    println!("⏰ Distribute transaction monitoring timeout");
                    return Err(anyhow::anyhow!("Transaction monitoring timeout"));
                }
            }
        } else {
            println!("⚠️ No RewardRedistributor address configured");
        }
        
        Ok(())
    }
}