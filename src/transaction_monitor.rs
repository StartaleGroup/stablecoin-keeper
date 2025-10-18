use alloy::primitives::{B256, U256};
use alloy::providers::Provider;
use alloy::network::Ethereum;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct TransactionReceipt {
    #[allow(dead_code)] // Used in tests and public API
    pub hash: B256,
    pub block_number: u64,
    pub gas_used: U256,
    pub status: TransactionStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransactionStatus {
    Success,
    Failed,
    Timeout,
}

pub struct TransactionMonitor {
    provider: Arc<dyn Provider<Ethereum>>,
    max_wait_time: Duration,
    poll_interval: Duration,
}

impl TransactionMonitor {
    pub fn new(provider: Arc<dyn Provider<Ethereum>>, max_wait_time: Duration, poll_interval: Duration) -> Self {
        Self {
            provider,
            max_wait_time,
            poll_interval,
        }
    }

    pub async fn monitor_transaction(&self, tx_hash: B256) -> Result<TransactionReceipt> {
        println!("🔍 Monitoring transaction: {:?}", tx_hash);
        
        let start_time = std::time::Instant::now();
        
        loop {
            if start_time.elapsed() > self.max_wait_time {
                println!("⏰ Transaction monitoring timeout after {:?}", self.max_wait_time);
                return Ok(TransactionReceipt {
                    hash: tx_hash,
                    block_number: 0,
                    gas_used: U256::ZERO,
                    status: TransactionStatus::Timeout,
                });
            }
            
            match self.provider.get_transaction_receipt(tx_hash).await {
                Ok(Some(receipt)) => {
                    let status = if receipt.status() {
                        TransactionStatus::Success
                    } else {
                        TransactionStatus::Failed
                    };
                    
                    println!("✅ Transaction confirmed: {:?} (Status: {:?})", tx_hash, status);
                    
                    return Ok(TransactionReceipt {
                        hash: tx_hash,
                        block_number: receipt.block_number.unwrap_or(0),
                        gas_used: U256::from(receipt.gas_used),
                        status,
                    });
                }
                Ok(None) => {
                    println!("⏳ Transaction pending, waiting...");
                    // Transaction is still pending, continue monitoring
                    // Note: We don't return Pending status here as we continue monitoring
                }
                Err(e) => {
                    println!("❌ Error checking transaction status: {}", e);
                }
            }
            
            sleep(self.poll_interval).await;
        }
    }
    
}

