use crate::blockchain::BlockchainClient;
use alloy::primitives::{Address, TxKind, B256, U256};
use alloy::rpc::types::{TransactionInput, TransactionRequest};
use alloy::sol;
use alloy::sol_types::SolCall;
use anyhow::Result;
use std::sync::Arc;

sol! {
    #[sol(rpc)]
    interface IEarnVault {
        function onBoostReward(address token, uint256 amount) external;
    }
}

#[derive(Clone)]
pub struct EarnVaultContract {
    address: Address,
    client: Arc<BlockchainClient>,
}

impl EarnVaultContract {
    pub fn new(
        address: Address,
        _provider: Arc<dyn alloy::providers::Provider<alloy::network::Ethereum>>,
        client: BlockchainClient,
    ) -> Self {
        Self {
            address,
            client: Arc::new(client),
        }
    }
    
    pub async fn on_boost_reward(&self, token: Address, amount: U256) -> Result<B256> {
        let call = IEarnVault::onBoostRewardCall {
            token,
            amount,
        };
        let data: Vec<u8> = call.abi_encode();
        
        let tx = TransactionRequest {
            to: Some(TxKind::Call(self.address)),
            input: TransactionInput::new(data.into()),
            ..Default::default()
        };
        
        let tx_hash = self.client.send_transaction(tx).await?;
        Ok(tx_hash)
    }
}