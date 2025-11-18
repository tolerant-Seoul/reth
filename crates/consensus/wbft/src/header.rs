//! WBFT block header extra data structures
//!
//! This module implements the WBFTExtra structure that is encoded in
//! the block header's extra_data field to store WBFT consensus proofs.

use crate::bls::WbftAggregatedSeal;
use alloy_primitives::{Address, U256};
use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};

/// Epoch information stored in genesis and epoch blocks
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct EpochInfo {
    /// Candidate validators with their diligence scores
    pub candidates: Vec<Candidate>,

    /// Indices of active validators (into candidates array)
    pub validators: Vec<u32>,

    /// BLS public keys for validators (48 bytes each, compressed)
    pub bls_public_keys: Vec<Vec<u8>>,
}

/// Validator candidate information
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct Candidate {
    /// Ethereum address of the candidate
    pub addr: Address,

    /// Diligence score (unit: 10^-6, e.g., 950_000 = 95%)
    pub diligence: u64,
}

/// WBFT consensus proof stored in block header extra data
///
/// This structure contains all WBFT-specific data needed to validate
/// and prove consensus for a block, including aggregated BLS signatures
/// and epoch information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WbftExtra {
    /// Vanity data (32 bytes) - arbitrary data for miner identification
    pub vanity_data: [u8; 32],

    /// RANDAO reveal value for randomness generation
    pub randao_reveal: Vec<u8>,

    /// Previous round number (for round change tracking)
    pub prev_round: u32,

    /// Previous round's prepared seal (if round change occurred)
    pub prev_prepared_seal: Option<WbftAggregatedSeal>,

    /// Previous round's committed seal (if round change occurred)
    pub prev_committed_seal: Option<WbftAggregatedSeal>,

    /// Current round number
    pub round: u32,

    /// Prepared seal (2f+1 PREPARE signatures)
    pub prepared_seal: Option<WbftAggregatedSeal>,

    /// Committed seal (2f+1 COMMIT signatures)
    pub committed_seal: Option<WbftAggregatedSeal>,

    /// Gas tip for priority fee
    pub gas_tip: U256,

    /// Epoch information (present only in epoch blocks)
    pub epoch_info: Option<EpochInfo>,
}

impl WbftExtra {
    /// Create a new WBFTExtra with default values
    pub fn new() -> Self {
        Self {
            vanity_data: [0u8; 32],
            randao_reveal: Vec::new(),
            prev_round: 0,
            prev_prepared_seal: None,
            prev_committed_seal: None,
            round: 0,
            prepared_seal: None,
            committed_seal: None,
            gas_tip: U256::ZERO,
            epoch_info: None,
        }
    }

    /// Encode WBFTExtra to bytes using RLP
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.encode_to(&mut buf);
        buf
    }

    /// Decode WBFTExtra from bytes
    ///
    /// # Errors
    ///
    /// Returns error if RLP decoding fails
    pub fn decode(data: &[u8]) -> Result<Self, alloy_rlp::Error> {
        Self::decode_from(&mut &data[..])
    }

    /// Encode to buffer
    fn encode_to(&self, buf: &mut Vec<u8>) {
        use alloy_rlp::Encodable;

        // Encode as RLP list
        let header = alloy_rlp::Header {
            list: true,
            payload_length: self.payload_length(),
        };
        header.encode(buf);

        // Encode fields in order
        self.vanity_data.encode(buf);
        self.randao_reveal.encode(buf);
        self.prev_round.encode(buf);

        // Encode Options manually
        match &self.prev_prepared_seal {
            Some(seal) => seal.encode(buf),
            None => buf.push(0xC0), // Empty list
        }

        match &self.prev_committed_seal {
            Some(seal) => seal.encode(buf),
            None => buf.push(0xC0),
        }

        self.round.encode(buf);

        match &self.prepared_seal {
            Some(seal) => seal.encode(buf),
            None => buf.push(0xC0),
        }

        match &self.committed_seal {
            Some(seal) => seal.encode(buf),
            None => buf.push(0xC0),
        }

        self.gas_tip.encode(buf);

        match &self.epoch_info {
            Some(info) => info.encode(buf),
            None => buf.push(0xC0),
        }
    }

    /// Decode from buffer
    fn decode_from(buf: &mut &[u8]) -> Result<Self, alloy_rlp::Error> {
        use alloy_rlp::Decodable;

        let header = alloy_rlp::Header::decode(buf)?;
        if !header.list {
            return Err(alloy_rlp::Error::UnexpectedString);
        }

        let started_len = buf.len();

        let vanity_data = <[u8; 32]>::decode(buf)?;
        let randao_reveal = Vec::<u8>::decode(buf)?;
        let prev_round = u32::decode(buf)?;

        // Decode Options manually - check if empty list (0xC0)
        let prev_prepared_seal = Self::decode_option::<WbftAggregatedSeal>(buf)?;
        let prev_committed_seal = Self::decode_option::<WbftAggregatedSeal>(buf)?;

        let round = u32::decode(buf)?;

        let prepared_seal = Self::decode_option::<WbftAggregatedSeal>(buf)?;
        let committed_seal = Self::decode_option::<WbftAggregatedSeal>(buf)?;

        let gas_tip = U256::decode(buf)?;
        let epoch_info = Self::decode_option::<EpochInfo>(buf)?;

        let consumed = started_len - buf.len();
        if consumed != header.payload_length {
            return Err(alloy_rlp::Error::ListLengthMismatch {
                expected: header.payload_length,
                got: consumed,
            });
        }

        Ok(Self {
            vanity_data,
            randao_reveal,
            prev_round,
            prev_prepared_seal,
            prev_committed_seal,
            round,
            prepared_seal,
            committed_seal,
            gas_tip,
            epoch_info,
        })
    }

    /// Helper to decode Option<T> where T: Decodable
    fn decode_option<T: Decodable>(buf: &mut &[u8]) -> Result<Option<T>, alloy_rlp::Error> {
        use alloy_rlp::Decodable;

        // Check if next byte is 0xC0 (empty list)
        if !buf.is_empty() && buf[0] == 0xC0 {
            *buf = &buf[1..]; // Consume the byte
            Ok(None)
        } else {
            Ok(Some(T::decode(buf)?))
        }
    }

    /// Calculate payload length for RLP encoding
    fn payload_length(&self) -> usize {
        use alloy_rlp::Encodable;

        let mut len = 0;
        len += self.vanity_data.length();
        len += self.randao_reveal.length();
        len += self.prev_round.length();

        // Option types need special handling
        len += if let Some(ref seal) = self.prev_prepared_seal {
            seal.length()
        } else {
            1 // Empty list encoded as 0xC0
        };

        len += if let Some(ref seal) = self.prev_committed_seal {
            seal.length()
        } else {
            1
        };

        len += self.round.length();

        len += if let Some(ref seal) = self.prepared_seal {
            seal.length()
        } else {
            1
        };

        len += if let Some(ref seal) = self.committed_seal {
            seal.length()
        } else {
            1
        };

        len += self.gas_tip.length();

        len += if let Some(ref info) = self.epoch_info {
            info.length()
        } else {
            1
        };

        len
    }
}

