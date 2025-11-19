//! Contract Validator Provider
//!
//! This module provides a validator set provider that reads validator
//! information from on-chain contracts and caches the results.

use crate::{
    contracts::GovValidator,
    epoch::EpochManager,
    header::WbftExtra,
    validator::{DefaultValidator, DefaultValidatorSet, ValidatorSet},
};
use alloy_primitives::{Address, B256, U256};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use thiserror::Error;

/// Errors that can occur when providing validator sets
#[derive(Debug, Error)]
pub enum ValidatorProviderError {
    /// Failed to read from contract
    #[error("contract read error: {0}")]
    ContractReadError(String),

    /// Failed to load epoch info
    #[error("epoch info error: {0}")]
    EpochInfoError(String),

    /// Cache error
    #[error("cache error: {0}")]
    CacheError(String),

    /// Invalid validator data
    #[error("invalid validator data: {0}")]
    InvalidValidatorData(String),

    /// Block reader error
    #[error("block reader error: {0}")]
    BlockReaderError(String),
}

/// Block reader trait for accessing block headers
pub trait BlockReader: Send + Sync {
    /// Get block extra data by block number
    fn get_block_extra_data(&self, block_number: u64) -> Result<Vec<u8>, String>;

    /// Get block hash by block number
    fn get_block_hash(&self, block_number: u64) -> Result<B256, String>;
}

/// Storage reader trait for accessing contract storage
pub trait StorageReader: Send + Sync {
    /// Read storage value at given address and slot
    fn read_storage(&self, address: Address, slot: B256) -> Result<U256, String>;
}

/// Provider for validator sets from on-chain contracts
///
/// This provider reads validator information from the GovValidator contract
/// and caches the results for efficient access.
pub struct ContractValidatorProvider {
    /// GovValidator contract interface
    gov_validator: GovValidator,

    /// Epoch manager for epoch calculations
    epoch_manager: EpochManager,

    /// Cache for validator sets by epoch
    cache: RwLock<HashMap<u64, Arc<dyn ValidatorSet>>>,

    /// Maximum cache size
    max_cache_size: usize,
}

impl std::fmt::Debug for ContractValidatorProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContractValidatorProvider")
            .field("gov_validator", &self.gov_validator)
            .field("epoch_manager", &self.epoch_manager)
            .field("max_cache_size", &self.max_cache_size)
            .field("cache_entries", &self.cache.read().map(|c| c.len()).unwrap_or(0))
            .finish()
    }
}

impl ContractValidatorProvider {
    /// Create a new provider
    ///
    /// # Arguments
    ///
    /// * `gov_validator` - GovValidator contract interface
    /// * `epoch_manager` - Epoch manager for calculations
    pub fn new(gov_validator: GovValidator, epoch_manager: EpochManager) -> Self {
        Self {
            gov_validator,
            epoch_manager,
            cache: RwLock::new(HashMap::new()),
            max_cache_size: 100,
        }
    }

    /// Create with custom cache size
    ///
    /// # Arguments
    ///
    /// * `gov_validator` - GovValidator contract interface
    /// * `epoch_manager` - Epoch manager for calculations
    /// * `max_cache_size` - Maximum number of epochs to cache
    pub fn with_cache_size(
        gov_validator: GovValidator,
        epoch_manager: EpochManager,
        max_cache_size: usize,
    ) -> Self {
        Self {
            gov_validator,
            epoch_manager,
            cache: RwLock::new(HashMap::new()),
            max_cache_size,
        }
    }

    /// Get validators for a specific block number
    ///
    /// This method first checks the cache, then falls back to loading
    /// from epoch info or the contract.
    ///
    /// # Arguments
    ///
    /// * `block_number` - Block number to get validators for
    /// * `block_reader` - Block reader for accessing headers
    /// * `storage_reader` - Storage reader for contract access
    ///
    /// # Returns
    ///
    /// Validator set for the block
    pub fn get_validators(
        &self,
        block_number: u64,
        block_reader: &dyn BlockReader,
        storage_reader: &dyn StorageReader,
    ) -> Result<Arc<dyn ValidatorSet>, ValidatorProviderError> {
        let epoch = self.epoch_manager.epoch_number(block_number);

        // Check cache first
        {
            let cache = self.cache.read().map_err(|e| {
                ValidatorProviderError::CacheError(format!("Failed to acquire read lock: {}", e))
            })?;

            if let Some(validator_set) = cache.get(&epoch) {
                return Ok(validator_set.clone());
            }
        }

        // Load from epoch block or contract
        let validator_set = self.load_validators(epoch, block_reader, storage_reader)?;

        // Cache the result
        self.cache_validator_set(epoch, validator_set.clone())?;

        Ok(validator_set)
    }

