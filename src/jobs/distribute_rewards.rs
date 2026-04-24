use crate::blockchain::BlockchainClient;
use crate::config::ChainConfig;
use crate::contracts::reward_redistributor::{RewardRedistributorContract, TxOverrides};
use crate::contracts::usdsc::USDSCContract;
use crate::retry::{execute_with_retry, RetryConfig};
use crate::transaction_monitor::{TransactionMonitor, TransactionStatus};
use alloy::primitives::{Address, U256};
use anyhow::Result;
use std::str::FromStr;
use std::time::Duration;

pub struct DistributeRewardsJob {
    config: ChainConfig,
    dry_run: bool,
}

impl DistributeRewardsJob {
    pub fn new(config: ChainConfig, dry_run: bool) -> Self {
        Self { config, dry_run }
    }

    fn priority_fee_wei(&self) -> Option<u128> {
        self.config
            .transaction
            .max_priority_fee_gwei
            .map(|gwei| (gwei * 1_000_000_000.0) as u128)
    }

    fn build_tx_overrides(&self, base_fee: u128) -> TxOverrides {
        match self.priority_fee_wei() {
            Some(tip) => TxOverrides {
                max_priority_fee_per_gas: Some(tip),
                // baseFee * 2 gives headroom for fee fluctuation across blocks
                max_fee_per_gas: Some(base_fee * 2 + tip),
                ..Default::default()
            },
            None => TxOverrides::default(),
        }
    }

    async fn wait_for_next_block(client: &BlockchainClient) -> Result<()> {
        let initial_block = client.get_block_number().await?;
        println!("⏳ Waiting for next block (current: {})...", initial_block);

        loop {
            // Todo: This is specific to Soneium Block time, Need to this to config later
            tokio::time::sleep(Duration::from_secs(3)).await; // Block time is 2 seconds , keeping a buffer of 1 second
            let current_block = client.get_block_number().await?;
            if current_block > initial_block {
                println!("✅ New block confirmed: {}", current_block);
                return Ok(());
            }
        }
    }

