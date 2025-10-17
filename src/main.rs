mod config;
mod jobs;
mod blockchain;
mod contracts;
mod retry;

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
        dry_run: bool,
    },
    DistributeRewards {
        #[arg(long)]
        chain_id: u64,
        
        #[arg(long)]
        config: String,
        
        #[arg(long)]
        dry_run: bool,
    },
}


#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    let cli = Cli::parse();

    match cli.command {
        Commands::ClaimYield { chain_id, config, dry_run } => {
            println!("ClaimYield command: chain_id={}, config={}, dry_run={}", 
                     chain_id, config, dry_run);
            let chain_config = ChainConfig::load(&config)?;
            

            // Validate chain_id matches config
            if chain_config.chain.chain_id != chain_id {
                return Err(anyhow::anyhow!(
                    "Chain ID mismatch: CLI={}, Config={}", 
                    chain_id, chain_config.chain.chain_id
                ));
            }
            println!("ClaimYield: Ready to execute on chain {}", chain_id);
            let job = ClaimYieldJob::new(chain_config, dry_run);
            job.execute().await?;
        }
        Commands::DistributeRewards { chain_id, config, dry_run } => {
            println!("Loading config from: {}", config);
            let chain_config = ChainConfig::load(&config)?;
            
            // Validate chain_id matches config
            if chain_config.chain.chain_id != chain_id {
                return Err(anyhow::anyhow!(
                    "Chain ID mismatch: CLI={}, Config={}", 
                    chain_id, chain_config.chain.chain_id
                ));
            }
            
            println!("DistributeRewards: Ready to execute on chain {}", chain_id);
            let job = DistributeRewardsJob::new(chain_config, dry_run);
            job.execute().await?;
        }
    }
    
    Ok(())
}
