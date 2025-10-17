use alloy::primitives::{Address, U256, B256};
use alloy::sol;
use anyhow::Result;
use std::sync::Arc;
use crate::blockchain::HttpProvider;

sol! {
    #[sol(rpc)]
    interface IRewardRedistributor {
        function distribute() external;
        function previewDistribute() external view returns (
            uint256 couldBeMinted,
            uint256 feeToStartale,
            uint256 toEarn,
            uint256 toOn,
            uint256 toStartaleExtra,
            uint256 S_base,
            uint256 T_earn,
            uint256 T_yield
        );
    }
}

#[derive(Clone)]
pub struct RewardRedistributorContract {
    address: Address,
    provider: Arc<HttpProvider>,
}

impl RewardRedistributorContract {
    pub fn new(address: Address, provider: Arc<HttpProvider>) -> Self {
        Self { address, provider }
    }
    
    pub async fn preview_distribute(&self) -> Result<(U256, U256, U256, U256, U256, U256, U256, U256)> {
        let contract = IRewardRedistributor::new(self.address, &self.provider);
        let result = contract.previewDistribute().call().await?;
        Ok((
            result.couldBeMinted,
            result.feeToStartale,
            result.toEarn,
            result.toOn,
            result.toStartaleExtra,
            result.S_base,
            result.T_earn,
            result.T_yield,
        ))
    }
    
    pub async fn distribute(&self) -> Result<B256> {
        let contract = IRewardRedistributor::new(self.address, &self.provider);
        let pending_tx = contract.distribute().send().await?;
        let tx_hash = *pending_tx.tx_hash();
        Ok(tx_hash)
    }
}