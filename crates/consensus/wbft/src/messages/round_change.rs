//! ROUND-CHANGE message implementation
//!
//! ROUND-CHANGE is sent by validators when they detect that consensus progress
//! has stalled (e.g., timeout waiting for messages). It triggers a view change
//! to attempt consensus with a new proposer.

use super::{MessageError, WbftMessage};
use crate::types::{Subject, View};
use alloy_primitives::{Address, B256};
use alloy_rlp::{RlpDecodable, RlpEncodable};

/// ROUND-CHANGE message sent by validators
///
/// This message is sent when a validator believes consensus has stalled and
/// a new round with a different proposer is needed. When 2f+1 validators
/// send ROUND-CHANGE for the same view, a new round begins.
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
#[rlp(trailing)]
pub struct RoundChange {
    /// Target view (sequence + new round number) being proposed
    pub view: View,

    /// Address of the validator sending this ROUND-CHANGE
    pub sender: Address,

    /// BLS signature (96 bytes)
    pub signature: [u8; 96],

    /// Optional: Hash of the highest prepared block (if any)
    ///
    /// If the validator had prepared a block in a previous round,
    /// this contains that block's hash. Otherwise, it's zero.
    pub prepared_digest: Option<B256>,

    /// Optional: View where the prepared block was prepared
    ///
    /// If prepared_digest is Some, this must also be Some and indicate
    /// the view where the block was prepared.
    pub prepared_view: Option<View>,
}

impl RoundChange {
    /// Create a new ROUND-CHANGE message without prepared certificate
    ///
    /// # Arguments
    ///
    /// * `view` - Target view for the new round
    /// * `sender` - Address of validator
    /// * `signature` - BLS signature over the message
    pub fn new(view: View, sender: Address, signature: [u8; 96]) -> Self {
        Self { view, sender, signature, prepared_digest: None, prepared_view: None }
    }

    /// Create a new ROUND-CHANGE message with prepared certificate
    ///
    /// # Arguments
    ///
    /// * `view` - Target view for the new round
    /// * `sender` - Address of validator
    /// * `signature` - BLS signature over the message
    /// * `prepared_digest` - Hash of the prepared block
    /// * `prepared_view` - View where the block was prepared
    pub fn with_prepared(
        view: View,
        sender: Address,
        signature: [u8; 96],
        prepared_digest: B256,
        prepared_view: View,
    ) -> Self {
        Self {
            view,
            sender,
            signature,
            prepared_digest: Some(prepared_digest),
            prepared_view: Some(prepared_view),
        }
    }

    /// Check if this ROUND-CHANGE includes a prepared certificate
    pub fn has_prepared(&self) -> bool {
        self.prepared_digest.is_some() && self.prepared_view.is_some()
    }

    /// Get the prepared block hash if any
    pub fn prepared_block(&self) -> Option<B256> {
        self.prepared_digest
    }
}

