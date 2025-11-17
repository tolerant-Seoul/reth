//! COMMIT message implementation
//!
//! COMMIT is the third phase message sent by validators after receiving enough PREPAREs.
//! When a validator has seen 2f+1 PREPARE messages (including its own), it broadcasts
//! a COMMIT message indicating it is committed to the block.

use super::{MessageError, WbftMessage};
use crate::types::{Subject, View};
use alloy_primitives::{Address, B256};
use alloy_rlp::{RlpDecodable, RlpEncodable};

/// COMMIT message sent by validators
///
/// This message indicates that a validator has received enough PREPARE messages
/// and is now committed to the proposed block. When 2f+1 COMMIT messages are
/// collected, the block can be finalized.
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct Commit {
    /// View (sequence + round) for this message
    pub view: View,

    /// Block hash being committed
    pub digest: B256,

    /// Address of the validator sending this COMMIT
    pub sender: Address,

    /// BLS signature (96 bytes) over the message
    ///
    /// This is the validator's individual signature that will be aggregated
    /// with other validators' signatures to form the final block seal.
    pub signature: [u8; 96],

    /// BLS signature (96 bytes) over the PREPARE subject
    ///
    /// This is the validator's signature from the PREPARE phase, included
    /// to enable proof that the validator participated in the PREPARE phase.
    pub commit_seal: [u8; 96],
}

impl Commit {
    /// Create a new COMMIT message
    ///
    /// # Arguments
    ///
    /// * `view` - Consensus view (sequence + round)
    /// * `digest` - Block hash being committed
    /// * `sender` - Address of validator
    /// * `signature` - BLS signature over the COMMIT message
    /// * `commit_seal` - BLS signature from the PREPARE phase
    pub fn new(
        view: View,
        digest: B256,
        sender: Address,
        signature: [u8; 96],
        commit_seal: [u8; 96],
    ) -> Self {
        Self { view, digest, sender, signature, commit_seal }
    }

    /// Get the commit seal (PREPARE phase signature)
    pub fn seal(&self) -> &[u8; 96] {
        &self.commit_seal
    }
}

impl WbftMessage for Commit {
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

        // Encode: view || digest || sender || commit_seal
        // (exclude signature for signing)
        self.view.encode(&mut buf);
        self.digest.encode(&mut buf);
        self.sender.encode(&mut buf);
        self.commit_seal.encode(&mut buf);

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

        // Validate commit seal is not all zeros
        if self.commit_seal == [0u8; 96] {
            return Err(MessageError::ValidationFailed("commit seal cannot be zero".into()));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bls::SecretKey;
    use alloy_rlp::{Decodable, Encodable};

    fn create_test_commit() -> (Commit, SecretKey) {
        use alloy_primitives::U256;

        let sk = SecretKey::random();
        let view = View { sequence: U256::from(1u64), round: U256::from(0u64) };
        let digest = B256::from([0x42; 32]);
        let sender = Address::from([0x01; 20]);

        // Create a mock commit seal (would be from PREPARE phase in real scenario)
        let commit_seal_data = b"prepare phase signature";
        let commit_seal_sig = sk.sign(commit_seal_data);

        let mut commit = Commit::new(view, digest, sender, [0u8; 96], commit_seal_sig.to_bytes());

        // Sign the COMMIT message
        let message = commit.encode_for_signing();
        let signature = sk.sign(&message);
        commit.signature = signature.to_bytes();

        (commit, sk)
    }

    #[test]
    fn test_commit_creation() {
        use alloy_primitives::U256;

        let (commit, _) = create_test_commit();

        assert_eq!(commit.view.sequence, U256::from(1u64));
        assert_eq!(commit.view.round, U256::from(0u64));
        assert_eq!(commit.digest, B256::from([0x42; 32]));
        assert_ne!(commit.commit_seal, [0u8; 96]);
    }

    #[test]
    fn test_commit_subject() {
        use alloy_primitives::U256;

        let (commit, _) = create_test_commit();
        let subject = commit.subject();

        assert_eq!(subject.view, commit.view);
        assert_eq!(subject.digest, commit.digest);
    }

    #[test]
    fn test_commit_signature_verification() {
        use alloy_primitives::U256;

        let (commit, sk) = create_test_commit();
        let pk = sk.public_key();

        assert!(commit.verify_signature(&pk));
    }

    #[test]
    fn test_commit_signature_verification_fails_wrong_key() {
        use alloy_primitives::U256;

        let (commit, _) = create_test_commit();
        let wrong_sk = SecretKey::random();
        let wrong_pk = wrong_sk.public_key();

        assert!(!commit.verify_signature(&wrong_pk));
    }

    #[test]
    fn test_commit_seal_accessor() {
        use alloy_primitives::U256;

        let (commit, _) = create_test_commit();
        let seal = commit.seal();

        assert_eq!(seal, &commit.commit_seal);
        assert_ne!(*seal, [0u8; 96]);
    }

    #[test]
    fn test_commit_validation() {
        use alloy_primitives::U256;

        let (commit, _) = create_test_commit();
        assert!(commit.validate().is_ok());
    }

    #[test]
    fn test_commit_validation_zero_sequence() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(0u64), round: U256::from(0u64) };
        let commit = Commit::new(
            view,
            B256::from([0x42; 32]),
            Address::from([0x01; 20]),
            [0u8; 96],
            [0x01; 96],
        );

