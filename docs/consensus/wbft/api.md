# WBFT API Reference

Complete API reference for the WBFT consensus implementation.

## BLS Module

### SecretKey

```rust
pub struct SecretKey(/* private */);

impl SecretKey {
    /// Generate a random secret key
    pub fn random() -> Self;

    /// Create from raw bytes (32 bytes)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BlsError>;

    /// Export to bytes
    pub fn to_bytes(&self) -> [u8; 32];

    /// Derive public key
    pub fn public_key(&self) -> PublicKey;

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Signature;
}
```

### PublicKey

```rust
pub struct PublicKey(/* private */);

impl PublicKey {
    /// Create from raw bytes (48 bytes)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BlsError>;

    /// Export to bytes
    pub fn to_bytes(&self) -> [u8; 48];

    /// Verify a signature
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<bool, BlsError>;
}
```

### Signature

```rust
pub struct Signature(/* private */);

impl Signature {
    /// Create from raw bytes (96 bytes)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BlsError>;

    /// Export to bytes
    pub fn to_bytes(&self) -> [u8; 96];
}
```

### SealerSet

```rust
pub struct SealerSet(/* private */);

impl SealerSet {
    /// Create new bitmap for given number of validators
    pub fn new(size: usize) -> Self;

    /// Mark validator as signer
    pub fn set_sealer(&mut self, index: u32);

    /// Check if validator signed
    pub fn is_sealer(&self, index: u32) -> bool;

    /// Get indices of all signers
    pub fn get_sealers(&self) -> Vec<u32>;

    /// Count number of signers
    pub fn count(&self) -> usize;
}
```

### WbftAggregatedSeal

```rust
pub struct WbftAggregatedSeal {
    pub bitmap: SealerSet,
    pub signature: [u8; 96],
}

impl WbftAggregatedSeal {
    /// Create new aggregated seal
    pub fn new(bitmap: SealerSet, signature: [u8; 96]) -> Self;

    /// Verify aggregated signature
    pub fn verify(&self, public_keys: &[PublicKey], message: &[u8]) -> Result<bool, BlsError>;

    /// Get number of signers
    pub fn signer_count(&self) -> usize;
}
```

### Aggregation Functions

```rust
/// Aggregate multiple signatures into one
pub fn aggregate_signatures(signatures: &[Signature]) -> Result<Signature, BlsError>;

/// Verify an aggregated signature against multiple public keys
pub fn verify_aggregated(
    public_keys: &[&PublicKey],
    message: &[u8],
    signature: &Signature
) -> Result<bool, BlsError>;
```

## Validator Module

### Validator Trait

```rust
pub trait Validator: Clone + Send + Sync {
    /// Get validator's Ethereum address
    fn address(&self) -> Address;

    /// Get validator's BLS public key bytes
    fn bls_public_key(&self) -> &[u8];
}
```

### DefaultValidator

```rust
pub struct DefaultValidator {
    addr: Address,
    bls_key: [u8; 48],
}

impl DefaultValidator {
    /// Create new validator
    pub fn new(addr: Address, bls_key: [u8; 48]) -> Self;

    /// Create from BLS key bytes (validates length)
    pub fn from_bytes(addr: Address, bls_key: &[u8]) -> Result<Self, ValidatorError>;
}
```

### ValidatorSet Trait

```rust
pub trait ValidatorSet: Send + Sync {
    /// Get current proposer
    fn proposer(&self) -> &dyn Validator;

    /// Get validator by address
    fn get_by_address(&self, addr: &Address) -> Result<&dyn Validator, ValidatorError>;

    /// Get validator by index
    fn get_by_index(&self, index: usize) -> Result<&dyn Validator, ValidatorError>;

    /// Get number of validators
    fn size(&self) -> usize;

    /// Get all validators
    fn list(&self) -> Vec<&dyn Validator>;

    /// Calculate Byzantine fault tolerance (f)
    fn f(&self) -> usize;

    /// Calculate quorum size (2f + 1)
    fn quorum_size(&self) -> usize;

    /// Check if address is a validator
    fn is_validator(&self, addr: &Address) -> bool;

    /// Get index of validator by address
    fn get_index(&self, addr: &Address) -> Result<usize, ValidatorError>;
}
```

### DefaultValidatorSet

