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
use std::time::Duration;

pub struct BoostRewardsJob {
    config: ChainConfig,
    token_address: Address,
    total_amount: String,
    start_date: NaiveDate,
    end_date: NaiveDate,
    duration_days: u64,  // Calculated from start_date and end_date
    campaign_id: Option<String>,
    dry_run: bool,
}

impl BoostRewardsJob {
    pub fn new(config: ChainConfig, token_address: String, total_amount: String, start_date: String, end_date: String, campaign_id: Option<String>, dry_run: bool) -> Result<Self> {
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
        
        // Calculate duration in days
        let duration_days = (end - start).num_days() as u64;
        if duration_days == 0 {
            return Err(anyhow::anyhow!("Campaign duration must be at least 1 day"));
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

        // 1. Setup retry config and initialize client
        let retry_config = RetryConfig::new(
            self.config.retry.max_attempts,
            Duration::from_secs(self.config.retry.base_delay_seconds),
            Duration::from_secs(self.config.retry.max_delay_seconds),
            self.config.retry.backoff_multiplier,
        );

        let kms_config = self.config.kms.as_ref()
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

        // 2. Validate token contract and get decimals
        println!("🔍 Validating token contract...");
        let token_contract = ERC20Contract::new(
            self.token_address,
            client.provider(),
            client.clone(),
        );

        let keeper_address = client.keeper_address();
        // Get token details and keeper balance
        let (token_decimals, token_symbol, keeper_balance) = tokio::try_join!(
            token_contract.decimals(),
            token_contract.symbol(),
            token_contract.balance_of(keeper_address),
        )?;
        
        println!("   Token: {} ({} decimals)", token_symbol, token_decimals);

        // 3. Calculate daily amount
        // Parse amount - supports both integer and decimal formats (e.g., "1000" or "100.232")
        let total_amount_wei = if self.total_amount.contains('.') {
            // Decimal format: parse as f64, then convert to wei
            let amount_f64: f64 = self.total_amount.parse()
                .map_err(|_| anyhow::anyhow!("Invalid decimal amount format: {}", self.total_amount))?;
            
            if amount_f64 < 0.0 {
                return Err(anyhow::anyhow!("Amount cannot be negative: {}", self.total_amount));
            }
            
            let multiplier = 10_f64.powi(token_decimals as i32);
            let amount_wei_f64 = amount_f64 * multiplier;
            
            // Check for overflow
            if amount_wei_f64 > u128::MAX as f64 {
                return Err(anyhow::anyhow!("Amount too large: {}", self.total_amount));
            }
            
            // Convert to U256 (round to nearest integer)
            U256::from(amount_wei_f64.round() as u128)
        } else {
            // Integer format: parse as U256, then multiply by decimals
            U256::from_str(&self.total_amount)?
                .checked_mul(U256::from(10_u64.pow(token_decimals as u32)))
                .ok_or_else(|| anyhow::anyhow!("Amount overflow"))?
        };

        let daily_amount_wei = total_amount_wei
            .checked_div(U256::from(self.duration_days))
            .ok_or_else(|| anyhow::anyhow!("Division by zero"))?;

        let daily_amount_human = total_amount_wei.to_string().parse::<f64>()? 
            / 10_f64.powi(token_decimals as i32) 
            / self.duration_days as f64;
        
        println!("💰 Campaign Details:");
        println!("   Total Amount: {} {}", self.total_amount, token_symbol);
        println!("   Duration: {} days", self.duration_days);
        println!("   Daily Amount: {:.2} {}", daily_amount_human, token_symbol);

        // 4. Validate date range
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

        let days_elapsed = (today - self.start_date).num_days();
        let days_remaining = (self.end_date - today).num_days();
        println!("📅 Date Validation:");
        println!("   Start Date: {}", self.start_date);
        println!("   End Date: {}", self.end_date);
        println!("   Today: {}", today);
        println!("   Days Elapsed: {}", days_elapsed);
        println!("   Days Remaining: {}", days_remaining);

        
        // Check daily amount (required)
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
            .ok_or_else(|| anyhow::anyhow!("Amount overflow when calculating remaining campaign amount"))?;
        
        let keeper_balance_human = keeper_balance.to_string().parse::<f64>()? 
            / 10_f64.powi(token_decimals as i32);
        let remaining_amount_human = remaining_amount_wei.to_string().parse::<f64>()? 
            / 10_f64.powi(token_decimals as i32);
        
        println!("   Keeper Balance: {:.2} {}", keeper_balance_human, token_symbol);
        println!("   Daily Amount Required: {:.2} {}", daily_amount_human, token_symbol);
        println!("   Remaining Campaign Amount Required: {:.2} {} ({} days remaining)", 
                 remaining_amount_human, token_symbol, days_for_remaining_calc);
        
        if keeper_balance < remaining_amount_wei {
            println!("   ⚠️  WARNING: Keeper balance ({:.2} {}) is less than remaining campaign amount ({:.2} {}).", 
                     keeper_balance_human, token_symbol, remaining_amount_human, token_symbol);
            println!("   ⚠️  Campaign will proceed, but may fail on future days if balance is not replenished.");
        } else {
            println!("   ✅ Sufficient balance for remaining campaign duration");
        }

        // 6. Get earn vault address
        let earn_vault_address = self.config.contracts.earn_vault_address.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Earn vault address not configured"))?;
        let earn_vault_addr = Address::from_str(earn_vault_address)?;

        if self.dry_run {
            println!("✅ DRY RUN: Would transfer {} {} to Earn Vault", 
                     daily_amount_human, token_symbol);
            println!("✅ DRY RUN: Would call onBoostReward({}, {})", 
                     self.token_address, daily_amount_wei);
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
        ).await?;

        println!("   Transfer TX: {:?}", transfer_tx);

        // Monitor transfer transaction
        let timeout_gas_used = U256::from_str(&self.config.monitoring.timeout_gas_used)?;
        let monitor = TransactionMonitor::new_with_timeout_values(
            client.provider(),
            Duration::from_secs(self.config.monitoring.transaction_timeout_seconds),
            Duration::from_secs(self.config.monitoring.poll_interval_seconds),
            self.config.monitoring.timeout_block_number,
            timeout_gas_used,
        );

        let transfer_receipt = monitor.monitor_transaction(transfer_tx).await?;
        match transfer_receipt.status {
            TransactionStatus::Success => {
                println!("✅ Transfer confirmed in block {}", transfer_receipt.block_number);
            }
            TransactionStatus::Failed => {
                return Err(anyhow::anyhow!("Token transfer failed"));
            }
            TransactionStatus::Timeout => {
                return Err(anyhow::anyhow!("Token transfer monitoring timeout"));
            }
        }

        // 8. Call onBoostReward
        println!("📞 Calling onBoostReward on Earn Vault...");
        let earn_vault = EarnVaultContract::new(
            earn_vault_addr,
            client.provider(),
            client.clone(),
        );

        let boost_reward_tx = execute_with_retry(
            || {
                let contract = earn_vault.clone();
                let token = self.token_address;
                let amount = daily_amount_wei;
                async move { contract.on_boost_reward(token, amount).await }
            },
            &retry_config,
            "onBoostReward call",
        ).await?;

        println!("   onBoostReward TX: {:?}", boost_reward_tx);

        // Monitor onBoostReward transaction
        let boost_reward_receipt = monitor.monitor_transaction(boost_reward_tx).await?;
        match boost_reward_receipt.status {
            TransactionStatus::Success => {
                println!("✅ onBoostReward confirmed in block {}", boost_reward_receipt.block_number);
                println!("🎉 Distribution completed successfully!");
                println!("   Days Remaining: {}", days_remaining);
            }
            TransactionStatus::Failed => {
                return Err(anyhow::anyhow!("onBoostReward call failed - tokens already transferred"));
            }
            TransactionStatus::Timeout => {
                return Err(anyhow::anyhow!("onBoostReward monitoring timeout - tokens already transferred"));
            }
        }

        Ok(())
    }
}