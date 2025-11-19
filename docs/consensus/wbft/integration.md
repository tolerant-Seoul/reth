# WBFT Integration Guide

This guide explains how to integrate WBFT consensus into a reth-based node.

## Prerequisites

- Rust toolchain (nightly)
- reth codebase
- BLS keys for validators

## Adding WBFT to Your Project

### 1. Add Dependency

In your `Cargo.toml`:

```toml
[dependencies]
reth-consensus-wbft = { path = "path/to/reth/crates/consensus/wbft" }
```

### 2. Genesis Configuration

Create a genesis JSON file with WBFT configuration:

```json
{
  "config": {
    "chainId": 1234,
    "wbft": {
      "wbft": {
        "requestTimeoutSeconds": 2,
        "blockPeriodSeconds": 1,
        "proposerPolicy": 0,
        "epochLength": 100
      },
      "init": {
        "validators": [
          "0x1111111111111111111111111111111111111111",
          "0x2222222222222222222222222222222222222222",
          "0x3333333333333333333333333333333333333333",
          "0x4444444444444444444444444444444444444444"
        ],
        "blsPublicKeys": [
          "0xaec4...(96 hex chars)",
          "0xb1ae...(96 hex chars)",
          "0xc2bf...(96 hex chars)",
          "0xd3c0...(96 hex chars)"
        ]
      },
      "systemContracts": {
        "govValidator": {
          "address": "0x0000000000000000000000000000000000000400",
          "version": "v1",
          "params": {
            "gasTip": "1000000000"
          }
        }
      }
    }
  },
  "alloc": {
    "0x1111111111111111111111111111111111111111": {
      "balance": "0x200000000000000000000000000000000000000000000000000000000000000"
    }
  },
  "coinbase": "0x0000000000000000000000000000000000000000",
  "difficulty": "0x1",
  "gasLimit": "0x1000000",
  "nonce": "0x0",
  "timestamp": "0x0"
}
```

### 3. Configuration Options

#### WBFT Config

| Field | Type | Description | Default |
|-------|------|-------------|---------|
| `requestTimeoutSeconds` | u64 | Initial round timeout | 2 |
| `blockPeriodSeconds` | u64 | Target block time | 1 |
| `proposerPolicy` | u64 | 0=RoundRobin, 1=Sticky | 0 |
| `epochLength` | u64 | Blocks per epoch | 100 |
| `maxRequestTimeoutSeconds` | u64? | Maximum timeout cap | None |

#### Init Config

| Field | Type | Description |
|-------|------|-------------|
| `validators` | Vec<Address> | Initial validator addresses |
| `blsPublicKeys` | Vec<String> | BLS public keys (0x + 96 hex) |

#### System Contracts

| Contract | Address | Purpose |
|----------|---------|---------|
| `govValidator` | 0x...0400 | Validator management |
| `nativeCoinAdapter` | Optional | Native coin operations |
| `govMinter` | Optional | Token minting |
| `govMasterMinter` | Optional | Minter management |
| `govCouncil` | Optional | Governance council |

### 4. Creating a Validator Node

#### Generate BLS Keys

```rust
use reth_consensus_wbft::bls::SecretKey;

// Generate new key
let secret_key = SecretKey::random();
let public_key = secret_key.public_key();

// Export keys
let sk_bytes = secret_key.to_bytes();  // 32 bytes
let pk_bytes = public_key.to_bytes();  // 48 bytes

println!("Secret key: 0x{}", hex::encode(sk_bytes));
println!("Public key: 0x{}", hex::encode(pk_bytes));
```

#### Load Existing Key

```rust
use reth_consensus_wbft::bls::SecretKey;

let key_bytes = hex::decode("your_secret_key_hex").unwrap();
let secret_key = SecretKey::from_bytes(&key_bytes).unwrap();
```

### 5. Node Configuration

#### Create Consensus Instance

```rust
use reth_consensus_wbft::{WbftConsensus, WbftConfig};
use std::sync::Arc;

// Load chain spec with WBFT config
let chain_spec = load_chain_spec("genesis.json")?;

// Create consensus config
let config = WbftConfig {
    epoch_length: 100,
    request_timeout_seconds: 2,
    block_period_seconds: 1,
    proposer_policy: 0,
    max_request_timeout_seconds: None,
};

// Create consensus instance
let consensus = WbftConsensus::new(Arc::new(chain_spec), config);
```

#### Configure Validator Provider

