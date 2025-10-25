# Vault Keeper Service

Automated USDSC yield distribution keeper for Ethereum and Soneium networks.

## 🎯 Overview

A Rust-based keeper service that automates yield distribution across two networks:
- **Ethereum**: Claims USDSC yield to treasury EOA
- **Soneium**: Claims USDSC yield to RewardRedistributor and triggers distribution

## 🚀 Quick Start

### Prerequisites
- Rust 1.70+
- Private keys for keeper wallets
- RPC endpoints for both networks

### Setup
1. **Clone and build:**
   ```bash
   git clone <repo>
   cd vault-keeper
   cargo build --release
   ```

2. **Configure environment:**
   ```bash
   cp .env.example .env
   # Edit .env with your actual private keys and contract addresses
   ```

3. **Test with dry run:**
   ```bash
   cargo run -- claim-yield --chain-id=1 --config=ethereum.toml --dry-run
   cargo run -- distribute-rewards --chain-id=1946 --config=soneium.toml --dry-run
   ```

## 🔧 Configuration

### Environment Variables
All sensitive data is stored in `.env`:
- `ETH_PRIVATE_KEY` - Ethereum keeper wallet
- `SONEIUM_PRIVATE_KEY` - Soneium keeper wallet  
- `ETH_USDSC_ADDRESS` - USDSC contract on Ethereum
- `SONEIUM_REWARD_REDISTRIBUTOR_ADDRESS` - RewardRedistributor contract
- `KMS_KEY_ID` - AWS KMS key ID for secure signing
- `KMS_REGION` - AWS region for KMS operations
- See `.env.example` for complete list

### Network Configs
- `ethereum.toml` - Ethereum network settings
- `soneium.toml` - Soneium network settings

### AWS KMS Configuration
For enhanced security, we can use AWS KMS instead of private keys:

1. **Setup KMS Key:**
   ```bash
   # Get KMS address (for role assignment)
   cargo run --bin get-kms-address -- --key-id <KMS_KEY_ID> --region <AWS_REGION>
   ```

2. **Configure KMS in network configs:**
   ```toml
   # soneium.toml
   [kms]
   key_id = "${KMS_KEY_ID}"
   region = "${KMS_REGION}"
   ```

3. **Environment variables:**
   ```bash
   # .env
   KMS_KEY_ID=your-kms-key-id
   KMS_REGION=ap-northeast-1
   ```

4. **Grant contract roles to KMS address:**
   - Assign `DISTRIBUTOR_ROLE` to KMS address in RewardRedistributor contract
   - Assign `MINTER_ROLE` to KMS address in USDSC contract (if needed)

## 🎮 Usage

### Manual Execution
```bash
# Claim yield on Ethereum
cargo run -- claim-yield --chain-id=1 --config=ethereum.toml

# Distribute rewards on Soneium  
cargo run -- distribute-rewards --chain-id=11155111 --config=soneium.toml

# Override private key from CLI (more secure)
cargo run -- claim-yield --chain-id=1 --config=ethereum.toml --private-key=0x...

# Use KMS for signing (Soneium)
cargo run -- distribute-rewards --chain-id=11155111 --config=soneium.toml --kms-key-id=your-kms-key-id

# Dry run mode (no transactions)
cargo run -- claim-yield --chain-id=1 --config=ethereum.toml --dry-run
```

### Production Scheduling
Use Kubernetes CronJobs or traditional cron:
```bash
# Every 10 minutes - Ethereum yield claiming
*/10 * * * * /path/to/vault-keeper claim-yield --chain-id=1 --config=ethereum.toml

# Every 3 hours - Soneium distribution
0 */3 * * * /path/to/vault-keeper distribute-rewards --chain-id=1946 --config=soneium.toml
```

## 🏗️ Architecture

### Core Components
- **BlockchainClient** - RPC connection and wallet management
- **USDSCContract** - USDSC token interactions (`yield()`, `claimYield()`)
- **RewardRedistributorContract** - Distribution logic (`distribute()`, `previewDistribute()`)
- **Job System** - Independent batch jobs for each operation

### Job Types
- **ClaimYield** - Claims USDSC yield to recipient (Ethereum → EOA, Soneium → RewardRedistributor)
- **DistributeRewards** - Checks USDSC yield threshold, then triggers distribution to vaults (Soneium only)

## 🔐 Security

- **Environment Variables** - All private keys stored in `.env` (never committed)
- **CLI Private Key Override** - Pass private keys via CLI for enhanced security
- **AWS KMS Support** - Secure transaction signing using AWS Key Management Service
- **Separate Wallets** - Different keys for Ethereum and Soneium
- **Dry Run Mode** - Test operations without sending transactions
- **Chain ID Validation** - Prevents accidental cross-chain operations
- **Transaction Monitoring** - Real-time transaction status tracking with timeout handling

## 📁 Project Structure

```
src/
├── main.rs              # CLI interface
├── config.rs           # Configuration loading
├── blockchain.rs       # RPC client and wallet
├── kms_signer.rs       # AWS KMS signer integration
├── contracts/          # Smart contract interfaces
│   ├── usdsc.rs
│   └── reward_redistributor.rs
└── jobs/               # Keeper job implementations
    ├── claim_yield.rs
    └── distribute_rewards.rs
```

## 🚀 Production Deployment

### Kubernetes CronJobs (Recommended)
```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: ethereum-claim
spec:
  schedule: "*/10 * * * *"
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: vault-keeper
            image: vault-keeper:latest
            command: ["vault-keeper", "claim-yield", "--chain-id=1", "--config=ethereum.toml"]
            env:
            - name: ETH_PRIVATE_KEY
              valueFrom:
                secretKeyRef:
                  name: keeper-secrets
                  key: eth-private-key
```

### Environment Setup
See `ENV_SETUP.md` for detailed configuration guide.

## 🔄 Next Steps

- [ ] Real contract addresses and private keys
- [ ] Database integration for transaction history
- [ ] Monitoring and alerting setup
- [ ] Gas optimization and retry logic
- [ ] Multi-signature wallet support