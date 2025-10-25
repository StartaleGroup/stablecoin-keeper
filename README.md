# Vault Keeper Service

Automated USDSC yield distribution keeper for Ethereum and Soneium networks.

## 🎯 Overview

A Rust-based keeper service that automates yield distribution across two networks:
- **Ethereum**: Claims USDSC yield to treasury EOA
- **Soneium**: Claims USDSC yield to RewardRedistributor and triggers distribution

## 🚀 Quick Start

### Prerequisites
- Rust 1.70+
- AWS KMS setup for secure signing
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
   # Edit .env with your KMS settings and contract addresses
   ```

3. **Test with dry run:**
   ```bash
   cargo run -- claim-yield --chain-id=1 --config=ethereum.toml --dry-run
   cargo run -- distribute-rewards --chain-id=1946 --config=soneium.toml --dry-run
   ```

## 🔧 Configuration

### Environment Variables
All sensitive data is stored in `.env`:
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

3. **Environment variables (automatic defaults):**
   ```bash
   # .env - These are automatically substituted in TOML files
   KMS_KEY_ID=your-kms-key-id
   KMS_REGION=ap-northeast-1
   ```
   
   **How it works**: The TOML files use `${KMS_KEY_ID}` and `${KMS_REGION}` placeholders that are automatically replaced with values from your `.env` file when the config is loaded.

4. **Grant contract roles to KMS address:**
   - Assign `DISTRIBUTOR_ROLE` to KMS address in RewardRedistributor contract
   - Assign `MINTER_ROLE` to KMS address in USDSC contract (if needed)

### KMS Configuration Priority
The system follows this priority order for KMS settings:
1. **CLI arguments** (`--kms-key-id`, `--kms-region`) - highest priority
2. **Config file with environment substitution** (`soneium.toml`, `ethereum.toml`) 
3. **Defaults** (`ap-northeast-1` for region)

**Note**: Environment variables are automatically loaded via `${KMS_KEY_ID}` and `${KMS_REGION}` placeholders in TOML files.

### CLI Override Benefits
- **Security**: Pass sensitive KMS keys via CLI instead of storing in files
- **Flexibility**: Use different KMS keys for different operations without changing configs
- **Environment switching**: Override for dev/staging/prod environments
- **Testing**: Use test KMS keys without modifying configuration files

## 🎮 Usage

### Manual Execution
```bash
# Claim yield on Ethereum
cargo run -- claim-yield --chain-id=1 --config=ethereum.toml

# Distribute rewards on Soneium  
cargo run -- distribute-rewards --chain-id=11155111 --config=soneium.toml

# Use KMS for Ethereum operations
cargo run -- claim-yield --chain-id=1 --config=ethereum.toml --kms-key-id=eth-kms-key --kms-region=us-east-1

# Use KMS with custom region (override both key and region)
cargo run -- distribute-rewards --chain-id=11155111 --config=soneium.toml --kms-key-id=your-kms-key-id --kms-region=us-west-2

# Use KMS with defaults from .env (no CLI args needed)
cargo run -- distribute-rewards --chain-id=11155111 --config=soneium.toml

# Override just the key ID, use region from .env
cargo run -- distribute-rewards --chain-id=11155111 --config=soneium.toml --kms-key-id=different-key

# Override just the region, use key from .env
cargo run -- distribute-rewards --chain-id=11155111 --config=soneium.toml --kms-region=eu-west-1

# Use KMS for Ethereum operations
cargo run -- claim-yield --chain-id=1 --config=ethereum.toml --kms-key-id=eth-kms-key --kms-region=us-east-1

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

- **AWS KMS Support** - Secure transaction signing using AWS Key Management Service
- **Environment Variables** - All sensitive data stored in `.env` (never committed)
- **CLI Override Support** - Override KMS settings via CLI for enhanced security
- **Separate KMS Keys** - Different KMS keys for Ethereum and Soneium
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