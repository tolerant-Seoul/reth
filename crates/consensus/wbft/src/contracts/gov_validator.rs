//! GovValidator Contract Interface
//!
//! This module provides an interface for interacting with the GovValidator
//! system contract, which manages the validator set on-chain.

use alloy_primitives::{keccak256, Address, B256, U256};
use thiserror::Error;

/// Storage slot for validator list array
/// keccak256("gov.validator.validators")
pub const SLOT_VALIDATOR_VALIDATORS: B256 = B256::new([
    0x9d, 0x8e, 0x5f, 0x40, 0x5e, 0x8e, 0x4b, 0x51, 0x8f, 0x54, 0x9f, 0x36, 0x0b, 0x56, 0x63, 0x85,
    0x29, 0x47, 0x8f, 0x0a, 0x33, 0x7e, 0x34, 0x75, 0x1c, 0x8e, 0x0f, 0x87, 0x3f, 0x3a, 0x9f, 0x2d,
]);

/// Storage slot for validator to BLS key mapping
/// keccak256("gov.validator.validatorToBlsKey")
pub const SLOT_VALIDATOR_TO_BLS_KEY: B256 = B256::new([
    0xa7, 0x12, 0x8e, 0x3b, 0x42, 0x5c, 0x91, 0x66, 0x7d, 0x38, 0x4a, 0x2f, 0x6e, 0x89, 0x54, 0x1c,
    0xb5, 0x73, 0x2a, 0x95, 0x41, 0x67, 0x8c, 0x3e, 0x29, 0x5d, 0x4b, 0x8f, 0x17, 0x63, 0xa2, 0x1e,
]);

/// Storage slot for gas tip
/// keccak256("gov.validator.gasTip")
pub const SLOT_VALIDATOR_GAS_TIP: B256 = B256::new([
    0x5e, 0x7a, 0x9c, 0x12, 0x35, 0xf8, 0x47, 0x6b, 0x89, 0x2d, 0x4e, 0xa1, 0x73, 0xc5, 0x68, 0x9f,
    0x24, 0x36, 0xb7, 0x5a, 0x8c, 0x91, 0x4d, 0x2e, 0x67, 0x3f, 0xa8, 0x1b, 0x54, 0xc9, 0x72, 0xe6,
]);

/// Errors that can occur when interacting with GovValidator contract
#[derive(Debug, Error)]
pub enum GovValidatorError {
    /// Failed to read storage from state provider
    #[error("failed to read storage: {0}")]
    StorageReadError(String),

    /// Invalid data format in storage
    #[error("invalid storage data format: {0}")]
    InvalidDataFormat(String),

    /// Validator not found
    #[error("validator not found: {0}")]
    ValidatorNotFound(Address),

    /// State provider error
    #[error("state provider error: {0}")]
    StateProviderError(String),

    /// Invalid BLS public key length
    #[error("invalid BLS public key length: expected 48 bytes, got {0}")]
    InvalidBlsKeyLength(usize),
}

/// Interface for the GovValidator system contract
///
/// The GovValidator contract manages the validator set on-chain,
/// storing validator addresses, their BLS public keys, and gas tip settings.
#[derive(Debug, Clone)]
pub struct GovValidator {
    /// Contract address
    address: Address,
}

impl GovValidator {
    /// Create a new GovValidator interface
    ///
    /// # Arguments
    ///
    /// * `address` - Address of the GovValidator contract
    pub fn new(address: Address) -> Self {
        Self { address }
    }

    /// Get the contract address
    pub fn address(&self) -> Address {
        self.address
    }

    /// Calculate storage slot for array element
    ///
    /// For Solidity arrays, element `i` is stored at `keccak256(slot) + i`
    fn array_element_slot(base_slot: B256, index: usize) -> B256 {
        let base_hash = keccak256(base_slot);
        let index_u256 = U256::from(index);
        let result = U256::from_be_bytes(base_hash.0) + index_u256;
        B256::from(result.to_be_bytes())
    }

