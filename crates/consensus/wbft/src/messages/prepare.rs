//! PREPARE message implementation
//!
//! PREPARE is the second phase message sent by validators in response to a PRE-PREPARE.
//! When a validator receives and validates a PRE-PREPARE, it broadcasts a PREPARE message
//! to all other validators.

use super::{MessageError, WbftMessage};
use crate::types::{Subject, View};
use alloy_primitives::{Address, B256};
use alloy_rlp::{RlpDecodable, RlpEncodable};

/// PREPARE message sent by validators
///
/// This message indicates that a validator has accepted the PRE-PREPARE proposal
/// and is prepared to commit to the block if enough other validators also prepare.
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct Prepare {
    /// View (sequence + round) for this message
    pub view: View,

    /// Block hash being prepared
    pub digest: B256,

    /// Address of the validator sending this PREPARE
    pub sender: Address,

    /// BLS signature (96 bytes)
    pub signature: [u8; 96],
}

impl Prepare {
    /// Create a new PREPARE message
    ///
    /// # Arguments
    ///
    /// * `view` - Consensus view (sequence + round)
    /// * `digest` - Block hash being prepared
    /// * `sender` - Address of validator
    /// * `signature` - BLS signature over the message
    pub fn new(view: View, digest: B256, sender: Address, signature: [u8; 96]) -> Self {
        Self { view, digest, sender, signature }
    }
}

impl WbftMessage for Prepare {
    fn view(&self) -> &View {
        &self.view
    }

    fn sender(&self) -> &Address {
        &self.sender
    }

    fn signature(&self) -> &[u8; 96] {
        &self.signature
    }

    fn subject(&self) -> Subject {
        Subject { view: self.view.clone(), digest: self.digest }
    }

    fn encode_for_signing(&self) -> Vec<u8> {
        use alloy_rlp::Encodable;

        let mut buf = Vec::new();

        // Encode: view || digest || sender
        // (exclude signature for signing)
        self.view.encode(&mut buf);
        self.digest.encode(&mut buf);
        self.sender.encode(&mut buf);

        buf
    }

    fn validate(&self) -> Result<(), MessageError> {
        use alloy_primitives::U256;

        // Validate view
        if self.view.sequence == U256::from(0u64) {
            return Err(MessageError::ValidationFailed("sequence cannot be zero".into()));
        }

        // Validate digest is not zero
        if self.digest == B256::ZERO {
            return Err(MessageError::ValidationFailed("digest cannot be zero".into()));
        }

        // Validate sender is not zero address
        if self.sender == Address::ZERO {
            return Err(MessageError::InvalidSender(self.sender));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bls::SecretKey;
    use alloy_rlp::{Decodable, Encodable};

    fn create_test_prepare() -> (Prepare, SecretKey) {
        use alloy_primitives::U256;

        let sk = SecretKey::random();
        let view = View { sequence: U256::from(1u64), round: U256::from(0u64) };
        let digest = B256::from([0x42; 32]);
        let sender = Address::from([0x01; 20]);

        let mut prepare = Prepare::new(view, digest, sender, [0u8; 96]);

        // Sign the message
        let message = prepare.encode_for_signing();
        let signature = sk.sign(&message);
        prepare.signature = signature.to_bytes();

        (prepare, sk)
    }

    #[test]
    fn test_prepare_creation() {
        use alloy_primitives::U256;

        let (prepare, _) = create_test_prepare();

        assert_eq!(prepare.view.sequence, U256::from(1u64));
        assert_eq!(prepare.view.round, U256::from(0u64));
        assert_eq!(prepare.digest, B256::from([0x42; 32]));
    }

    #[test]
    fn test_prepare_subject() {
        let (prepare, _) = create_test_prepare();
        let subject = prepare.subject();

        assert_eq!(subject.view, prepare.view);
        assert_eq!(subject.digest, prepare.digest);
    }

    #[test]
    fn test_prepare_signature_verification() {
        let (prepare, sk) = create_test_prepare();
        let pk = sk.public_key();

        assert!(prepare.verify_signature(&pk));
    }

    #[test]
    fn test_prepare_signature_verification_fails_wrong_key() {
        let (prepare, _) = create_test_prepare();
        let wrong_sk = SecretKey::random();
        let wrong_pk = wrong_sk.public_key();

        assert!(!prepare.verify_signature(&wrong_pk));
    }

    #[test]
    fn test_prepare_validation() {
        let (prepare, _) = create_test_prepare();
        assert!(prepare.validate().is_ok());
    }

    #[test]
    fn test_prepare_validation_zero_sequence() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(0u64), round: U256::from(0u64) };
        let prepare =
            Prepare::new(view, B256::from([0x42; 32]), Address::from([0x01; 20]), [0u8; 96]);

        assert!(prepare.validate().is_err());
    }

    #[test]
    fn test_prepare_validation_zero_digest() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(1u64), round: U256::from(0u64) };
        let prepare = Prepare::new(view, B256::ZERO, Address::from([0x01; 20]), [0u8; 96]);

        assert!(prepare.validate().is_err());
    }

    #[test]
    fn test_prepare_validation_zero_sender() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(1u64), round: U256::from(0u64) };
        let prepare = Prepare::new(view, B256::from([0x42; 32]), Address::ZERO, [0u8; 96]);

        assert!(prepare.validate().is_err());
    }

    #[test]
    fn test_prepare_rlp_roundtrip() {
        let (prepare, _) = create_test_prepare();

        let mut encoded = Vec::new();
        prepare.encode(&mut encoded);
        let decoded = Prepare::decode(&mut &encoded[..]).expect("decode failed");

        assert_eq!(prepare, decoded);
    }

    #[test]
    fn test_prepare_encode_for_signing() {
        let (prepare, _) = create_test_prepare();
        let encoded = prepare.encode_for_signing();

        // Should not be empty
        assert!(!encoded.is_empty());

        // Should be deterministic
        let encoded2 = prepare.encode_for_signing();
        assert_eq!(encoded, encoded2);
    }

    #[test]
    fn test_prepare_different_views() {
        use alloy_primitives::U256;

        let view1 = View { sequence: U256::from(1u64), round: U256::from(0u64) };
        let view2 = View { sequence: U256::from(2u64), round: U256::from(0u64) };

        let prepare1 =
            Prepare::new(view1, B256::from([0x42; 32]), Address::from([0x01; 20]), [0u8; 96]);
        let prepare2 =
            Prepare::new(view2, B256::from([0x42; 32]), Address::from([0x01; 20]), [0u8; 96]);

        assert_ne!(prepare1, prepare2);
        assert_ne!(prepare1.subject(), prepare2.subject());
    }
}
