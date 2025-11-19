//! WBFT Block Sealer
//!
//! This module implements the block sealing mechanism for WBFT consensus.
//! The sealer is responsible for collecting signatures and creating the
//! final sealed block with aggregated BLS signatures.

use crate::{
    bls::WbftAggregatedSeal,
    header::WbftExtra,
};
use alloy_primitives::{Bytes, B256};
use thiserror::Error;

/// Errors that can occur during block sealing
#[derive(Debug, Error)]
pub enum SealerError {
    /// Failed to decode extra data
    #[error("failed to decode extra data: {0}")]
    DecodeError(String),

    /// Failed to encode extra data
    #[error("failed to encode extra data: {0}")]
    EncodeError(String),

    /// Consensus not reached
    #[error("consensus not reached: {reason}")]
    ConsensusNotReached {
        /// Reason consensus failed
        reason: String,
    },

    /// Invalid block for sealing
    #[error("invalid block for sealing: {0}")]
    InvalidBlock(String),

    /// Sealing timeout
    #[error("sealing timeout after {0} ms")]
    Timeout(u64),
}

/// Result of a successful seal operation
#[derive(Debug, Clone)]
pub struct SealResult {
    /// Aggregated prepare seal from validators
    pub prepared_seal: WbftAggregatedSeal,
    /// Aggregated commit seal from validators
    pub committed_seal: WbftAggregatedSeal,
}

impl SealResult {
    /// Create a new seal result
    pub fn new(prepared_seal: WbftAggregatedSeal, committed_seal: WbftAggregatedSeal) -> Self {
        Self {
            prepared_seal,
            committed_seal,
        }
    }
}

/// Block sealer for WBFT consensus
///
/// The sealer collects signatures from validators and creates the final
/// sealed block with BLS signature aggregation.
#[derive(Debug)]
pub struct WbftSealer {
    /// Timeout for sealing operations in milliseconds
    seal_timeout_ms: u64,
}

impl WbftSealer {
    /// Create a new sealer with default timeout
    pub fn new() -> Self {
        Self {
            seal_timeout_ms: 10000, // 10 seconds default
        }
    }

    /// Create a new sealer with custom timeout
    ///
    /// # Arguments
    ///
    /// * `timeout_ms` - Timeout in milliseconds for seal operations
    pub fn with_timeout(timeout_ms: u64) -> Self {
        Self {
            seal_timeout_ms: timeout_ms,
        }
    }

    /// Get the seal timeout in milliseconds
    pub fn timeout_ms(&self) -> u64 {
        self.seal_timeout_ms
    }

    /// Apply seals to block extra data
    ///
    /// Takes the original extra data and the seal result, and produces
    /// new extra data with the seals included.
    ///
    /// # Arguments
    ///
    /// * `extra_data` - Original block extra data
    /// * `seal_result` - Seal result containing prepared and committed seals
    ///
    /// # Returns
    ///
    /// New extra data bytes with seals applied
    pub fn apply_seals(
        &self,
        extra_data: &[u8],
        seal_result: &SealResult,
    ) -> Result<Bytes, SealerError> {
        // Decode existing extra data
        let mut extra = WbftExtra::decode(&mut &extra_data[..])
            .map_err(|e| SealerError::DecodeError(e.to_string()))?;

        // Apply the seals
        extra.prepared_seal = Some(seal_result.prepared_seal.clone());
        extra.committed_seal = Some(seal_result.committed_seal.clone());

        // Encode and return
        let encoded = extra.encode();
        Ok(Bytes::from(encoded))
    }