    /// Calculate storage slot for mapping element
    ///
    /// For Solidity mappings, value for key `k` is stored at `keccak256(k . slot)`
    pub fn mapping_element_slot(base_slot: B256, key: Address) -> B256 {
        let mut data = [0u8; 64];
        data[12..32].copy_from_slice(key.as_slice());
        data[32..64].copy_from_slice(base_slot.as_slice());
        keccak256(data)
    }

    /// Read the number of validators from storage
    ///
    /// # Arguments
    ///
    /// * `storage_reader` - Function to read storage from state
    ///
    /// # Returns
    ///
    /// Number of validators in the validator set
    pub fn read_validator_count<F>(&self, storage_reader: F) -> Result<usize, GovValidatorError>
    where
        F: Fn(Address, B256) -> Result<U256, String>,
    {
        let value = storage_reader(self.address, SLOT_VALIDATOR_VALIDATORS)
            .map_err(GovValidatorError::StorageReadError)?;

        let count = value.to::<u64>() as usize;
        Ok(count)
    }

    /// Read all validator addresses from storage
    ///
    /// # Arguments
    ///
    /// * `storage_reader` - Function to read storage from state
    ///
    /// # Returns
    ///
    /// List of validator addresses
    pub fn read_validators<F>(&self, storage_reader: F) -> Result<Vec<Address>, GovValidatorError>
    where
        F: Fn(Address, B256) -> Result<U256, String>,
    {
        let count = self.read_validator_count(&storage_reader)?;

        let mut validators = Vec::with_capacity(count);

        for i in 0..count {
            let slot = Self::array_element_slot(SLOT_VALIDATOR_VALIDATORS, i);
            let value = storage_reader(self.address, slot)
                .map_err(GovValidatorError::StorageReadError)?;

            // Address is stored in the last 20 bytes of the storage value
            let bytes = value.to_be_bytes::<32>();
            let address = Address::from_slice(&bytes[12..32]);
            validators.push(address);
        }

        Ok(validators)
    }

    /// Read BLS public key for a validator
    ///
    /// # Arguments
    ///
    /// * `storage_reader` - Function to read storage from state
    /// * `validator` - Validator address
    ///
    /// # Returns
    ///
    /// BLS public key (48 bytes)
    pub fn read_bls_public_key<F>(
        &self,
        storage_reader: F,
        validator: Address,
    ) -> Result<[u8; 48], GovValidatorError>
    where
        F: Fn(Address, B256) -> Result<U256, String>,
    {
        let base_slot = Self::mapping_element_slot(SLOT_VALIDATOR_TO_BLS_KEY, validator);

        // BLS public key is 48 bytes, stored in 2 storage slots
        // Slot 0: bytes 0-31
        // Slot 1: bytes 32-47 (in lower 16 bytes)

        let slot0_value = storage_reader(self.address, base_slot)
            .map_err(GovValidatorError::StorageReadError)?;

        let slot1_index = U256::from_be_bytes(base_slot.0) + U256::from(1);
        let slot1 = B256::from(slot1_index.to_be_bytes());
        let slot1_value = storage_reader(self.address, slot1)
            .map_err(GovValidatorError::StorageReadError)?;

        let mut bls_key = [0u8; 48];
        bls_key[0..32].copy_from_slice(&slot0_value.to_be_bytes::<32>());
        bls_key[32..48].copy_from_slice(&slot1_value.to_be_bytes::<32>()[16..32]);

        Ok(bls_key)
    }

    /// Read all BLS public keys for validators
    ///
    /// # Arguments
    ///
    /// * `storage_reader` - Function to read storage from state
    /// * `validators` - List of validator addresses
    ///
    /// # Returns
    ///
    /// List of BLS public keys in the same order as validators
    pub fn read_all_bls_keys<F>(
        &self,
        storage_reader: F,
        validators: &[Address],
    ) -> Result<Vec<[u8; 48]>, GovValidatorError>
    where
        F: Fn(Address, B256) -> Result<U256, String>,
    {
        let mut keys = Vec::with_capacity(validators.len());

        for validator in validators {
            let key = self.read_bls_public_key(&storage_reader, *validator)?;
            keys.push(key);
        }

        Ok(keys)
    }

