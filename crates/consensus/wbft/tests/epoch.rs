//! Epoch Transition Integration Tests
//!
//! Tests for WBFT epoch transitions including validator set changes
//! and consensus with new validators.

use alloy_primitives::{Address, U256};
use reth_consensus_wbft::{
    bls::{aggregate_signatures, SealerSet, SecretKey},
    consensus::WbftConfig,
    epoch::EpochManager,
    header::WbftExtra,
    sealer::{SealResult, WbftSealer},
    validator::{DefaultValidator, DefaultValidatorSet},
    Validator, ValidatorSet, WbftAggregatedSeal,
};

/// Create multiple validators with BLS keys
fn create_validators(count: usize) -> Vec<(DefaultValidator, SecretKey)> {
    (0..count)
        .map(|i| {
            let sk = SecretKey::random();
            let pk = sk.public_key();
            let address = Address::from([i as u8 + 1; 20]);
            (DefaultValidator::new(address, pk.to_bytes()), sk)
        })
        .collect()
}

/// Create aggregated seal
fn create_aggregated_seal(
    validators: &[(DefaultValidator, SecretKey)],
    signing_indices: &[usize],
    message: &[u8],
) -> WbftAggregatedSeal {
    let total = validators.len();
    let mut bitmap = SealerSet::new(total);

    let signatures: Vec<_> = signing_indices
        .iter()
        .map(|&i| {
            bitmap.set_sealer(i as u32);
            validators[i].1.sign(message)
        })
        .collect();

    let agg_sig = aggregate_signatures(&signatures).unwrap();
    WbftAggregatedSeal::new(bitmap, agg_sig.to_bytes())
}

/// Create test extra data
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

/// Create test WBFT config
fn create_test_config(epoch_length: u64) -> WbftConfig {
    WbftConfig {
        epoch_length,
        request_timeout_seconds: 4,
        block_period_seconds: 2,
        proposer_policy: 0,
        max_request_timeout_seconds: None,
    }
}

#[test]
fn test_epoch_manager_initialization() {
    let epoch_length = 100;
    let config = create_test_config(epoch_length);
    let manager = EpochManager::new(config);

    assert_eq!(manager.epoch_length(), epoch_length);
}

#[test]
fn test_epoch_block_detection() {
    let epoch_length = 100;
    let config = create_test_config(epoch_length);
    let manager = EpochManager::new(config);

    // Block 0 is genesis, not epoch block
    assert!(!manager.is_epoch_block(0));

    // Block 100 is first epoch block
    assert!(manager.is_epoch_block(100));

    // Block 200 is second epoch block
    assert!(manager.is_epoch_block(200));

    // Non-epoch blocks
    assert!(!manager.is_epoch_block(50));
    assert!(!manager.is_epoch_block(99));
    assert!(!manager.is_epoch_block(101));
}

#[test]
fn test_epoch_number_calculation() {
    let epoch_length = 100;
    let config = create_test_config(epoch_length);
    let manager = EpochManager::new(config);

    // Epoch 0: blocks 0-99
    assert_eq!(manager.epoch_number(0), 0);
    assert_eq!(manager.epoch_number(50), 0);
    assert_eq!(manager.epoch_number(99), 0);

    // Epoch 1: blocks 100-199
    assert_eq!(manager.epoch_number(100), 1);
    assert_eq!(manager.epoch_number(150), 1);
    assert_eq!(manager.epoch_number(199), 1);

    // Epoch 2: blocks 200-299
    assert_eq!(manager.epoch_number(200), 2);
}

#[test]
fn test_epoch_start_block() {
    let epoch_length = 100;
    let config = create_test_config(epoch_length);
    let manager = EpochManager::new(config);

    // Epoch 0 starts at block 0
    assert_eq!(manager.epoch_start_block(0), 0);

    // Epoch 1 starts at block 100
    assert_eq!(manager.epoch_start_block(1), 100);

    // Epoch 5 starts at block 500
    assert_eq!(manager.epoch_start_block(5), 500);
}

#[test]
fn test_epoch_end_block() {
    let epoch_length = 100;
    let config = create_test_config(epoch_length);
    let manager = EpochManager::new(config);

    // Epoch 0 ends at block 99
    assert_eq!(manager.epoch_end_block(0), 99);

    // Epoch 1 ends at block 199
    assert_eq!(manager.epoch_end_block(1), 199);

    // Epoch 5 ends at block 599
    assert_eq!(manager.epoch_end_block(5), 599);
}

#[test]
fn test_validator_set_change_simulation() {
    // Initial validators (epoch 0)
    let initial_validators = create_validators(4);
    let initial_set: Vec<_> = initial_validators.iter().map(|(v, _)| v.clone()).collect();
    let set = DefaultValidatorSet::new(initial_set).unwrap();

    assert_eq!(set.size(), 4);
    assert_eq!(set.quorum_size(), 3);

    // New validators (epoch 1)
    let new_validators = create_validators(7);
    let new_set: Vec<_> = new_validators.iter().map(|(v, _)| v.clone()).collect();
    let new_validator_set = DefaultValidatorSet::new(new_set).unwrap();

    // New set has different quorum
    assert_eq!(new_validator_set.size(), 7);
    assert_eq!(new_validator_set.quorum_size(), 5);
}

