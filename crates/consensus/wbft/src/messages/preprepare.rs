//! PRE-PREPARE message implementation
//!
//! PRE-PREPARE is the first phase message sent by the proposer to all validators.
//! It contains the proposed block and initiates the consensus round.

use super::{MessageError, WbftMessage};
use crate::types::{Subject, View};
use alloy_primitives::{Address, Bytes, B256};
use alloy_rlp::{RlpDecodable, RlpEncodable};

/// PRE-PREPARE message sent by block proposer
///
/// This message initiates a consensus round by proposing a block to validators.
/// Validators respond with PREPARE messages if they accept the proposal.
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct PrePrepare {
    /// View (sequence + round) for this proposal
    pub view: View,

    /// Proposed block hash
    pub proposal: B256,

    /// Complete block data (RLP-encoded)
    pub proposal_block: Bytes,

    /// Address of the proposer
    pub sender: Address,

    /// BLS signature (96 bytes)
    pub signature: [u8; 96],
}

impl PrePrepare {
    /// Create a new PRE-PREPARE message
    ///
    /// # Arguments
    ///
    /// * `view` - Consensus view (sequence + round)
    /// * `proposal` - Block hash being proposed
    /// * `proposal_block` - Complete block data
    /// * `sender` - Address of proposer
    /// * `signature` - BLS signature over the message
    pub fn new(
        view: View,
        proposal: B256,
        proposal_block: Bytes,
        sender: Address,
        signature: [u8; 96],
    ) -> Self {
        Self { view, proposal, proposal_block, sender, signature }
    }

    /// Get the proposed block data
    pub fn block_data(&self) -> &Bytes {
        &self.proposal_block
    }

    /// Get the size of the proposal in bytes
    pub fn proposal_size(&self) -> usize {
        self.proposal_block.len()
    }
}

impl WbftMessage for PrePrepare {
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
        Subject { view: self.view.clone(), digest: self.proposal }
    }

    fn encode_for_signing(&self) -> Vec<u8> {
        use alloy_rlp::Encodable;

        let mut buf = Vec::new();

        // Encode: view || proposal || proposal_block || sender
        // (exclude signature for signing)
        self.view.encode(&mut buf);
        self.proposal.encode(&mut buf);
        self.proposal_block.encode(&mut buf);
        self.sender.encode(&mut buf);

        buf
    }

    fn validate(&self) -> Result<(), MessageError> {
        use alloy_primitives::U256;

        // Validate view
        if self.view.sequence == U256::from(0u64) {
            return Err(MessageError::ValidationFailed("sequence cannot be zero".into()));
        }

        // Validate proposal hash is not zero
        if self.proposal == B256::ZERO {
            return Err(MessageError::ValidationFailed("proposal hash cannot be zero".into()));
        }

        // Validate block data is not empty
        if self.proposal_block.is_empty() {
            return Err(MessageError::ValidationFailed("proposal block cannot be empty".into()));
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

    fn create_test_preprepare() -> (PrePrepare, SecretKey) {
        use alloy_primitives::U256;

        let sk = SecretKey::random();
        let view = View { sequence: U256::from(1u64), round: U256::from(0u64) };
        let proposal = B256::from([0x42; 32]);
        let proposal_block = Bytes::from(vec![1, 2, 3, 4]);
        let sender = Address::from([0x01; 20]);

        let mut preprepare = PrePrepare::new(view, proposal, proposal_block, sender, [0u8; 96]);

        // Sign the message
        let message = preprepare.encode_for_signing();
        let signature = sk.sign(&message);
        preprepare.signature = signature.to_bytes();

        (preprepare, sk)
    }

    #[test]
    fn test_preprepare_creation() {
        use alloy_primitives::U256;

        let (preprepare, _) = create_test_preprepare();

        assert_eq!(preprepare.view.sequence, U256::from(1u64));
        assert_eq!(preprepare.view.round, U256::from(0u64));
        assert_eq!(preprepare.proposal, B256::from([0x42; 32]));
        assert_eq!(preprepare.proposal_block, Bytes::from(vec![1, 2, 3, 4]));
        assert_eq!(preprepare.proposal_size(), 4);
    }

    #[test]
    fn test_preprepare_subject() {
        let (preprepare, _) = create_test_preprepare();
        let subject = preprepare.subject();

        assert_eq!(subject.view, preprepare.view);
        assert_eq!(subject.digest, preprepare.proposal);
    }

    #[test]
    fn test_preprepare_signature_verification() {
        let (preprepare, sk) = create_test_preprepare();
        let pk = sk.public_key();

        assert!(preprepare.verify_signature(&pk));
    }

    #[test]
    fn test_preprepare_signature_verification_fails_wrong_key() {
        let (preprepare, _) = create_test_preprepare();
        let wrong_sk = SecretKey::random();
        let wrong_pk = wrong_sk.public_key();

        assert!(!preprepare.verify_signature(&wrong_pk));
    }

    #[test]
    fn test_preprepare_validation() {
        let (preprepare, _) = create_test_preprepare();
        assert!(preprepare.validate().is_ok());
    }

    #[test]
    fn test_preprepare_validation_zero_sequence() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(0u64), round: U256::from(0u64) };
        let preprepare = PrePrepare::new(
            view,
            B256::from([0x42; 32]),
            Bytes::from(vec![1, 2, 3]),
            Address::from([0x01; 20]),
            [0u8; 96],
        );

        assert!(preprepare.validate().is_err());
    }

    #[test]
    fn test_preprepare_validation_zero_proposal() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(1u64), round: U256::from(0u64) };
        let preprepare = PrePrepare::new(
            view,
            B256::ZERO,
            Bytes::from(vec![1, 2, 3]),
            Address::from([0x01; 20]),
            [0u8; 96],
        );

        assert!(preprepare.validate().is_err());
    }

    #[test]
    fn test_preprepare_validation_empty_block() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(1u64), round: U256::from(0u64) };
        let preprepare = PrePrepare::new(
            view,
            B256::from([0x42; 32]),
            Bytes::new(),
            Address::from([0x01; 20]),
            [0u8; 96],
        );

        assert!(preprepare.validate().is_err());
    }

    #[test]
    fn test_preprepare_validation_zero_sender() {
        use alloy_primitives::U256;

        let view = View { sequence: U256::from(1u64), round: U256::from(0u64) };
        let preprepare = PrePrepare::new(
            view,
            B256::from([0x42; 32]),
            Bytes::from(vec![1, 2, 3]),
            Address::ZERO,
            [0u8; 96],
        );

        assert!(preprepare.validate().is_err());
    }

    #[test]
    fn test_preprepare_rlp_roundtrip() {
        let (preprepare, _) = create_test_preprepare();

        let mut encoded = Vec::new();
        preprepare.encode(&mut encoded);
        let decoded = PrePrepare::decode(&mut &encoded[..]).expect("decode failed");

        assert_eq!(preprepare, decoded);
    }

    #[test]
    fn test_preprepare_encode_for_signing() {
        let (preprepare, _) = create_test_preprepare();
        let encoded = preprepare.encode_for_signing();

        // Should not be empty
        assert!(!encoded.is_empty());

        // Should be deterministic
        let encoded2 = preprepare.encode_for_signing();
        assert_eq!(encoded, encoded2);
    }
}
