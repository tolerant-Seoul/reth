//! WBFT consensus configuration types
//!
//! This module defines the configuration structures for WBFT (Byzantine Fault Tolerant)
//! consensus, corresponding to go-stablenet's AnzeonConfig.

use alloc::collections::BTreeMap as HashMap;
use alloc::{string::String, vec::Vec};
use alloy_primitives::{hex, Address, U256};
use serde::{Deserialize, Serialize};

/// WBFT chain-level configuration (equivalent to go-stablenet's AnzeonConfig)
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WbftChainConfig {
    /// WBFT consensus parameters
    pub wbft: WbftConfig,

    /// Initial validator set configuration
    pub init: WbftInit,

    /// System contracts configuration
    pub system_contracts: SystemContracts,
}

/// WBFT consensus configuration (equivalent to go-stablenet's WBFTConfig)
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WbftConfig {
    /// Request timeout in seconds
    #[serde(default = "default_request_timeout")]
    pub request_timeout_seconds: u64,

    /// Block period in seconds
    #[serde(default = "default_block_period")]
    pub block_period_seconds: u64,

    /// Proposer selection policy (0 = RoundRobin, 1 = Sticky)
    #[serde(default)]
    pub proposer_policy: u64,

    /// Epoch length in blocks
    #[serde(default = "default_epoch_length")]
    pub epoch_length: u64,

    /// Maximum request timeout in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_request_timeout_seconds: Option<u64>,
}

fn default_request_timeout() -> u64 {
    2
}

fn default_block_period() -> u64 {
    1
}

fn default_epoch_length() -> u64 {
    10
}

impl Default for WbftConfig {
    fn default() -> Self {
        Self {
            request_timeout_seconds: default_request_timeout(),
            block_period_seconds: default_block_period(),
            proposer_policy: 0,
            epoch_length: default_epoch_length(),
            max_request_timeout_seconds: None,
        }
    }
}

/// Initial WBFT validator set
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WbftInit {
    /// Initial validator addresses (order matters!)
    pub validators: Vec<Address>,

    /// BLS public keys in hex format (must match validators order)
    #[serde(rename = "blsPublicKeys")]
    pub bls_public_keys: Vec<String>, // "0x..." format
}

/// System contracts configuration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemContracts {
    /// GovValidator contract
    pub gov_validator: SystemContract,

    /// Optional contracts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_coin_adapter: Option<SystemContract>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gov_minter: Option<SystemContract>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gov_master_minter: Option<SystemContract>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gov_council: Option<SystemContract>,
}

/// Individual system contract configuration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemContract {
    /// Contract address
    pub address: Address,

    /// Contract version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Additional parameters (e.g., gasTip)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, String>,
}

/// Block-based WBFT configuration transitions
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WbftTransition {
    /// Block number where transition occurs
    pub block: U256,

    /// New request timeout (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_timeout_seconds: Option<u64>,

    /// New block period (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_period_seconds: Option<u64>,

    /// New epoch length (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epoch_length: Option<u64>,

    /// New proposer policy (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposer_policy: Option<u64>,

    /// New max timeout (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_request_timeout_seconds: Option<u64>,
}

/// System contract upgrade specification
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemContractUpgrade {
    /// Block number where upgrade occurs
    pub block: U256,

    /// New contract addresses (only specified ones are updated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gov_validator: Option<Address>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_coin_adapter: Option<Address>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gov_minter: Option<Address>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gov_master_minter: Option<Address>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gov_council: Option<Address>,
}

impl WbftChainConfig {
    /// Get active WBFT config for given block number
    ///
    /// TODO: Apply transitions based on block number
    pub fn get_config(&self, _block_number: u64) -> WbftConfig {
        self.wbft.clone()
    }

