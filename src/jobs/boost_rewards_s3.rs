use crate::config::ChainConfig;
use crate::jobs::boost_rewards::CampaignStatus;
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
    pub fn new(config: ChainConfig, campaign_source: Box<dyn CampaignConfigSource>) -> Self {
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
        let execution_result = self.process_campaigns_for_today(today, all_campaigns).await;

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

    async fn process_campaigns_for_today(
        &self,
        today: NaiveDate,
        all_campaigns: Vec<CampaignConfig>,
    ) -> Result<()> {
        println!("📅 Processing campaigns for date: {}", today);
        println!("   Found {} total campaigns", all_campaigns.len());

        // Filter and collect active campaigns for today
        let active_campaigns: Vec<_> = all_campaigns
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

        // Filter out campaigns that were already processed today (idempotency check)
        let mut campaigns_to_process: Vec<_> = active_campaigns
            .iter()
            .filter(|campaign| {
                if let Some(last_date) = campaign.last_distribution_date {
                    if last_date >= today {
                        println!(
                            "   ⏭️  Skipping campaign {} - already processed on {}",
                            campaign.id, last_date
                        );
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        println!(
            "   Found {} campaigns to process (after idempotency check)",
            campaigns_to_process.len()
        );

        if campaigns_to_process.is_empty() {
            println!("   All campaigns already processed today, skipping...");
            return Ok(());
        }

        // Sort campaigns by start date (earliest first)
        campaigns_to_process.sort_by_key(|x| x.start_date);

        // Process each campaign sequentially
        for (index, campaign) in campaigns_to_process.iter().enumerate() {
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
                campaigns_to_process.len()
            );

            let is_last_day = today >= campaign.end_date;
            match self
                .process_single_campaign(campaign, today, is_last_day)
                .await
            {
                Ok(_) => {
                    println!("   ✅ Campaign {} completed successfully", campaign.id);
                }
                Err(e) => {
                    eprintln!("   ❌ Campaign {} failed: {}", campaign.id, e);
                    // Continue with next campaign
                }
            }
        }

        Ok(())
    }

    async fn process_single_campaign(
        &self,
        campaign: &CampaignConfig,
        today: NaiveDate,
        is_last_day: bool,
    ) -> Result<()> {
        let job =
            BoostRewardsJob::from_campaign_config(self.config.clone(), campaign.clone(), false)?;

        // Execute the distribution
        job.execute().await?;

        // Update S3 with last_distribution_date
        // Only update status to "completed" on the last day; otherwise keep existing status
        let new_status = if is_last_day {
            println!(
                "   📝 Last day of campaign {} processed, marking as completed",
                campaign.id
            );
            Some(CampaignStatus::Completed)
        } else {
            // None means don't change the status (preserves "active" or "paused")
            None
        };

        self.campaign_source
            .update_campaign(&campaign.id, Some(today), new_status)
            .await?;

        println!(
            "   ✅ Updated campaign {} in S3: last_distribution_date = {}",
            campaign.id, today
        );

        Ok(())
    }
}
