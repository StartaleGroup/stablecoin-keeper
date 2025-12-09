use crate::jobs::boost_rewards::{CampaignConfig, CampaignConfigSource, CampaignStatus};
use alloy::primitives::Address;
use anyhow::Result;
use aws_sdk_s3::Client as S3Client;
use chrono::{Duration, NaiveDate, Utc};
use serde::Deserialize;
use std::str::FromStr;
use toml;

#[derive(Debug, Deserialize)]
struct S3CampaignsConfig {
    campaigns: Vec<S3Campaign>,
}

#[derive(Debug, Deserialize)]
struct S3Campaign {
    id: String,
    token_address: String,
    total_amount: f64,
    start_date: String,
    end_date: String,
    status: String,
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
        let config: S3CampaignsConfig = toml::from_str(&content)
            .map_err(|e: toml::de::Error| anyhow::anyhow!("Failed to parse S3 config TOML: {}", e))?;

        // Convert to CampaignConfig
        let mut campaigns = Vec::new();
        for s3_campaign in config.campaigns {
            let campaign_id = s3_campaign.id.clone(); // Clone for error messages
            let status = match s3_campaign.status.as_str() {
                "active" => CampaignStatus::Active,
                "paused" => CampaignStatus::Paused,
                "completed" => CampaignStatus::Completed,
                _ => return Err(anyhow::anyhow!("Invalid campaign status: {}", s3_campaign.status)),
            };

            let start_date = NaiveDate::parse_from_str(&s3_campaign.start_date, "%Y-%m-%d")
                .map_err(|e| anyhow::anyhow!("Invalid start_date format for campaign {}: {} (expected YYYY-MM-DD)", campaign_id, e))?;
            let end_date = NaiveDate::parse_from_str(&s3_campaign.end_date, "%Y-%m-%d")
                .map_err(|e| anyhow::anyhow!("Invalid end_date format for campaign {}: {} (expected YYYY-MM-DD)", campaign_id, e))?;

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

            campaigns.push(CampaignConfig {
                id: s3_campaign.id,
                token_address: Address::from_str(&s3_campaign.token_address)
                    .map_err(|e| anyhow::anyhow!("Invalid token_address for campaign {}: {}", campaign_id, e))?,
                total_amount: s3_campaign.total_amount,
                start_date,
                end_date,
                status,
            });
        }

        Ok(campaigns)
    }
}