    /// Validate configuration
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Validator count doesn't match BLS key count
    /// - No validators specified
    /// - Invalid BLS key format
    /// - Invalid epoch length
    pub fn validate(&self) -> Result<(), String> {
        // 1. Check validator count matches BLS key count
        if self.init.validators.len() != self.init.bls_public_keys.len() {
            return Err(format!(
                "Validator count ({}) doesn't match BLS key count ({})",
                self.init.validators.len(),
                self.init.bls_public_keys.len()
            ));
        }

        // 2. Check minimum validator count (at least 1)
        if self.init.validators.is_empty() {
            return Err("At least one validator required".to_string());
        }

        // 3. Validate BLS public key format
        for (i, key) in self.init.bls_public_keys.iter().enumerate() {
            if !key.starts_with("0x") {
                return Err(format!("BLS key {} must start with 0x", i));
            }

            // BLS public key is 48 bytes = 96 hex chars + "0x"
            if key.len() != 98 {
                return Err(format!(
                    "BLS key {} has invalid length {} (expected 98)",
                    i,
                    key.len()
                ));
            }

            // Verify it's valid hex
            if hex::decode(key.trim_start_matches("0x")).is_err() {
                return Err(format!("BLS key {} is not valid hex", i));
            }
        }

        // 4. Check epoch length minimum
        if self.wbft.epoch_length == 0 {
            return Err("Epoch length must be > 0".to_string());
        }

        Ok(())
    }

    /// Get initial BLS public keys as bytes
    ///
    /// # Errors
    ///
    /// Returns error if BLS key hex decoding fails
    pub fn initial_bls_public_keys(&self) -> Result<Vec<Vec<u8>>, String> {
        self.init
            .bls_public_keys
            .iter()
            .map(|s| {
                hex::decode(s.trim_start_matches("0x"))
                    .map_err(|e| format!("Invalid BLS key hex: {}", e))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn create_valid_config() -> WbftChainConfig {
        WbftChainConfig {
            wbft: WbftConfig::default(),
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
    fn test_valid_config() {
        let config = create_valid_config();
        match config.validate() {
            Ok(()) => (),
            Err(e) => panic!("Validation failed: {}", e),
        }
    }

    #[test]
    fn test_validator_bls_mismatch() {
        let mut config = create_valid_config();
        config.init.bls_public_keys.pop(); // Remove one BLS key

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("doesn't match"));
    }

    #[test]
    fn test_empty_validators() {
        let mut config = create_valid_config();
        config.init.validators.clear();
        config.init.bls_public_keys.clear();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("At least one validator"));
    }

    #[test]
    fn test_invalid_bls_format_no_prefix() {
        let mut config = create_valid_config();
        config.init.bls_public_keys[0] = "aec493af8fa358a1c6f05499f2dd712721ade88c477d21b799d38e9b84582b6fbe4f4adc21e1e454bc37522eb3478b9b".to_string();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must start with 0x"));
    }

    #[test]
    fn test_invalid_bls_format_wrong_length() {
        let mut config = create_valid_config();
        config.init.bls_public_keys[0] = "0xaabbcc".to_string(); // Too short

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid length"));
    }

    #[test]
    fn test_invalid_bls_format_not_hex() {
        let mut config = create_valid_config();
        config.init.bls_public_keys[0] = "0xzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz".to_string();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not valid hex"));
    }

    #[test]
    fn test_zero_epoch_length() {
        let mut config = create_valid_config();
        config.wbft.epoch_length = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Epoch length must be > 0"));
    }

    #[test]
    fn test_initial_bls_public_keys() {
        let config = create_valid_config();
        let keys = config.initial_bls_public_keys().unwrap();

        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0].len(), 48); // 96 hex chars = 48 bytes
        assert_eq!(keys[1].len(), 48);
        assert_eq!(keys[2].len(), 48);
    }

    #[test]
    fn test_wbft_config_default() {
        let config = WbftConfig::default();

        assert_eq!(config.request_timeout_seconds, 2);
        assert_eq!(config.block_period_seconds, 1);
        assert_eq!(config.proposer_policy, 0);
        assert_eq!(config.epoch_length, 10);
        assert!(config.max_request_timeout_seconds.is_none());
    }

    #[test]
    fn test_json_serialization() {
        let config = create_valid_config();

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&config).unwrap();

        // Deserialize back
        let deserialized: WbftChainConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_get_config() {
        let config = create_valid_config();

        // Currently returns the same config for all blocks (no transitions yet)
        let config_at_0 = config.get_config(0);
        let config_at_100 = config.get_config(100);

        assert_eq!(config_at_0, config.wbft);
        assert_eq!(config_at_100, config.wbft);
    }
}
