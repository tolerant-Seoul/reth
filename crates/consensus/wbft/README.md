# reth-consensus-wbft

WBFT (Byzantine Fault Tolerant) consensus implementation for Reth.

## Overview

This crate implements the WBFT consensus protocol, a Byzantine Fault Tolerant consensus mechanism featuring BLS signature aggregation for efficient multi-validator consensus.

## Features

- **3-Phase Commit Protocol**: PRE-PREPARE → PREPARE → COMMIT
- **BLS12-381 Signatures**: Efficient signature aggregation for validator consensus
- **Dynamic Validator Sets**: Epoch-based validator rotation
- **Round Change Protocol**: Automatic recovery from timeout scenarios

## Architecture

### Core Components

- **State Machine** (`types`): Core data structures (View, Subject, State)
- **BLS Signatures** (`bls`): Key management, signing, and aggregation
- **Message Protocol** (`messages`): PRE-PREPARE, PREPARE, COMMIT, ROUND-CHANGE messages
- **Core Engine** (`core`): Consensus state machine and message processing
- **Validator Management** (`validator`): Validator set management and proposer selection
- **Network Layer** (planned): P2P message broadcasting and handling

## Current Implementation Status

### Phase 1-4: Core Implementation (Completed)

- ✅ BLS12-381 signature system with aggregation
- ✅ Validator set management and proposer selection
- ✅ 3-phase consensus protocol (PRE-PREPARE, PREPARE, COMMIT)
- ✅ Round change mechanism with timeout handling
- ✅ Block header extra data (WbftExtra)
- ✅ Consensus trait implementation for reth integration
- ✅ ChainSpec and Genesis integration
- ✅ System contract interfaces
- ✅ Epoch management

### Phase 5: Testing and Documentation (Completed)

- ✅ 249 unit tests
- ✅ 40 integration tests (single/multi-validator, round change, epoch)
- ✅ Performance benchmarks (BLS, messages, consensus)
- ✅ Comprehensive documentation

## Documentation

- [Architecture Guide](../../../docs/consensus/wbft/architecture.md) - System design and components
- [Integration Guide](../../../docs/consensus/wbft/integration.md) - How to integrate WBFT
- [API Reference](../../../docs/consensus/wbft/api.md) - Complete API documentation

## Usage

```rust
use reth_consensus_wbft::{
    SecretKey, PublicKey, Signature,
    aggregate_signatures, verify_aggregated,
    View, State, Subject,
    PrePrepare, Prepare, Commit, RoundChange,
    Validator, ValidatorSet, DefaultValidator, DefaultValidatorSet,
    ProposerPolicy, calc_proposer,
    WbftExtra, EpochInfo, Candidate, SealType, prepare_seal_hash,
    WbftConfig, WbftConsensus,
};
use alloy_primitives::{Address, Bytes, B256, U256};
use reth_chainspec::MAINNET;

// Generate BLS keys
let sk = SecretKey::random();
let pk = sk.public_key();

// Sign a message
let message = b"block hash";
let signature = sk.sign(message);

// Verify signature
assert!(pk.verify(message, &signature));

// Create consensus messages
let view = View { sequence: U256::from(1), round: U256::from(0) };
let proposal = B256::from([0x42; 32]);

// PRE-PREPARE message from proposer
let preprepare = PrePrepare::new(
    view.clone(),
    proposal,
    Bytes::from(vec![/* block data */]),
    Address::from([0x01; 20]),
    signature.to_bytes(),
);

// PREPARE message from validator
let prepare = Prepare::new(
    view.clone(),
    proposal,
    Address::from([0x02; 20]),
    signature.to_bytes(),
);

// Aggregate multiple signatures
let signatures = vec![sig1, sig2, sig3];
let aggregated = aggregate_signatures(&signatures)?;

// Verify aggregated signature
let public_keys = vec![&pk1, &pk2, &pk3];
assert!(verify_aggregated(&public_keys, message, &aggregated)?);

// Create validator set
let validators = vec![
    DefaultValidator::new(Address::from([0x01; 20]), pk1.to_bytes()),
    DefaultValidator::new(Address::from([0x02; 20]), pk2.to_bytes()),
    DefaultValidator::new(Address::from([0x03; 20]), pk3.to_bytes()),
    DefaultValidator::new(Address::from([0x04; 20]), pk4.to_bytes()),
];
let mut validator_set = DefaultValidatorSet::new(validators)?;

// Calculate proposer using round-robin
let proposer = calc_proposer(
    ProposerPolicy::RoundRobin,
    &mut validator_set,
    Address::ZERO,
    0, // round
)?;

// Quorum calculation (2f+1 where f = (n-1)/3)
let quorum = validator_set.quorum_size(); // 3 for 4 validators

// Create block header extra data
let mut extra = WbftExtra::new();
extra.round = 0;
extra.gas_tip = U256::from(1_000_000_000u64); // 1 Gwei

// Add epoch information (for genesis/epoch blocks)
let epoch_info = EpochInfo {
    candidates: vec![
        Candidate { addr: Address::from([0x01; 20]), diligence: 950_000 },
    ],
    validators: vec![0],
    bls_public_keys: vec![vec![0x42; 48]],
};
extra.epoch_info = Some(epoch_info);

// Encode extra data for block header
let encoded_extra = extra.encode();

// Decode extra data from block header
let decoded_extra = WbftExtra::decode(&encoded_extra)?;

// Calculate prepare seal hash for signing
let block_hash = B256::from([0x42; 32]);
let round = 0;
let prepare_hash = prepare_seal_hash(block_hash, round, SealType::Prepare);
let commit_hash = prepare_seal_hash(block_hash, round, SealType::Commit);

// Create WBFT consensus instance
let wbft_config = WbftConfig {
    request_timeout_seconds: 2,
    block_period_seconds: 1,
    proposer_policy: 0, // RoundRobin
    epoch_length: 10,
    max_request_timeout_seconds: None,
};
let consensus = WbftConsensus::new(MAINNET.clone(), wbft_config);

// Check if a block is an epoch block
let is_epoch = consensus.is_epoch_block(10); // true
let is_regular = consensus.is_epoch_block(5); // false
```

## Testing

Run tests with:

```bash
# Unit tests
cargo test -p reth-consensus-wbft --lib

# Integration tests
cargo test -p reth-consensus-wbft --test '*'

# All tests
cargo test -p reth-consensus-wbft

# Benchmarks
cargo bench -p reth-consensus-wbft
```

**Test Coverage:**

- **249 unit tests** covering all modules
- **40 integration tests** covering:
  - Single validator consensus
  - Multi-validator consensus (3/4/7 validators)
  - Round change scenarios
  - Epoch transitions
  - Quorum verification
  - Proposer rotation

**Benchmarks:**

- BLS signature operations (sign, verify, aggregate)
- Message encoding/decoding throughput
- Consensus round latency
- Validator set operations

## Dependencies

- `blst`: BLS12-381 signature library
- `alloy-primitives`: Ethereum primitive types
- `alloy-rlp`: RLP encoding/decoding
- `reth-primitives`: Reth primitive types
- `reth-consensus`: Consensus trait definitions

## License

Licensed under either of:

- Apache License, Version 2.0, ([LICENSE-APACHE](../../../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