```rust
pub struct DefaultValidatorSet {
    validators: Vec<DefaultValidator>,
    proposer_index: usize,
}

impl DefaultValidatorSet {
    /// Create new validator set (must not be empty)
    pub fn new(validators: Vec<DefaultValidator>) -> Result<Self, ValidatorError>;

    /// Set proposer by index
    pub fn set_proposer(&mut self, index: usize) -> Result<(), ValidatorError>;

    /// Set proposer by address
    pub fn set_proposer_by_address(&mut self, addr: &Address) -> Result<(), ValidatorError>;

    /// Calculate next proposer (round-robin)
    pub fn calc_proposer(&mut self, round: u64);

    /// Get validators as slice
    pub fn validators(&self) -> &[DefaultValidator];

    /// Get proposer index
    pub fn proposer_index(&self) -> usize;
}
```

### ProposerPolicy

```rust
pub enum ProposerPolicy {
    RoundRobin = 0,
    Sticky = 1,
}

/// Calculate proposer based on policy
pub fn calc_proposer(
    policy: ProposerPolicy,
    validators: &[Address],
    last_proposer: Address,
    round: u64
) -> Address;
```

## Header Module

### WbftExtra

```rust
pub struct WbftExtra {
    pub vanity_data: [u8; 32],
    pub randao_reveal: Vec<u8>,
    pub prev_round: u32,
    pub prev_prepared_seal: Option<WbftAggregatedSeal>,
    pub prev_committed_seal: Option<WbftAggregatedSeal>,
    pub round: u32,
    pub prepared_seal: Option<WbftAggregatedSeal>,
    pub committed_seal: Option<WbftAggregatedSeal>,
    pub gas_tip: U256,
    pub epoch_info: Option<EpochInfo>,
}

impl WbftExtra {
    /// RLP encode to bytes
    pub fn encode(&self) -> Vec<u8>;

    /// RLP decode from bytes
    pub fn decode(data: &mut &[u8]) -> Result<Self, DecodeError>;
}
```

### EpochInfo

```rust
pub struct EpochInfo {
    pub candidates: Vec<Candidate>,
    pub validators: Vec<u32>,
    pub bls_public_keys: Vec<Vec<u8>>,
}

pub struct Candidate {
    pub addr: Address,
    pub diligence: u64,
}
```

## Sealer Module

### WbftSealer

```rust
pub struct WbftSealer;

impl WbftSealer {
    /// Create new sealer
    pub fn new() -> Self;

    /// Apply seals to extra data
    pub fn apply_seals(&self, extra_data: &[u8], seals: &SealResult) -> Result<Vec<u8>, SealerError>;

    /// Extract seals from extra data
    pub fn extract_seals(&self, extra_data: &[u8]) -> Result<SealResult, SealerError>;

    /// Check if extra data has seals
    pub fn has_seals(&self, extra_data: &[u8]) -> bool;

    /// Create empty seals for genesis
    pub fn create_genesis_seals(&self) -> SealResult;
}
```

### SealResult

```rust
pub struct SealResult {
    pub prepared_seal: WbftAggregatedSeal,
    pub committed_seal: WbftAggregatedSeal,
}

impl SealResult {
    pub fn new(prepared_seal: WbftAggregatedSeal, committed_seal: WbftAggregatedSeal) -> Self;
}
```

## Consensus Module

### WbftConfig

```rust
pub struct WbftConfig {
    pub epoch_length: u64,
    pub request_timeout_seconds: u64,
    pub block_period_seconds: u64,
    pub proposer_policy: u64,
    pub max_request_timeout_seconds: Option<u64>,
}
```

### WbftConsensus

```rust
pub struct WbftConsensus<C> {
    chain_spec: Arc<C>,
    config: WbftConfig,
}

impl<C> WbftConsensus<C> {
    /// Create new consensus instance
    pub fn new(chain_spec: Arc<C>, config: WbftConfig) -> Self;

    /// Check if block is epoch boundary
    pub fn is_epoch_block(&self, number: u64) -> bool;
}

// Implements reth_consensus::Consensus trait
impl<C: EthereumHardforks> Consensus for WbftConsensus<C> {
    type Error = ConsensusError;

    fn validate_header(&self, header: &SealedHeader) -> Result<(), Self::Error>;
    fn validate_header_against_parent(&self, header: &SealedHeader, parent: &SealedHeader) -> Result<(), Self::Error>;
    fn validate_body_against_header(&self, body: &BlockBody, header: &SealedHeader) -> Result<(), Self::Error>;
    fn validate_block_pre_execution(&self, block: &SealedBlock) -> Result<(), Self::Error>;
}
```

## Epoch Module

### EpochManager

