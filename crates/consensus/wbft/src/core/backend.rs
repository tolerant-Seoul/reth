//! Backend trait for blockchain integration
//!
//! Defines the interface between consensus engine and blockchain state.

use crate::validator::ValidatorSet;
use alloy_primitives::{Address, Bytes, B256};
use std::sync::Arc;

/// Backend error type
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendError(pub String);

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Backend error: {}", self.0)
    }
}

impl std::error::Error for BackendError {}

impl From<String> for BackendError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for BackendError {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Backend trait for blockchain operations
///
/// This trait provides the interface for the consensus engine to interact
/// with the underlying blockchain state, including block validation,
/// finalization, message broadcasting, and cryptographic operations.
pub trait Backend: Send + Sync {
    /// Get the address of this node
    ///
    /// Returns the Ethereum address associated with this validator node.
    fn address(&self) -> Address;

    /// Get validator set for a proposal
    ///
    /// Returns the validator set that should validate the given proposal.
    /// The validator set may change based on epoch transitions.
    ///
    /// # Arguments
    ///
    /// * `proposal` - RLP-encoded block proposal
    ///
    /// # Returns
    ///
    /// Arc-wrapped ValidatorSet for the proposal
    fn validators(&self, proposal: &Bytes) -> Result<Arc<dyn ValidatorSet>, BackendError>;

    /// Verify a block proposal
    ///
    /// Validates the proposed block against blockchain rules without
    /// executing it. This includes checks for:
    /// - Valid block structure
    /// - Valid transactions
    /// - State transition validity
    ///
    /// # Arguments
    ///
    /// * `proposal` - RLP-encoded block data
    ///
    /// # Errors
    ///
    /// Returns error if proposal is invalid
    fn verify_proposal(&self, proposal: &Bytes) -> Result<(), BackendError>;

    /// Commit a finalized block
    ///
    /// Finalizes and executes the block, updating blockchain state.
    /// The block has received quorum of COMMIT messages and is considered
    /// final.
    ///
    /// # Arguments
    ///
    /// * `hash` - Block hash to commit
    /// * `seals` - Aggregated commit seals from validators
    ///
    /// # Errors
    ///
    /// Returns error if commit fails
    fn commit(&self, hash: B256, seals: Vec<[u8; 96]>) -> Result<(), BackendError>;

    /// Broadcast a message to all validators (including self)
    ///
    /// Sends the consensus message to all validators in the current set,
    /// including this node (for local processing).
    ///
    /// # Arguments
    ///
    /// * `message_code` - Type of consensus message
    /// * `payload` - RLP-encoded message data
    ///
    /// # Errors
    ///
    /// Returns error if broadcast fails
    fn broadcast(&self, message_code: u8, payload: &[u8]) -> Result<(), BackendError>;

    /// Gossip a message to other validators (excluding self)
    ///
    /// Sends the consensus message to all other validators in the current set,
    /// excluding this node.
    ///
    /// # Arguments
    ///
    /// * `message_code` - Type of consensus message
    /// * `payload` - RLP-encoded message data
    ///
    /// # Errors
    ///
    /// Returns error if gossip fails
    fn gossip(&self, message_code: u8, payload: &[u8]) -> Result<(), BackendError>;

    /// Sign data with ECDSA private key
    ///
    /// Signs the given data using this node's ECDSA private key.
    /// Used for signing consensus messages.
    ///
    /// # Arguments
    ///
    /// * `data` - Data to sign
    ///
    /// # Returns
    ///
    /// ECDSA signature bytes
    fn sign(&self, data: &[u8]) -> Vec<u8>;

    /// Sign data with BLS private key
    ///
    /// Signs the given data using this node's BLS12-381 private key.
    /// Used for creating prepare and commit seals.
    ///
    /// # Arguments
    ///
    /// * `data` - Data to sign
    ///
    /// # Returns
    ///
    /// BLS signature bytes (96 bytes)
    fn sign_bls(&self, data: &[u8]) -> [u8; 96];

    /// Verify ECDSA signature
    ///
    /// Checks that the given signature was created by the specified address.
    ///
    /// # Arguments
    ///
    /// * `data` - Original signed data
    /// * `addr` - Expected signer address
    /// * `sig` - Signature to verify
    ///
    /// # Errors
    ///
    /// Returns error if signature is invalid
    fn check_signature(&self, data: &[u8], addr: Address, sig: &[u8]) -> Result<(), BackendError>;

    /// Get current block number
    ///
    /// Returns the height of the latest finalized block.
    fn current_block(&self) -> u64 {
        0
    }