impl WbftMessage for RoundChange {
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
        Subject { view: self.view.clone(), digest: self.prepared_digest.unwrap_or(B256::ZERO) }
    }

    fn encode_for_signing(&self) -> Vec<u8> {
        use alloy_rlp::Encodable;

        let mut buf = Vec::new();

        // Encode: view || sender || prepared_digest || prepared_view
        // (exclude signature for signing)
        self.view.encode(&mut buf);
        self.sender.encode(&mut buf);

        // Encode Option fields
        match &self.prepared_digest {
            Some(digest) => digest.encode(&mut buf),
            None => buf.push(0x80), // RLP encoding for empty
        }
        match &self.prepared_view {
            Some(view) => view.encode(&mut buf),
            None => buf.push(0x80), // RLP encoding for empty
        }

        buf
    }

    fn validate(&self) -> Result<(), MessageError> {
        use alloy_primitives::U256;

        // Validate view
        if self.view.sequence == U256::from(0u64) {
            return Err(MessageError::ValidationFailed("sequence cannot be zero".into()));
        }

        // Validate sender is not zero address
        if self.sender == Address::ZERO {
            return Err(MessageError::InvalidSender(self.sender));
        }

        // If prepared digest is present, prepared view must also be present
        match (self.prepared_digest, &self.prepared_view) {
            (Some(digest), Some(view)) => {
                // Prepared digest must not be zero
                if digest == B256::ZERO {
                    return Err(MessageError::ValidationFailed(
                        "prepared digest cannot be zero".into(),
                    ));
                }

                // Prepared view must be less than target view
                if view.sequence > self.view.sequence {
                    return Err(MessageError::ValidationFailed(
                        "prepared view sequence must be <= target view sequence".into(),
                    ));
                }

                // If same sequence, prepared round must be less than target round
                if view.sequence == self.view.sequence && view.round >= self.view.round {
                    return Err(MessageError::ValidationFailed(
                        "prepared view round must be < target view round".into(),
                    ));
                }
            }
            (Some(_), None) => {
                return Err(MessageError::ValidationFailed(
                    "prepared digest without prepared view".into(),
                ));
            }
            (None, Some(_)) => {
                return Err(MessageError::ValidationFailed(
                    "prepared view without prepared digest".into(),
                ));
            }
            (None, None) => {
                // Valid: no prepared certificate
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bls::SecretKey;
    use alloy_rlp::{Decodable, Encodable};

    fn create_test_round_change() -> (RoundChange, SecretKey) {
        use alloy_primitives::U256;

        let sk = SecretKey::random();
        let view = View { sequence: U256::from(1u64), round: U256::from(1u64) };
        let sender = Address::from([0x01; 20]);

        let mut round_change = RoundChange::new(view, sender, [0u8; 96]);

        // Sign the message
        let message = round_change.encode_for_signing();
        let signature = sk.sign(&message);
        round_change.signature = signature.to_bytes();

        (round_change, sk)
    }

    fn create_test_round_change_with_prepared() -> (RoundChange, SecretKey) {
        use alloy_primitives::U256;

        let sk = SecretKey::random();
        let view = View { sequence: U256::from(1u64), round: U256::from(1u64) };
        let sender = Address::from([0x01; 20]);
        let prepared_digest = B256::from([0x42; 32]);
        let prepared_view = View { sequence: U256::from(1u64), round: U256::from(0u64) };

        let mut round_change =
            RoundChange::with_prepared(view, sender, [0u8; 96], prepared_digest, prepared_view);

        // Sign the message
        let message = round_change.encode_for_signing();
        let signature = sk.sign(&message);
        round_change.signature = signature.to_bytes();

        (round_change, sk)
    }

    #[test]
    fn test_round_change_creation() {
        use alloy_primitives::U256;

        let (round_change, _) = create_test_round_change();

        assert_eq!(round_change.view.sequence, U256::from(1u64));
        assert_eq!(round_change.view.round, U256::from(1u64));
        assert!(!round_change.has_prepared());
        assert_eq!(round_change.prepared_block(), None);
    }

    #[test]
    fn test_round_change_with_prepared_creation() {
        use alloy_primitives::U256;

        let (round_change, _) = create_test_round_change_with_prepared();

        assert_eq!(round_change.view.sequence, U256::from(1u64));
        assert_eq!(round_change.view.round, U256::from(1u64));
        assert!(round_change.has_prepared());
        assert_eq!(round_change.prepared_block(), Some(B256::from([0x42; 32])));
        assert_eq!(
            round_change.prepared_view,
            Some(View { sequence: U256::from(1u64), round: U256::from(0u64) })
        );
    }

    #[test]
    fn test_round_change_subject() {
        use alloy_primitives::U256;

        let (round_change, _) = create_test_round_change();
        let subject = round_change.subject();

        assert_eq!(subject.view, round_change.view);
        assert_eq!(subject.digest, B256::ZERO); // No prepared block
    }

    #[test]
    fn test_round_change_with_prepared_subject() {
        use alloy_primitives::U256;

        let (round_change, _) = create_test_round_change_with_prepared();
        let subject = round_change.subject();

        assert_eq!(subject.view, round_change.view);
        assert_eq!(subject.digest, B256::from([0x42; 32]));
    }

    #[test]
    fn test_round_change_signature_verification() {
        use alloy_primitives::U256;

        let (round_change, sk) = create_test_round_change();
        let pk = sk.public_key();

        assert!(round_change.verify_signature(&pk));
    }

    #[test]
    fn test_round_change_signature_verification_fails_wrong_key() {
        use alloy_primitives::U256;

        let (round_change, _) = create_test_round_change();
        let wrong_sk = SecretKey::random();
        let wrong_pk = wrong_sk.public_key();

        assert!(!round_change.verify_signature(&wrong_pk));
    }

    #[test]
    fn test_round_change_validation() {
        use alloy_primitives::U256;

        let (round_change, _) = create_test_round_change();
        assert!(round_change.validate().is_ok());
    }

    #[test]
    fn test_round_change_with_prepared_validation() {
        use alloy_primitives::U256;

        let (round_change, _) = create_test_round_change_with_prepared();
        assert!(round_change.validate().is_ok());
    }

    #[test]
    fn test_round_change_validation_zero_sequence() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(0u64), round: U256::from(1u64) };
        let round_change = RoundChange::new(view, Address::from([0x01; 20]), [0u8; 96]);

        assert!(round_change.validate().is_err());
    }

    #[test]
    fn test_round_change_validation_zero_sender() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(1u64), round: U256::from(1u64) };
        let round_change = RoundChange::new(view, Address::ZERO, [0u8; 96]);

        assert!(round_change.validate().is_err());
    }

    #[test]
    fn test_round_change_validation_prepared_digest_without_view() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(1u64), round: U256::from(1u64) };
        let mut round_change = RoundChange::new(view, Address::from([0x01; 20]), [0u8; 96]);
        round_change.prepared_digest = Some(B256::from([0x42; 32]));

        assert!(round_change.validate().is_err());
    }

    #[test]
    fn test_round_change_validation_prepared_view_without_digest() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(1u64), round: U256::from(1u64) };
        let mut round_change = RoundChange::new(view, Address::from([0x01; 20]), [0u8; 96]);
        round_change.prepared_view =
            Some(View { sequence: U256::from(1u64), round: U256::from(0u64) });

        assert!(round_change.validate().is_err());
    }

    #[test]
    fn test_round_change_validation_zero_prepared_digest() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(1u64), round: U256::from(1u64) };
        let prepared_view = View { sequence: U256::from(1u64), round: U256::from(0u64) };
        let round_change = RoundChange::with_prepared(
            view,
            Address::from([0x01; 20]),
            [0u8; 96],
            B256::ZERO,
            prepared_view,
        );

        assert!(round_change.validate().is_err());
    }

    #[test]
    fn test_round_change_validation_prepared_view_too_high_sequence() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(1u64), round: U256::from(1u64) };
        let prepared_view = View { sequence: U256::from(2u64), round: U256::from(0u64) };
        let round_change = RoundChange::with_prepared(
            view,
            Address::from([0x01; 20]),
            [0u8; 96],
            B256::from([0x42; 32]),
            prepared_view,
        );

        assert!(round_change.validate().is_err());
    }

    #[test]
    fn test_round_change_validation_prepared_round_too_high() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(1u64), round: U256::from(1u64) };
        let prepared_view = View { sequence: U256::from(1u64), round: U256::from(1u64) };
        let round_change = RoundChange::with_prepared(
            view,
            Address::from([0x01; 20]),
            [0u8; 96],
            B256::from([0x42; 32]),
            prepared_view,
        );

        assert!(round_change.validate().is_err());
    }

    #[test]
    fn test_round_change_rlp_roundtrip() {
        use alloy_primitives::U256;

        let (round_change, _) = create_test_round_change();

        let mut encoded = Vec::new();
        round_change.encode(&mut encoded);
        let decoded = RoundChange::decode(&mut &encoded[..]).expect("decode failed");

        assert_eq!(round_change, decoded);
    }

    #[test]
    fn test_round_change_with_prepared_rlp_roundtrip() {
        use alloy_primitives::U256;

        let (round_change, _) = create_test_round_change_with_prepared();

        let mut encoded = Vec::new();
        round_change.encode(&mut encoded);
        let decoded = RoundChange::decode(&mut &encoded[..]).expect("decode failed");

        assert_eq!(round_change, decoded);
    }

    #[test]
    fn test_round_change_encode_for_signing() {
        use alloy_primitives::U256;

        let (round_change, _) = create_test_round_change();
        let encoded = round_change.encode_for_signing();

        // Should not be empty
        assert!(!encoded.is_empty());

        // Should be deterministic
        let encoded2 = round_change.encode_for_signing();
        assert_eq!(encoded, encoded2);
    }
}
