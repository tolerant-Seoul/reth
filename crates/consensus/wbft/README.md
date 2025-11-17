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
- **Message Protocol** (planned): PRE-PREPARE, PREPARE, COMMIT, ROUND-CHANGE messages
- **Validator Management** (planned): Validator set management and proposer selection
- **Network Layer** (planned): P2P message broadcasting and handling

## Current Implementation Status

### Phase 1: Foundation (Completed)

- ✅ Core data structures (View, Subject, State)
- ✅ BLS signature implementation
- ✅ Signature aggregation
- ✅ Sealer set bitmap
- ✅ Comprehensive test coverage (29 tests passing)

### Next Phases

- Phase 2: Message types and protocol logic
- Phase 3: Consensus trait implementation
- Phase 4: Network integration
- Phase 5: Testing and optimization

## Usage

```rust
use reth_consensus_wbft::{
    SecretKey, PublicKey, Signature,
    aggregate_signatures, verify_aggregated,
    View, State, Subject,
};

// Generate BLS keys
let sk = SecretKey::random();
let pk = sk.public_key();

// Sign a message
let message = b"block hash";
let signature = sk.sign(message);

// Verify signature
assert!(pk.verify(message, &signature));

// Aggregate multiple signatures
let signatures = vec![sig1, sig2, sig3];
let aggregated = aggregate_signatures(&signatures)?;

// Verify aggregated signature
let public_keys = vec![&pk1, &pk2, &pk3];
assert!(verify_aggregated(&public_keys, message, &aggregated)?);
```

## Testing

Run tests with:

```bash
cargo test -p reth-consensus-wbft
```

All 29 unit tests currently pass, covering:
- BLS key generation and serialization
- Signature creation and verification
- Signature aggregation (2-7 validators)
- Sealer set bitmap operations
- View ordering and comparison
- RLP encoding/decoding

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