    /// Load validators for an epoch
    fn load_validators(
        &self,
        epoch: u64,
        block_reader: &dyn BlockReader,
        storage_reader: &dyn StorageReader,
    ) -> Result<Arc<dyn ValidatorSet>, ValidatorProviderError> {
        // Try to load from epoch block first
        let epoch_block = self.epoch_manager.epoch_start_block(epoch);

        if epoch_block > 0 {
            // Try to load from block header
            match self.load_from_epoch_block(epoch_block, block_reader) {
                Ok(validator_set) => return Ok(validator_set),
                Err(_) => {
                    // Fall through to contract query
                }
            }
        }

        // Load from contract
        self.load_from_contract(storage_reader)
    }

    /// Load validators from epoch block header
    fn load_from_epoch_block(
        &self,
        block_number: u64,
        block_reader: &dyn BlockReader,
    ) -> Result<Arc<dyn ValidatorSet>, ValidatorProviderError> {
        let extra_data = block_reader
            .get_block_extra_data(block_number)
            .map_err(ValidatorProviderError::BlockReaderError)?;

        let extra = WbftExtra::decode(&mut &extra_data[..])
            .map_err(|e| ValidatorProviderError::EpochInfoError(e.to_string()))?;

        let epoch_info = extra.epoch_info.ok_or_else(|| {
            ValidatorProviderError::EpochInfoError("Missing epoch info".to_string())
        })?;

        // Extract validators from epoch info
        let validator_set = self
            .epoch_manager
            .extract_validator_set(&epoch_info)
            .map_err(|e| ValidatorProviderError::InvalidValidatorData(e.to_string()))?;

        Ok(validator_set)
    }

    /// Load validators from contract storage
    fn load_from_contract(
        &self,
        storage_reader: &dyn StorageReader,
    ) -> Result<Arc<dyn ValidatorSet>, ValidatorProviderError> {
        let contract_addr = self.gov_validator.address();

        // Create storage reader closure
        let reader = |addr: Address, slot: B256| -> Result<U256, String> {
            storage_reader.read_storage(addr, slot)
        };

        // Read validators from contract
        let addresses = self
            .gov_validator
            .read_validators(&reader)
            .map_err(|e| ValidatorProviderError::ContractReadError(e.to_string()))?;

        // Read BLS keys for all validators
        let bls_keys = self
            .gov_validator
            .read_all_bls_keys(&reader, &addresses)
            .map_err(|e| ValidatorProviderError::ContractReadError(e.to_string()))?;

        // Create validators
        let validators: Vec<DefaultValidator> = addresses
            .into_iter()
            .zip(bls_keys.into_iter())
            .map(|(addr, key)| DefaultValidator::new(addr, key))
            .collect();

        let validator_set = DefaultValidatorSet::new(validators)
            .map_err(|e| ValidatorProviderError::InvalidValidatorData(e.to_string()))?;

        Ok(Arc::new(validator_set))
    }

    /// Cache a validator set
    fn cache_validator_set(
        &self,
        epoch: u64,
        validator_set: Arc<dyn ValidatorSet>,
    ) -> Result<(), ValidatorProviderError> {
        let mut cache = self.cache.write().map_err(|e| {
            ValidatorProviderError::CacheError(format!("Failed to acquire write lock: {}", e))
        })?;

        // Evict old entries if cache is full
        if cache.len() >= self.max_cache_size {
            // Find oldest epoch
            if let Some(&oldest) = cache.keys().min() {
                cache.remove(&oldest);
            }
        }

        cache.insert(epoch, validator_set);
        Ok(())
    }

    /// Clear the cache
    pub fn clear_cache(&self) -> Result<(), ValidatorProviderError> {
        let mut cache = self.cache.write().map_err(|e| {
            ValidatorProviderError::CacheError(format!("Failed to acquire write lock: {}", e))
        })?;

        cache.clear();
        Ok(())
    }

    /// Get cache size
    pub fn cache_size(&self) -> Result<usize, ValidatorProviderError> {
        let cache = self.cache.read().map_err(|e| {
            ValidatorProviderError::CacheError(format!("Failed to acquire read lock: {}", e))
        })?;

        Ok(cache.len())
    }

