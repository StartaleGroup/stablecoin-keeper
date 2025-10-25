use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::fs;
use std::env;
use regex::Regex;

#[derive(Debug, Deserialize, Serialize)]
pub struct ChainConfig {
    pub chain: ChainSettings,
    pub contracts: ContractAddresses,
    pub thresholds: Thresholds,
    pub retry: RetrySettings,
    pub monitoring: MonitoringSettings,
    pub transaction: TransactionSettings,
    pub kms: Option<KmsSettings>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ChainSettings {
    pub chain_id: u64,
    pub rpc_url: String,
    pub rpc_backup_url: Option<String>,
    pub private_key: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ContractAddresses {
    pub usdsc_address: String,
    pub recipient_address: Option<String>,
    pub reward_redistributor_address: Option<String>,
    pub earn_vault_address: Option<String>,
    pub susdsc_vault_address: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Thresholds {
    pub min_yield_threshold: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RetrySettings {
    pub max_attempts: u32,
    pub base_delay_seconds: u64,
    pub max_delay_seconds: u64,
    pub backoff_multiplier: f64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MonitoringSettings {
    pub transaction_timeout_seconds: u64,
    pub poll_interval_seconds: u64,
    pub timeout_block_number: u64,
    pub timeout_gas_used: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TransactionSettings {
    pub value_wei: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct KmsSettings {
    pub key_id: String,
    pub region: Option<String>,
}

impl ChainConfig {
    pub fn load(path: &str) -> Result<Self> {
        // Load .env file if it exists
        dotenv::dotenv().ok();
        
        let content = fs::read_to_string(path)?;
        
        // Simple environment variable substitution
        let content = Self::substitute_env_vars(content)?;
        
        let config: ChainConfig = toml::from_str(&content)?;
        Ok(config)
    }
    
    fn substitute_env_vars(content: String) -> Result<String> {
        let re = Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)\}")?;
        let mut result = content.clone();
        
        for cap in re.captures_iter(&content) {
            let var_name = &cap[1];
            if let Ok(value) = env::var(var_name) {
                let placeholder = cap[0].to_string();
                result = result.replace(&placeholder, &value);
            }
        }
        
        Ok(result)
    }
}