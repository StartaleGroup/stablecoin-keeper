use crate::blockchain::BlockchainClient;
use alloy::network::Ethereum;
use alloy::primitives::{Address, Bytes, TxKind, B256, U256};
use alloy::providers::Provider;
use alloy::rpc::types::{TransactionInput, TransactionRequest};
use alloy::sol;
use alloy::sol_types::SolCall;
use anyhow::Result;
use std::str::FromStr;
use std::sync::Arc;
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
    client: Arc<BlockchainClient>,
}

impl USDSCContract {
    pub fn new(
        address: Address,
        provider: Arc<dyn Provider<Ethereum>>,
        client: BlockchainClient,
    ) -> Self {
        Self {
            address,
            provider,
            client: Arc::new(client),
        }
    }

    pub async fn get_pending_yield(&self) -> Result<U256> {
        let call = IUSDSC::r#yieldCall {};
        let data: Vec<u8> = call.abi_encode();

        let result = self
            .provider
            .call(alloy::rpc::types::TransactionRequest {
                to: Some(TxKind::Call(self.address)),
                input: TransactionInput::new(Bytes::from(data)),
                ..Default::default()
            })
            .await?;

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
            ..Default::default()
        };

        // Use the unified transaction sending (works for both private key and KMS)
        let tx_hash = self.client.send_transaction(tx).await?;
        Ok(tx_hash)
    }
}
