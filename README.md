# Stablecoin Keeper Service

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
   cargo run -- claim-yield --config=ethereum.toml --dry-run
   cargo run -- distribute-rewards --config=soneium.toml --dry-run
   ```

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

### Production Scheduling
```bash
# Every 10 minutes - Ethereum yield claiming
*/10 * * * * /path/to/vault-keeper claim-yield --config=configs/ethereum-mainnet.toml

# Every 3 hours - Soneium distribution
0 */3 * * * /path/to/vault-keeper distribute-rewards --config=configs/soneium-mainnet.toml
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
│   └── reward_redistributor.rs
└── jobs/               # Keeper job implementations
    ├── claim_yield.rs
    └── distribute_rewards.rs

tests/                 # Test suites
├── unit_tests.rs       # Pure unit tests
├── integration_tests.rs # Component integration tests
└── cli_tests.rs       # CLI functionality tests
```
