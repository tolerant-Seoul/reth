//! WBFT Consensus Implementation
//!
//! This crate implements the WBFT (Byzantine Fault Tolerant) consensus protocol for reth.
//! WBFT provides immediate block finality using BLS12-381 signature aggregation for
//! efficient multi-validator consensus.
//!
//! # Features
//!
//! - **Immediate Finality**: Blocks are final once committed (no probabilistic finality)
//! - **BLS Signature Aggregation**: Constant-size signatures regardless of validator count
//! - **Byzantine Fault Tolerance**: Tolerates up to f = floor((n-1)/3) faulty validators
//! - **Dynamic Validator Sets**: Epoch-based validator rotation via system contracts
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use reth_consensus_wbft::{
//!     aggregate_signatures, verify_aggregated, SealerSet, SecretKey, WbftAggregatedSeal,
//! };
//!
//! // Generate BLS keys
//! let sk = SecretKey::random();
//! let pk = sk.public_key();
//!
//! // Sign a message
//! let message = b"block hash";
//! let signature = sk.sign(message);
//!
//! // Verify signature
//! assert!(pk.verify(message, &signature).unwrap());
//!
//! // Create aggregated seal with multiple validators
//! let mut bitmap = SealerSet::new(4);
//! bitmap.set_sealer(0);
//! let seal = WbftAggregatedSeal::new(bitmap, signature.to_bytes());
//! ```
//!
//! # Architecture
//!
//! - **Core State Machine**: 3-phase commit (PRE-PREPARE → PREPARE → COMMIT)
//! - **BLS Signatures**: Efficient aggregation using BLS12-381 curve
//! - **Validator Management**: Dynamic sets with epoch-based rotation
//! - **Round Changes**: Automatic recovery from timeout scenarios
//!
//! # Modules
//!
//! - [`types`]: Core data structures (View, Subject, State)
//! - [`bls`]: BLS signature generation, verification, and aggregation
//! - [`messages`]: Protocol message types and encoding
//! - [`core`]: State machine and consensus logic
//! - [`validator`]: Validator set management and proposer selection
//! - [`network`]: P2P message handling and broadcasting
//! - [`header`]: Block header extra data structures
//! - [`genesis`]: Genesis block initialization
//! - [`sealer`]: Block sealing with BLS signatures
//! - [`epoch`]: Epoch management and validator transitions
//! - [`consensus`]: Reth Consensus trait implementation
//! - [`contracts`]: System contract interfaces
//!
//! # Quorum Calculation
//!
//! For n validators, the system tolerates f = floor((n-1)/3) Byzantine faults.
//! Quorum size is 2f + 1.
//!
//! | Validators | f | Quorum |
//! |------------|---|--------|
//! | 3          | 0 | 1      |
//! | 4          | 1 | 3      |
//! | 7          | 2 | 5      |
//! | 10         | 3 | 7      |

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod bls;
pub mod consensus;
pub mod contracts;
pub mod core;
pub mod epoch;
pub mod genesis;
pub mod header;
pub mod messages;
pub mod network;
pub mod sealer;
pub mod types;
pub mod validator;

// Re-export commonly used types
pub use types::{State, Subject, View};

// Re-export BLS types
pub use bls::{
    aggregate_signatures, verify_aggregated, PublicKey, SealerSet, SecretKey, Signature,
    WbftAggregatedSeal,
};

// Re-export message types
pub use messages::{Commit, MessageError, PrePrepare, Prepare, RoundChange, WbftMessage};

// Re-export core types
pub use core::{Backend, Core, CoreError, MessageSet};

// Re-export validator types
pub use validator::{
    calc_proposer, DefaultValidator, DefaultValidatorSet, ProposerPolicy, Validator,
    ValidatorError, ValidatorSet,
};

// Re-export header types
pub use header::{prepare_seal_hash, Candidate, EpochInfo, SealType, WbftExtra};

// Re-export consensus types
pub use consensus::{WbftConfig, WbftConsensus};

// Re-export genesis types
pub use genesis::{create_initial_extra_data, validate_genesis_extra_data, GenesisError};

// Re-export network types
pub use network::{
    is_valid_message_code, supported_message_codes, WbftCapability, WbftMessageCode,
    WbftProtocolMessage, WBFT_PROTOCOL_ID, WBFT_VERSION,
};

// Re-export sealer types
pub use sealer::{SealResult, SealResultBuilder, SealVerificationContext, SealerError, WbftSealer};

// Re-export epoch types
pub use epoch::{EpochError, EpochManager};

// Re-export contracts types
pub use contracts::{
    ContractValidatorProvider, GovValidator, GovValidatorError, ValidatorProviderError,
};
