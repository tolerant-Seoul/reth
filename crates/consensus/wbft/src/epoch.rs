//! WBFT Epoch Management
//!
//! This module handles epoch transitions and validator set management.
//! Epochs are fixed-length periods after which the validator set may change.

use crate::{
    consensus::WbftConfig,
    header::{EpochInfo, WbftExtra},
    validator::{DefaultValidator, DefaultValidatorSet, ValidatorSet},
};
use alloy_primitives::Address;
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur during epoch management
#[derive(Debug, Error)]
pub enum EpochError {
    /// Failed to decode extra data
    #[error("failed to decode extra data: {0}")]
    DecodeError(String),

    /// Missing epoch info in epoch block
    #[error("missing epoch info in block {block_number}")]
    MissingEpochInfo {
        /// Block number that should contain epoch info
        block_number: u64,
    },

    /// Invalid epoch info
    #[error("invalid epoch info: {0}")]
    InvalidEpochInfo(String),

    /// Validator index out of bounds
    #[error("validator index {index} out of bounds (max {max})")]
    ValidatorIndexOutOfBounds {
        /// Invalid index
        index: u32,
        /// Maximum valid index
        max: usize,
    },

    /// BLS key count mismatch
    #[error("BLS key count mismatch: expected {expected}, got {actual}")]
    BlsKeyCountMismatch {
        /// Expected count
        expected: usize,
        /// Actual count
        actual: usize,
    },
}

/// Epoch manager for validator set transitions
///
/// Manages epoch boundaries and validator set updates based on
/// the WBFT configuration.
#[derive(Debug, Clone)]
pub struct EpochManager {
    /// WBFT configuration
    config: WbftConfig,
}

impl EpochManager {
    /// Create a new epoch manager
    ///
    /// # Arguments
    ///
    /// * `config` - WBFT configuration with epoch settings
    pub fn new(config: WbftConfig) -> Self {
        Self { config }
    }

    /// Get the epoch length
    pub fn epoch_length(&self) -> u64 {
        self.config.epoch_length
    }

    /// Check if a block number is an epoch block
    ///
    /// Epoch blocks contain validator set updates.
    ///
    /// # Arguments
    ///
    /// * `block_number` - Block number to check
    pub fn is_epoch_block(&self, block_number: u64) -> bool {
        block_number > 0 && block_number % self.config.epoch_length == 0
    }

    /// Get the epoch number for a block
    ///
    /// # Arguments
    ///
    /// * `block_number` - Block number
    ///
    /// # Returns
    ///
    /// Epoch number (0-indexed)
    pub fn epoch_number(&self, block_number: u64) -> u64 {
        block_number / self.config.epoch_length
    }

    /// Get the first block of an epoch
    ///
    /// # Arguments
    ///
    /// * `epoch` - Epoch number
    ///
    /// # Returns
    ///
    /// Block number of the first block in the epoch
    pub fn epoch_start_block(&self, epoch: u64) -> u64 {
        epoch * self.config.epoch_length
    }

    /// Get the last block of an epoch
    ///
    /// # Arguments
    ///
    /// * `epoch` - Epoch number
    ///
    /// # Returns
    ///
    /// Block number of the last block in the epoch
    pub fn epoch_end_block(&self, epoch: u64) -> u64 {
        (epoch + 1) * self.config.epoch_length - 1
    }

    /// Load epoch info from block extra data
    ///
    /// # Arguments
    ///
    /// * `block_number` - Block number
    /// * `extra_data` - Block header extra data
    ///
    /// # Returns
    ///
    /// EpochInfo if this is an epoch block, None otherwise
    pub fn load_epoch_info(
        &self,
        block_number: u64,
        extra_data: &[u8],
    ) -> Result<Option<EpochInfo>, EpochError> {
        if !self.is_epoch_block(block_number) {
            return Ok(None);
        }

        let extra = WbftExtra::decode(&mut &extra_data[..])
            .map_err(|e| EpochError::DecodeError(e.to_string()))?;

        extra.epoch_info.ok_or(EpochError::MissingEpochInfo {
            block_number,
        }).map(Some)
    }

