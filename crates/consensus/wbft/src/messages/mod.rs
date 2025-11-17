//! WBFT consensus message types
//!
//! This module implements the 4 message types used in WBFT consensus:
//! - PRE-PREPARE: Block proposal from proposer
//! - PREPARE: Validator acceptance of proposal
//! - COMMIT: Validator commitment to block
//! - ROUND-CHANGE: Request for view change

use crate::types::{Subject, View};
use alloy_primitives::Address;

mod commit;
mod prepare;
mod preprepare;
mod round_change;

pub use commit::Commit;
pub use prepare::Prepare;
pub use preprepare::PrePrepare;
pub use round_change::RoundChange;

/// Common trait for all WBFT consensus messages
///
/// All consensus messages must implement this trait to provide:
/// - Message encoding/decoding
/// - Signature verification
/// - Message validation
pub trait WbftMessage: Send + Sync + std::fmt::Debug {
    /// Get the view (sequence + round) this message is for
    fn view(&self) -> &View;

    /// Get the message sender's address
    fn sender(&self) -> &Address;

    /// Get the message signature
    fn signature(&self) -> &[u8; 96];

    /// Get the subject (view + digest) this message is for
    fn subject(&self) -> Subject;

    /// Encode message for signing (without signature field)
    ///
    /// This returns the canonical encoding of the message that should be
    /// signed by the validator.
    fn encode_for_signing(&self) -> Vec<u8>;

    /// Verify message signature against sender's public key
    ///
    /// # Arguments
    ///
    /// * `public_key` - Public key of the expected sender
    ///
    /// # Returns
    ///
    /// `true` if signature is valid for this message and public key
    fn verify_signature(&self, public_key: &crate::bls::PublicKey) -> bool {
        use crate::bls::Signature;

        let message = self.encode_for_signing();
        let signature = match Signature::from_bytes(self.signature()) {
            Ok(sig) => sig,
            Err(_) => return false,
        };

        public_key.verify(&message, &signature)
    }

    /// Validate message structure and contents
    ///
    /// # Errors
    ///
    /// Returns error if message is malformed or invalid
    fn validate(&self) -> Result<(), MessageError>;
}

/// Errors that can occur during message processing
#[derive(Debug, thiserror::Error)]
pub enum MessageError {
    /// Invalid message signature
    #[error("invalid message signature")]
    InvalidSignature,

    /// Message from wrong view
    #[error("message from wrong view: expected {expected:?}, got {actual:?}")]
    WrongView {
        /// Expected view
        expected: View,
        /// Actual view in message
        actual: View,
    },

    /// Invalid message sender
    #[error("invalid message sender: {0}")]
    InvalidSender(Address),

    /// Message validation failed
    #[error("message validation failed: {0}")]
    ValidationFailed(String),

    /// RLP encoding/decoding error
    #[error("RLP error: {0}")]
    RlpError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_error_display() {
        use alloy_primitives::U256;

        let err = MessageError::InvalidSignature;
        assert_eq!(err.to_string(), "invalid message signature");

        let err = MessageError::WrongView {
            expected: View { sequence: U256::from(1u64), round: U256::from(0u64) },
            actual: View { sequence: U256::from(2u64), round: U256::from(0u64) },
        };
        assert!(err.to_string().contains("wrong view"));
    }
}
