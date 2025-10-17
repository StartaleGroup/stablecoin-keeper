use alloy::network::Ethereum;
use alloy::primitives::Address;
use alloy::providers::{Provider, RootProvider};
use alloy::rpc::client::RpcClient;
use alloy::transports::http::Http;
use alloy::signers::Signer;
use alloy::signers::local::PrivateKeySigner;
use anyhow::Result;
use std::str::FromStr;
use std::sync::Arc;
use url::Url;

pub type HttpProvider = RootProvider<Ethereum>;

pub struct BlockchainClient {
    provider: Arc<HttpProvider>,
}

impl BlockchainClient {
    pub async fn new(rpc_url: &str, expected_chain_id: u64, private_key: &str) -> Result<Self> {
        println!("🔗 Connecting to RPC: {}", rpc_url);
        
        let url = Url::parse(rpc_url)?;
        let transport = Http::new(url);
        let rpc_client = RpcClient::new(transport, true);
        let provider = RootProvider::new(rpc_client);
        
        // Create signer
        let signer = PrivateKeySigner::from_str(private_key)?;
        let signer = signer.with_chain_id(Some(expected_chain_id));
        
        // Test connection and verify chain ID
        let chain_id = provider.get_chain_id().await?;
        if chain_id != expected_chain_id {
            return Err(anyhow::anyhow!(
                "Chain ID mismatch: expected {}, got {}", 
                expected_chain_id, chain_id
            ));
        }
        
        println!("✅ Connected to chain {}", expected_chain_id);
        println!("🔑 Wallet address: {}", signer.address());
        
        Ok(Self {
            provider: Arc::new(provider),
        })
    }
    
    pub fn provider(&self) -> Arc<HttpProvider> {
        self.provider.clone()
    }
    
    
    pub async fn get_block_number(&self) -> Result<u64> {
        let block_number = self.provider.get_block_number().await?;
        Ok(block_number)
    }
    
    pub fn parse_address(addr: &str) -> Result<Address> {
        Address::from_str(addr).map_err(|e| anyhow::anyhow!("Invalid address {}: {}", addr, e))
    }
}