#[test]
fn test_consensus_with_new_validators() {
    // Old validator set
    let old_validators = create_validators(4);

    // New validator set (completely different)
    let new_validators: Vec<_> = (0..7)
        .map(|i| {
            let sk = SecretKey::random();
            let pk = sk.public_key();
            let address = Address::from([i as u8 + 100; 20]); // Different addresses
            (DefaultValidator::new(address, pk.to_bytes()), sk)
        })
        .collect();

    // Last block with old validators
    let old_block_hash = b"last block old epoch";
    let old_seal = create_aggregated_seal(&old_validators, &[0, 1, 2], old_block_hash);

    // First block with new validators
    let new_block_hash = b"first block new epoch";
    let new_seal = create_aggregated_seal(&new_validators, &[0, 1, 2, 3, 4], new_block_hash);

    // Both seals should be valid
    assert_eq!(old_seal.signer_count(), 3);
    assert_eq!(new_seal.signer_count(), 5);

    // Verify old validators seal
    let old_pks: Vec<_> = vec![0, 1, 2].iter().map(|&i| old_validators[i].1.public_key()).collect();
    assert!(old_seal.verify(&old_pks, old_block_hash).unwrap());

    // Verify new validators seal
    let new_pks: Vec<_> =
        vec![0, 1, 2, 3, 4].iter().map(|&i| new_validators[i].1.public_key()).collect();
    assert!(new_seal.verify(&new_pks, new_block_hash).unwrap());
}

#[test]
fn test_block_sealing_at_epoch_boundary() {
    let validators = create_validators(4);
    let sealer = WbftSealer::new();

    let block_hash = b"epoch boundary block";
    let prepared_seal = create_aggregated_seal(&validators, &[0, 1, 2], block_hash);
    let committed_seal = create_aggregated_seal(&validators, &[0, 1, 2], block_hash);
    let seal_result = SealResult::new(prepared_seal, committed_seal);

    let extra = create_unsealed_extra();
    let extra_data = extra.encode();
    let sealed_data = sealer.apply_seals(&extra_data, &seal_result).unwrap();

    // Verify seals
    assert!(sealer.has_seals(&sealed_data));
    let extracted = sealer.extract_seals(&sealed_data).unwrap();
    assert_eq!(extracted.prepared_seal.signer_count(), 3);
    assert_eq!(extracted.committed_seal.signer_count(), 3);
}

#[test]
fn test_multiple_epoch_transitions() {
    let epoch_length = 10; // Short epoch for testing
    let config = create_test_config(epoch_length);
    let manager = EpochManager::new(config);
    let sealer = WbftSealer::new();

    // Simulate multiple epochs
    for epoch in 1..=3 {
        let validators = create_validators(4);
        let epoch_block = epoch * epoch_length;

        assert!(manager.is_epoch_block(epoch_block));

        // Create block
        let block_hash = format!("epoch {} block", epoch);
        let prepared_seal = create_aggregated_seal(&validators, &[0, 1, 2], block_hash.as_bytes());
        let committed_seal = create_aggregated_seal(&validators, &[0, 1, 2], block_hash.as_bytes());
        let seal_result = SealResult::new(prepared_seal, committed_seal);

        let extra = create_unsealed_extra();
        let extra_data = extra.encode();
        let sealed_data = sealer.apply_seals(&extra_data, &seal_result).unwrap();

        assert!(sealer.has_seals(&sealed_data));
    }
}

#[test]
fn test_validator_quorum_changes() {
    // Different validator counts have different quorum requirements

    // 3 validators: f=0, quorum=1
    let v3 = create_validators(3);
    let set3: Vec<_> = v3.iter().map(|(v, _)| v.clone()).collect();
    let vs3 = DefaultValidatorSet::new(set3).unwrap();
    assert_eq!(vs3.f(), 0);
    assert_eq!(vs3.quorum_size(), 1);

    // 4 validators: f=1, quorum=3
    let v4 = create_validators(4);
    let set4: Vec<_> = v4.iter().map(|(v, _)| v.clone()).collect();
    let vs4 = DefaultValidatorSet::new(set4).unwrap();
    assert_eq!(vs4.f(), 1);
    assert_eq!(vs4.quorum_size(), 3);

    // 7 validators: f=2, quorum=5
    let v7 = create_validators(7);
    let set7: Vec<_> = v7.iter().map(|(v, _)| v.clone()).collect();
    let vs7 = DefaultValidatorSet::new(set7).unwrap();
    assert_eq!(vs7.f(), 2);
    assert_eq!(vs7.quorum_size(), 5);

    // 10 validators: f=3, quorum=7
    let v10 = create_validators(10);
    let set10: Vec<_> = v10.iter().map(|(v, _)| v.clone()).collect();
    let vs10 = DefaultValidatorSet::new(set10).unwrap();
    assert_eq!(vs10.f(), 3);
    assert_eq!(vs10.quorum_size(), 7);
}

#[test]
fn test_epoch_config_values() {
    let config = WbftConfig {
        epoch_length: 100,
        request_timeout_seconds: 4,
        block_period_seconds: 2,
        proposer_policy: 0,
        max_request_timeout_seconds: Some(10),
    };

    assert_eq!(config.epoch_length, 100);
    assert_eq!(config.request_timeout_seconds, 4);
    assert_eq!(config.block_period_seconds, 2);
    assert_eq!(config.max_request_timeout_seconds, Some(10));
}