    /// Check if epoch is cached
    pub fn is_cached(&self, epoch: u64) -> Result<bool, ValidatorProviderError> {
        let cache = self.cache.read().map_err(|e| {
            ValidatorProviderError::CacheError(format!("Failed to acquire read lock: {}", e))
        })?;

        Ok(cache.contains_key(&epoch))
    }

    /// Get gas tip from contract
    ///
    /// # Arguments
    ///
    /// * `storage_reader` - Storage reader for contract access
    ///
    /// # Returns
    ///
    /// Current gas tip value
    pub fn get_gas_tip(
        &self,
        storage_reader: &dyn StorageReader,
    ) -> Result<U256, ValidatorProviderError> {
        let reader = |addr: Address, slot: B256| -> Result<U256, String> {
            storage_reader.read_storage(addr, slot)
        };

        self.gov_validator
            .read_gas_tip(reader)
            .map_err(|e| ValidatorProviderError::ContractReadError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::WbftConfig;

    /// Mock block reader for testing
    struct MockBlockReader {
        blocks: HashMap<u64, Vec<u8>>,
    }

    impl MockBlockReader {
        fn new() -> Self {
            Self {
                blocks: HashMap::new(),
            }
        }

        fn add_block(&mut self, number: u64, extra_data: Vec<u8>) {
            self.blocks.insert(number, extra_data);
        }
    }

    impl BlockReader for MockBlockReader {
        fn get_block_extra_data(&self, block_number: u64) -> Result<Vec<u8>, String> {
            self.blocks
                .get(&block_number)
                .cloned()
                .ok_or_else(|| format!("Block {} not found", block_number))
        }

        fn get_block_hash(&self, _block_number: u64) -> Result<B256, String> {
            Ok(B256::ZERO)
        }
    }

    /// Mock storage reader for testing
    struct MockStorageReader {
        storage: HashMap<(Address, B256), U256>,
    }

    impl MockStorageReader {
        fn new() -> Self {
            Self {
                storage: HashMap::new(),
            }
        }

        fn set_storage(&mut self, addr: Address, slot: B256, value: U256) {
            self.storage.insert((addr, slot), value);
        }
    }

    impl StorageReader for MockStorageReader {
        fn read_storage(&self, address: Address, slot: B256) -> Result<U256, String> {
            self.storage
                .get(&(address, slot))
                .copied()
                .ok_or_else(|| format!("Storage not found: {:?} {:?}", address, slot))
        }
    }

    fn create_test_provider() -> ContractValidatorProvider {
        let gov_validator = GovValidator::new(Address::from([0x01; 20]));
        let config = WbftConfig::default();
        let epoch_manager = EpochManager::new(config);

        ContractValidatorProvider::new(gov_validator, epoch_manager)
    }

    #[test]
    fn test_provider_creation() {
        let provider = create_test_provider();

        assert_eq!(provider.max_cache_size, 100);
        assert_eq!(provider.cache_size().unwrap(), 0);
    }

    #[test]
    fn test_provider_with_cache_size() {
        let gov_validator = GovValidator::new(Address::from([0x01; 20]));
        let config = WbftConfig::default();
        let epoch_manager = EpochManager::new(config);

        let provider = ContractValidatorProvider::with_cache_size(
            gov_validator,
            epoch_manager,
            50,
        );

        assert_eq!(provider.max_cache_size, 50);
    }

    #[test]
    fn test_cache_operations() {
        let provider = create_test_provider();

        // Initially empty
        assert_eq!(provider.cache_size().unwrap(), 0);
        assert!(!provider.is_cached(0).unwrap());

        // After clearing (should work on empty cache)
        assert!(provider.clear_cache().is_ok());
        assert_eq!(provider.cache_size().unwrap(), 0);
    }

    #[test]
    fn test_load_from_contract() {
        use crate::contracts::gov_validator::{
            SLOT_VALIDATOR_TO_BLS_KEY, SLOT_VALIDATOR_VALIDATORS,
        };

        let contract_addr = Address::from([0x01; 20]);
        let gov_validator = GovValidator::new(contract_addr);
        let config = WbftConfig::default();
        let epoch_manager = EpochManager::new(config);

        let provider = ContractValidatorProvider::new(gov_validator, epoch_manager);

        // Setup mock storage
        let mut storage_reader = MockStorageReader::new();

        // Validator count = 1
        storage_reader.set_storage(contract_addr, SLOT_VALIDATOR_VALIDATORS, U256::from(1));

        // Validator address
        let validator = Address::from([0x42; 20]);
        let base_hash = alloy_primitives::keccak256(SLOT_VALIDATOR_VALIDATORS);
        let mut bytes = [0u8; 32];
        bytes[12..32].copy_from_slice(validator.as_slice());
        storage_reader.set_storage(contract_addr, base_hash, U256::from_be_bytes(bytes));

        // BLS key (48 bytes in 2 slots)
        let bls_key = [0x11u8; 48];
        let key_base_slot =
            GovValidator::mapping_element_slot(SLOT_VALIDATOR_TO_BLS_KEY, validator);

        let mut slot0_bytes = [0u8; 32];
        slot0_bytes.copy_from_slice(&bls_key[0..32]);
        storage_reader.set_storage(contract_addr, key_base_slot, U256::from_be_bytes(slot0_bytes));

        let slot1 = B256::from((U256::from_be_bytes(key_base_slot.0) + U256::from(1)).to_be_bytes());
        let mut slot1_bytes = [0u8; 32];
        slot1_bytes[16..32].copy_from_slice(&bls_key[32..48]);
        storage_reader.set_storage(contract_addr, slot1, U256::from_be_bytes(slot1_bytes));

        // Load from contract
        let result = provider.load_from_contract(&storage_reader);
        assert!(result.is_ok());

        let validator_set = result.unwrap();
        assert_eq!(validator_set.size(), 1);
        assert!(validator_set.is_validator(&validator));
    }

    #[test]
    fn test_get_validators_caches_result() {
        use crate::contracts::gov_validator::{
            SLOT_VALIDATOR_TO_BLS_KEY, SLOT_VALIDATOR_VALIDATORS,
        };

        let contract_addr = Address::from([0x01; 20]);
        let gov_validator = GovValidator::new(contract_addr);
        let config = WbftConfig::default();
        let epoch_manager = EpochManager::new(config);

        let provider = ContractValidatorProvider::new(gov_validator, epoch_manager);

        // Setup mock storage with a single validator
        let mut storage_reader = MockStorageReader::new();

        storage_reader.set_storage(contract_addr, SLOT_VALIDATOR_VALIDATORS, U256::from(1));

        let validator = Address::from([0x42; 20]);
        let base_hash = alloy_primitives::keccak256(SLOT_VALIDATOR_VALIDATORS);
        let mut bytes = [0u8; 32];
        bytes[12..32].copy_from_slice(validator.as_slice());
        storage_reader.set_storage(contract_addr, base_hash, U256::from_be_bytes(bytes));

        let bls_key = [0x11u8; 48];
        let key_base_slot =
            GovValidator::mapping_element_slot(SLOT_VALIDATOR_TO_BLS_KEY, validator);

        let mut slot0_bytes = [0u8; 32];
        slot0_bytes.copy_from_slice(&bls_key[0..32]);
        storage_reader.set_storage(contract_addr, key_base_slot, U256::from_be_bytes(slot0_bytes));

        let slot1 = B256::from((U256::from_be_bytes(key_base_slot.0) + U256::from(1)).to_be_bytes());
        let mut slot1_bytes = [0u8; 32];
        slot1_bytes[16..32].copy_from_slice(&bls_key[32..48]);
        storage_reader.set_storage(contract_addr, slot1, U256::from_be_bytes(slot1_bytes));

        let block_reader = MockBlockReader::new();

        // First call should load from contract
        let result = provider.get_validators(1, &block_reader, &storage_reader);
        assert!(result.is_ok());

        // Should be cached now
        assert!(provider.is_cached(0).unwrap());
        assert_eq!(provider.cache_size().unwrap(), 1);
    }

    #[test]
    fn test_error_types() {
        let err = ValidatorProviderError::ContractReadError("test".to_string());
        assert!(err.to_string().contains("contract read error"));

        let err = ValidatorProviderError::EpochInfoError("test".to_string());
        assert!(err.to_string().contains("epoch info error"));

        let err = ValidatorProviderError::CacheError("test".to_string());
        assert!(err.to_string().contains("cache error"));

        let err = ValidatorProviderError::InvalidValidatorData("test".to_string());
        assert!(err.to_string().contains("invalid validator data"));

        let err = ValidatorProviderError::BlockReaderError("test".to_string());
        assert!(err.to_string().contains("block reader error"));
    }
}
