mod config;
mod jobs;
mod blockchain;
mod contracts;
mod retry;
mod transaction_monitor;
mod kms_signer;

use config::ChainConfig;
use jobs::{ClaimYieldJob, DistributeRewardsJob};
use anyhow::Result;

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
        chain_id: u64,
        
        #[arg(long)]
        config: String,
        
        #[arg(long)]
        kms_key_id: Option<String>,
        
        #[arg(long)]
        kms_region: Option<String>,
        
        #[arg(long)]
        dry_run: bool,
    },
    DistributeRewards {
        #[arg(long)]
        chain_id: u64,
        
        #[arg(long)]
        config: String,
        
        #[arg(long)]
        kms_key_id: Option<String>,
        
        #[arg(long)]
        kms_region: Option<String>,
        
        #[arg(long)]
        dry_run: bool,
    },
}


fn setup_config(chain_id: u64, config_path: &str, kms_key_id: Option<String>, kms_region: Option<String>) -> Result<ChainConfig> {
    let mut chain_config = ChainConfig::load(config_path)?;
    
    // Override KMS settings from CLI if provided
    if let Some(key_id) = kms_key_id {
        let region = kms_region
            .or_else(|| chain_config.kms.as_ref().and_then(|kms| kms.region.clone()))
            .unwrap_or_else(|| "ap-northeast-1".to_string());
            
        chain_config.kms = Some(crate::config::KmsSettings {
            key_id,
            region: Some(region),
        });
    }

    if chain_config.chain.chain_id != chain_id {
        return Err(anyhow::anyhow!(
            "Chain ID mismatch: CLI={}, Config={}", 
            chain_id, chain_config.chain.chain_id
        ));
    }
    
    Ok(chain_config)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ClaimYield { chain_id, config, kms_key_id, kms_region, dry_run } => {
            let chain_config = setup_config(chain_id, &config, kms_key_id, kms_region)?;
            let job = ClaimYieldJob::new(chain_config, dry_run);
            job.execute().await?;
        }
        Commands::DistributeRewards { chain_id, config, kms_key_id, kms_region, dry_run } => {
            let chain_config = setup_config(chain_id, &config, kms_key_id, kms_region)?;
            let job = DistributeRewardsJob::new(chain_config, dry_run);
            job.execute().await?;
        }
    }
    
    Ok(())
}
