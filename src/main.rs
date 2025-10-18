mod config;
mod jobs;
mod blockchain;
mod contracts;
mod retry;
mod transaction_monitor;

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
        private_key: Option<String>,
        
        #[arg(long)]
        dry_run: bool,
    },
    DistributeRewards {
        #[arg(long)]
        chain_id: u64,
        
        #[arg(long)]
        config: String,
        
        #[arg(long)]
        private_key: Option<String>,
        
        #[arg(long)]
        dry_run: bool,
    },
}


#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ClaimYield { chain_id, config, private_key, dry_run } => {
            let mut chain_config = ChainConfig::load(&config)?;
            
            if let Some(pk) = private_key {
                chain_config.chain.private_key = pk;
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
        Commands::DistributeRewards { chain_id, config, private_key, dry_run } => {
            let mut chain_config = ChainConfig::load(&config)?;
            
            if let Some(pk) = private_key {
                chain_config.chain.private_key = pk;
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
