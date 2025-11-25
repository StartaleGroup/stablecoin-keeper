use alloy::primitives::{Address, TxKind, B256, U256};
use alloy::providers::Provider;
use alloy::rpc::types::{TransactionInput, TransactionRequest};
use alloy::sol;
use alloy::sol_types::SolCall;
use anyhow::Result;
use std::sync::Arc;
use alloy::network::Ethereum;

sol! {
    #[sol(rpc)]
    interface IEarnVault {
        function onBoostReward(address token, uint256 amount) external;
    }
}

#[derive(Clone)]
pub struct EarnVaultContract {
    address: Address,
    provider: Arc<dyn Provider<Ethereum>>,
}

impl EarnVaultContract {
    pub fn new(
        address: Address,
        provider: Arc<dyn Provider<Ethereum>>,
    ) -> Self {
        Self {
            address,
            provider,
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
        
        // Use provider.send_transaction directly - provider already has signer attached
        let pending = self.provider.send_transaction(tx).await?;
        let tx_hash = *pending.tx_hash();
        Ok(tx_hash)
    }
}