//! WBFT Consensus Implementation
//!
//! This crate implements the WBFT consensus protocol for reth.
//! WBFT is a Byzantine Fault Tolerant consensus mechanism with BLS signature aggregation.
//!
//! # Architecture
//!
//! - **Core State Machine**: 3-phase commit protocol (PRE-PREPARE → PREPARE → COMMIT)
//! - **BLS Signatures**: Efficient signature aggregation using BLS12-381
//! - **Validator Management**: Dynamic validator set with epoch-based rotation
//! - **Message Types**: PRE-PREPARE, PREPARE, COMMIT, ROUND-CHANGE
//!
//! # Modules
//!
//! - `types`: Core data structures (View, Subject, State)
//! - `bls`: BLS signature generation, verification, and aggregation
//! - `messages`: Protocol message types and encoding
//! - `core`: State machine and consensus logic
//! - `validator`: Validator set management and proposer selection
//! - `network`: P2P message handling and broadcasting
//! - `header`: Block header extra data structures
//! - `genesis`: Genesis block initialization

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod bls;
pub mod core;
pub mod messages;
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