    async fn get_current_timestamp(client: &BlockchainClient) -> Result<U256> {
        let block_number = client.get_block_number().await?;
        let block = client
            .provider()
            .get_block_by_number(block_number.into())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Block not found"))?;

        let timestamp = block.header.timestamp;
        Ok(U256::from(timestamp))
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
        let usdsc_contract = USDSCContract::new(
            Address::from_str(&self.config.contracts.usdsc_address)?,
            client.provider(),
            client.clone(),
        );

        // Check pending yield (no retry for lightweight read operations)
        let pending_yield = usdsc_contract.get_pending_yield().await?;
        println!("💰 Pending yield: {}", pending_yield);

        // Check if yield is above threshold
        let min_threshold = U256::from_str(&self.config.thresholds.min_yield_threshold)?;

        if pending_yield < min_threshold {
            println!(
                "⏳ Yield below threshold ({} < {}), skipping distribution",
                pending_yield, min_threshold
            );
            return Ok(());
        }

        println!(
            "💰 Yield above threshold ({} >= {}), proceeding with distribution...",
            pending_yield, min_threshold
        );

        if let Some(redistributor_addr) = &self.config.contracts.reward_redistributor_address {
            // Create RewardRedistributor contract instance
            let redistributor_address = BlockchainClient::parse_address(redistributor_addr)?;
            let redistributor_contract = RewardRedistributorContract::new(
                redistributor_address,
                client.provider(),
                client.clone(),
            );

            // ===== STEP 1: Check snapshot state =====
            println!("📸 Checking snapshot state...");

            let (
                last_snapshot_timestamp,
                last_snapshot_block,
                max_age_seconds,
                last_susdsc_tvl,
                last_earn_tvl,
                current_block,
                current_timestamp,
                base_fee,
            ) = tokio::try_join!(
                redistributor_contract.last_snapshot_timestamp(),
                redistributor_contract.last_snapshot_block_number(),
                redistributor_contract.snapshot_max_age(),
                redistributor_contract.last_susdsc_tvl(),
                redistributor_contract.last_earn_tvl(),
                client.get_block_number(),
                Self::get_current_timestamp(&client),
                client.get_base_fee_per_gas(),
            )?;

            println!("   Last snapshot timestamp: {}", last_snapshot_timestamp);
            println!("   Last snapshot block: {}", last_snapshot_block);
            println!("   Last sUSDSC vault TVL (snapshot): {}", last_susdsc_tvl);
            println!("   Last Earn vault TVL (snapshot): {}", last_earn_tvl);
            println!("   Max age: {}s", max_age_seconds);
            println!("   Current block: {}", current_block);
            println!("   Current timestamp: {}", current_timestamp);

            let current_block_u256 = U256::from(current_block);
            let needs_snapshot =
                if last_snapshot_timestamp == U256::ZERO || last_snapshot_block == U256::ZERO {
                    println!("   ⚠️  No snapshot exists");
                    true
                } else {
                    // Check if snapshot is too old (time-based)
                    let snapshot_age = current_timestamp.saturating_sub(last_snapshot_timestamp);
                    if snapshot_age > max_age_seconds {
                        println!(
                            "   ⚠️  Snapshot expired (age {}s > max {}s)",
                            snapshot_age, max_age_seconds
                        );
                        true
                    } else {
                        // Check if snapshot is in same block (block-based)
                        if current_block_u256 <= last_snapshot_block {
                            println!(
                            "   ⚠️  Snapshot in same or future block (current {} <= snapshot {})",
                            current_block, last_snapshot_block
                        );
                            // We'll wait for next block below
                            false
                        } else {
                            println!(
                                "   ✅ Snapshot is valid (block {} > {}, age {}s <= max {}s)",
                                current_block, last_snapshot_block, snapshot_age, max_age_seconds
                            );
                            false
                        }
                    }
                };

            let timeout_gas_used = U256::from_str(&self.config.monitoring.timeout_gas_used)?;
            let monitor = TransactionMonitor::new_with_timeout_values(
                client.provider(),
                Duration::from_secs(self.config.monitoring.transaction_timeout_seconds),
                Duration::from_secs(self.config.monitoring.poll_interval_seconds),
                self.config.monitoring.timeout_block_number,
                timeout_gas_used,
            );

            // ===== STEP 2: Take snapshot if needed =====
            if needs_snapshot {
                println!("📸 Taking new snapshot (sUSDSC + Earn vault TVLs)...");

                if self.dry_run {
                    println!("✅ DRY RUN: Would call snapshotVaultTVLs()");
                    return Ok(());
                }

                let snapshot_overrides = self.build_tx_overrides(base_fee);
                let snapshot_tx = execute_with_retry(
                    || {
                        let contract = redistributor_contract.clone();
                        let value_wei = self.config.transaction.value_wei.clone();
                        let overrides = snapshot_overrides.clone();
                        async move { contract.snapshot_vault_tvls(&value_wei, overrides).await }
                    },
                    &retry_config,
                    "Snapshot transaction",
                )
                .await?;

                println!("✅ Snapshot transaction sent: {:?}", snapshot_tx);

                let snapshot_receipt = monitor.monitor_transaction(snapshot_tx).await?;
                match snapshot_receipt.status {
                    TransactionStatus::Success => {
                        println!(
                            "🎉 Snapshot confirmed in block {}",
                            snapshot_receipt.block_number
                        );

                        let (new_susdsc, new_earn) = tokio::try_join!(
                            redistributor_contract.last_susdsc_tvl(),
                            redistributor_contract.last_earn_tvl(),
                        )?;
                        println!("📸 New sUSDSC vault TVL: {}", new_susdsc);
                        println!("📸 New Earn vault TVL: {}", new_earn);
                    }
                    TransactionStatus::Failed => {
                        return Err(anyhow::anyhow!("Snapshot transaction failed"));
                    }
                    TransactionStatus::Timeout => {
                        return Err(anyhow::anyhow!("Snapshot transaction monitoring timeout"));
                    }
                }

                // ===== STEP 3: Preview =====
                println!("📊 Previewing distribution...");
                let preview = redistributor_contract.preview_distribute().await?;
                println!("📊 Distribution preview:");
                println!("   Could be minted: {}", preview.0);
                println!("   Fee to Startale: {}", preview.1);
                println!("   To Earn: {}", preview.2);
                println!("   To sUSDSC: {}", preview.3);
                println!("   To Startale Treasury: {}", preview.4);

                // ===== STEP 4: Distribute — submitted immediately after snapshot confirms =====
                println!("🚀 Distributing immediately after snapshot (targeting next block)...");

                let dist_base_fee = client.get_base_fee_per_gas().await?;
                let dist_overrides = self.build_tx_overrides(dist_base_fee);

                let dist_tx = execute_with_retry(
                    || {
                        let contract = redistributor_contract.clone();
                        let value_wei = self.config.transaction.value_wei.clone();
                        let overrides = dist_overrides.clone();
                        async move { contract.distribute(&value_wei, overrides).await }
                    },
                    &retry_config,
                    "Distribute transaction",
                )
                .await?;

                println!("✅ Distribute transaction sent: {:?}", dist_tx);

                let dist_receipt = monitor.monitor_transaction(dist_tx).await?;
                match dist_receipt.status {
                    TransactionStatus::Success => {
                        let block_delta = dist_receipt.block_number - snapshot_receipt.block_number;
                        println!(
                            "🎉 Distribute confirmed in block {} ({} block(s) after snapshot)",
                            dist_receipt.block_number, block_delta
                        );
                        println!("⛽ Gas used: {}", dist_receipt.gas_used);
                    }
                    TransactionStatus::Failed => {
                        return Err(anyhow::anyhow!("Distribute transaction failed"));
                    }
                    TransactionStatus::Timeout => {
                        return Err(anyhow::anyhow!("Distribute transaction monitoring timeout"));
                    }
                }
            } else {
                // Snapshot is valid — wait only if we're in the same block as the snapshot
                if current_block_u256 <= last_snapshot_block {
                    println!("⏳ Waiting for next block before distributing...");
                    Self::wait_for_next_block(&client).await?;
                }

                // ===== STEP 3: Preview =====
                println!("📊 Previewing distribution...");
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

                // ===== STEP 4: Distribute =====
                println!("🚀 Calling distribute() on RewardRedistributor...");
                let dist_base_fee = client.get_base_fee_per_gas().await?;
                let dist_overrides = self.build_tx_overrides(dist_base_fee);

                let dist_tx = execute_with_retry(
                    || {
                        let contract = redistributor_contract.clone();
                        let value_wei = self.config.transaction.value_wei.clone();
                        let overrides = dist_overrides.clone();
                        async move { contract.distribute(&value_wei, overrides).await }
                    },
                    &retry_config,
                    "Distribute transaction",
                )
                .await?;

                println!("✅ Distribute transaction sent: {:?}", dist_tx);

                let dist_receipt = monitor.monitor_transaction(dist_tx).await?;
                match dist_receipt.status {
                    TransactionStatus::Success => {
                        println!(
                            "🎉 Distribute confirmed in block {}",
                            dist_receipt.block_number
                        );
                        println!("⛽ Gas used: {}", dist_receipt.gas_used);
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
            }
        } else {
            println!("⚠️ No RewardRedistributor address configured");
        }

        Ok(())
    }
}
