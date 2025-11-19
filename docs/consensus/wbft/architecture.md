# WBFT Consensus Architecture

This document describes the architecture of the WBFT (Byzantine Fault Tolerant) consensus implementation in reth.

## Overview

WBFT is a BFT consensus protocol that uses BLS12-381 signature aggregation for efficient multi-validator block finality. It provides immediate block finality with Byzantine fault tolerance for up to f = floor((n-1)/3) faulty validators.

## Core Components

### 1. BLS Signature System (`src/bls/`)

The BLS module provides cryptographic primitives for signature operations:

```
bls/
├── mod.rs           # Module exports and SecretKey/PublicKey/Signature types
├── aggregation.rs   # WbftAggregatedSeal and signature aggregation
└── sealer_set.rs    # SealerSet bitmap for tracking signers
```

**Key Types:**
- `SecretKey`: 32-byte BLS12-381 secret key
- `PublicKey`: 48-byte BLS12-381 public key
- `Signature`: 96-byte BLS12-381 signature
- `SealerSet`: Bitmap tracking which validators have signed
- `WbftAggregatedSeal`: Combined bitmap + aggregated signature

**Operations:**
- Individual signing and verification
- Multi-signature aggregation
- Fast aggregate verification

### 2. Validator Management (`src/validator/`)

Manages validator sets and proposer selection:

```
validator/
├── mod.rs      # Validator trait and DefaultValidator
├── set.rs      # ValidatorSet trait and DefaultValidatorSet
└── policy.rs   # ProposerPolicy (RoundRobin, Sticky)
```

**Key Traits:**
- `Validator`: Interface for individual validators (address, BLS public key)
- `ValidatorSet`: Interface for validator collections with quorum calculation

**Quorum Calculation:**
```
n = number of validators
f = floor((n-1)/3)      # Maximum Byzantine faults tolerated
quorum = 2f + 1         # Minimum signatures needed
```

| Validators | f | Quorum |
|------------|---|--------|
| 3          | 0 | 1      |
| 4          | 1 | 3      |
| 7          | 2 | 5      |
| 10         | 3 | 7      |

### 3. Header Extra Data (`src/header.rs`)

WBFT consensus data stored in block header's extra_data field:

```rust
pub struct WbftExtra {
    pub vanity_data: [u8; 32],           // Vanity bytes
    pub randao_reveal: Vec<u8>,          // RANDAO reveal (96 bytes)
    pub prev_round: u32,                 // Previous round number
    pub prev_prepared_seal: Option<WbftAggregatedSeal>,
    pub prev_committed_seal: Option<WbftAggregatedSeal>,
    pub round: u32,                      // Current round
    pub prepared_seal: Option<WbftAggregatedSeal>,
    pub committed_seal: Option<WbftAggregatedSeal>,
    pub gas_tip: U256,                   // Minimum gas tip
    pub epoch_info: Option<EpochInfo>,   // Epoch transition data
}
```

### 4. Consensus Messages (`src/messages/`)

Four message types for the consensus protocol:

```
messages/
├── mod.rs          # WbftMessage trait
├── preprepare.rs   # PRE-PREPARE message
├── prepare.rs      # PREPARE message
├── commit.rs       # COMMIT message
└── round_change.rs # ROUND-CHANGE message
```

**Message Flow:**
1. **PRE-PREPARE**: Proposer broadcasts block proposal
2. **PREPARE**: Validators vote for block (BLS signature)
3. **COMMIT**: Validators commit to block (BLS signature)
4. **ROUND-CHANGE**: Timeout triggers new round

### 5. Block Sealing (`src/sealer.rs`)

Manages seal application and extraction:

```rust
pub struct WbftSealer;

impl WbftSealer {
    pub fn apply_seals(&self, extra_data: &[u8], seals: &SealResult) -> Result<Vec<u8>>;
    pub fn extract_seals(&self, extra_data: &[u8]) -> Result<SealResult>;
    pub fn has_seals(&self, extra_data: &[u8]) -> bool;
}
```

### 6. Consensus Engine (`src/consensus.rs`)

Implements reth's `Consensus` and `HeaderValidator` traits:

