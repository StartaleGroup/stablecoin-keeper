use alloy::network::Ethereum;
use alloy::primitives::{Address, U256, B256, TxKind, Bytes};
use alloy::rpc::types::{TransactionRequest, TransactionInput};
use alloy::sol;
use alloy::sol_types::SolCall;
use alloy::providers::Provider;
use anyhow::Result;
use std::sync::Arc;

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
    provider: Arc<dyn Provider<Ethereum>>,
}

impl RewardRedistributorContract {
    pub fn new(address: Address, provider: Arc<dyn Provider<Ethereum>>) -> Self {
        Self { address, provider }
    }
    
    pub async fn preview_distribute(&self) -> Result<(U256, U256, U256, U256, U256, U256, U256, U256)> {
        let data = hex::decode("12345678")?;
        
        let _result = self.provider.call(
            alloy::rpc::types::TransactionRequest {
                to: Some(TxKind::Call(self.address)),
                input: TransactionInput::new(Bytes::from(data)),
                ..Default::default()
            }
        ).await?;
        
        Ok((
            U256::ZERO, U256::ZERO, U256::ZERO, U256::ZERO,
            U256::ZERO, U256::ZERO, U256::ZERO, U256::ZERO,
        ))
    }
    
    pub async fn distribute(&self) -> Result<B256> {
        let call = IRewardRedistributor::distributeCall {};
        let data: Vec<u8> = call.abi_encode();
        
        let tx = TransactionRequest {
            to: Some(TxKind::Call(self.address)),
            input: TransactionInput::new(data.into()),
            value: Some(U256::ZERO),
            ..Default::default()
        };
        
        let pending = self.provider.send_transaction(tx).await?;
        Ok(*pending.tx_hash())
    }
}