    /// Check if this node should propose for current view
    ///
    /// Determines if this node is the designated proposer based on
    /// validator set and round-robin or other proposer selection policy.
    fn is_proposer(&self) -> bool {
        false
    }

    /// Get last proposer address
    ///
    /// Returns the address of the proposer for the last finalized block.
    fn last_proposer(&self) -> Address {
        Address::ZERO
    }

    /// Check if block number is an epoch block
    ///
    /// Epoch blocks contain validator set updates.
    fn is_epoch_block(&self, number: u64) -> bool {
        number > 0 && number % 10 == 0 // Default: every 10 blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::{DefaultValidator, DefaultValidatorSet};

    struct TestBackend {
        address: Address,
        block_number: u64,
        validators: Arc<DefaultValidatorSet>,
    }

    impl TestBackend {
        fn new() -> Self {
            let validators = vec![
                DefaultValidator::new(Address::from([0x01; 20]), [0x42; 48]),
                DefaultValidator::new(Address::from([0x02; 20]), [0x43; 48]),
                DefaultValidator::new(Address::from([0x03; 20]), [0x44; 48]),
            ];
            Self {
                address: Address::from([0x01; 20]),
                block_number: 0,
                validators: Arc::new(DefaultValidatorSet::new(validators).unwrap()),
            }
        }
    }

    impl Backend for TestBackend {
        fn address(&self) -> Address {
            self.address
        }

        fn validators(&self, _proposal: &Bytes) -> Result<Arc<dyn ValidatorSet>, BackendError> {
            Ok(self.validators.clone())
        }

        fn verify_proposal(&self, _proposal: &Bytes) -> Result<(), BackendError> {
            Ok(())
        }

        fn commit(&self, _hash: B256, _seals: Vec<[u8; 96]>) -> Result<(), BackendError> {
            Ok(())
        }

        fn broadcast(&self, _message_code: u8, _payload: &[u8]) -> Result<(), BackendError> {
            Ok(())
        }

        fn gossip(&self, _message_code: u8, _payload: &[u8]) -> Result<(), BackendError> {
            Ok(())
        }

        fn sign(&self, _data: &[u8]) -> Vec<u8> {
            vec![0x00; 65] // Dummy ECDSA signature
        }

        fn sign_bls(&self, _data: &[u8]) -> [u8; 96] {
            [0x00; 96] // Dummy BLS signature
        }

        fn check_signature(
            &self,
            _data: &[u8],
            _addr: Address,
            _sig: &[u8],
        ) -> Result<(), BackendError> {
            Ok(())
        }

        fn current_block(&self) -> u64 {
            self.block_number
        }
    }

    #[test]
    fn test_backend_trait() {
        let backend = TestBackend::new();

        assert_eq!(backend.current_block(), 0);
        assert_eq!(backend.address(), Address::from([0x01; 20]));
        assert!(backend.verify_proposal(&Bytes::new()).is_ok());
        assert!(backend.commit(B256::ZERO, vec![]).is_ok());
    }

    #[test]
    fn test_backend_validators() {
        let backend = TestBackend::new();

        let validators = backend.validators(&Bytes::new()).unwrap();
        assert_eq!(validators.size(), 3);
    }

    #[test]
    fn test_backend_sign() {
        let backend = TestBackend::new();

        let ecdsa_sig = backend.sign(b"test data");
        assert_eq!(ecdsa_sig.len(), 65);

        let bls_sig = backend.sign_bls(b"test data");
        assert_eq!(bls_sig.len(), 96);
    }

    #[test]
    fn test_backend_broadcast_gossip() {
        let backend = TestBackend::new();

        assert!(backend.broadcast(0x12, b"payload").is_ok());
        assert!(backend.gossip(0x13, b"payload").is_ok());
    }

    #[test]
    fn test_backend_check_signature() {
        let backend = TestBackend::new();

        let result = backend.check_signature(b"data", Address::ZERO, &[0x00; 65]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_backend_default_methods() {
        let backend = TestBackend::new();

        assert!(!backend.is_proposer());
        assert_eq!(backend.last_proposer(), Address::ZERO);
        assert!(!backend.is_epoch_block(5));
        assert!(backend.is_epoch_block(10));
        assert!(backend.is_epoch_block(20));
        assert!(!backend.is_epoch_block(0));
    }

    #[test]
    fn test_backend_error() {
        let err = BackendError::from("test error");
        assert_eq!(err.0, "test error");
        assert!(err.to_string().contains("Backend error"));

        let err = BackendError::from(String::from("string error"));
        assert_eq!(err.0, "string error");
    }
}