    /// Verify that seals are present and valid in extra data
    ///
    /// # Arguments
    ///
    /// * `extra_data` - Block extra data to verify
    ///
    /// # Returns
    ///
    /// Ok(()) if seals are present and structurally valid
    pub fn verify_seals(&self, extra_data: &[u8]) -> Result<(), SealerError> {
        let extra = WbftExtra::decode(&mut &extra_data[..])
            .map_err(|e| SealerError::DecodeError(e.to_string()))?;

        if extra.prepared_seal.is_none() {
            return Err(SealerError::InvalidBlock(
                "missing prepared seal".to_string(),
            ));
        }

        if extra.committed_seal.is_none() {
            return Err(SealerError::InvalidBlock(
                "missing committed seal".to_string(),
            ));
        }

        Ok(())
    }

    /// Extract seal result from sealed block extra data
    ///
    /// # Arguments
    ///
    /// * `extra_data` - Sealed block extra data
    ///
    /// # Returns
    ///
    /// Extracted seal result
    pub fn extract_seals(&self, extra_data: &[u8]) -> Result<SealResult, SealerError> {
        let extra = WbftExtra::decode(&mut &extra_data[..])
            .map_err(|e| SealerError::DecodeError(e.to_string()))?;

        let prepared_seal = extra.prepared_seal.ok_or_else(|| {
            SealerError::InvalidBlock("missing prepared seal".to_string())
        })?;

        let committed_seal = extra.committed_seal.ok_or_else(|| {
            SealerError::InvalidBlock("missing committed seal".to_string())
        })?;

        Ok(SealResult::new(prepared_seal, committed_seal))
    }

    /// Create empty seals for genesis block
    ///
    /// Genesis blocks have empty seals since there's no consensus round.
    pub fn create_genesis_seals(&self) -> SealResult {
        use crate::bls::SealerSet;
        SealResult::new(
            WbftAggregatedSeal::new(SealerSet::new(0), [0u8; 96]),
            WbftAggregatedSeal::new(SealerSet::new(0), [0u8; 96]),
        )
    }

    /// Check if block has valid seals
    ///
    /// # Arguments
    ///
    /// * `extra_data` - Block extra data
    ///
    /// # Returns
    ///
    /// True if seals are present
    pub fn has_seals(&self, extra_data: &[u8]) -> bool {
        self.verify_seals(extra_data).is_ok()
    }
}

impl Default for WbftSealer {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating seal results during consensus
#[derive(Debug, Default)]
pub struct SealResultBuilder {
    prepared_seal: Option<WbftAggregatedSeal>,
    committed_seal: Option<WbftAggregatedSeal>,
}

impl SealResultBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the prepared seal
    pub fn with_prepared_seal(mut self, seal: WbftAggregatedSeal) -> Self {
        self.prepared_seal = Some(seal);
        self
    }

    /// Set the committed seal
    pub fn with_committed_seal(mut self, seal: WbftAggregatedSeal) -> Self {
        self.committed_seal = Some(seal);
        self
    }

    /// Build the seal result
    ///
    /// # Returns
    ///
    /// SealResult if both seals are set, error otherwise
    pub fn build(self) -> Result<SealResult, SealerError> {
        let prepared_seal = self.prepared_seal.ok_or_else(|| {
            SealerError::ConsensusNotReached {
                reason: "prepared seal not collected".to_string(),
            }
        })?;

        let committed_seal = self.committed_seal.ok_or_else(|| {
            SealerError::ConsensusNotReached {
                reason: "committed seal not collected".to_string(),
            }
        })?;

        Ok(SealResult::new(prepared_seal, committed_seal))
    }

    /// Check if builder has prepared seal
    pub fn has_prepared(&self) -> bool {
        self.prepared_seal.is_some()
    }

    /// Check if builder has committed seal
    pub fn has_committed(&self) -> bool {
        self.committed_seal.is_some()
    }

    /// Check if builder is complete
    pub fn is_complete(&self) -> bool {
        self.has_prepared() && self.has_committed()
    }
}

/// Seal verification context
#[derive(Debug, Clone)]
pub struct SealVerificationContext {
    /// Block hash being sealed
    pub block_hash: B256,
    /// Expected number of signers for quorum
    pub quorum_size: usize,
    /// Round number
    pub round: u64,
}

