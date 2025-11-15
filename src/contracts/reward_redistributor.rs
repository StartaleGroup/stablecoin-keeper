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
        function snapshotSusdscTVL() external;
        function lastSnapshotTimestamp() external view returns (uint256);
        function lastSnapshotBlockNumber() external view returns (uint256);
        function snapshotMaxAge() external view returns (uint256);
    }
}

#[derive(Clone)]
pub struct RewardRedistributorContract {
    address: Address,
    provider: Arc<dyn Provider<Ethereum>>,
    client: Arc<BlockchainClient>,
}

impl RewardRedistributorContract {
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

    pub async fn preview_distribute(
        &self,
    ) -> Result<(U256, U256, U256, U256, U256, U256, U256, U256)> {
        let call = IRewardRedistributor::previewDistributeCall {};
        let data: Vec<u8> = call.abi_encode();

        let result = self
            .provider
            .call(alloy::rpc::types::TransactionRequest {
                to: Some(TxKind::Call(self.address)),
                input: TransactionInput::new(Bytes::from(data)),
                ..Default::default()
            })
            .await?;

        // Decode the 8-tuple return type using Alloy's ABI decoder
        let decoded = IRewardRedistributor::previewDistributeCall::abi_decode_returns(&result)?;

        Ok((
            decoded.couldBeMinted,   // couldBeMinted
            decoded.feeToStartale,   // feeToStartale
            decoded.toEarn,          // toEarn
            decoded.toOn,            // toOn
            decoded.toStartaleExtra, // toStartaleExtra
            decoded.S_base,          // S_base
            decoded.T_earn,          // T_earn
            decoded.T_yield,         // T_yield
        ))
    }

    // Distribute functions
    pub async fn distribute(&self, value_wei: &str) -> Result<B256> {
        let call = IRewardRedistributor::distributeCall {};
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

    // Snapshot functions
    pub async fn snapshot_susdsc_tvl(&self, value_wei: &str) -> Result<B256> {
        let call = IRewardRedistributor::snapshotSusdscTVLCall {};
        let data: Vec<u8> = call.abi_encode();

        let tx_value = U256::from_str(value_wei)?;

        let tx = TransactionRequest {
            to: Some(TxKind::Call(self.address)),
            input: TransactionInput::new(data.into()),
            value: Some(tx_value),
            ..Default::default()
        };

        let tx_hash = self.client.send_transaction(tx).await?;
        Ok(tx_hash)
    }

    pub async fn last_snapshot_timestamp(&self) -> Result<U256> {
        let call = IRewardRedistributor::lastSnapshotTimestampCall {};
        let data: Vec<u8> = call.abi_encode();

        let result = self
            .provider
            .call(alloy::rpc::types::TransactionRequest {
                to: Some(TxKind::Call(self.address)),
                input: TransactionInput::new(Bytes::from(data)),
                ..Default::default()
            })
            .await?;

        let decoded = IRewardRedistributor::lastSnapshotTimestampCall::abi_decode_returns(&result)?;
        Ok(decoded)
    }
    
    pub async fn last_snapshot_block_number(&self) -> Result<U256> {
        let call = IRewardRedistributor::lastSnapshotBlockNumberCall {};
        let data: Vec<u8> = call.abi_encode();

        let result = self
            .provider
            .call(alloy::rpc::types::TransactionRequest {
                to: Some(TxKind::Call(self.address)),
                input: TransactionInput::new(Bytes::from(data)),
                ..Default::default()
            })
            .await?;

        let decoded = IRewardRedistributor::lastSnapshotBlockNumberCall::abi_decode_returns(&result)?;
        Ok(decoded)
    }

    pub async fn snapshot_max_age(&self) -> Result<U256> {
        let call = IRewardRedistributor::snapshotMaxAgeCall {};
        let data: Vec<u8> = call.abi_encode();

        let result = self
            .provider
            .call(alloy::rpc::types::TransactionRequest {
                to: Some(TxKind::Call(self.address)),
                input: TransactionInput::new(Bytes::from(data)),
                ..Default::default()
            })
            .await?;

        let decoded = IRewardRedistributor::snapshotMaxAgeCall::abi_decode_returns(&result)?;
        Ok(decoded)
    }
}
