use crate::config::ChainConfig;
use crate::jobs::boost_rewards::{BoostRewardsJob, CampaignConfig, CampaignConfigSource};
use anyhow::Result;
use chrono::{NaiveDate, NaiveTime, Timelike, Utc};
use std::time::Duration;

// Service that runs continuously and processes campaigns from S3
pub struct BoostRewardsService {
    config: ChainConfig,
    campaign_source: Box<dyn CampaignConfigSource>,
    delay_between_campaigns: Duration,
    execution_time: (u32, u32), // (hour, minute) in UTC
}

impl BoostRewardsService {
    pub fn new(
        config: ChainConfig,
        campaign_source: Box<dyn CampaignConfigSource>,
        _poll_interval_seconds: u64, // Reserved for future use (e.g., S3 polling frequency)
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
        let time = NaiveTime::parse_from_str(time_str, "%H:%M").map_err(|_| {
            anyhow::anyhow!(
                "Invalid execution_time format: '{}'. Expected 'HH:MM' (e.g., '12:00')",
                time_str
            )
        })?;
        Ok((time.hour(), time.minute()))
    }

    pub async fn run(&self) -> Result<()> {
        self.run_with_test_mode(false).await
    }

    pub async fn run_with_test_mode(&self, test_mode: bool) -> Result<()> {
        if test_mode {
            println!("🧪 Boost Rewards Service Starting in TEST MODE...");
        } else {
            println!("🚀 Boost Rewards Service Starting...");
            println!(
                "   Service will run daily at {:02}:{:02} UTC",
                self.execution_time.0, self.execution_time.1
            );
        }

        loop {
            if !test_mode {
                let current_time = Utc::now();

                // Calculate next run time (00:00 UTC of next day)
                let next_run = self.calculate_next_run_time(current_time);
                let wait_duration = next_run.signed_duration_since(current_time);

                if wait_duration.num_seconds() > 0 {
                    println!("⏰ Next run scheduled for: {}", next_run);
                    println!("   Waiting {} seconds...", wait_duration.num_seconds());
                    tokio::time::sleep(Duration::from_secs(wait_duration.num_seconds() as u64))
                        .await;
                }
            }

            // Recalculate today after wait (in case we crossed midnight)
            let today = Utc::now().date_naive();

            // Process campaigns
            let execution_result = self.process_campaigns_for_today(today).await;
            match &execution_result {
                Ok(_) => {
                    println!("✅ Campaigns processed successfully for {}", today);
                    // TODO: Send success metrics to monitoring system (e.g., CloudWatch, Prometheus)
                }
                Err(e) => {
                    eprintln!("❌ Error processing campaigns: {}", e);
                    // TODO: Send failure metrics to monitoring system
                    // TODO: Consider alerting mechanism for repeated failures
                }
            }

            // Log execution status for monitoring
            let status = if execution_result.is_ok() {
                "success"
            } else {
                "failure"
            };
            println!("📊 Execution status for {}: {}", today, status);

            // In test mode, exit after one run
            if test_mode {
                println!("🧪 Test mode: Exiting after one run");
                break;
            }
        }

        Ok(())
    }

    fn calculate_next_run_time(&self, now: chrono::DateTime<Utc>) -> chrono::DateTime<Utc> {
        let today = now.date_naive();
        let execution_time =
            chrono::NaiveTime::from_hms_opt(self.execution_time.0, self.execution_time.1, 0)
                .expect("Invalid execution time"); // Should never fail as we validate in new()

        // Check if today's execution time has passed
        let today_execution = chrono::NaiveDateTime::new(today, execution_time);
        let today_execution_utc =
            chrono::DateTime::from_naive_utc_and_offset(today_execution, chrono::Utc);

        if now < today_execution_utc {
            // Today's execution time hasn't passed yet, schedule for today
            today_execution_utc
        } else {
            // Today's execution time has passed, schedule for tomorrow
            let tomorrow = today + chrono::Duration::days(1);
            let tomorrow_execution = chrono::NaiveDateTime::new(tomorrow, execution_time);
            chrono::DateTime::from_naive_utc_and_offset(tomorrow_execution, chrono::Utc)
        }
    }

    async fn process_campaigns_for_today(&self, today: NaiveDate) -> Result<()> {
        println!("📅 Processing campaigns for date: {}", today);

        let all_campaigns = self.campaign_source.get_campaigns().await?;
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
