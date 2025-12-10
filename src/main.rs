mod blockchain;
mod config;
mod contracts;
mod jobs;
mod kms_signer;
mod retry;
mod sources;
mod transaction_monitor;

use anyhow::Result;
use config::ChainConfig;
use jobs::{BoostRewardsJob, ClaimYieldJob, DistributeRewardsJob};

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "vault-keeper")]
#[command(about = "Automated USDSC yield distribution keeper")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    ClaimYield {
        #[arg(long)]
        config: String,

        #[arg(long)]
        kms_key_id: Option<String>,

        #[arg(long)]
        aws_region: Option<String>,

        #[arg(long)]
        dry_run: bool,
    },
    DistributeRewards {
        #[arg(long)]
        config: String,

        #[arg(long)]
        kms_key_id: Option<String>,

        #[arg(long)]
        aws_region: Option<String>,

        #[arg(long)]
        dry_run: bool,
    },
    BoostRewardsDistribute {
        #[arg(long)]
        config: String,

        #[arg(long)]
        token_address: String,

        #[arg(long)]
        total_amount: f64,

        #[arg(long)]
        start_date: String,

        #[arg(long)]
        end_date: String,

        #[arg(long)]
        campaign_id: Option<String>,

        #[arg(long)]
        kms_key_id: Option<String>,

        #[arg(long)]
        aws_region: Option<String>,

        #[arg(long)]
        dry_run: bool,
    },
    BoostRewardsS3 {
        #[arg(long)]
        config: String,
        #[arg(long)]
        campaigns_s3: String, // Format: s3://bucket/key or bucket/key
        #[arg(long)]
        kms_key_id: Option<String>,
        #[arg(long)]
        aws_region: Option<String>, // AWS region for KMS
        #[arg(long)]
        s3_region: Option<String>, // AWS region for S3
    },
}

fn setup_config(
    config_path: &str,
    kms_key_id: Option<String>,
    aws_region: Option<String>,
) -> Result<ChainConfig> {
    let mut chain_config = ChainConfig::load(config_path)?;

    // Override KMS settings from CLI if provided
    if let Some(key_id) = kms_key_id {
        let region = aws_region
            .or_else(|| chain_config.kms.as_ref().and_then(|kms| kms.region.clone()))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "KMS region not specified. Use --aws-region or configure region in {}",
                    config_path
                )
            })?;

        chain_config.kms = Some(crate::config::KmsSettings {
            key_id,
            region: Some(region),
        });
    }

    Ok(chain_config)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ClaimYield {
            config,
            kms_key_id,
            aws_region,
            dry_run,
        } => {
            let chain_config = setup_config(&config, kms_key_id, aws_region)?;
            let job = ClaimYieldJob::new(chain_config, dry_run);
            job.execute().await?;
        }
        Commands::DistributeRewards {
            config,
            kms_key_id,
            aws_region,
            dry_run,
        } => {
            let chain_config = setup_config(&config, kms_key_id, aws_region)?;
            let job = DistributeRewardsJob::new(chain_config, dry_run);
            job.execute().await?;
        }
        Commands::BoostRewardsDistribute {
            config,
            token_address,
            total_amount,
            end_date,
            start_date,
            campaign_id,
            kms_key_id,
            aws_region,
            dry_run,
        } => {
            let chain_config = setup_config(&config, kms_key_id, aws_region)?;
            let job = BoostRewardsJob::new(
                chain_config,
                token_address,
                total_amount,
                start_date,
                end_date,
                campaign_id,
                dry_run,
            )?;
            job.execute().await?;
        }
        Commands::BoostRewardsS3 {
            config,
            campaigns_s3,
            kms_key_id,
            aws_region,
            s3_region,
        } => {
            let chain_config = setup_config(&config, kms_key_id, aws_region)?;

            // Get S3 region: CLI arg -> env var -> KMS region
            let s3_region = s3_region
                .or_else(|| std::env::var("S3_REGION").ok())
                .or_else(|| std::env::var("AWS_REGION").ok())
                .or_else(|| chain_config.kms.as_ref().and_then(|kms| kms.region.clone()))
                .unwrap();

            // Parse S3 path (supports both s3://bucket/key and bucket/key)
            let (bucket, key) = if campaigns_s3.starts_with("s3://") {
                let path = campaigns_s3.strip_prefix("s3://").unwrap();
                let parts: Vec<&str> = path.splitn(2, '/').collect();
                if parts.len() != 2 {
                    return Err(anyhow::anyhow!("Invalid S3 path format: {}", campaigns_s3));
                }
                (parts[0].to_string(), parts[1].to_string())
            } else {
                let parts: Vec<&str> = campaigns_s3.splitn(2, '/').collect();
                if parts.len() != 2 {
                    return Err(anyhow::anyhow!("Invalid S3 path format: {}", campaigns_s3));
                }
                (parts[0].to_string(), parts[1].to_string())
            };

            // Initialize S3 client (same pattern as KMS)
            println!("🔧 Initializing S3 client...");
            println!("   Region: {}", s3_region);
            println!("   Bucket: {}", bucket);
            println!("   Key: {}", key);

            let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
                .region(aws_config::Region::new(s3_region.clone()))
                .load()
                .await;
            let s3_client = aws_sdk_s3::Client::new(&aws_config);

            // Create S3 campaign source
            let campaign_source = Box::new(
                crate::sources::s3_campaign_source::S3CampaignSource::new(s3_client, bucket, key),
            );

            // Run job
            let job =
                crate::jobs::boost_rewards_s3::BoostRewardsS3::new(chain_config, campaign_source);
            job.run().await?;
        }
    }

    Ok(())
}