impl SealVerificationContext {
    /// Create a new verification context
    pub fn new(block_hash: B256, quorum_size: usize, round: u64) -> Self {
        Self {
            block_hash,
            quorum_size,
            round,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bls::{SealerSet, WbftAggregatedSeal};
    use crate::header::WbftExtra;
    use alloy_primitives::U256;

    fn create_test_seal() -> WbftAggregatedSeal {
        let mut sealers = SealerSet::new(4);
        sealers.set_sealer(0);
        sealers.set_sealer(1);
        sealers.set_sealer(2);
        WbftAggregatedSeal::new(sealers, [0x42; 96])
    }

    fn create_test_extra() -> WbftExtra {
        WbftExtra {
            vanity_data: [0u8; 32],
            randao_reveal: Vec::new(),
            prev_round: 0,
            prev_prepared_seal: None,
            prev_committed_seal: None,
            round: 1,
            prepared_seal: None,
            committed_seal: None,
            gas_tip: U256::ZERO,
            epoch_info: None,
        }
    }

    #[test]
    fn test_sealer_creation() {
        let sealer = WbftSealer::new();
        assert_eq!(sealer.timeout_ms(), 10000);

        let sealer = WbftSealer::with_timeout(5000);
        assert_eq!(sealer.timeout_ms(), 5000);
    }

    #[test]
    fn test_sealer_default() {
        let sealer = WbftSealer::default();
        assert_eq!(sealer.timeout_ms(), 10000);
    }

    #[test]
    fn test_seal_result_creation() {
        let prepared = create_test_seal();
        let committed = create_test_seal();

        let result = SealResult::new(prepared.clone(), committed.clone());

        assert_eq!(result.prepared_seal.signer_count(), 3);
        assert_eq!(result.committed_seal.signer_count(), 3);
    }

    #[test]
    fn test_apply_seals() {
        let sealer = WbftSealer::new();
        let extra = create_test_extra();
        let extra_data = extra.encode();

        let seal_result = SealResult::new(create_test_seal(), create_test_seal());

        let sealed_data = sealer.apply_seals(&extra_data, &seal_result).unwrap();

        // Verify seals were applied
        let decoded = WbftExtra::decode(&mut &sealed_data[..]).unwrap();
        assert!(decoded.prepared_seal.is_some());
        assert!(decoded.committed_seal.is_some());
    }

    #[test]
    fn test_verify_seals_success() {
        let sealer = WbftSealer::new();

        let mut extra = create_test_extra();
        extra.prepared_seal = Some(create_test_seal());
        extra.committed_seal = Some(create_test_seal());

        let extra_data = extra.encode();

        assert!(sealer.verify_seals(&extra_data).is_ok());
    }

    #[test]
    fn test_verify_seals_missing_prepared() {
        let sealer = WbftSealer::new();

        let mut extra = create_test_extra();
        extra.committed_seal = Some(create_test_seal());

        let extra_data = extra.encode();

        let result = sealer.verify_seals(&extra_data);
        assert!(matches!(result, Err(SealerError::InvalidBlock(_))));
    }

    #[test]
    fn test_verify_seals_missing_committed() {
        let sealer = WbftSealer::new();

        let mut extra = create_test_extra();
        extra.prepared_seal = Some(create_test_seal());

        let extra_data = extra.encode();

        let result = sealer.verify_seals(&extra_data);
        assert!(matches!(result, Err(SealerError::InvalidBlock(_))));
    }

    #[test]
    fn test_extract_seals() {
        let sealer = WbftSealer::new();

        let mut extra = create_test_extra();
        extra.prepared_seal = Some(create_test_seal());
        extra.committed_seal = Some(create_test_seal());

        let extra_data = extra.encode();

        let result = sealer.extract_seals(&extra_data).unwrap();
        assert_eq!(result.prepared_seal.signer_count(), 3);
        assert_eq!(result.committed_seal.signer_count(), 3);
    }

    #[test]
    fn test_extract_seals_missing() {
        let sealer = WbftSealer::new();
        let extra = create_test_extra();
        let extra_data = extra.encode();

        let result = sealer.extract_seals(&extra_data);
        assert!(matches!(result, Err(SealerError::InvalidBlock(_))));
    }

    #[test]
    fn test_create_genesis_seals() {
        let sealer = WbftSealer::new();
        let seals = sealer.create_genesis_seals();

        assert_eq!(seals.prepared_seal.signer_count(), 0);
        assert_eq!(seals.committed_seal.signer_count(), 0);
    }

    #[test]
    fn test_has_seals() {
        let sealer = WbftSealer::new();

        // Without seals
        let extra = create_test_extra();
        let extra_data = extra.encode();
        assert!(!sealer.has_seals(&extra_data));

        // With seals
        let mut extra = create_test_extra();
        extra.prepared_seal = Some(create_test_seal());
        extra.committed_seal = Some(create_test_seal());
        let extra_data = extra.encode();
        assert!(sealer.has_seals(&extra_data));
    }

    #[test]
    fn test_seal_result_builder() {
        let mut builder = SealResultBuilder::new();

        assert!(!builder.has_prepared());
        assert!(!builder.has_committed());
        assert!(!builder.is_complete());

        builder = builder.with_prepared_seal(create_test_seal());
        assert!(builder.has_prepared());
        assert!(!builder.is_complete());

        builder = builder.with_committed_seal(create_test_seal());
        assert!(builder.has_committed());
        assert!(builder.is_complete());

        let result = builder.build().unwrap();
        assert_eq!(result.prepared_seal.signer_count(), 3);
        assert_eq!(result.committed_seal.signer_count(), 3);
    }

    #[test]
    fn test_seal_result_builder_incomplete() {
        let builder = SealResultBuilder::new()
            .with_prepared_seal(create_test_seal());

        let result = builder.build();
        assert!(matches!(result, Err(SealerError::ConsensusNotReached { .. })));
    }

    #[test]
    fn test_seal_verification_context() {
        let ctx = SealVerificationContext::new(B256::ZERO, 3, 1);

        assert_eq!(ctx.block_hash, B256::ZERO);
        assert_eq!(ctx.quorum_size, 3);
        assert_eq!(ctx.round, 1);
    }

    #[test]
    fn test_sealer_error_display() {
        let err = SealerError::Timeout(5000);
        assert!(err.to_string().contains("5000"));

        let err = SealerError::ConsensusNotReached {
            reason: "test reason".to_string(),
        };
        assert!(err.to_string().contains("test reason"));

        let err = SealerError::InvalidBlock("bad block".to_string());
        assert!(err.to_string().contains("bad block"));
    }

    #[test]
    fn test_apply_seals_roundtrip() {
        let sealer = WbftSealer::new();

        // Create original extra
        let extra = create_test_extra();
        let original_data = extra.encode();

        // Apply seals
        let seal_result = SealResult::new(create_test_seal(), create_test_seal());
        let sealed_data = sealer.apply_seals(&original_data, &seal_result).unwrap();

        // Extract seals
        let extracted = sealer.extract_seals(&sealed_data).unwrap();

        // Verify
        assert_eq!(extracted.prepared_seal.signer_count(), seal_result.prepared_seal.signer_count());
        assert_eq!(extracted.committed_seal.signer_count(), seal_result.committed_seal.signer_count());
    }

    #[test]
    fn test_sealer_invalid_extra_data() {
        let sealer = WbftSealer::new();

        // Invalid data
        let invalid_data = vec![0x01, 0x02, 0x03];

        let result = sealer.verify_seals(&invalid_data);
        assert!(matches!(result, Err(SealerError::DecodeError(_))));
    }
}