```rust
pub struct WbftConsensus<C> {
    chain_spec: Arc<C>,
    config: WbftConfig,
}

impl<C: EthereumHardforks> Consensus for WbftConsensus<C> {
    fn validate_header(&self, header: &SealedHeader) -> Result<()>;
    fn validate_header_against_parent(&self, header: &SealedHeader, parent: &SealedHeader) -> Result<()>;
    fn validate_block_pre_execution(&self, block: &SealedBlock) -> Result<()>;
}
```

### 7. Epoch Management (`src/epoch.rs`)

Handles periodic validator set updates:

```rust
pub struct EpochManager {
    config: WbftConfig,
}

impl EpochManager {
    pub fn is_epoch_block(&self, number: u64) -> bool;
    pub fn epoch_number(&self, block_number: u64) -> u64;
    pub fn epoch_start_block(&self, epoch: u64) -> u64;
    pub fn epoch_end_block(&self, epoch: u64) -> u64;
}
```

### 8. System Contracts (`src/contracts/`)

Interface to on-chain validator management:

```
contracts/
├── mod.rs       # Contract interfaces
└── provider.rs  # ValidatorProvider implementation
```

**GovValidator Contract:**
- Reads active validator list
- Reads BLS public keys
- Reads gas tip configuration

## State Machine

The WBFT consensus operates as a state machine:

```
                    ┌─────────────────┐
                    │  AcceptRequest  │
                    └────────┬────────┘
                             │ PRE-PREPARE received
                             ▼
                    ┌─────────────────┐
                    │   Preprepared   │
                    └────────┬────────┘
                             │ Quorum PREPARE messages
                             ▼
                    ┌─────────────────┐
                    │    Prepared     │
                    └────────┬────────┘
                             │ Quorum COMMIT messages
                             ▼
                    ┌─────────────────┐
                    │   Committed     │
                    └─────────────────┘

    Timeout at any state → ROUND-CHANGE → New round
```

## Consensus Flow

### Normal Case (Round 0)

1. **Block Production**
   - Proposer (validator 0 for round 0) creates block
   - Broadcasts PRE-PREPARE with unsigned block

2. **Prepare Phase**
   - Validators verify block and proposer
   - Sign block hash with BLS key
   - Broadcast PREPARE message

3. **Commit Phase**
   - Upon receiving 2f+1 PREPARE messages
   - Aggregate signatures into prepared_seal
   - Sign commit message with BLS key
   - Broadcast COMMIT message

4. **Finalization**
   - Upon receiving 2f+1 COMMIT messages
   - Aggregate signatures into committed_seal
   - Apply seals to block header
   - Block is finalized

### Round Change

When timeout occurs:
1. Validator broadcasts ROUND-CHANGE for next round
2. Upon f+1 ROUND-CHANGE messages, move to new round
3. Upon 2f+1 ROUND-CHANGE messages, new proposer starts
4. If prepared block exists, it must be re-proposed

## Seal Verification

Block seals are verified by:

1. **Extract seals** from header extra_data
2. **Load validator set** for the block height
3. **Compute seal hash**: `keccak256(header_hash_with_round || seal_type)`
4. **Verify aggregated signature** against signing validators' public keys
5. **Check quorum**: signer_count >= quorum_size

## Performance Characteristics

- **Finality**: Immediate (no probabilistic finality)
- **Signature size**: 96 bytes (constant, regardless of validator count)
- **Verification**: O(n) where n = number of signers
- **Aggregation**: O(n) for combining signatures

## Security Properties

- **Safety**: No two conflicting blocks can be finalized
- **Liveness**: Progress guaranteed with 2f+1 honest validators
- **Accountability**: Byzantine validators can be identified via signatures

## Configuration

```rust
pub struct WbftConfig {
    pub epoch_length: u64,              // Blocks per epoch
    pub request_timeout_seconds: u64,   // Initial timeout
    pub block_period_seconds: u64,      // Target block time
    pub proposer_policy: u64,           // 0=RoundRobin, 1=Sticky
    pub max_request_timeout_seconds: Option<u64>,
}
```

## Related Files

- `crates/consensus/wbft/` - Main implementation
- `crates/chainspec/src/wbft.rs` - Chain configuration
- `docs/consensus/wbft/integration.md` - Integration guide
- `docs/consensus/wbft/api.md` - API reference
