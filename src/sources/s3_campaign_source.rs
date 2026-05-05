use crate::jobs::boost_rewards::{CampaignConfig, CampaignConfigSource, CampaignStatus};
use alloy::primitives::Address;
use anyhow::Result;
use aws_sdk_s3::Client as S3Client;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use toml;

#[derive(Debug, Deserialize, Serialize)]
struct S3CampaignsConfig {
    campaigns: Vec<S3Campaign>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct S3Campaign {
    id: String,
    token_address: String,
    total_amount: f64,
    start_date: String,
    end_date: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_distribution_date: Option<String>,
}

pub struct S3CampaignSource {
    s3_client: S3Client,
    bucket: String,
    key: String,
}

impl S3CampaignSource {
    pub fn new(s3_client: S3Client, bucket: String, key: String) -> Self {
        Self {
            s3_client,
            bucket,
            key,
        }
    }
}

#[async_trait::async_trait]
impl CampaignConfigSource for S3CampaignSource {
    async fn get_campaigns(&self) -> Result<Vec<CampaignConfig>> {
        // Get object from S3
        let response = self
            .s3_client
            .get_object()
            .bucket(&self.bucket)
            .key(&self.key)
            .send()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to get S3 object from s3://{}/{}: {}\n\
                    💡 Troubleshooting:\n\
                    - Check AWS credentials are configured (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, or AWS_PROFILE)\n\
                    - Verify bucket '{}' exists and is accessible\n\
                    - Verify key '{}' exists in the bucket\n\
                    - Check AWS_REGION environment variable matches bucket region\n\
                    - Ensure IAM user/role has s3:GetObject permission",
                    self.bucket,
                    self.key,
                    e,
                    self.bucket,
                    self.key
                )
            })?;

        // Read body
        let bytes = response
            .body
            .collect()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read S3 body: {}", e))?;

        let content = String::from_utf8(bytes.to_vec())
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in S3 object: {}", e))?;

        // Parse TOML
        let config: S3CampaignsConfig =
            toml::from_str(&content).map_err(|e: toml::de::Error| {
                anyhow::anyhow!("Failed to parse S3 config TOML: {}", e)
            })?;

        // Convert to CampaignConfig
        let mut campaigns = Vec::new();
        for s3_campaign in config.campaigns {
            let campaign_id = s3_campaign.id.clone(); // Clone for error messages
            let status = match s3_campaign.status.as_str() {
                "active" => CampaignStatus::Active,
                "paused" => CampaignStatus::Paused,
                "completed" => CampaignStatus::Completed,
                _ => {
                    return Err(anyhow::anyhow!(
                        "Invalid campaign status: {}",
                        s3_campaign.status
                    ));
                }
            };

            let start_date = NaiveDate::parse_from_str(&s3_campaign.start_date, "%Y-%m-%d")
                .map_err(|e| {
                    anyhow::anyhow!(
                        "Invalid start_date format for campaign {}: {} (expected YYYY-MM-DD)",
                        campaign_id,
                        e
                    )
                })?;
            let end_date =
                NaiveDate::parse_from_str(&s3_campaign.end_date, "%Y-%m-%d").map_err(|e| {
                    anyhow::anyhow!(
                        "Invalid end_date format for campaign {}: {} (expected YYYY-MM-DD)",
                        campaign_id,
                        e
                    )
                })?;

            // Validate date range
            if end_date <= start_date {
                return Err(anyhow::anyhow!(
                    "Invalid date range for campaign {}: end_date ({}) must be after start_date ({})",
                    campaign_id,
                    end_date,
                    start_date
                ));
            }

            // Validate total_amount is positive
            if s3_campaign.total_amount <= 0.0 {
                return Err(anyhow::anyhow!(
                    "Invalid total_amount for campaign {}: must be positive, got {}",
                    campaign_id,
                    s3_campaign.total_amount
                ));
            }

            let last_distribution_date = if let Some(date_str) = &s3_campaign.last_distribution_date
            {
                Some(
                    NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|e| {
                        anyhow::anyhow!(
                            "Invalid last_distribution_date format for campaign {}: {} (expected YYYY-MM-DD)",
                            campaign_id,
                            e
                        )
                    })?,
                )
            } else {
                None
            };

            campaigns.push(CampaignConfig {
                id: s3_campaign.id,
                token_address: Address::from_str(&s3_campaign.token_address).map_err(|e| {
                    anyhow::anyhow!("Invalid token_address for campaign {}: {}", campaign_id, e)
                })?,
                total_amount: s3_campaign.total_amount,
                start_date,
                end_date,
                status,
                last_distribution_date,
            });
        }

        Ok(campaigns)
    }

    async fn update_campaign_state(
        &self,
        campaign_id: &str,
        last_distribution_date: Option<NaiveDate>,
        status: Option<CampaignStatus>,
    ) -> Result<CampaignConfig> {
        // Get current campaigns
        let mut campaigns = self.get_campaigns().await?;

        // Find and update the campaign
        let campaign = campaigns
            .iter_mut()
            .find(|c| c.id == campaign_id)
            .ok_or_else(|| anyhow::anyhow!("Campaign not found: {}", campaign_id))?;

        // Update state fields (runtime values, not configs)
        if let Some(date) = last_distribution_date {
            campaign.last_distribution_date = Some(date);
        }
        if let Some(new_status) = status {
            campaign.status = new_status;
        }

        // Save to S3
        self.save_campaigns_to_s3(&campaigns).await?;

        // Return the updated campaign
        campaigns
            .into_iter()
            .find(|c| c.id == campaign_id)
            .ok_or_else(|| anyhow::anyhow!("Campaign not found after update: {}", campaign_id))
    }
}

// Helper methods for S3CampaignSource (not part of the trait)
impl S3CampaignSource {
    // Helper method to save campaigns to S3 (shared by both update methods)
    async fn save_campaigns_to_s3(&self, campaigns: &[CampaignConfig]) -> Result<()> {
        // Convert back to S3 format
        let s3_campaigns: Vec<S3Campaign> = campaigns
            .iter()
            .map(|c| S3Campaign {
                id: c.id.clone(),
                token_address: format!("{:#x}", c.token_address),
                total_amount: c.total_amount,
                start_date: c.start_date.format("%Y-%m-%d").to_string(),
                end_date: c.end_date.format("%Y-%m-%d").to_string(),
                status: match c.status {
                    CampaignStatus::Active => "active".to_string(),
                    CampaignStatus::Paused => "paused".to_string(),
                    CampaignStatus::Completed => "completed".to_string(),
                },
                last_distribution_date: c
                    .last_distribution_date
                    .map(|d| d.format("%Y-%m-%d").to_string()),
            })
            .collect();

        let config = S3CampaignsConfig {
            campaigns: s3_campaigns,
        };

        // Serialize to TOML
        let toml_content = toml::to_string_pretty(&config)
            .map_err(|e| anyhow::anyhow!("Failed to serialize campaigns to TOML: {}", e))?;

        // Upload to S3
        self.s3_client
            .put_object()
            .bucket(&self.bucket)
            .key(&self.key)
            .body(aws_sdk_s3::primitives::ByteStream::from(
                toml_content.into_bytes(),
            ))
            .content_type("text/plain")
            .send()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to update S3 object at s3://{}/{}: {}\n\
                    💡 Troubleshooting:\n\
                    - Check AWS credentials are configured\n\
                    - Verify IAM user/role has s3:PutObject permission",
                    self.bucket,
                    self.key,
                    e
                )
            })?;

        Ok(())
    }
}
