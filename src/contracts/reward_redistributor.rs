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
        // Manual RPC call like vault-relayer
        // Build calldata for previewDistribute() function
        // Function selector: keccak("previewDistribute()")[0:4] = 0x12345678 (placeholder)
        let data = hex::decode("12345678")?; // This needs to be the actual selector
        
        let result = self.provider.call(
            alloy::rpc::types::TransactionRequest {
                to: Some(TxKind::Call(self.address)),
                input: TransactionInput::new(Bytes::from(data)),
                ..Default::default()
            }
        ).await?;
        
        // Parse the result - this is a complex return type with 8 uint256 values
        // For now, return placeholder values
        // TODO: Implement proper parsing of the return data
        Ok((
            U256::ZERO, U256::ZERO, U256::ZERO, U256::ZERO,
            U256::ZERO, U256::ZERO, U256::ZERO, U256::ZERO,
        ))
    }
    
    pub async fn distribute(&self) -> Result<B256> {
        // Build the calldata for distribute() (like vault-relayer)
        let call = IRewardRedistributor::distributeCall {};
        let data: Vec<u8> = call.abi_encode();
        
        // Create transaction request
        let tx = TransactionRequest {
            to: Some(TxKind::Call(self.address)),
            input: TransactionInput::new(data.into()),
            value: Some(U256::ZERO),
            ..Default::default()
        };
        
        // Send transaction (provider already includes signer)
        let pending = self.provider.send_transaction(tx).await?;
        Ok(*pending.tx_hash())
    }
}