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
    provider: Arc<dyn Provider<Ethereum>>,
}

impl USDSCContract {
    pub fn new(address: Address, provider: Arc<dyn Provider<Ethereum>>) -> Self {
        Self { address, provider }
    }
    
    pub async fn get_pending_yield(&self) -> Result<U256> {
        // Manual RPC call like vault-relayer
        let client = reqwest::Client::new();
        
        // Build calldata for yield() function
        // Function selector: keccak("yield()")[0:4] = 0x3f8c4f33
        let data = hex::decode("3f8c4f33")?;
        
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": format!("{:#x}", self.address),
                "data": format!("0x{}", hex::encode(&data)),
            }, "latest"],
            "id": 1
        });
        
        // We need the RPC URL, but we don't have it in the contract
        // Let's use the provider's call method instead
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
    
    
    pub async fn claim_yield(&self) -> Result<B256> {
        // Build the calldata for claimYield() (like vault-relayer)
        let call = IUSDSC::claimYieldCall {};
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