```rust
use reth_consensus_wbft::contracts::ContractValidatorProvider;

// For contract-based validator lookup
let validator_provider = ContractValidatorProvider::new(
    gov_validator_address,
    blockchain_reader,
);

// For genesis-based validator lookup (testing)
let validator_provider = GenesisValidatorProvider::new(
    genesis_validators,
    genesis_bls_keys,
);
```

### 6. Block Validation

WBFT automatically validates blocks through the `Consensus` trait:

```rust
use reth_consensus::Consensus;

// Validate header seals
consensus.validate_header(&sealed_header)?;

// Validate against parent
consensus.validate_header_against_parent(&header, &parent)?;

// Validate block before execution
consensus.validate_block_pre_execution(&block)?;
```

### 7. Epoch Transitions

Epoch blocks contain validator set updates:

```rust
use reth_consensus_wbft::epoch::EpochManager;

let manager = EpochManager::new(config);

// Check if block is epoch boundary
if manager.is_epoch_block(block_number) {
    // Load new validator set from EpochInfo
    let extra = WbftExtra::decode(header.extra_data())?;
    if let Some(epoch_info) = extra.epoch_info {
        // Update validator set
    }
}
```

### 8. Running Multiple Validators

For a production network, run at least 4 validators:

```bash
# Validator 1
reth node --chain genesis.json --validator-key key1.hex --port 30303

# Validator 2
reth node --chain genesis.json --validator-key key2.hex --port 30304 --bootnodes enode://...

# Validator 3
reth node --chain genesis.json --validator-key key3.hex --port 30305 --bootnodes enode://...

# Validator 4
reth node --chain genesis.json --validator-key key4.hex --port 30306 --bootnodes enode://...
```

### 9. Monitoring

#### Metrics

WBFT exposes the following metrics:

- `wbft_round_duration_seconds` - Time per consensus round
- `wbft_round_changes_total` - Number of round changes
- `wbft_messages_received_total` - Messages by type
- `wbft_blocks_sealed_total` - Successfully sealed blocks

#### Logging

Enable debug logging for WBFT:

```bash
RUST_LOG=reth_consensus_wbft=debug reth node ...
```

### 10. Troubleshooting

#### Common Issues

**Block validation fails:**
- Check that all validators have correct BLS keys
- Verify quorum threshold is met
- Ensure validators are connected

**Round changes:**
- Normal if network latency is high
- Check timeout configuration
- Verify validator connectivity

**Epoch transition fails:**
- Verify EpochInfo in block extra_data
- Check validator contract state
- Ensure BLS keys are valid

#### Debug Commands

```rust
// Check seal validity
let sealer = WbftSealer::new();
if sealer.has_seals(extra_data) {
    let seals = sealer.extract_seals(extra_data)?;
    println!("Prepared signers: {}", seals.prepared_seal.signer_count());
    println!("Committed signers: {}", seals.committed_seal.signer_count());
}

// Verify validator set
let set = validator_provider.get_validators(block_number)?;
println!("Validators: {}", set.size());
println!("Quorum: {}", set.quorum_size());
println!("Proposer: {:?}", set.proposer().address());
```

## Testing

### Unit Tests

```bash
cargo test -p reth-consensus-wbft
```

### Integration Tests

```bash
cargo test -p reth-consensus-wbft --test '*'
```

### Benchmarks

```bash
cargo bench -p reth-consensus-wbft
```

## Example: Single Validator Test

```rust
use reth_consensus_wbft::{
    bls::{SecretKey, SealerSet},
    header::WbftExtra,
    sealer::{SealResult, WbftSealer},
    validator::{DefaultValidator, DefaultValidatorSet},
    WbftAggregatedSeal,
};

// Create validator
let sk = SecretKey::random();
let pk = sk.public_key();
let validator = DefaultValidator::new(address, pk.to_bytes());
let set = DefaultValidatorSet::new(vec![validator]).unwrap();

// Create block hash
let block_hash = b"test block";

// Sign with BLS
let signature = sk.sign(block_hash);

// Create aggregated seal
let mut bitmap = SealerSet::new(1);
bitmap.set_sealer(0);
let seal = WbftAggregatedSeal::new(bitmap, signature.to_bytes());

// Apply seals
let sealer = WbftSealer::new();
let seal_result = SealResult::new(seal.clone(), seal);
let extra = WbftExtra::default();
let sealed = sealer.apply_seals(&extra.encode(), &seal_result)?;

// Verify
assert!(sealer.has_seals(&sealed));
```

## Next Steps

- See [Architecture](architecture.md) for detailed design
- See [API Reference](api.md) for complete API documentation
- Check integration tests for more examples
