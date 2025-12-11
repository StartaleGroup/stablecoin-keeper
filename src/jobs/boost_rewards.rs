use crate::blockchain::BlockchainClient;
use crate::config::ChainConfig;
use crate::contracts::earn_vault::EarnVaultContract;
use crate::contracts::erc20::ERC20Contract;
use crate::retry::{execute_with_retry, RetryConfig};
use crate::transaction_monitor::{TransactionMonitor, TransactionStatus};
use alloy::primitives::{Address, U256};
use anyhow::Result;
use chrono::{NaiveDate, Utc};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

// Trait for getting campaigns (abstraction layer)
#[async_trait::async_trait]
pub trait CampaignConfigSource: Send + Sync {
    async fn get_campaigns(&self) -> Result<Vec<CampaignConfig>>;
}
pub struct BoostRewardsJob {
    config: ChainConfig,
    token_address: Address,
    total_amount: f64,
    start_date: NaiveDate,
    end_date: NaiveDate,
    duration_days: u64, // Calculated from start_date and end_date
    campaign_id: Option<String>,
    dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct CampaignConfig {
    pub id: String,
    pub token_address: Address,
    pub total_amount: f64,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub status: CampaignStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CampaignStatus {
    Active,
    Paused,
    Completed,
}

impl CampaignConfig {
    pub fn duration_days(&self) -> u64 {
        ((self.end_date - self.start_date).num_days() + 1) as u64
    }

    pub fn is_active_for_date(&self, date: NaiveDate) -> bool {
        self.status == CampaignStatus::Active && date >= self.start_date && date <= self.end_date
    }
}

impl BoostRewardsJob {
    pub fn new(
        config: ChainConfig,
        token_address: String,
        total_amount: f64,
        start_date: String,
        end_date: String,
        campaign_id: Option<String>,
        dry_run: bool,
    ) -> Result<Self> {
        let token_addr = Address::from_str(&token_address)?;
        let start = NaiveDate::parse_from_str(&start_date, "%Y-%m-%d")?;
        let end = NaiveDate::parse_from_str(&end_date, "%Y-%m-%d")?;

        // Validate end_date is after start_date
        if end <= start {
            return Err(anyhow::anyhow!(
                "End date ({}) must be after start date ({})",
                end_date,
                start_date
            ));
        }

        // Calculate duration in days (inclusive of both start and end dates)
        // Example: Jan 1 to Jan 3 = 3 days (Jan 1, Jan 2, Jan 3)
        let duration_days = ((end - start).num_days() + 1) as u64;
        if duration_days == 0 {
            return Err(anyhow::anyhow!("Campaign duration must be at least 1 day"));
        }

        // Validate total_amount is positive
        if total_amount <= 0.0 {
            return Err(anyhow::anyhow!(
                "Total amount must be positive: {}",
                total_amount
            ));
        }

        Ok(Self {
            config,
            token_address: token_addr,
            total_amount,
            start_date: start,
            end_date: end,
            duration_days,
            campaign_id,
            dry_run,
        })
    }

    pub async fn execute(&self) -> Result<()> {
        println!("🚀 Boost Rewards Distribution Starting...");
        if let Some(id) = &self.campaign_id {
            println!("   Campaign ID: {}", id);
        }

        // 0. Validate date range first (early return)
        let today = Utc::now().date_naive();
        if today < self.start_date {
            return Err(anyhow::anyhow!(
                "Campaign has not started yet. Start date: {}, Today: {}",
                self.start_date,
                today
            ));
        }
        if today > self.end_date {
            return Err(anyhow::anyhow!(
                "Campaign has ended. End date: {}, Today: {}",
                self.end_date,
                today
            ));
        }

        // 1. Setup retry config and initialize client
        let retry_config = RetryConfig::new(
            self.config.retry.max_attempts,
            Duration::from_secs(self.config.retry.base_delay_seconds),
            Duration::from_secs(self.config.retry.max_delay_seconds),
            self.config.retry.backoff_multiplier,
        );

        let kms_config = self
            .config
            .kms
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("KMS configuration is required"))?;

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

        // Create Arc once to avoid cloning
        let client_arc = Arc::new(client);

        // 2. Validate token contract and get decimals
        println!("🔍 Validating token contract...");
        let token_contract = ERC20Contract::new(self.token_address, client_arc.provider());

        let keeper_address = client_arc.keeper_address();
        // Get token details and keeper balance
        let (token_decimals, token_symbol, keeper_balance) = tokio::try_join!(
            token_contract.decimals(),
            token_contract.symbol(),
            token_contract.balance_of(keeper_address),
        )?;

        println!("   Token: {} ({} decimals)", token_symbol, token_decimals);

        // 3. Calculate daily amount with overflow checks
        let multiplier = 10_f64.powi(token_decimals as i32);

        // Validate multiplier
        if multiplier.is_infinite() || multiplier.is_nan() {
            return Err(anyhow::anyhow!(
                "Invalid multiplier calculation (decimals: {})",
                token_decimals
            ));
        }

        // Check for f64 overflow before multiplication
        let max_safe_amount_f64 = f64::MAX / multiplier;
        if self.total_amount > max_safe_amount_f64 {
            return Err(anyhow::anyhow!(
                "Amount too large: {} (multiplication would overflow f64 with {} decimals)",
                self.total_amount,
                token_decimals
            ));
        }

        // Check for u128 overflow before multiplication
        let max_safe_amount_u128 = u128::MAX as f64 / multiplier;
        if self.total_amount > max_safe_amount_u128 {
            return Err(anyhow::anyhow!(
                "Amount too large: {} (would exceed u128::MAX with {} decimals)",
                self.total_amount,
                token_decimals
            ));
        }

        // Perform multiplication and validate result
        let amount_wei_f64 = self.total_amount * multiplier;
        if amount_wei_f64.is_infinite() || amount_wei_f64.is_nan() {
            return Err(anyhow::anyhow!(
                "Invalid amount calculation result: {}",
                amount_wei_f64
            ));
        }
        if amount_wei_f64 > u128::MAX as f64 {
            return Err(anyhow::anyhow!(
                "Amount too large: {} (would overflow u128)",
                self.total_amount
            ));
        }

        // Convert to U256 (round to nearest integer)
        let total_amount_wei = U256::from(amount_wei_f64.round() as u128);

        let daily_amount_wei = total_amount_wei
            .checked_div(U256::from(self.duration_days))
            .ok_or_else(|| anyhow::anyhow!("Division by zero"))?;

        let daily_amount_human = self.total_amount / self.duration_days as f64;

        println!("💰 Campaign Details:");
        println!("   Total Amount: {} {}", self.total_amount, token_symbol);
        println!("   Duration: {} days", self.duration_days);
        println!(
            "   Daily Amount: {:.2} {}",
            daily_amount_human, token_symbol
        );

        // 4. Calculate days elapsed/remaining
        // Note: These are already validated to be non-negative by date range checks above
        let days_elapsed = (today - self.start_date).num_days().max(0);
        let days_remaining = (self.end_date - today).num_days().max(0);
        println!("📅 Date Validation:");
        println!("   Start Date: {}", self.start_date);
        println!("   End Date: {}", self.end_date);
        println!("   Today: {}", today);
        println!("   Days Elapsed: {}", days_elapsed);
        println!("   Days Remaining: {}", days_remaining);

        // 5. Check keeper balance
        if keeper_balance < daily_amount_wei {
            return Err(anyhow::anyhow!(
                "Insufficient token balance for today: keeper has {}, need {}",
                keeper_balance,
                daily_amount_wei
            ));
        }

        // Check remaining campaign amount (warning)
        // If today == end_date, days_remaining is 0 but we still need 1 day's worth
        let days_for_remaining_calc = days_remaining.max(1) as u64;
        let remaining_amount_wei = daily_amount_wei
            .checked_mul(U256::from(days_for_remaining_calc))
            .ok_or_else(|| {
                anyhow::anyhow!("Amount overflow when calculating remaining campaign amount")
            })?;

        let keeper_balance_human =
            keeper_balance.to_string().parse::<f64>()? / 10_f64.powi(token_decimals as i32);
        let remaining_amount_human =
            remaining_amount_wei.to_string().parse::<f64>()? / 10_f64.powi(token_decimals as i32);

        println!("💵 Balance Check:");
        println!(
            "   Keeper Balance: {:.2} {}",
            keeper_balance_human, token_symbol
        );
        println!(
            "   Daily Amount Required: {:.2} {}",
            daily_amount_human, token_symbol
        );
        println!(
            "   Remaining Campaign Amount Required: {:.2} {} ({} days remaining)",
            remaining_amount_human, token_symbol, days_for_remaining_calc
        );

        if keeper_balance < remaining_amount_wei {
            println!(
                "   ⚠️  WARNING: Keeper balance ({:.2} {}) is less than remaining campaign amount ({:.2} {}).",
                keeper_balance_human, token_symbol, remaining_amount_human, token_symbol
            );
            println!(
                "   ⚠️  Campaign will proceed, but may fail on future days if balance is not replenished."
            );
        } else {
            println!("   ✅ Sufficient balance for remaining campaign duration");
        }

        // 6. Get earn vault address
        let earn_vault_address = self
            .config
            .contracts
            .earn_vault_address
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Earn vault address not configured"))?;
        let earn_vault_addr = Address::from_str(earn_vault_address)?;

        if self.dry_run {
            println!(
                "✅ DRY RUN: Would transfer {} {} to Earn Vault",
                daily_amount_human, token_symbol
            );
            println!(
                "✅ DRY RUN: Would call onBoostReward({}, {})",
                self.token_address, daily_amount_wei
            );
            return Ok(());
        }

        // 7. Transfer tokens to Earn Vault
        println!("📤 Transferring tokens to Earn Vault...");
        let transfer_tx = execute_with_retry(
            || {
                let contract = token_contract.clone();
                let amount = daily_amount_wei;
                let to = earn_vault_addr;
                async move { contract.transfer(to, amount).await }
            },
            &retry_config,
            "Token transfer",
        )
        .await?;

        println!("   Transfer TX: {:?}", transfer_tx);

        // Monitor transfer transaction
        let timeout_gas_used = U256::from_str(&self.config.monitoring.timeout_gas_used)?;
        let monitor = TransactionMonitor::new_with_timeout_values(
            client_arc.provider(),
            Duration::from_secs(self.config.monitoring.transaction_timeout_seconds),
            Duration::from_secs(self.config.monitoring.poll_interval_seconds),
            self.config.monitoring.timeout_block_number,
            timeout_gas_used,
        );

        let transfer_receipt = monitor.monitor_transaction(transfer_tx).await?;
        match transfer_receipt.status {
            TransactionStatus::Success => {
                println!(
                    "✅ Transfer confirmed in block {}",
                    transfer_receipt.block_number
                );
            }
            TransactionStatus::Failed => {
                return Err(anyhow::anyhow!("Token transfer failed"));
            }
            TransactionStatus::Timeout => {
                return Err(anyhow::anyhow!("Token transfer monitoring timeout"));
            }
        }

        println!("📞 Calling onBoostReward on Earn Vault...");
        let earn_vault = EarnVaultContract::new(earn_vault_addr, client_arc.provider());

        let boost_reward_tx = execute_with_retry(
            || {
                let contract = earn_vault.clone();
                let token = self.token_address;
                let amount = daily_amount_wei;
                async move { contract.on_boost_reward(token, amount).await }
            },
            &retry_config,
            "onBoostReward call",
        )
        .await?;

        println!("   onBoostReward TX: {:?}", boost_reward_tx);

        // Monitor onBoostReward transaction
        let boost_reward_receipt = monitor.monitor_transaction(boost_reward_tx).await?;
        match boost_reward_receipt.status {
            TransactionStatus::Success => {
                println!(
                    "✅ onBoostReward confirmed in block {}",
                    boost_reward_receipt.block_number
                );
                println!("🎉 Distribution completed successfully!");
                println!("   Days Remaining: {}", days_remaining);
            }
            TransactionStatus::Failed => {
                return Err(anyhow::anyhow!(
                    "onBoostReward call failed - tokens already transferred"
                ));
            }
            TransactionStatus::Timeout => {
                return Err(anyhow::anyhow!(
                    "onBoostReward monitoring timeout - tokens already transferred"
                ));
            }
        }

        Ok(())
    }

    pub fn from_campaign_config(
        config: ChainConfig,
        campaign: CampaignConfig,
        dry_run: bool,
    ) -> Result<Self> {
        // Validate campaign config (same validations as new())
        if campaign.end_date <= campaign.start_date {
            return Err(anyhow::anyhow!(
                "Invalid campaign config for {}: end_date ({}) must be after start_date ({})",
                campaign.id,
                campaign.end_date,
                campaign.start_date
            ));
        }

        if campaign.total_amount <= 0.0 {
            return Err(anyhow::anyhow!(
                "Invalid campaign config for {}: total_amount must be positive, got {}",
                campaign.id,
                campaign.total_amount
            ));
        }

        Ok(Self {
            config,
            token_address: campaign.token_address,
            total_amount: campaign.total_amount,
            start_date: campaign.start_date,
            end_date: campaign.end_date,
            duration_days: campaign.duration_days(),
            campaign_id: Some(campaign.id),
            dry_run,
        })
    }
}
