use alloy::network::Ethereum;
use alloy::primitives::{Address, Bytes, TxKind, B256, U256};
use alloy::providers::Provider;
use alloy::rpc::types::{TransactionInput, TransactionRequest};
use alloy::sol;
use alloy::sol_types::SolCall;
use anyhow::Result;
use std::sync::Arc;

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function transfer(address to, uint256 amount) external returns (bool);
        function balanceOf(address account) external view returns (uint256);
        function decimals() external view returns (uint8);
        function symbol() external view returns (string);
    }
}

#[derive(Clone)]
pub struct ERC20Contract {
    address: Address,
    provider: Arc<dyn Provider<Ethereum>>,
}

impl ERC20Contract {
    pub fn new(address: Address, provider: Arc<dyn Provider<Ethereum>>) -> Self {
        Self { address, provider }
    }

    pub async fn balance_of(&self, account: Address) -> Result<U256> {
        let call = IERC20::balanceOfCall { account };
        let data: Vec<u8> = call.abi_encode();

        let result = self
            .provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(self.address)),
                input: TransactionInput::new(Bytes::from(data)),
                ..Default::default()
            })
            .await?;

        let decoded = IERC20::balanceOfCall::abi_decode_returns(&result)?;
        Ok(decoded)
    }

    pub async fn decimals(&self) -> Result<u8> {
        let call = IERC20::decimalsCall {};
        let data: Vec<u8> = call.abi_encode();

        let result = self
            .provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(self.address)),
                input: TransactionInput::new(Bytes::from(data)),
                ..Default::default()
            })
            .await?;

        // Decode uint8: Solidity pads uint8 to 32 bytes, value is in last byte
        // Convert to U256 first, then to u8
        let decimals_u256 = U256::from_be_slice(&result);
        let decimals = decimals_u256.to::<u8>();
        Ok(decimals)
    }

    pub async fn symbol(&self) -> Result<String> {
        let call = IERC20::symbolCall {};
        let data: Vec<u8> = call.abi_encode();

        let result = self
            .provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(self.address)),
                input: TransactionInput::new(Bytes::from(data)),
                ..Default::default()
            })
            .await?;

        // Decode string: Alloy's abi_decode_returns should work for strings
        let decoded = IERC20::symbolCall::abi_decode_returns(&result)?;
        Ok(decoded)
    }

    pub async fn transfer(&self, to: Address, amount: U256) -> Result<B256> {
        let call = IERC20::transferCall { to, amount };
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
