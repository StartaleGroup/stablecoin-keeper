use stablecoin_backend::kms_signer::KmsSigner;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <KMS_KEY_ID> <AWS_REGION>", args[0]);
        std::process::exit(1);
    }
    
    let key_id = &args[1];
    let region = &args[2];
    println!("🔐 Deriving Ethereum address from KMS key: {}", key_id);
    println!("🌍 Using AWS region: {}", region);
    
    // Use provided region and default chain ID for address derivation
    let signer = KmsSigner::new(key_id.to_string(), region.to_string(), 1).await?;
    let address = signer.address();
    
    println!("✅ KMS Ethereum Address: 0x{}", hex::encode(address.as_slice()));
    println!("💰 Send ETH to this address for gas payments");
    println!("🔍 You can verify this address on Etherscan: https://etherscan.io/address/0x{}", hex::encode(address.as_slice()));
    
    Ok(())
}

