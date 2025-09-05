use anyhow::Result;
use std::env;

pub struct Config {
    pub database_url: String,
    pub rpc_url: String,
    pub server_port: u16,
    // TODO: Add more config fields as needed
}

impl Config {
    pub fn load() -> Result<Self> {
        Ok(Config {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://localhost/stablecoin".to_string()),
            rpc_url: env::var("RPC_URL")
                .unwrap_or_else(|_| "http://localhost:8545".to_string()),
            server_port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()?,
        })
    }
}
