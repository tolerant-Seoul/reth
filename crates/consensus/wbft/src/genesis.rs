//! WBFT Genesis initialization
//!
//! This module provides functions to create WBFT-specific extra data
//! for genesis blocks, compatible with go-stablenet's CreateInitialExtraData.

use crate::header::{Candidate, EpochInfo, WbftExtra};
use alloy_primitives::{hex, Bytes, U256};
use reth_chainspec::{WbftChainConfig, WbftInit};

/// Error type for genesis initialization
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenesisError {
    /// Configuration validation failed
    ConfigValidation(String),
    /// Invalid BLS public key format
    InvalidBlsKey { index: usize, reason: String },
    /// Invalid gas tip value
    InvalidGasTip(String),
}

impl core::fmt::Display for GenesisError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ConfigValidation(msg) => write!(f, "Configuration validation failed: {}", msg),
            Self::InvalidBlsKey { index, reason } => {
                write!(f, "Invalid BLS key at index {}: {}", index, reason)
            }
            Self::InvalidGasTip(msg) => write!(f, "Invalid gas tip: {}", msg),
        }
    }
}

impl std::error::Error for GenesisError {}

/// Create initial extra data for WBFT genesis block
///
/// This function generates the extra data that should be placed in the
/// genesis block header for a WBFT network. It corresponds to go-stablenet's
/// `CreateInitialExtraData` function.
///
/// # Arguments
///
/// * `config` - WBFT chain configuration containing initial validators and settings
///
/// # Returns
///
/// RLP-encoded extra data bytes suitable for genesis block header
///
/// # Errors
///
/// Returns `GenesisError` if:
/// - Configuration validation fails
/// - BLS key format is invalid
/// - Gas tip parsing fails
///
/// # Examples
///
/// ```ignore
/// use reth_chainspec::WbftChainConfig;
/// use reth_consensus_wbft::genesis::create_initial_extra_data;
///
/// let config = WbftChainConfig { /* ... */ };
/// let extra_data = create_initial_extra_data(&config)?;
/// ```
pub fn create_initial_extra_data(config: &WbftChainConfig) -> Result<Bytes, GenesisError> {
    // 1. Validate configuration
    config
        .validate()
        .map_err(GenesisError::ConfigValidation)?;

    // 2. Create epoch info from initial configuration
    let epoch_info = create_initial_epoch_info(&config.init)?;

    // 3. Parse gas tip from system contract params
    let gas_tip = parse_gas_tip(config)?;

    // 4. Create WBFTExtra structure for genesis block
    let extra = WbftExtra {
        vanity_data: [0u8; 32],
        randao_reveal: Vec::new(),
        prev_round: 0,
        prev_prepared_seal: None,
        prev_committed_seal: None,
        round: 0,
        prepared_seal: None,
        committed_seal: None,
        gas_tip,
        epoch_info: Some(epoch_info),
    };

    // 5. RLP encode the extra data
    let encoded = extra.encode();
    Ok(Bytes::from(encoded))
}

/// Create initial epoch info from WBFT init configuration
///
/// # Arguments
///
/// * `init` - Initial WBFT configuration with validators and BLS keys
///
/// # Returns
///
/// EpochInfo structure suitable for genesis block
///
/// # Errors
///
/// Returns error if BLS key decoding fails
fn create_initial_epoch_info(init: &WbftInit) -> Result<EpochInfo, GenesisError> {
    let mut candidates = Vec::with_capacity(init.validators.len());
    let mut validators = Vec::with_capacity(init.validators.len());
    let mut bls_public_keys = Vec::with_capacity(init.bls_public_keys.len());

    // Process each validator
    for (i, addr) in init.validators.iter().enumerate() {
        // Add as candidate with default diligence (95%)
        candidates.push(Candidate {
            addr: *addr,
            diligence: 950_000, // 95% in units of 10^-6
        });

        // Add validator index
        validators.push(i as u32);
    }

    // Decode BLS public keys
    for (i, key_hex) in init.bls_public_keys.iter().enumerate() {
        let key_bytes = hex::decode(key_hex.trim_start_matches("0x")).map_err(|e| {
            GenesisError::InvalidBlsKey {
                index: i,
                reason: format!("hex decode failed: {}", e),
            }
        })?;

        // Validate key length (48 bytes for compressed BLS12-381 public key)
        if key_bytes.len() != 48 {
            return Err(GenesisError::InvalidBlsKey {
                index: i,
                reason: format!("expected 48 bytes, got {}", key_bytes.len()),
            });
        }

        bls_public_keys.push(key_bytes);
    }

    Ok(EpochInfo {
        candidates,
        validators,
        bls_public_keys,
    })
}

