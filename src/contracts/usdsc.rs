use alloy::primitives::{Address, U256, B256};
use alloy::sol;
use anyhow::Result;
use std::sync::Arc;
use crate::blockchain::HttpProvider;

sol! {
    #[sol(rpc)]
    interface IUSDSC {
        // From IMYieldToOne interface
        function yield() external view returns (uint256);
        function claimYield() external returns (uint256);
        function yieldRecipient() external view returns (address);
        
        // Standard ERC20 functions
        function balanceOf(address account) external view returns (uint256);
        function totalSupply() external view returns (uint256);
    }
}

#[derive(Clone)]
pub struct USDSCContract {
    address: Address,
    provider: Arc<HttpProvider>,
}

impl USDSCContract {
    pub fn new(address: Address, provider: Arc<HttpProvider>) -> Self {
        Self { address, provider }
    }
    
    pub async fn get_pending_yield(&self) -> Result<U256> {
        let contract = IUSDSC::new(self.address, &self.provider);
        let yield_amount = contract.r#yield().call().await?;
        Ok(yield_amount)
    }
    
    
    pub async fn claim_yield(&self) -> Result<B256> {
        let contract = IUSDSC::new(self.address, &self.provider);
        let pending_tx = contract.claimYield().send().await?;
        let tx_hash = *pending_tx.tx_hash();
        Ok(tx_hash)
    }
}