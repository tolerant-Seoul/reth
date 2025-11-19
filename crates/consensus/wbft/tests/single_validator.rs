//! Single Validator Integration Tests
//!
//! Tests for WBFT consensus with a single validator node.
//! This is the minimum viable setup for block production.

use alloy_primitives::{Address, Bytes, U256};
use reth_consensus_wbft::{
    bls::{SealerSet, SecretKey},
    consensus::WbftConfig,
    header::WbftExtra,
    sealer::{SealResult, WbftSealer},
    validator::{DefaultValidator, DefaultValidatorSet},
    Validator, ValidatorSet, WbftAggregatedSeal,
};

/// Create a test validator with BLS key
fn create_validator(index: u8) -> (DefaultValidator, SecretKey) {
    let sk = SecretKey::random();
    let pk = sk.public_key();
    let address = Address::from([index; 20]);
    (DefaultValidator::new(address, pk.to_bytes()), sk)
}

/// Create test extra data without seals
fn create_unsealed_extra() -> WbftExtra {
    WbftExtra {
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

#[test]
fn test_single_validator_block_sealing() {
    // Setup: Create a single validator
    let (validator, secret_key) = create_validator(1);
    let validator_set = DefaultValidatorSet::new(vec![validator.clone()]).unwrap();

    // Verify quorum is 1 for single validator
    assert_eq!(validator_set.quorum_size(), 1);

    // Create block hash to sign
    let block_hash = [0x42u8; 32];

    // Create prepared seal (single signature)
    let prepared_sig = secret_key.sign(&block_hash);
    let mut prepared_bitmap = SealerSet::new(1);
    prepared_bitmap.set_sealer(0);
    let prepared_seal = WbftAggregatedSeal::new(prepared_bitmap, prepared_sig.to_bytes());

    // Create committed seal (single signature)
    let committed_sig = secret_key.sign(&block_hash);
    let mut committed_bitmap = SealerSet::new(1);
    committed_bitmap.set_sealer(0);
    let committed_seal = WbftAggregatedSeal::new(committed_bitmap, committed_sig.to_bytes());

    // Create seal result
    let seal_result = SealResult::new(prepared_seal.clone(), committed_seal.clone());

    // Apply seals to extra data
    let sealer = WbftSealer::new();
    let extra = create_unsealed_extra();
    let extra_data = extra.encode();

    let sealed_data = sealer.apply_seals(&extra_data, &seal_result).unwrap();

    // Verify seals were applied
    assert!(sealer.has_seals(&sealed_data));

    // Extract and verify seals
    let extracted = sealer.extract_seals(&sealed_data).unwrap();
    assert_eq!(extracted.prepared_seal.signer_count(), 1);
    assert_eq!(extracted.committed_seal.signer_count(), 1);
}

#[test]
fn test_single_validator_seal_verification() {
    // Setup
    let (validator, secret_key) = create_validator(1);
    let public_key = secret_key.public_key();

    // Create and sign block hash
    let block_hash = b"test block hash";
    let signature = secret_key.sign(block_hash);

    // Create aggregated seal
    let mut bitmap = SealerSet::new(1);
    bitmap.set_sealer(0);
    let seal = WbftAggregatedSeal::new(bitmap, signature.to_bytes());

    // Verify seal
    let public_keys = vec![public_key];
    assert!(seal.verify(&public_keys, block_hash).unwrap());
}

#[test]
fn test_single_validator_header_extra_roundtrip() {
    // Create extra data with seals
    let (_, secret_key) = create_validator(1);
    let block_hash = [0x42u8; 32];

    // Create seals
    let sig = secret_key.sign(&block_hash);
    let mut bitmap = SealerSet::new(1);
    bitmap.set_sealer(0);
    let seal = WbftAggregatedSeal::new(bitmap, sig.to_bytes());

    // Create extra with seals
    let mut extra = create_unsealed_extra();
    extra.prepared_seal = Some(seal.clone());
    extra.committed_seal = Some(seal);

    // Encode and decode
    let encoded = extra.encode();
    let decoded = WbftExtra::decode(&mut &encoded[..]).unwrap();

    // Verify
    assert_eq!(decoded.round, extra.round);
    assert!(decoded.prepared_seal.is_some());
    assert!(decoded.committed_seal.is_some());
    assert_eq!(decoded.prepared_seal.unwrap().signer_count(), 1);
}

#[test]
fn test_single_validator_config() {
    // Create config for single validator
    let config = WbftConfig {
        epoch_length: 10,
        request_timeout_seconds: 4,
        block_period_seconds: 2,
        proposer_policy: 0,
        max_request_timeout_seconds: None,
    };

    assert_eq!(config.epoch_length, 10);
    assert_eq!(config.block_period_seconds, 2);
}

#[test]
fn test_single_validator_quorum_requirements() {
    // Single validator: f=0, quorum=1
    let (validator, _) = create_validator(1);
    let set = DefaultValidatorSet::new(vec![validator]).unwrap();

    assert_eq!(set.size(), 1);
    assert_eq!(set.f(), 0);
    assert_eq!(set.quorum_size(), 1);
}

#[test]
fn test_single_validator_proposer_selection() {
    let (validator, _) = create_validator(1);
    let address = validator.address();
    let mut set = DefaultValidatorSet::new(vec![validator]).unwrap();

    // For single validator, proposer is always the same
    assert_eq!(set.proposer().address(), address);

    // Round doesn't matter for single validator
    set.calc_proposer(0);
    assert_eq!(set.proposer().address(), address);

    set.calc_proposer(100);
    assert_eq!(set.proposer().address(), address);
}

#[test]
fn test_single_validator_genesis_seals() {
    let sealer = WbftSealer::new();
    let genesis_seals = sealer.create_genesis_seals();

    // Genesis has empty seals
    assert_eq!(genesis_seals.prepared_seal.signer_count(), 0);
    assert_eq!(genesis_seals.committed_seal.signer_count(), 0);
}

#[test]
fn test_single_validator_multiple_blocks() {
    // Simulate sealing multiple blocks
    let (_, secret_key) = create_validator(1);
    let sealer = WbftSealer::new();

    for block_num in 1..=5 {
        let block_hash = [block_num as u8; 32];

        // Create seals for this block
        let sig = secret_key.sign(&block_hash);
        let mut bitmap = SealerSet::new(1);
        bitmap.set_sealer(0);
        let seal = WbftAggregatedSeal::new(bitmap, sig.to_bytes());

        let seal_result = SealResult::new(seal.clone(), seal);

        // Apply seals
        let extra = create_unsealed_extra();
        let extra_data = extra.encode();
        let sealed_data = sealer.apply_seals(&extra_data, &seal_result).unwrap();

        // Verify
        assert!(sealer.has_seals(&sealed_data));
    }
}