```rust
pub struct EpochManager {
    config: WbftConfig,
}

impl EpochManager {
    /// Create new epoch manager
    pub fn new(config: WbftConfig) -> Self;

    /// Get epoch length
    pub fn epoch_length(&self) -> u64;

    /// Check if block is epoch boundary
    pub fn is_epoch_block(&self, number: u64) -> bool;

    /// Get epoch number for block
    pub fn epoch_number(&self, block_number: u64) -> u64;

    /// Get first block of epoch
    pub fn epoch_start_block(&self, epoch: u64) -> u64;

    /// Get last block of epoch
    pub fn epoch_end_block(&self, epoch: u64) -> u64;
}
```

## Messages Module

### WbftMessage Trait

```rust
pub trait WbftMessage {
    /// Validate message fields
    fn validate(&self) -> Result<(), MessageError>;
}
```

### RoundChange

```rust
pub struct RoundChange {
    pub view: View,
    pub sender: Address,
    pub signature: [u8; 96],
    pub prepared_digest: Option<B256>,
    pub prepared_view: Option<View>,
}

impl RoundChange {
    /// Create new round change message
    pub fn new(view: View, sender: Address, signature: [u8; 96]) -> Self;

    /// Create with prepared block info
    pub fn with_prepared(
        view: View,
        sender: Address,
        signature: [u8; 96],
        prepared_digest: B256,
        prepared_view: View
    ) -> Self;

    /// Check if has prepared block
    pub fn has_prepared(&self) -> bool;
}
```

## Types Module

### View

```rust
pub struct View {
    pub sequence: U256,
    pub round: U256,
}

impl View {
    /// Create genesis view
    pub fn genesis() -> Self;

    /// Create new view
    pub fn new(sequence: U256, round: U256) -> Self;

    /// Compare views
    pub fn cmp(&self, other: &View) -> Ordering;

    /// Get next round view
    pub fn next_round(&self) -> View;

    /// Get next sequence view
    pub fn next_sequence(&self) -> View;
}
```

### State

```rust
pub enum State {
    AcceptRequest,
    Preprepared,
    Prepared,
    Committed,
}
```

### Subject

```rust
pub struct Subject {
    pub view: View,
    pub digest: B256,
}
```

## Error Types

### BlsError

```rust
pub enum BlsError {
    InvalidSecretKey,
    InvalidPublicKey,
    InvalidSignature,
    AggregationFailed(String),
    VerificationFailed(String),
}
```

### ValidatorError

```rust
pub enum ValidatorError {
    EmptySet,
    NotFound(Address),
    InvalidIndex { index: usize, size: usize },
    InvalidBlsKey,
}
```

### SealerError

```rust
pub enum SealerError {
    DecodeFailed(String),
    MissingSeals,
    InvalidFormat,
}
```

### ConsensusError

```rust
pub enum ConsensusError {
    InvalidSeal,
    InvalidProposer,
    InsufficientQuorum,
    InvalidExtraData,
    // ... other variants
}
```

## Genesis Module

### Genesis Functions

```rust
/// Create initial extra data for genesis block
pub fn create_initial_extra_data(
    validators: &[Address],
    bls_public_keys: &[Vec<u8>],
    gas_tip: U256
) -> Result<Vec<u8>, GenesisError>;

/// Create initial epoch info
pub fn create_initial_epoch_info(
    validators: &[Address],
    bls_public_keys: &[Vec<u8>]
) -> EpochInfo;
```

## Contract Provider

### ValidatorProvider Trait

```rust
pub trait ValidatorProvider: Send + Sync {
    /// Get validator set for block number
    fn get_validators(&self, block_number: u64) -> Result<Arc<dyn ValidatorSet>, ProviderError>;
}
```

### ContractValidatorProvider

```rust
pub struct ContractValidatorProvider {
    // ...
}

impl ContractValidatorProvider {
    /// Create new provider
    pub fn new(gov_validator: Address, blockchain: Arc<dyn BlockReader>) -> Self;
}
```

## Re-exports

The crate root re-exports commonly used types:

```rust
pub use bls::{
    aggregate_signatures, verify_aggregated,
    PublicKey, SecretKey, SealerSet, Signature,
};
pub use header::WbftExtra;
pub use sealer::{SealResult, WbftSealer};
pub use validator::{DefaultValidator, DefaultValidatorSet, Validator, ValidatorSet};
pub use WbftAggregatedSeal;
pub use messages::WbftMessage;
pub use types::{State, Subject, View};
pub use consensus::{WbftConfig, WbftConsensus};
pub use epoch::EpochManager;
```
