//! WBFT System Contracts
//!
//! This module provides interfaces for interacting with on-chain system contracts
//! used by WBFT consensus, particularly for validator management.

pub mod gov_validator;
pub mod provider;

pub use gov_validator::{GovValidator, GovValidatorError};
pub use provider::{ContractValidatorProvider, ValidatorProviderError};
