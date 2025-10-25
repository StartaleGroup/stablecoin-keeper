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
        dry_run: bool,
    },
}


#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ClaimYield { chain_id, config, kms_key_id, dry_run } => {
            let mut chain_config = ChainConfig::load(&config)?;
            
            // Override KMS key ID if provided via CLI
            if let Some(key_id) = kms_key_id {
                chain_config.kms = Some(crate::config::KmsSettings {
                    key_id,
                    region: None,
                });
            }

            if chain_config.chain.chain_id != chain_id {
                return Err(anyhow::anyhow!(
                    "Chain ID mismatch: CLI={}, Config={}", 
                    chain_id, chain_config.chain.chain_id
                ));
            }
            
            let job = ClaimYieldJob::new(chain_config, dry_run);
            job.execute().await?;
        }
        Commands::DistributeRewards { chain_id, config, kms_key_id, dry_run } => {
            let mut chain_config = ChainConfig::load(&config)?;
            
            // Override KMS key ID if provided via CLI
            if let Some(key_id) = kms_key_id {
                chain_config.kms = Some(crate::config::KmsSettings {
                    key_id,
                    region: None,
                });
            }
            
            if chain_config.chain.chain_id != chain_id {
                return Err(anyhow::anyhow!(
                    "Chain ID mismatch: CLI={}, Config={}", 
                    chain_id, chain_config.chain.chain_id
                ));
            }
            
            let job = DistributeRewardsJob::new(chain_config, dry_run);
            job.execute().await?;
        }
    }
    
    Ok(())
}
