mod blockchain;
mod config;
mod contracts;
mod jobs;
mod kms_signer;
mod retry;
mod transaction_monitor;

use anyhow::Result;
use config::ChainConfig;
use jobs::{ClaimYieldJob, DistributeRewardsJob, BoostRewardsJob};

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
        total_amount: String,
        
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
        },
        Commands::DistributeRewards {
            config,
            kms_key_id,
            aws_region,
            dry_run,
        } => {
            let chain_config = setup_config(&config, kms_key_id, aws_region)?;
            let job = DistributeRewardsJob::new(chain_config, dry_run);
            job.execute().await?;
        },
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
            let job = BoostRewardsJob::new(chain_config, token_address, total_amount, start_date, end_date, campaign_id, dry_run)?;
            job.execute().await?;
        },
    }

    Ok(())
}
