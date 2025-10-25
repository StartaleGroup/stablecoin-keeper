use alloy::network::Ethereum;
use alloy::primitives::{Address, U256, B256, TxKind, Bytes};
use alloy::rpc::types::{TransactionRequest, TransactionInput};
use alloy::sol;
use alloy::sol_types::SolCall;
use alloy::providers::Provider;
use anyhow::Result;
use std::sync::Arc;
use std::str::FromStr;

sol! {
    #[sol(rpc)]
    interface IUSDSC {
        function yield() external view returns (uint256);
        function claimYield() external returns (uint256);
    }
}

#[derive(Clone)]
pub struct USDSCContract {
    address: Address,
    provider: Arc<dyn Provider<Ethereum>>,
    client: Arc<crate::blockchain::BlockchainClient>,
}

impl USDSCContract {
    pub fn new(address: Address, provider: Arc<dyn Provider<Ethereum>>, client: Arc<crate::blockchain::BlockchainClient>) -> Self {
        Self { address, provider, client }
    }
    
    pub async fn get_pending_yield(&self) -> Result<U256> {
        let call = IUSDSC::r#yieldCall {};
        let data: Vec<u8> = call.abi_encode();
        
        let result = self.provider.call(
            alloy::rpc::types::TransactionRequest {
                to: Some(TxKind::Call(self.address)),
                input: TransactionInput::new(Bytes::from(data)),
                ..Default::default()
            }
        ).await?;
        
        let yield_amount = U256::from_be_slice(&result);
        Ok(yield_amount)
    }
    
    
    pub async fn claim_yield(&self, value_wei: &str) -> Result<B256> {
        let call = IUSDSC::claimYieldCall {};
        let data: Vec<u8> = call.abi_encode();
        
        let tx_value = U256::from_str(value_wei)?;
        
        let tx = TransactionRequest {
            to: Some(TxKind::Call(self.address)),
            input: TransactionInput::new(data.into()),
            value: Some(tx_value),
            gas: Some(300000), // Set reasonable gas limit for claimYield
            ..Default::default()
        };
        
        // Use the unified transaction sending (works for both private key and KMS)
        let tx_hash = self.client.send_transaction(tx).await?;
        Ok(tx_hash)
    }
}