    /// Extract validator set from epoch info
    ///
    /// Creates a ValidatorSet from the epoch info by selecting
    /// active validators from the candidate list.
    ///
    /// # Arguments
    ///
    /// * `epoch_info` - Epoch information from block header
    ///
    /// # Returns
    ///
    /// ValidatorSet for the new epoch
    pub fn extract_validator_set(
        &self,
        epoch_info: &EpochInfo,
    ) -> Result<Arc<dyn ValidatorSet>, EpochError> {
        // Validate epoch info
        self.validate_epoch_info(epoch_info)?;

        // Extract active validators
        let mut validators = Vec::with_capacity(epoch_info.validators.len());

        for &idx in &epoch_info.validators {
            let candidate = epoch_info.candidates.get(idx as usize).ok_or(
                EpochError::ValidatorIndexOutOfBounds {
                    index: idx,
                    max: epoch_info.candidates.len(),
                },
            )?;

            let bls_key = epoch_info.bls_public_keys.get(idx as usize).ok_or(
                EpochError::ValidatorIndexOutOfBounds {
                    index: idx,
                    max: epoch_info.bls_public_keys.len(),
                },
            )?;

            // Convert Vec<u8> to [u8; 48]
            let bls_key_array: [u8; 48] = bls_key
                .as_slice()
                .try_into()
                .map_err(|_| EpochError::InvalidEpochInfo(
                    format!("BLS key at index {} has invalid length: expected 48, got {}", idx, bls_key.len())
                ))?;

            validators.push(DefaultValidator::new(candidate.addr, bls_key_array));
        }

        let validator_set = DefaultValidatorSet::new(validators)
            .map_err(|e| EpochError::InvalidEpochInfo(e.to_string()))?;

        Ok(Arc::new(validator_set))
    }

    /// Validate epoch info structure
    ///
    /// Checks that the epoch info is well-formed.
    fn validate_epoch_info(&self, epoch_info: &EpochInfo) -> Result<(), EpochError> {
        // Check BLS key count matches candidates
        if epoch_info.bls_public_keys.len() != epoch_info.candidates.len() {
            return Err(EpochError::BlsKeyCountMismatch {
                expected: epoch_info.candidates.len(),
                actual: epoch_info.bls_public_keys.len(),
            });
        }

        // Check all validator indices are valid
        for &idx in &epoch_info.validators {
            if idx as usize >= epoch_info.candidates.len() {
                return Err(EpochError::ValidatorIndexOutOfBounds {
                    index: idx,
                    max: epoch_info.candidates.len(),
                });
            }
        }

        // Check we have at least one validator
        if epoch_info.validators.is_empty() {
            return Err(EpochError::InvalidEpochInfo(
                "no validators in epoch info".to_string(),
            ));
        }

        Ok(())
    }

    /// Get validator addresses from epoch info
    ///
    /// # Arguments
    ///
    /// * `epoch_info` - Epoch information
    ///
    /// # Returns
    ///
    /// List of active validator addresses
    pub fn get_validator_addresses(&self, epoch_info: &EpochInfo) -> Result<Vec<Address>, EpochError> {
        self.validate_epoch_info(epoch_info)?;

        let addresses = epoch_info
            .validators
            .iter()
            .filter_map(|&idx| {
                epoch_info.candidates.get(idx as usize).map(|c| c.addr)
            })
            .collect();

        Ok(addresses)
    }

    /// Check if an address is a validator in the epoch
    ///
    /// # Arguments
    ///
    /// * `epoch_info` - Epoch information
    /// * `address` - Address to check
    ///
    /// # Returns
    ///
    /// True if address is an active validator
    pub fn is_validator(&self, epoch_info: &EpochInfo, address: Address) -> bool {
        epoch_info.validators.iter().any(|&idx| {
            epoch_info
                .candidates
                .get(idx as usize)
                .map(|c| c.addr == address)
                .unwrap_or(false)
        })
    }