        assert!(commit.validate().is_err());
    }

    #[test]
    fn test_commit_validation_zero_digest() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(1u64), round: U256::from(0u64) };
        let commit =
            Commit::new(view, B256::ZERO, Address::from([0x01; 20]), [0u8; 96], [0x01; 96]);

        assert!(commit.validate().is_err());
    }

    #[test]
    fn test_commit_validation_zero_sender() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(1u64), round: U256::from(0u64) };
        let commit =
            Commit::new(view, B256::from([0x42; 32]), Address::ZERO, [0u8; 96], [0x01; 96]);

        assert!(commit.validate().is_err());
    }

    #[test]
    fn test_commit_validation_zero_commit_seal() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(1u64), round: U256::from(0u64) };
        let commit = Commit::new(
            view,
            B256::from([0x42; 32]),
            Address::from([0x01; 20]),
            [0u8; 96],
            [0u8; 96],
        );

        assert!(commit.validate().is_err());
    }

    #[test]
    fn test_commit_rlp_roundtrip() {
        use alloy_primitives::U256;

        let (commit, _) = create_test_commit();

        let mut encoded = Vec::new();
        commit.encode(&mut encoded);
        let decoded = Commit::decode(&mut &encoded[..]).expect("decode failed");

        assert_eq!(commit, decoded);
    }

    #[test]
    fn test_commit_encode_for_signing() {
        use alloy_primitives::U256;

        let (commit, _) = create_test_commit();
        let encoded = commit.encode_for_signing();

        // Should not be empty
        assert!(!encoded.is_empty());

        // Should be deterministic
        let encoded2 = commit.encode_for_signing();
        assert_eq!(encoded, encoded2);
    }

    #[test]
    fn test_commit_different_views() {
        use alloy_primitives::U256;

        let view1 = View { sequence: U256::from(1u64), round: U256::from(0u64) };
        let view2 = View { sequence: U256::from(2u64), round: U256::from(0u64) };

        let commit1 = Commit::new(
            view1,
            B256::from([0x42; 32]),
            Address::from([0x01; 20]),
            [0u8; 96],
            [0x01; 96],
        );
        let commit2 = Commit::new(
            view2,
            B256::from([0x42; 32]),
            Address::from([0x01; 20]),
            [0u8; 96],
            [0x01; 96],
        );

        assert_ne!(commit1, commit2);
        assert_ne!(commit1.subject(), commit2.subject());
    }
}
