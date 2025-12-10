use crate::config::ChainConfig;
use crate::jobs::boost_rewards::{BoostRewardsJob, CampaignConfig, CampaignConfigSource};
use anyhow::Result;
use chrono::{NaiveDate, NaiveTime, Timelike, Utc};
use std::time::Duration;

// CronJob that processes boost reward campaigns from S3
// Designed to run hourly (e.g., `0 * * * *`) and process campaigns if execution time has passed
pub struct BoostRewardsS3 {
    config: ChainConfig,
    campaign_source: Box<dyn CampaignConfigSource>,
    delay_between_campaigns: Duration,
    execution_time: (u32, u32), // (hour, minute) in UTC
}

impl BoostRewardsS3 {
    pub fn new(
        config: ChainConfig,
        campaign_source: Box<dyn CampaignConfigSource>,
        execution_time: Option<String>, // Optional: "HH:MM" format, defaults to "12:00"
    ) -> Result<Self> {
        // Parse execution_time or use default
        let (hour, minute) = if let Some(time_str) = execution_time {
            Self::parse_execution_time(&time_str)?
        } else {
            (12, 0) // Default: 12:00 PM UTC
        };

        Ok(Self {
            config,
            campaign_source,
            delay_between_campaigns: Duration::from_secs(30), // Default: 30 seconds between campaigns
            execution_time: (hour, minute),
        })
    }

    fn parse_execution_time(time_str: &str) -> Result<(u32, u32)> {
        let time = NaiveTime::parse_from_str(time_str, "%H:%M").map_err(|e| {
            anyhow::anyhow!(
                "Invalid execution_time format: '{}'. Expected 'HH:MM' (e.g., '12:00'). Error: {}",
                time_str,
                e
            )
        })?;
        Ok((time.hour(), time.minute()))
    }

    pub async fn run(&self) -> Result<()> {
        self.run_with_test_mode(false).await
    }

    pub async fn run_with_test_mode(&self, test_mode: bool) -> Result<()> {
        let current_time = Utc::now();
        let today = current_time.date_naive();
        let current_hour = current_time.hour();
        let current_minute = current_time.minute();

        // Always scan S3 to detect new campaigns (even if not processing yet)
        // This allows campaigns added after execution time to be detected on next hourly run
        println!("📡 Scanning S3 for campaigns...");
        let all_campaigns = self.campaign_source.get_campaigns().await?;
        println!("   Found {} total campaigns in S3", all_campaigns.len());

        let execution_result = if test_mode {
            println!("🧪 Boost Rewards Service Starting in TEST MODE...");
            println!("   Will process campaigns immediately (no time check)");

            // Test mode: process campaigns
            self.process_campaigns_for_today_with_campaigns(today, all_campaigns)
                .await
        } else {
            println!("🚀 Boost Rewards Service Starting (CronJob mode)...");
            println!(
                "   Execution time: {:02}:{:02} UTC",
                self.execution_time.0, self.execution_time.1
            );
            println!(
                "   Current time: {:02}:{:02} UTC",
                current_hour, current_minute
            );

            // Check if it's time to process (current time >= execution time for today)
            let should_process = self.should_process_now(current_time);

            if !should_process {
                println!(
                    "⏭️  Skipping processing: Current time ({:02}:{:02}) is before execution time ({:02}:{:02})",
                    current_hour, current_minute,
                    self.execution_time.0, self.execution_time.1
                );
                println!("   Will process on next run when execution time has passed");
                return Ok(());
            }

            // Process campaigns for today (using already-fetched campaigns)
            self.process_campaigns_for_today_with_campaigns(today, all_campaigns)
                .await
        };

        // Handle execution result
        match &execution_result {
            Ok(_) => {
                println!("✅ Campaigns processed successfully for {}", today);
            }
            Err(e) => {
                eprintln!("❌ Error processing campaigns: {}", e);
            }
        }

        // Log execution status for monitoring
        let status = if execution_result.is_ok() {
            "success"
        } else {
            "failure"
        };
        println!("📊 Execution status for {}: {}", today, status);

        execution_result
    }

    /// Check if current time is at or past the execution time for today
    /// Note: In CronJob mode, this ensures we only process once per day
    /// (when the execution hour arrives, subsequent hourly runs will skip)
    fn should_process_now(&self, now: chrono::DateTime<Utc>) -> bool {
        let current_hour = now.hour();
        let current_minute = now.minute();
        let execution_hour = self.execution_time.0;
        let execution_minute = self.execution_time.1;

        // Only process if we're in the execution hour and at or past the execution minute
        // This ensures we process once per day when the execution time arrives
        if current_hour == execution_hour {
            return current_minute >= execution_minute;
        }

        // If we're past the execution hour, we've already processed (or missed the window)
        // Don't process again to avoid duplicate processing
        // Note: If execution time is missed (e.g., CronJob was down), it will process
        // on the next day's execution hour
        false
    }

    async fn process_campaigns_for_today_with_campaigns(
        &self,
        today: NaiveDate,
        all_campaigns: Vec<CampaignConfig>,
    ) -> Result<()> {
        println!("📅 Processing campaigns for date: {}", today);
        println!("   Found {} total campaigns", all_campaigns.len());

        // Filter and collect active campaigns for today
        let mut active_campaigns: Vec<_> = all_campaigns
            .into_iter()
            .filter(|x| x.is_active_for_date(today))
            .collect();

        println!(
            "   Found {} active campaigns for today",
            active_campaigns.len()
        );

        if active_campaigns.is_empty() {
            println!("   No active campaigns, skipping...");
            return Ok(());
        }

        // Sort campaigns by start date (earliest first)
        active_campaigns.sort_by_key(|x| x.start_date);

        // Process each campaign sequentially
        for (index, campaign) in active_campaigns.iter().enumerate() {
            // Add delay before processing (except for the first campaign)
            if index > 0 {
                println!(
                    "   ⏸️  Waiting {} seconds before next campaign...",
                    self.delay_between_campaigns.as_secs()
                );
                tokio::time::sleep(self.delay_between_campaigns).await;
            }

            println!(
                "🎯 Processing campaign: {} ({}/{})",
                campaign.id,
                index + 1,
                active_campaigns.len()
            );
            match self.process_single_campaign(campaign).await {
                Ok(_) => println!("   ✅ Campaign {} completed successfully", campaign.id),
                Err(e) => {
                    eprintln!("   ❌ Campaign {} failed: {}", campaign.id, e);
                    // Continue with next campaign
                }
            }
        }

        Ok(())
    }

    async fn process_single_campaign(&self, campaign: &CampaignConfig) -> Result<()> {
        let job =
            BoostRewardsJob::from_campaign_config(self.config.clone(), campaign.clone(), false)?;

        job.execute().await
    }
}