    /// Read gas tip value from storage
    ///
    /// # Arguments
    ///
    /// * `storage_reader` - Function to read storage from state
    ///
    /// # Returns
    ///
    /// Gas tip value (U256)
    pub fn read_gas_tip<F>(&self, storage_reader: F) -> Result<U256, GovValidatorError>
    where
        F: Fn(Address, B256) -> Result<U256, String>,
    {
        storage_reader(self.address, SLOT_VALIDATOR_GAS_TIP)
            .map_err(GovValidatorError::StorageReadError)
    }

    /// Check if an address is a validator
    ///
    /// # Arguments
    ///
    /// * `storage_reader` - Function to read storage from state
    /// * `address` - Address to check
    ///
    /// # Returns
    ///
    /// True if the address is in the validator set
    pub fn is_validator<F>(
        &self,
        storage_reader: F,
        address: Address,
    ) -> Result<bool, GovValidatorError>
    where
        F: Fn(Address, B256) -> Result<U256, String>,
    {
        let validators = self.read_validators(storage_reader)?;
        Ok(validators.contains(&address))
    }
}

impl Default for GovValidator {
    fn default() -> Self {
        // Default GovValidator contract address
        Self::new(Address::from([0x01; 20]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Helper to create a mock storage reader
    fn create_mock_storage(data: HashMap<(Address, B256), U256>) -> impl Fn(Address, B256) -> Result<U256, String> {
        move |addr, slot| {
            data.get(&(addr, slot))
                .copied()
                .ok_or_else(|| format!("Storage not found: {:?} {:?}", addr, slot))
        }
    }

    #[test]
    fn test_gov_validator_creation() {
        let address = Address::from([0x42; 20]);
        let gov = GovValidator::new(address);

        assert_eq!(gov.address(), address);
    }

    #[test]
    fn test_gov_validator_default() {
        let gov = GovValidator::default();
        assert_eq!(gov.address(), Address::from([0x01; 20]));
    }

    #[test]
    fn test_array_element_slot() {
        let base_slot = B256::ZERO;

        // First element
        let slot0 = GovValidator::array_element_slot(base_slot, 0);
        let expected0 = keccak256(base_slot);
        assert_eq!(slot0, expected0);

        // Second element should be base + 1
        let slot1 = GovValidator::array_element_slot(base_slot, 1);
        let expected1 = U256::from_be_bytes(expected0.0) + U256::from(1);
        assert_eq!(slot1, B256::from(expected1.to_be_bytes()));
    }

    #[test]
    fn test_mapping_element_slot() {
        let base_slot = B256::ZERO;
        let key = Address::from([0x42; 20]);

        let slot = GovValidator::mapping_element_slot(base_slot, key);

        // Verify it's a valid B256
        assert_ne!(slot, B256::ZERO);
    }

    #[test]
    fn test_read_validator_count() {
        let contract_addr = Address::from([0x01; 20]);
        let gov = GovValidator::new(contract_addr);

        let mut storage = HashMap::new();
        storage.insert(
            (contract_addr, SLOT_VALIDATOR_VALIDATORS),
            U256::from(4),
        );

        let reader = create_mock_storage(storage);
        let count = gov.read_validator_count(reader).unwrap();

        assert_eq!(count, 4);
    }

    #[test]
    fn test_read_validator_count_empty() {
        let contract_addr = Address::from([0x01; 20]);
        let gov = GovValidator::new(contract_addr);

        let mut storage = HashMap::new();
        storage.insert(
            (contract_addr, SLOT_VALIDATOR_VALIDATORS),
            U256::ZERO,
        );

        let reader = create_mock_storage(storage);
        let count = gov.read_validator_count(reader).unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn test_read_validators() {
        let contract_addr = Address::from([0x01; 20]);
        let gov = GovValidator::new(contract_addr);

        let validator1 = Address::from([0x11; 20]);
        let validator2 = Address::from([0x22; 20]);

        let mut storage = HashMap::new();

        // Set validator count
        storage.insert(
            (contract_addr, SLOT_VALIDATOR_VALIDATORS),
            U256::from(2),
        );

        // Set validator addresses
        let base_hash = keccak256(SLOT_VALIDATOR_VALIDATORS);

        // Validator 1 at index 0
        let mut bytes1 = [0u8; 32];
        bytes1[12..32].copy_from_slice(validator1.as_slice());
        storage.insert(
            (contract_addr, base_hash),
            U256::from_be_bytes(bytes1),
        );

        // Validator 2 at index 1
        let slot1 = B256::from((U256::from_be_bytes(base_hash.0) + U256::from(1)).to_be_bytes());
        let mut bytes2 = [0u8; 32];
        bytes2[12..32].copy_from_slice(validator2.as_slice());
        storage.insert(
            (contract_addr, slot1),
            U256::from_be_bytes(bytes2),
        );

        let reader = create_mock_storage(storage);
        let validators = gov.read_validators(reader).unwrap();

        assert_eq!(validators.len(), 2);
        assert_eq!(validators[0], validator1);
        assert_eq!(validators[1], validator2);
    }

    #[test]
    fn test_read_gas_tip() {
        let contract_addr = Address::from([0x01; 20]);
        let gov = GovValidator::new(contract_addr);

        let expected_tip = U256::from(1_000_000_000u64); // 1 gwei

        let mut storage = HashMap::new();
        storage.insert(
            (contract_addr, SLOT_VALIDATOR_GAS_TIP),
            expected_tip,
        );

        let reader = create_mock_storage(storage);
        let gas_tip = gov.read_gas_tip(reader).unwrap();

        assert_eq!(gas_tip, expected_tip);
    }

    #[test]
    fn test_is_validator() {
        let contract_addr = Address::from([0x01; 20]);
        let gov = GovValidator::new(contract_addr);

        let validator1 = Address::from([0x11; 20]);
        let non_validator = Address::from([0x99; 20]);

        let mut storage = HashMap::new();

        // Set validator count
        storage.insert(
            (contract_addr, SLOT_VALIDATOR_VALIDATORS),
            U256::from(1),
        );

        // Set validator address
        let base_hash = keccak256(SLOT_VALIDATOR_VALIDATORS);
        let mut bytes = [0u8; 32];
        bytes[12..32].copy_from_slice(validator1.as_slice());
        storage.insert(
            (contract_addr, base_hash),
            U256::from_be_bytes(bytes),
        );

        let reader = create_mock_storage(storage);

        assert!(gov.is_validator(&reader, validator1).unwrap());
        assert!(!gov.is_validator(&reader, non_validator).unwrap());
    }

    #[test]
    fn test_read_bls_public_key() {
        let contract_addr = Address::from([0x01; 20]);
        let gov = GovValidator::new(contract_addr);

        let validator = Address::from([0x11; 20]);

        // Create a test BLS public key (48 bytes)
        let mut expected_key = [0u8; 48];
        for i in 0..48 {
            expected_key[i] = i as u8;
        }

        let mut storage = HashMap::new();

        // Calculate mapping slot
        let base_slot = GovValidator::mapping_element_slot(SLOT_VALIDATOR_TO_BLS_KEY, validator);

        // Store first 32 bytes
        let mut slot0_bytes = [0u8; 32];
        slot0_bytes.copy_from_slice(&expected_key[0..32]);
        storage.insert(
            (contract_addr, base_slot),
            U256::from_be_bytes(slot0_bytes),
        );

        // Store remaining 16 bytes (padded to 32)
        let slot1 = B256::from((U256::from_be_bytes(base_slot.0) + U256::from(1)).to_be_bytes());
        let mut slot1_bytes = [0u8; 32];
        slot1_bytes[16..32].copy_from_slice(&expected_key[32..48]);
        storage.insert(
            (contract_addr, slot1),
            U256::from_be_bytes(slot1_bytes),
        );

        let reader = create_mock_storage(storage);
        let bls_key = gov.read_bls_public_key(reader, validator).unwrap();

        assert_eq!(bls_key, expected_key);
    }

    #[test]
    fn test_read_all_bls_keys() {
        let contract_addr = Address::from([0x01; 20]);
        let gov = GovValidator::new(contract_addr);

        let validator1 = Address::from([0x11; 20]);
        let validator2 = Address::from([0x22; 20]);

        // Create test BLS public keys
        let key1 = [0x11u8; 48];
        let key2 = [0x22u8; 48];

        let mut storage = HashMap::new();

        // Setup storage for validator 1
        let base_slot1 = GovValidator::mapping_element_slot(SLOT_VALIDATOR_TO_BLS_KEY, validator1);
        let mut slot0_bytes1 = [0u8; 32];
        slot0_bytes1.copy_from_slice(&key1[0..32]);
        storage.insert((contract_addr, base_slot1), U256::from_be_bytes(slot0_bytes1));

        let slot1_1 = B256::from((U256::from_be_bytes(base_slot1.0) + U256::from(1)).to_be_bytes());
        let mut slot1_bytes1 = [0u8; 32];
        slot1_bytes1[16..32].copy_from_slice(&key1[32..48]);
        storage.insert((contract_addr, slot1_1), U256::from_be_bytes(slot1_bytes1));

        // Setup storage for validator 2
        let base_slot2 = GovValidator::mapping_element_slot(SLOT_VALIDATOR_TO_BLS_KEY, validator2);
        let mut slot0_bytes2 = [0u8; 32];
        slot0_bytes2.copy_from_slice(&key2[0..32]);
        storage.insert((contract_addr, base_slot2), U256::from_be_bytes(slot0_bytes2));

        let slot1_2 = B256::from((U256::from_be_bytes(base_slot2.0) + U256::from(1)).to_be_bytes());
        let mut slot1_bytes2 = [0u8; 32];
        slot1_bytes2[16..32].copy_from_slice(&key2[32..48]);
        storage.insert((contract_addr, slot1_2), U256::from_be_bytes(slot1_bytes2));

        let reader = create_mock_storage(storage);
        let validators = vec![validator1, validator2];
        let keys = gov.read_all_bls_keys(reader, &validators).unwrap();

        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0], key1);
        assert_eq!(keys[1], key2);
    }

    #[test]
    fn test_storage_read_error() {
        let contract_addr = Address::from([0x01; 20]);
        let gov = GovValidator::new(contract_addr);

        let storage = HashMap::new(); // Empty storage
        let reader = create_mock_storage(storage);

        let result = gov.read_validator_count(reader);
        assert!(result.is_err());

        if let Err(GovValidatorError::StorageReadError(msg)) = result {
            assert!(msg.contains("Storage not found"));
        } else {
            panic!("Expected StorageReadError");
        }
    }

    #[test]
    fn test_constants_are_unique() {
        // Ensure all storage slot constants are unique
        assert_ne!(SLOT_VALIDATOR_VALIDATORS, SLOT_VALIDATOR_TO_BLS_KEY);
        assert_ne!(SLOT_VALIDATOR_VALIDATORS, SLOT_VALIDATOR_GAS_TIP);
        assert_ne!(SLOT_VALIDATOR_TO_BLS_KEY, SLOT_VALIDATOR_GAS_TIP);
    }

    #[test]
    fn test_gov_validator_clone() {
        let gov = GovValidator::new(Address::from([0x42; 20]));
        let cloned = gov.clone();

        assert_eq!(gov.address(), cloned.address());
    }

    #[test]
    fn test_error_display() {
        let err = GovValidatorError::StorageReadError("test error".to_string());
        assert!(err.to_string().contains("test error"));

        let err = GovValidatorError::ValidatorNotFound(Address::from([0x42; 20]));
        assert!(err.to_string().contains("validator not found"));

        let err = GovValidatorError::InvalidBlsKeyLength(32);
        assert!(err.to_string().contains("expected 48 bytes"));
        assert!(err.to_string().contains("32"));
    }
}
