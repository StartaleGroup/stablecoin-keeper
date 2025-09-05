mod config;
mod database;
mod keeper;
mod api;

use anyhow::Result;
use dotenv::dotenv;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting Stablecoin Backend Service");

    // TODO: Initialize services
    // let config = config::load()?;
    // let database = database::connect(&config.database_url).await?;
    // let keeper = keeper::KeeperService::new(config, database).await?;
    // let api_server = api::Server::new(config, database);

    // TODO: Start services
    println!("Backend service skeleton ready!");
    println!("Add your implementation in the respective modules:");
    println!("- config.rs: Configuration management");
    println!("- database.rs: Database operations");
    println!("- keeper.rs: Keeper bots");
    println!("- api.rs: REST API");

    Ok(())
}
