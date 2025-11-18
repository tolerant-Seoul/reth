//! Validator management for WBFT consensus
//!
//! This module provides validator set management, including validator
//! representation and proposer selection policies.

use alloy_primitives::Address;
use std::fmt;

pub mod policy;
pub mod set;

pub use policy::{ProposerPolicy, calc_proposer};
pub use set::{DefaultValidatorSet, ValidatorSet};

/// Trait representing a consensus validator
///
/// Each validator has an address and a BLS public key for signature
/// verification.
pub trait Validator: Send + Sync + fmt::Debug {
    /// Get validator's Ethereum address
    fn address(&self) -> Address;

    /// Get validator's BLS public key (48 bytes compressed)
    fn bls_public_key(&self) -> &[u8; 48];

    /// Check if this validator matches the given address
    fn matches(&self, addr: &Address) -> bool {
        self.address() == *addr
    }
}

/// Default implementation of Validator
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultValidator {
    /// Ethereum address of validator
    addr: Address,

    /// BLS public key (48 bytes compressed)
    bls_key: [u8; 48],
}

impl DefaultValidator {
    /// Create a new validator
    ///
    /// # Arguments
    ///
    /// * `addr` - Ethereum address
    /// * `bls_key` - BLS public key (48 bytes)
    pub fn new(addr: Address, bls_key: [u8; 48]) -> Self {
        Self { addr, bls_key }
    }

    /// Create validator from address and key bytes
    ///
    /// # Errors
    ///
    /// Returns error if BLS key is not exactly 48 bytes
    pub fn from_bytes(addr: Address, bls_key: &[u8]) -> Result<Self, ValidatorError> {
        if bls_key.len() != 48 {
            return Err(ValidatorError::InvalidKeyLength {
                expected: 48,
                actual: bls_key.len(),
            });
        }

        let mut key = [0u8; 48];
        key.copy_from_slice(bls_key);

        Ok(Self { addr, bls_key: key })
    }
}

impl Validator for DefaultValidator {
    fn address(&self) -> Address {
        self.addr
    }

    fn bls_public_key(&self) -> &[u8; 48] {
        &self.bls_key
    }
}

/// Errors that can occur during validator operations
#[derive(Debug, thiserror::Error)]
pub enum ValidatorError {
    /// Invalid BLS key length
    #[error("invalid BLS key length: expected {expected}, got {actual}")]
    InvalidKeyLength {
        /// Expected length
        expected: usize,
        /// Actual length
        actual: usize,
    },

    /// Validator not found
    #[error("validator not found: {0}")]
    NotFound(Address),

    /// Empty validator set
    #[error("validator set is empty")]
    EmptySet,

    /// Invalid validator index
    #[error("invalid validator index: {index} (set size: {size})")]
    InvalidIndex {
        /// Requested index
        index: usize,
        /// Set size
        size: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bls::SecretKey;

    #[test]
    fn test_default_validator_creation() {
        let addr = Address::from([0x01; 20]);
        let bls_key = [0x42; 48];

        let validator = DefaultValidator::new(addr, bls_key);

        assert_eq!(validator.address(), addr);
        assert_eq!(validator.bls_public_key(), &bls_key);
    }

    #[test]
    fn test_validator_from_bytes() {
        let addr = Address::from([0x01; 20]);
        let bls_key_vec = vec![0x42; 48];

        let validator = DefaultValidator::from_bytes(addr, &bls_key_vec).unwrap();

        assert_eq!(validator.address(), addr);
        assert_eq!(validator.bls_public_key(), &[0x42; 48]);
    }

    #[test]
    fn test_validator_from_bytes_invalid_length() {
        let addr = Address::from([0x01; 20]);
        let bls_key_vec = vec![0x42; 47]; // Wrong length

        let result = DefaultValidator::from_bytes(addr, &bls_key_vec);
        assert!(result.is_err());
    }

    #[test]
    fn test_validator_matches() {
        let addr = Address::from([0x01; 20]);
        let validator = DefaultValidator::new(addr, [0x42; 48]);

        assert!(validator.matches(&addr));
        assert!(!validator.matches(&Address::from([0x02; 20])));
    }

    #[test]
    fn test_validator_with_real_bls_key() {
        let sk = SecretKey::random();
        let pk = sk.public_key();
        let addr = Address::from([0x01; 20]);

        let validator = DefaultValidator::new(addr, pk.to_bytes());

        assert_eq!(validator.address(), addr);
        assert_eq!(validator.bls_public_key(), &pk.to_bytes());
    }

    #[test]
    fn test_validator_clone() {
        let validator = DefaultValidator::new(Address::from([0x01; 20]), [0x42; 48]);
        let cloned = validator.clone();

        assert_eq!(validator.address(), cloned.address());
        assert_eq!(validator.bls_public_key(), cloned.bls_public_key());
    }
}