impl Default for WbftExtra {
    fn default() -> Self {
        Self::new()
    }
}

/// Seal type for prepare seal hashing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealType {
    /// Prepare seal (PREPARE phase)
    Prepare = 0,
    /// Commit seal (COMMIT phase)
    Commit = 1,
}

/// Calculate prepare seal hash for BLS signing
///
/// This function creates the message that validators sign during
/// PREPARE and COMMIT phases. The hash includes:
/// 1. Block hash with round number
/// 2. Seal type (Prepare or Commit)
///
/// # Arguments
///
/// * `block_hash` - Hash of the block (without seals)
/// * `round` - Current consensus round
/// * `seal_type` - Type of seal (Prepare or Commit)
///
/// # Returns
///
/// 32-byte hash to be signed by validators
pub fn prepare_seal_hash(
    block_hash: alloy_primitives::B256,
    round: u32,
    seal_type: SealType,
) -> alloy_primitives::B256 {
    use alloy_primitives::keccak256;

    // 1. Create hash with round number: keccak256(block_hash || round)
    let mut data = Vec::with_capacity(32 + 4);
    data.extend_from_slice(block_hash.as_slice());
    data.extend_from_slice(&round.to_be_bytes());
    let hash_with_round = keccak256(&data);

    // 2. Append seal type byte and hash again: keccak256(hash_with_round || seal_type)
    let mut final_data = Vec::with_capacity(32 + 1);
    final_data.extend_from_slice(hash_with_round.as_slice());
    final_data.push(seal_type as u8);

    keccak256(&final_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bls::{SealerSet, SecretKey};

    #[test]
    fn test_wbft_extra_new() {
        let extra = WbftExtra::new();

        assert_eq!(extra.vanity_data, [0u8; 32]);
        assert_eq!(extra.randao_reveal, Vec::<u8>::new());
        assert_eq!(extra.prev_round, 0);
        assert!(extra.prev_prepared_seal.is_none());
        assert!(extra.prev_committed_seal.is_none());
        assert_eq!(extra.round, 0);
        assert!(extra.prepared_seal.is_none());
        assert!(extra.committed_seal.is_none());
        assert_eq!(extra.gas_tip, U256::ZERO);
        assert!(extra.epoch_info.is_none());
    }

    #[test]
    fn test_wbft_extra_default() {
        let extra = WbftExtra::default();
        assert_eq!(extra, WbftExtra::new());
    }

    #[test]
    fn test_wbft_extra_roundtrip_empty() {
        let extra = WbftExtra::new();

        let encoded = extra.encode();
        let decoded = WbftExtra::decode(&encoded).unwrap();

        assert_eq!(extra, decoded);
    }

    #[test]
    fn test_wbft_extra_roundtrip_with_seals() {
        let sk = SecretKey::random();
        let signature = sk.sign(b"test message");

        let mut bitmap = SealerSet::new(4);
        bitmap.set_sealer(0);
        bitmap.set_sealer(2);

        let seal = WbftAggregatedSeal {
            bitmap,
            signature: signature.to_bytes(),
        };

        let extra = WbftExtra {
            vanity_data: [0x42; 32],
            randao_reveal: vec![1, 2, 3],
            prev_round: 1,
            prev_prepared_seal: Some(seal.clone()),
            prev_committed_seal: Some(seal.clone()),
            round: 2,
            prepared_seal: Some(seal.clone()),
            committed_seal: Some(seal),
            gas_tip: U256::from(1_000_000_000u64),
            epoch_info: None,
        };

        let encoded = extra.encode();
        let decoded = WbftExtra::decode(&encoded).unwrap();

        assert_eq!(extra, decoded);
    }

    #[test]
    fn test_wbft_extra_roundtrip_with_epoch_info() {
        let candidates = vec![
            Candidate {
                addr: Address::from([0x01; 20]),
                diligence: 950_000,
            },
            Candidate {
                addr: Address::from([0x02; 20]),
                diligence: 980_000,
            },
        ];

        let epoch_info = EpochInfo {
            candidates,
            validators: vec![0, 1],
            bls_public_keys: vec![vec![0x42; 48], vec![0x43; 48]],
        };

        let extra = WbftExtra {
            vanity_data: [0u8; 32],
            randao_reveal: Vec::new(),
            prev_round: 0,
            prev_prepared_seal: None,
            prev_committed_seal: None,
            round: 0,
            prepared_seal: None,
            committed_seal: None,
            gas_tip: U256::from(1_000_000_000u64),
            epoch_info: Some(epoch_info),
        };

        let encoded = extra.encode();
        let decoded = WbftExtra::decode(&encoded).unwrap();

        assert_eq!(extra, decoded);
    }

    #[test]
    fn test_candidate_creation() {
        let candidate = Candidate {
            addr: Address::from([0x01; 20]),
            diligence: 950_000,
        };

        assert_eq!(candidate.addr, Address::from([0x01; 20]));
        assert_eq!(candidate.diligence, 950_000);
    }

    #[test]
    fn test_epoch_info_creation() {
        let epoch_info = EpochInfo {
            candidates: vec![
                Candidate {
                    addr: Address::from([0x01; 20]),
                    diligence: 950_000,
                },
            ],
            validators: vec![0],
            bls_public_keys: vec![vec![0x42; 48]],
        };

        assert_eq!(epoch_info.candidates.len(), 1);
        assert_eq!(epoch_info.validators.len(), 1);
        assert_eq!(epoch_info.bls_public_keys.len(), 1);
    }

    #[test]
    fn test_seal_type_values() {
        assert_eq!(SealType::Prepare as u8, 0);
        assert_eq!(SealType::Commit as u8, 1);
    }

    #[test]
    fn test_prepare_seal_hash_different_rounds() {
        use alloy_primitives::B256;

        let block_hash = B256::from([0x42; 32]);

        let hash_round_0 = prepare_seal_hash(block_hash, 0, SealType::Prepare);
        let hash_round_1 = prepare_seal_hash(block_hash, 1, SealType::Prepare);

        // Different rounds should produce different hashes
        assert_ne!(hash_round_0, hash_round_1);
    }

    #[test]
    fn test_prepare_seal_hash_different_seal_types() {
        use alloy_primitives::B256;

        let block_hash = B256::from([0x42; 32]);
        let round = 0;

        let prepare_hash = prepare_seal_hash(block_hash, round, SealType::Prepare);
        let commit_hash = prepare_seal_hash(block_hash, round, SealType::Commit);

        // Different seal types should produce different hashes
        assert_ne!(prepare_hash, commit_hash);
    }

    #[test]
    fn test_prepare_seal_hash_deterministic() {
        use alloy_primitives::B256;

        let block_hash = B256::from([0x42; 32]);
        let round = 5;

        let hash1 = prepare_seal_hash(block_hash, round, SealType::Prepare);
        let hash2 = prepare_seal_hash(block_hash, round, SealType::Prepare);

        // Same inputs should produce same hash
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_prepare_seal_hash_different_blocks() {
        use alloy_primitives::B256;

        let block_hash_1 = B256::from([0x42; 32]);
        let block_hash_2 = B256::from([0x43; 32]);

        let hash1 = prepare_seal_hash(block_hash_1, 0, SealType::Prepare);
        let hash2 = prepare_seal_hash(block_hash_2, 0, SealType::Prepare);

        // Different blocks should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_wbft_extra_with_gas_tip() {
        let extra = WbftExtra {
            vanity_data: [0u8; 32],
            randao_reveal: Vec::new(),
            prev_round: 0,
            prev_prepared_seal: None,
            prev_committed_seal: None,
            round: 0,
            prepared_seal: None,
            committed_seal: None,
            gas_tip: U256::from(5_000_000_000u64),
            epoch_info: None,
        };

        let encoded = extra.encode();
        let decoded = WbftExtra::decode(&encoded).unwrap();

        assert_eq!(decoded.gas_tip, U256::from(5_000_000_000u64));
    }
}
