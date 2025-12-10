use crate::config::ChainConfig;
use crate::jobs::boost_rewards::{BoostRewardsJob, CampaignConfig, CampaignConfigSource};
use anyhow::Result;
use chrono::{NaiveDate, Utc};
use std::time::Duration;

// CronJob that processes boost reward campaigns from S3
// Designed to run once daily (e.g., `0 12 * * *` for 12:00 UTC daily)
pub struct BoostRewardsS3 {
    config: ChainConfig,
    campaign_source: Box<dyn CampaignConfigSource>,
    delay_between_campaigns: Duration,
}

impl BoostRewardsS3 {
    pub fn new(
        config: ChainConfig,
        campaign_source: Box<dyn CampaignConfigSource>,
    ) -> Self {
        Self {
            config,
            campaign_source,
            delay_between_campaigns: Duration::from_secs(30), // Default: 30 seconds between campaigns
        }
    }

    pub async fn run(&self) -> Result<()> {
        let today = Utc::now().date_naive();

        println!("🚀 Boost Rewards Service Starting (Daily CronJob)...");

        // Scan S3 for campaigns
        println!("📡 Scanning S3 for campaigns...");
        let all_campaigns = self.campaign_source.get_campaigns().await?;
        println!("   Found {} total campaigns in S3", all_campaigns.len());

        // Process campaigns for today
        let execution_result = self
            .process_campaigns_for_today_with_campaigns(today, all_campaigns)
            .await;

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