/// Parse gas tip from system contract parameters
///
/// # Arguments
///
/// * `config` - WBFT chain configuration
///
/// # Returns
///
/// Gas tip as U256
fn parse_gas_tip(config: &WbftChainConfig) -> Result<U256, GenesisError> {
    // Try to get gas tip from GovValidator params
    let gas_tip_str = config
        .system_contracts
        .gov_validator
        .params
        .get("gasTip")
        .map(|s| s.as_str())
        .unwrap_or("1000000000"); // Default: 1 Gwei

    // Parse as u64 first, then convert to U256
    let gas_tip_value: u64 = gas_tip_str.parse().map_err(|e| {
        GenesisError::InvalidGasTip(format!("failed to parse '{}': {}", gas_tip_str, e))
    })?;

    Ok(U256::from(gas_tip_value))
}

/// Validate genesis block extra data
///
/// Checks that the extra data contains valid WBFT genesis information.
///
/// # Arguments
///
/// * `extra_data` - Raw extra data bytes from genesis block
///
/// # Returns
///
/// Ok(()) if valid, Err with description if invalid
///
/// # Errors
///
/// Returns error if:
/// - Extra data cannot be decoded
/// - Epoch info is missing
/// - Validator count is zero
pub fn validate_genesis_extra_data(extra_data: &[u8]) -> Result<(), GenesisError> {
    // Decode extra data
    let extra = WbftExtra::decode(extra_data).map_err(|e| {
        GenesisError::ConfigValidation(format!("failed to decode extra data: {}", e))
    })?;

    // Genesis must have epoch info
    let epoch_info = extra.epoch_info.ok_or_else(|| {
        GenesisError::ConfigValidation("genesis block must contain epoch info".to_string())
    })?;

    // Must have at least one validator
    if epoch_info.validators.is_empty() {
        return Err(GenesisError::ConfigValidation(
            "genesis must have at least one validator".to_string(),
        ));
    }

    // Validator count must match BLS key count
    if epoch_info.validators.len() != epoch_info.bls_public_keys.len() {
        return Err(GenesisError::ConfigValidation(format!(
            "validator count ({}) doesn't match BLS key count ({})",
            epoch_info.validators.len(),
            epoch_info.bls_public_keys.len()
        )));
    }

    // Validators must reference valid candidates
    for (i, &validator_idx) in epoch_info.validators.iter().enumerate() {
        if validator_idx as usize >= epoch_info.candidates.len() {
            return Err(GenesisError::ConfigValidation(format!(
                "validator index {} at position {} exceeds candidate count {}",
                validator_idx,
                i,
                epoch_info.candidates.len()
            )));
        }
    }

    // Genesis should not have seals (not yet signed)
    if extra.prepared_seal.is_some() || extra.committed_seal.is_some() {
        return Err(GenesisError::ConfigValidation(
            "genesis block should not have seals".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap as HashMap;
    use alloy_primitives::Address;
    use reth_chainspec::{SystemContract, SystemContracts, WbftConfig};

    fn create_test_config() -> WbftChainConfig {
        WbftChainConfig {
            wbft: WbftConfig {
                request_timeout_seconds: 2,
                block_period_seconds: 1,
                proposer_policy: 0,
                epoch_length: 10,
                max_request_timeout_seconds: None,
            },
            init: WbftInit {
                validators: vec![
                    Address::from([0x01; 20]),
                    Address::from([0x02; 20]),
                    Address::from([0x03; 20]),
                ],
                bls_public_keys: vec![
                    "0xaec493af8fa358a1c6f05499f2dd712721ade88c477d21b799d38e9b84582b6fbe4f4adc21e1e454bc37522eb3478b9b".to_string(),
                    "0xb1ae18fdcbcc6a80d7a0c4cfec1a04bc1bee78e519eaadd689108077d946e0849a2c30ac96462be32023f34ca67ebcf6".to_string(),
                    "0xc2d493af8fa358a1c6f05499f2dd712721ade88c477d21b799d38e9b84582b6fbe4f4adc21e1e454bc37522eb3478b9c".to_string(),
                ],
            },
            system_contracts: SystemContracts {
                gov_validator: SystemContract {
                    address: Address::from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00]),
                    version: Some("v1".to_string()),
                    params: {
                        let mut map = HashMap::new();
                        map.insert("gasTip".to_string(), "1000000000".to_string());
                        map
                    },
                },
                native_coin_adapter: None,
                gov_minter: None,
                gov_master_minter: None,
                gov_council: None,
            },
        }
    }

    #[test]
    fn test_create_initial_extra_data_success() {
        let config = create_test_config();
        let result = create_initial_extra_data(&config);

        assert!(result.is_ok(), "Expected success but got: {:?}", result.err());

        let extra_data = result.unwrap();
        assert!(!extra_data.is_empty());

        // Verify it can be decoded back
        let decoded = WbftExtra::decode(&extra_data);
        assert!(decoded.is_ok());

        let extra = decoded.unwrap();
        assert!(extra.epoch_info.is_some());

        let epoch_info = extra.epoch_info.unwrap();
        assert_eq!(epoch_info.candidates.len(), 3);
        assert_eq!(epoch_info.validators.len(), 3);
        assert_eq!(epoch_info.bls_public_keys.len(), 3);
    }

    #[test]
    fn test_create_initial_extra_data_gas_tip() {
        let config = create_test_config();
        let extra_data = create_initial_extra_data(&config).unwrap();

        let decoded = WbftExtra::decode(&extra_data).unwrap();
        assert_eq!(decoded.gas_tip, U256::from(1_000_000_000u64));
    }

    #[test]
    fn test_create_initial_extra_data_default_gas_tip() {
        let mut config = create_test_config();
        config.system_contracts.gov_validator.params.clear(); // Remove gasTip

        let extra_data = create_initial_extra_data(&config).unwrap();
        let decoded = WbftExtra::decode(&extra_data).unwrap();

        // Should use default 1 Gwei
        assert_eq!(decoded.gas_tip, U256::from(1_000_000_000u64));
    }

    #[test]
    fn test_create_initial_extra_data_custom_gas_tip() {
        let mut config = create_test_config();
        config.system_contracts.gov_validator.params.insert(
            "gasTip".to_string(),
            "5000000000".to_string(), // 5 Gwei
        );

        let extra_data = create_initial_extra_data(&config).unwrap();
        let decoded = WbftExtra::decode(&extra_data).unwrap();

        assert_eq!(decoded.gas_tip, U256::from(5_000_000_000u64));
    }

    #[test]
    fn test_create_initial_extra_data_genesis_structure() {
        let config = create_test_config();
        let extra_data = create_initial_extra_data(&config).unwrap();
        let extra = WbftExtra::decode(&extra_data).unwrap();

        // Genesis block should have:
        // - No seals (not yet signed)
        // - Round 0
        // - Epoch info present
        assert!(extra.prepared_seal.is_none());
        assert!(extra.committed_seal.is_none());
        assert_eq!(extra.round, 0);
        assert_eq!(extra.prev_round, 0);
        assert!(extra.epoch_info.is_some());
    }

    #[test]
    fn test_create_initial_epoch_info_diligence() {
        let config = create_test_config();
        let extra_data = create_initial_extra_data(&config).unwrap();
        let extra = WbftExtra::decode(&extra_data).unwrap();

        let epoch_info = extra.epoch_info.unwrap();

        // All candidates should have 95% diligence
        for candidate in &epoch_info.candidates {
            assert_eq!(candidate.diligence, 950_000);
        }
    }

    #[test]
    fn test_create_initial_epoch_info_validator_indices() {
        let config = create_test_config();
        let extra_data = create_initial_extra_data(&config).unwrap();
        let extra = WbftExtra::decode(&extra_data).unwrap();

        let epoch_info = extra.epoch_info.unwrap();

        // Validator indices should be sequential
        assert_eq!(epoch_info.validators, vec![0, 1, 2]);
    }

    #[test]
    fn test_create_initial_extra_data_invalid_config() {
        let mut config = create_test_config();
        config.init.validators.pop(); // Mismatch validator/key count

        let result = create_initial_extra_data(&config);
        assert!(result.is_err());

        match result.unwrap_err() {
            GenesisError::ConfigValidation(_) => {}
            e => panic!("Expected ConfigValidation error, got: {:?}", e),
        }
    }

    #[test]
    fn test_create_initial_extra_data_invalid_bls_key() {
        let mut config = create_test_config();
        config.init.bls_public_keys[0] = "0xINVALIDHEX".to_string();

        // This will fail at validate() in WbftChainConfig
        let result = create_initial_extra_data(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_initial_extra_data_invalid_gas_tip() {
        let mut config = create_test_config();
        config.system_contracts.gov_validator.params.insert(
            "gasTip".to_string(),
            "not_a_number".to_string(),
        );

        let result = create_initial_extra_data(&config);
        assert!(result.is_err());

        match result.unwrap_err() {
            GenesisError::InvalidGasTip(_) => {}
            e => panic!("Expected InvalidGasTip error, got: {:?}", e),
        }
    }

    #[test]
    fn test_validate_genesis_extra_data_success() {
        let config = create_test_config();
        let extra_data = create_initial_extra_data(&config).unwrap();

        let result = validate_genesis_extra_data(&extra_data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_genesis_extra_data_missing_epoch_info() {
        let extra = WbftExtra::new(); // No epoch info
        let encoded = extra.encode();

        let result = validate_genesis_extra_data(&encoded);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("epoch info"));
    }

    #[test]
    fn test_validate_genesis_extra_data_empty_validators() {
        let extra = WbftExtra {
            vanity_data: [0u8; 32],
            randao_reveal: Vec::new(),
            prev_round: 0,
            prev_prepared_seal: None,
            prev_committed_seal: None,
            round: 0,
            prepared_seal: None,
            committed_seal: None,
            gas_tip: U256::ZERO,
            epoch_info: Some(EpochInfo {
                candidates: vec![],
                validators: vec![],
                bls_public_keys: vec![],
            }),
        };
        let encoded = extra.encode();

        let result = validate_genesis_extra_data(&encoded);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least one validator"));
    }

    #[test]
    fn test_validate_genesis_extra_data_validator_bls_mismatch() {
        let extra = WbftExtra {
            vanity_data: [0u8; 32],
            randao_reveal: Vec::new(),
            prev_round: 0,
            prev_prepared_seal: None,
            prev_committed_seal: None,
            round: 0,
            prepared_seal: None,
            committed_seal: None,
            gas_tip: U256::ZERO,
            epoch_info: Some(EpochInfo {
                candidates: vec![Candidate {
                    addr: Address::from([0x01; 20]),
                    diligence: 950_000,
                }],
                validators: vec![0],
                bls_public_keys: vec![], // Mismatch
            }),
        };
        let encoded = extra.encode();

        let result = validate_genesis_extra_data(&encoded);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("doesn't match"));
    }

    #[test]
    fn test_validate_genesis_extra_data_invalid_validator_index() {
        let extra = WbftExtra {
            vanity_data: [0u8; 32],
            randao_reveal: Vec::new(),
            prev_round: 0,
            prev_prepared_seal: None,
            prev_committed_seal: None,
            round: 0,
            prepared_seal: None,
            committed_seal: None,
            gas_tip: U256::ZERO,
            epoch_info: Some(EpochInfo {
                candidates: vec![Candidate {
                    addr: Address::from([0x01; 20]),
                    diligence: 950_000,
                }],
                validators: vec![5], // Invalid index
                bls_public_keys: vec![vec![0x42; 48]],
            }),
        };
        let encoded = extra.encode();

        let result = validate_genesis_extra_data(&encoded);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds candidate count"));
    }

    #[test]
    fn test_genesis_error_display() {
        let err = GenesisError::ConfigValidation("test error".to_string());
        assert!(err.to_string().contains("Configuration validation failed"));

        let err = GenesisError::InvalidBlsKey {
            index: 1,
            reason: "bad key".to_string(),
        };
        assert!(err.to_string().contains("Invalid BLS key at index 1"));

        let err = GenesisError::InvalidGasTip("parse error".to_string());
        assert!(err.to_string().contains("Invalid gas tip"));
    }

    #[test]
    fn test_single_validator_genesis() {
        let config = WbftChainConfig {
            wbft: WbftConfig {
                request_timeout_seconds: 2,
                block_period_seconds: 1,
                proposer_policy: 0,
                epoch_length: 10,
                max_request_timeout_seconds: None,
            },
            init: WbftInit {
                validators: vec![Address::from([0x01; 20])],
                bls_public_keys: vec![
                    "0xaec493af8fa358a1c6f05499f2dd712721ade88c477d21b799d38e9b84582b6fbe4f4adc21e1e454bc37522eb3478b9b".to_string(),
                ],
            },
            system_contracts: SystemContracts {
                gov_validator: SystemContract {
                    address: Address::from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00]),
                    version: None,
                    params: HashMap::new(),
                },
                native_coin_adapter: None,
                gov_minter: None,
                gov_master_minter: None,
                gov_council: None,
            },
        };

        let result = create_initial_extra_data(&config);
        assert!(result.is_ok());

        let extra_data = result.unwrap();
        let decoded = WbftExtra::decode(&extra_data).unwrap();
        let epoch_info = decoded.epoch_info.unwrap();

        assert_eq!(epoch_info.validators.len(), 1);
        assert_eq!(epoch_info.candidates.len(), 1);
    }

    #[test]
    fn test_extra_data_roundtrip() {
        let config = create_test_config();
        let extra_data = create_initial_extra_data(&config).unwrap();

        // Encode and decode multiple times
        let decoded1 = WbftExtra::decode(&extra_data).unwrap();
        let encoded1 = decoded1.encode();
        let decoded2 = WbftExtra::decode(&encoded1).unwrap();

        assert_eq!(decoded1, decoded2);
    }
}