    /// Get the number of blocks remaining in current epoch
    ///
    /// # Arguments
    ///
    /// * `block_number` - Current block number
    ///
    /// # Returns
    ///
    /// Number of blocks until next epoch
    pub fn blocks_until_epoch(&self, block_number: u64) -> u64 {
        let epoch = self.epoch_number(block_number);
        let epoch_end = self.epoch_end_block(epoch);
        epoch_end - block_number
    }

    /// Get the progress within current epoch (0.0 to 1.0)
    ///
    /// # Arguments
    ///
    /// * `block_number` - Current block number
    ///
    /// # Returns
    ///
    /// Progress as a fraction (0.0 at start, 1.0 at end)
    pub fn epoch_progress(&self, block_number: u64) -> f64 {
        let epoch = self.epoch_number(block_number);
        let epoch_start = self.epoch_start_block(epoch);
        let position = block_number - epoch_start;
        position as f64 / self.config.epoch_length as f64
    }
}

impl Default for EpochManager {
    fn default() -> Self {
        Self::new(WbftConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::Candidate;

    fn create_test_config() -> WbftConfig {
        WbftConfig {
            epoch_length: 100,
            ..Default::default()
        }
    }

    fn create_test_epoch_info() -> EpochInfo {
        EpochInfo {
            candidates: vec![
                Candidate {
                    addr: Address::from([0x01; 20]),
                    diligence: 950_000,
                },
                Candidate {
                    addr: Address::from([0x02; 20]),
                    diligence: 900_000,
                },
                Candidate {
                    addr: Address::from([0x03; 20]),
                    diligence: 850_000,
                },
            ],
            validators: vec![0, 1, 2],
            bls_public_keys: vec![
                vec![0x42; 48],
                vec![0x43; 48],
                vec![0x44; 48],
            ],
        }
    }

    #[test]
    fn test_epoch_manager_creation() {
        let config = create_test_config();
        let manager = EpochManager::new(config);
        assert_eq!(manager.epoch_length(), 100);
    }

    #[test]
    fn test_epoch_manager_default() {
        let manager = EpochManager::default();
        assert_eq!(manager.epoch_length(), 10);
    }

    #[test]
    fn test_is_epoch_block() {
        let manager = EpochManager::new(create_test_config());

        assert!(!manager.is_epoch_block(0));
        assert!(!manager.is_epoch_block(1));
        assert!(!manager.is_epoch_block(99));
        assert!(manager.is_epoch_block(100));
        assert!(!manager.is_epoch_block(101));
        assert!(manager.is_epoch_block(200));
        assert!(manager.is_epoch_block(1000));
    }

    #[test]
    fn test_epoch_number() {
        let manager = EpochManager::new(create_test_config());

        assert_eq!(manager.epoch_number(0), 0);
        assert_eq!(manager.epoch_number(99), 0);
        assert_eq!(manager.epoch_number(100), 1);
        assert_eq!(manager.epoch_number(199), 1);
        assert_eq!(manager.epoch_number(200), 2);
        assert_eq!(manager.epoch_number(1000), 10);
    }

    #[test]
    fn test_epoch_start_block() {
        let manager = EpochManager::new(create_test_config());

        assert_eq!(manager.epoch_start_block(0), 0);
        assert_eq!(manager.epoch_start_block(1), 100);
        assert_eq!(manager.epoch_start_block(2), 200);
        assert_eq!(manager.epoch_start_block(10), 1000);
    }

    #[test]
    fn test_epoch_end_block() {
        let manager = EpochManager::new(create_test_config());

        assert_eq!(manager.epoch_end_block(0), 99);
        assert_eq!(manager.epoch_end_block(1), 199);
        assert_eq!(manager.epoch_end_block(2), 299);
        assert_eq!(manager.epoch_end_block(10), 1099);
    }

    #[test]
    fn test_blocks_until_epoch() {
        let manager = EpochManager::new(create_test_config());

        assert_eq!(manager.blocks_until_epoch(0), 99);
        assert_eq!(manager.blocks_until_epoch(50), 49);
        assert_eq!(manager.blocks_until_epoch(99), 0);
        assert_eq!(manager.blocks_until_epoch(100), 99);
    }

    #[test]
    fn test_epoch_progress() {
        let manager = EpochManager::new(create_test_config());

        assert_eq!(manager.epoch_progress(0), 0.0);
        assert_eq!(manager.epoch_progress(50), 0.5);
        assert_eq!(manager.epoch_progress(100), 0.0); // Start of new epoch
        assert_eq!(manager.epoch_progress(150), 0.5);
    }

    #[test]
    fn test_extract_validator_set() {
        let manager = EpochManager::new(create_test_config());
        let epoch_info = create_test_epoch_info();

        let validator_set = manager.extract_validator_set(&epoch_info).unwrap();

        assert_eq!(validator_set.size(), 3);
        assert!(validator_set.is_validator(&Address::from([0x01; 20])));
        assert!(validator_set.is_validator(&Address::from([0x02; 20])));
        assert!(validator_set.is_validator(&Address::from([0x03; 20])));
    }

    #[test]
    fn test_extract_validator_set_subset() {
        let manager = EpochManager::new(create_test_config());
        let mut epoch_info = create_test_epoch_info();
        epoch_info.validators = vec![0, 2]; // Only first and third

        let validator_set = manager.extract_validator_set(&epoch_info).unwrap();

        assert_eq!(validator_set.size(), 2);
        assert!(validator_set.is_validator(&Address::from([0x01; 20])));
        assert!(!validator_set.is_validator(&Address::from([0x02; 20])));
        assert!(validator_set.is_validator(&Address::from([0x03; 20])));
    }

    #[test]
    fn test_get_validator_addresses() {
        let manager = EpochManager::new(create_test_config());
        let epoch_info = create_test_epoch_info();

        let addresses = manager.get_validator_addresses(&epoch_info).unwrap();

        assert_eq!(addresses.len(), 3);
        assert_eq!(addresses[0], Address::from([0x01; 20]));
        assert_eq!(addresses[1], Address::from([0x02; 20]));
        assert_eq!(addresses[2], Address::from([0x03; 20]));
    }

    #[test]
    fn test_is_validator() {
        let manager = EpochManager::new(create_test_config());
        let epoch_info = create_test_epoch_info();

        assert!(manager.is_validator(&epoch_info, Address::from([0x01; 20])));
        assert!(manager.is_validator(&epoch_info, Address::from([0x02; 20])));
        assert!(manager.is_validator(&epoch_info, Address::from([0x03; 20])));
        assert!(!manager.is_validator(&epoch_info, Address::from([0x04; 20])));
    }

    #[test]
    fn test_validate_epoch_info_empty_validators() {
        let manager = EpochManager::new(create_test_config());
        let mut epoch_info = create_test_epoch_info();
        epoch_info.validators = vec![];

        let result = manager.extract_validator_set(&epoch_info);
        assert!(matches!(result, Err(EpochError::InvalidEpochInfo(_))));
    }

    #[test]
    fn test_validate_epoch_info_invalid_index() {
        let manager = EpochManager::new(create_test_config());
        let mut epoch_info = create_test_epoch_info();
        epoch_info.validators = vec![0, 1, 10]; // Index 10 is out of bounds

        let result = manager.extract_validator_set(&epoch_info);
        assert!(matches!(result, Err(EpochError::ValidatorIndexOutOfBounds { .. })));
    }

    #[test]
    fn test_validate_epoch_info_bls_key_mismatch() {
        let manager = EpochManager::new(create_test_config());
        let mut epoch_info = create_test_epoch_info();
        epoch_info.bls_public_keys = vec![vec![0x42; 48], vec![0x43; 48]]; // Missing one

        let result = manager.extract_validator_set(&epoch_info);
        assert!(matches!(result, Err(EpochError::BlsKeyCountMismatch { .. })));
    }

    #[test]
    fn test_validate_epoch_info_invalid_bls_key_length() {
        let manager = EpochManager::new(create_test_config());
        let mut epoch_info = create_test_epoch_info();
        epoch_info.bls_public_keys[1] = vec![0x43; 32]; // Wrong length

        let result = manager.extract_validator_set(&epoch_info);
        assert!(matches!(result, Err(EpochError::InvalidEpochInfo(_))));
    }

    #[test]
    fn test_load_epoch_info_non_epoch_block() {
        let manager = EpochManager::new(create_test_config());
        let extra = WbftExtra::default();
        let extra_data = extra.encode();

        let result = manager.load_epoch_info(50, &extra_data).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_load_epoch_info_epoch_block_with_info() {
        let manager = EpochManager::new(create_test_config());
        let mut extra = WbftExtra::default();
        extra.epoch_info = Some(create_test_epoch_info());
        let extra_data = extra.encode();

        let result = manager.load_epoch_info(100, &extra_data).unwrap();
        assert!(result.is_some());

        let epoch_info = result.unwrap();
        assert_eq!(epoch_info.candidates.len(), 3);
        assert_eq!(epoch_info.validators.len(), 3);
    }

    #[test]
    fn test_load_epoch_info_epoch_block_missing_info() {
        let manager = EpochManager::new(create_test_config());
        let extra = WbftExtra::default();
        let extra_data = extra.encode();

        let result = manager.load_epoch_info(100, &extra_data);
        assert!(matches!(result, Err(EpochError::MissingEpochInfo { .. })));
    }

    #[test]
    fn test_load_epoch_info_invalid_data() {
        let manager = EpochManager::new(create_test_config());
        let invalid_data = vec![0x01, 0x02, 0x03];

        let result = manager.load_epoch_info(100, &invalid_data);
        assert!(matches!(result, Err(EpochError::DecodeError(_))));
    }

    #[test]
    fn test_epoch_error_display() {
        let err = EpochError::MissingEpochInfo { block_number: 100 };
        assert!(err.to_string().contains("100"));

        let err = EpochError::ValidatorIndexOutOfBounds { index: 5, max: 3 };
        assert!(err.to_string().contains("5"));
        assert!(err.to_string().contains("3"));

        let err = EpochError::BlsKeyCountMismatch { expected: 3, actual: 2 };
        assert!(err.to_string().contains("3"));
        assert!(err.to_string().contains("2"));
    }

    #[test]
    fn test_single_validator_epoch() {
        let manager = EpochManager::new(create_test_config());
        let epoch_info = EpochInfo {
            candidates: vec![Candidate {
                addr: Address::from([0x01; 20]),
                diligence: 1_000_000,
            }],
            validators: vec![0],
            bls_public_keys: vec![vec![0x42; 48]],
        };

        let validator_set = manager.extract_validator_set(&epoch_info).unwrap();
        assert_eq!(validator_set.size(), 1);
    }

    #[test]
    fn test_epoch_boundaries() {
        let manager = EpochManager::new(create_test_config());

        // Test epoch 0
        assert_eq!(manager.epoch_start_block(0), 0);
        assert_eq!(manager.epoch_end_block(0), 99);

        // Test that end of epoch N is one before start of epoch N+1
        for epoch in 0..10 {
            let end = manager.epoch_end_block(epoch);
            let next_start = manager.epoch_start_block(epoch + 1);
            assert_eq!(end + 1, next_start);
        }
    }
}
