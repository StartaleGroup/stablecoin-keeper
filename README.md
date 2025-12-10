# Stablecoin Keeper Service

Automated USDSC yield distribution keeper for Ethereum and Soneium networks.

## 🎯 Overview

A Rust-based keeper service that automates yield distribution across two networks:
- **Ethereum**: Claims USDSC yield to treasury EOA
- **Soneium**: Claims USDSC yield to RewardRedistributor and triggers distribution
- **Boost Rewards**: Distributes boost tokens (e.g., ASTR, USDSC) to Earn Vault users proportionally based on their USDSC principal balance

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
   cargo run -- claim-yield --config=ethereum.toml --dry-run
   cargo run -- distribute-rewards --config=soneium.toml --dry-run
   ```

4. **Setup S3 for Boost Rewards (optional):**
   - Create an S3 bucket for campaign configurations
   - Upload campaign TOML file (see `test_campaigns.toml` for format)
   - Configure AWS credentials with S3 access

## 🔧 Configuration

### Environment Variables
- Check .env.examples

### Network Configs
- `configs/common.toml` - Shared configuration (retry, monitoring, thresholds)
- `configs/ethereum-mainnet.toml` - Ethereum mainnet
- `configs/ethereum-sepolia.toml` - Ethereum testnet
- `configs/soneium-minato.toml` - Soneium Minato
- `configs/soneium-mainnet.toml` - Soneium mainnet

### AWS KMS Configuration
For enhanced security, use AWS KMS instead of private keys:

1. **Get KMS address:**
   ```bash
   cargo run --bin get-kms-address -- <KMS_KEY_ID> <AWS_REGION>
   ```

2. **Configure in .env:**
   ```bash
   KMS_KEY_ID=your-kms-key-id
   AWS_REGION=aws-region
   ```

3. **Grant contract roles to KMS address** in your smart contracts

## 🎮 Usage

### CLI Parameters
- `--config` - Path to network configuration file
- `--kms-key-id` - Override KMS key ID (optional)
- `--aws-region` - Override AWS region (optional)
- `--dry-run` - Test mode without sending transactions


### Manual Execution
```bash
# Testing
cargo run -- claim-yield --config=configs/ethereum-sepolia.toml
cargo run -- distribute-rewards --config=configs/ethereum-sepolia.toml
cargo run -- distribute-rewards --config=configs/soneium-minato.toml

# Production
cargo run -- claim-yield --config=configs/ethereum-mainnet.toml

# With KMS (secure signing) - region from AWS_REGION env var
cargo run -- claim-yield --config=configs/ethereum-sepolia.toml --kms-key-id=your-key-id

# With KMS and custom region override
cargo run -- claim-yield --config=configs/ethereum-sepolia.toml --kms-key-id=your-key-id --aws-region=us-west-2

# Dry run (safe testing)
cargo run -- claim-yield --config=configs/ethereum-sepolia.toml --dry-run
```

### Boost Rewards S3 (Admin-Controlled Campaigns)

The boost rewards S3 job reads campaign configurations from S3 and processes them daily. Campaigns are managed via an admin UI.

**Prerequisites:**
- S3 bucket with campaign configuration TOML file
- AWS credentials configured (via environment variables or IAM role)
- Keeper address funded with tokens for the campaign

**Basic Usage:**
```bash
# Run boost rewards S3 job (reads campaigns from S3)
cargo run -- boost-rewards-s3 \
  --config=configs/ethereum-sepolia.toml \
  --campaigns-s3=s3://bucket-name/path/to/campaigns.toml

# With explicit S3 region
cargo run -- boost-rewards-s3 \
  --config=configs/ethereum-sepolia.toml \
  --campaigns-s3=s3://bucket-name/path/to/campaigns.toml \
  --s3-region=eu-central-1

# With KMS configuration
cargo run -- boost-rewards-s3 \
  --config=configs/ethereum-sepolia.toml \
  --campaigns-s3=s3://bucket-name/path/to/campaigns.toml \
  --kms-key-id=your-kms-key-id \
  --aws-region=eu-central-1
```

**S3 Path Format:**
- Full S3 URI: `s3://bucket-name/path/to/campaigns.toml`
- Short format: `bucket-name/path/to/campaigns.toml`

**Environment Variables:**
- `S3_REGION` - AWS region for S3 (defaults to `AWS_REGION` or KMS region)
- `AWS_ACCESS_KEY_ID` - AWS access key (or use IAM role)
- `AWS_SECRET_ACCESS_KEY` - AWS secret key (or use IAM role)
- `AWS_REGION` - AWS region (used as fallback for S3 region)

**Campaign Configuration Format:**
See `test_campaigns.toml` for example:
```toml
[[campaigns]]
id = "campaign-2025-01"
token_address = "0x7e426d026f604d1c47b50059752122d8ab1e2c28"
total_amount = 1000.0
start_date = "2025-01-01"
end_date = "2025-01-31"
status = "active"
```

**Production Scheduling (Kubernetes CronJob):**
```yaml
# Run daily at 12:00 PM UTC
schedule: "0 12 * * *"
```

**Important Notes:**
- Campaigns are processed **once per day** when the cron job runs
- To start a campaign **today**, create it before **12:00 PM UTC**
- Fund the keeper address with the **total campaign amount** before creating the campaign
- Multiple campaigns on the same day are processed **sequentially** with a 30-second delay between them
- Campaigns using the same token are safe from nonce race conditions (sequential processing with confirmation)

### Production Scheduling
```bash
# Every 10 minutes - Ethereum yield claiming
*/10 * * * * /path/to/vault-keeper claim-yield --config=configs/ethereum-mainnet.toml

# Every 3 hours - Soneium distribution
0 */3 * * * /path/to/vault-keeper distribute-rewards --config=configs/soneium-mainnet.toml

# Daily at 12:00 PM UTC - Boost rewards from S3
0 12 * * * /path/to/vault-keeper boost-rewards-s3 --config=configs/ethereum-sepolia.toml --campaigns-s3=s3://bucket/campaigns.toml
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
- **BoostRewardsS3** - Reads campaign configurations from S3 and distributes boost tokens to Earn Vault users daily
- **BoostRewardsDistribute** - Manual single-campaign distribution (CLI-based, for Phase 1)

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
configs/              # Configuration files
├── common.toml             # Shared configuration (retry, monitoring, thresholds)
├── ethereum-mainnet.toml   # Ethereum mainnet (production)
├── ethereum-sepolia.toml   # Ethereum testnet (testing)
├── soneium-minato.toml     # Soneium Minato testnet
└── soneium-mainnet.toml    # Soneium mainnet (future)

env.example           # Environment variables template

src/
├── main.rs              # CLI interface
├── config.rs           # Configuration loading
├── blockchain.rs       # RPC client and wallet
├── kms_signer.rs       # AWS KMS signer integration
├── contracts/          # Smart contract interfaces
│   ├── usdsc.rs
│   ├── reward_redistributor.rs
│   ├── erc20.rs        # ERC20 token interface
│   └── earn_vault.rs   # Earn Vault interface
├── jobs/               # Keeper job implementations
│   ├── claim_yield.rs
│   ├── distribute_rewards.rs
│   ├── boost_rewards.rs      # Boost rewards distribution logic
│   └── boost_rewards_s3.rs   # S3-based boost rewards cron job
└── sources/            # Campaign configuration sources
    └── s3_campaign_source.rs # S3 campaign source implementation

tests/                 # Test suites
├── unit_tests.rs       # Pure unit tests
├── integration_tests.rs # Component integration tests
└── cli_tests.rs       # CLI functionality tests
```
