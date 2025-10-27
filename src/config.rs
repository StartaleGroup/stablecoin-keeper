use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::fs;
use std::env;
use regex::Regex;
use toml::map::Map;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChainConfig {
    pub chain: ChainSettings,
    pub contracts: ContractAddresses,
    pub thresholds: Thresholds,
    pub retry: RetrySettings,
    pub monitoring: MonitoringSettings,
    pub transaction: TransactionSettings,
    pub kms: Option<KmsSettings>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChainSettings {
    pub chain_id: u64,
    pub rpc_url: String,
    pub rpc_backup_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContractAddresses {
    pub usdsc_address: String,
    pub recipient_address: Option<String>,
    pub reward_redistributor_address: Option<String>,
    pub earn_vault_address: Option<String>,
    pub susdsc_vault_address: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Thresholds {
    pub min_yield_threshold: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetrySettings {
    pub max_attempts: u32,
    pub base_delay_seconds: u64,
    pub max_delay_seconds: u64,
    pub backoff_multiplier: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MonitoringSettings {
    pub transaction_timeout_seconds: u64,
    pub poll_interval_seconds: u64,
    pub timeout_block_number: u64,
    pub timeout_gas_used: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransactionSettings {
    pub value_wei: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KmsSettings {
    pub key_id: String,
    pub region: Option<String>,
}

impl ChainConfig {
    pub fn load(path: &str) -> Result<Self> {
        // Load .env file if it exists
        dotenv::dotenv().ok();
        
        // Load common config first
        let common_content = Self::load_common_config()?;
        
        // Load specific config
        let specific_content = fs::read_to_string(path)?;
        
        // Merge common and specific configs
        let merged_content = Self::merge_configs(common_content, specific_content)?;
        
        // Simple environment variable substitution
        let content = Self::substitute_env_vars(merged_content)?;
        
        let config: ChainConfig = toml::from_str(&content)?;
        Ok(config)
    }
    
    fn load_common_config() -> Result<String> {
        let common_path = "configs/common.toml";
        match fs::read_to_string(common_path) {
            Ok(content) => Ok(content),
            Err(_) => {
                // If common.toml doesn't exist, return empty config
                Ok(String::new())
            }
        }
    }
    
    fn merge_configs(common: String, specific: String) -> Result<String> {
        if common.is_empty() {
            return Ok(specific);
        }
        
        // Parse both configs and merge them properly
        let common_toml: toml::Value = toml::from_str(&common)?;
        let specific_toml: toml::Value = toml::from_str(&specific)?;
        
        // Merge specific config into common config (specific overrides common)
        let merged = Self::merge_toml_values(common_toml, specific_toml);
        
        // Convert back to TOML string
        let merged_toml = toml::to_string_pretty(&merged)?;
        Ok(merged_toml)
    }
    
    fn merge_toml_values(mut base: toml::Value, override_val: toml::Value) -> toml::Value {
        match (&mut base, override_val) {
            (toml::Value::Table(base_map), toml::Value::Table(override_map)) => {
                for (key, value) in override_map {
                    base_map.insert(key.clone(), Self::merge_toml_values(
                        base_map.get(&key).cloned().unwrap_or(toml::Value::Table(Map::new())),
                        value
                    ));
                }
                base
            }
            (_, override_val) => override_val,
        }
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