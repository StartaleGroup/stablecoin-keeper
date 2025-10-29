use crate::blockchain::BlockchainClient;
use crate::config::ChainConfig;
use crate::contracts::usdsc::USDSCContract;
use crate::retry::{execute_with_retry, RetryConfig};
use crate::transaction_monitor::{TransactionMonitor, TransactionStatus};
use alloy::primitives::{Address, U256};
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

        let usdsc_contract = USDSCContract::new(
            Address::from_str(&self.config.contracts.usdsc_address)?,
            client.provider(),
            client.clone(),
        );

        let pending_yield = usdsc_contract.get_pending_yield().await?;
        println!("💰 Pending yield: {}", pending_yield);

        let min_threshold = U256::from_str(&self.config.thresholds.min_yield_threshold)?;

        if pending_yield >= min_threshold {
            println!(
                "💰 Yield above threshold ({} >= {}), claiming...",
                pending_yield, min_threshold
            );

            if self.dry_run {
                println!("✅ DRY RUN: Would claim yield transaction");
                return Ok(());
            }

            let tx_hash = execute_with_retry(
                || {
                    let contract = usdsc_contract.clone();
                    let value_wei = self.config.transaction.value_wei.clone();
                    async move { contract.claim_yield(&value_wei).await }
                },
                &retry_config,
                "Claim yield transaction",
            )
            .await?;
            println!("✅ Claim transaction sent: {:?}", tx_hash);

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
                    println!(
                        "🎉 Claim transaction confirmed in block {}",
                        receipt.block_number
                    );
                    println!("⛽ Gas used: {}", receipt.gas_used);
                }
                TransactionStatus::Failed => {
                    println!("❌ Claim transaction failed");
                    return Err(anyhow::anyhow!("Transaction failed"));
                }
                TransactionStatus::Timeout => {
                    println!("⏰ Claim transaction monitoring timeout");
                    return Err(anyhow::anyhow!("Transaction monitoring timeout"));
                }
            }
        } else {
            println!(
                "⏳ Yield below threshold ({} < {}), skipping claim",
                pending_yield, min_threshold
            );
        }

        Ok(())
    }
}
