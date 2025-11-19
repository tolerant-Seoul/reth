//! Multi-Validator Consensus Integration Tests
//!
//! Tests for WBFT consensus with multiple validators.
//! Covers 3, 4, and 7 validator scenarios.

use alloy_primitives::{Address, U256};
use reth_consensus_wbft::{
    bls::{aggregate_signatures, verify_aggregated, SealerSet, SecretKey},
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

/// Create aggregated seal from multiple validators
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

#[test]
fn test_three_validator_consensus() {
    // 3 validators: f=0, quorum=1
    let validators = create_validators(3);
    let validator_structs: Vec<_> = validators.iter().map(|(v, _)| v.clone()).collect();
    let set = DefaultValidatorSet::new(validator_structs).unwrap();

    assert_eq!(set.size(), 3);
    assert_eq!(set.f(), 0);
    assert_eq!(set.quorum_size(), 1);

    // Create block hash
    let block_hash = b"block hash for 3 validators";

    // All 3 validators sign
    let signing_indices: Vec<_> = (0..3).collect();
    let seal = create_aggregated_seal(&validators, &signing_indices, block_hash);

    assert_eq!(seal.signer_count(), 3);

    // Verify seal
    let public_keys: Vec<_> = validators.iter().map(|(_, sk)| sk.public_key()).collect();
    assert!(seal.verify(&public_keys, block_hash).unwrap());
}

#[test]
fn test_four_validator_consensus() {
    // 4 validators: f=1, quorum=3
    let validators = create_validators(4);
    let validator_structs: Vec<_> = validators.iter().map(|(v, _)| v.clone()).collect();
    let set = DefaultValidatorSet::new(validator_structs).unwrap();

    assert_eq!(set.size(), 4);
    assert_eq!(set.f(), 1);
    assert_eq!(set.quorum_size(), 3);

    let block_hash = b"block hash for 4 validators";

    // Quorum (3 out of 4) signs
    let signing_indices = vec![0, 1, 2];
    let seal = create_aggregated_seal(&validators, &signing_indices, block_hash);

    assert_eq!(seal.signer_count(), 3);

    // Verify with only signing validators' public keys
    let signing_pks: Vec<_> =
        signing_indices.iter().map(|&i| validators[i].1.public_key()).collect();
    assert!(seal.verify(&signing_pks, block_hash).unwrap());
}

#[test]
fn test_four_validator_full_consensus() {
    // All 4 validators sign
    let validators = create_validators(4);
    let block_hash = b"full consensus block";

    let signing_indices: Vec<_> = (0..4).collect();
    let seal = create_aggregated_seal(&validators, &signing_indices, block_hash);

    assert_eq!(seal.signer_count(), 4);

    let public_keys: Vec<_> = validators.iter().map(|(_, sk)| sk.public_key()).collect();
    assert!(seal.verify(&public_keys, block_hash).unwrap());
}

#[test]
fn test_seven_validator_consensus() {
    // 7 validators: f=2, quorum=5
    let validators = create_validators(7);
    let validator_structs: Vec<_> = validators.iter().map(|(v, _)| v.clone()).collect();
    let set = DefaultValidatorSet::new(validator_structs).unwrap();

    assert_eq!(set.size(), 7);
    assert_eq!(set.f(), 2);
    assert_eq!(set.quorum_size(), 5);

    let block_hash = b"block hash for 7 validators";

    // Quorum (5 out of 7) signs
    let signing_indices = vec![0, 1, 2, 3, 4];
    let seal = create_aggregated_seal(&validators, &signing_indices, block_hash);

    assert_eq!(seal.signer_count(), 5);

    let signing_pks: Vec<_> =
        signing_indices.iter().map(|&i| validators[i].1.public_key()).collect();
    assert!(seal.verify(&signing_pks, block_hash).unwrap());
}

#[test]
fn test_seven_validator_full_consensus() {
    // All 7 validators sign
    let validators = create_validators(7);
    let block_hash = b"full 7 validator consensus";

    let signing_indices: Vec<_> = (0..7).collect();
    let seal = create_aggregated_seal(&validators, &signing_indices, block_hash);

    assert_eq!(seal.signer_count(), 7);

    let public_keys: Vec<_> = validators.iter().map(|(_, sk)| sk.public_key()).collect();
    assert!(seal.verify(&public_keys, block_hash).unwrap());
}

#[test]
fn test_block_sealing_with_quorum() {
    // Test complete block sealing flow with 4 validators
    let validators = create_validators(4);
    let sealer = WbftSealer::new();

    let block_hash = b"sealed block hash";

    // Create prepared seal (3 out of 4)
    let prepared_seal = create_aggregated_seal(&validators, &[0, 1, 2], block_hash);

    // Create committed seal (3 out of 4)
    let committed_seal = create_aggregated_seal(&validators, &[0, 1, 3], block_hash);

    let seal_result = SealResult::new(prepared_seal, committed_seal);

    // Apply seals
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
fn test_proposer_rotation() {
    let validators = create_validators(4);
    let validator_structs: Vec<_> = validators.iter().map(|(v, _)| v.clone()).collect();
    let addresses: Vec<_> = validator_structs.iter().map(|v| v.address()).collect();
    let mut set = DefaultValidatorSet::new(validator_structs).unwrap();

    // Round-robin proposer selection
    set.calc_proposer(0);
    assert_eq!(set.proposer().address(), addresses[0]);

    set.calc_proposer(1);
    assert_eq!(set.proposer().address(), addresses[1]);

    set.calc_proposer(2);
    assert_eq!(set.proposer().address(), addresses[2]);

    set.calc_proposer(3);
    assert_eq!(set.proposer().address(), addresses[3]);

    set.calc_proposer(4);
    assert_eq!(set.proposer().address(), addresses[0]); // Wraps around
}

#[test]
fn test_insufficient_quorum_detected() {
    // 4 validators need 3 for quorum
    let validators = create_validators(4);
    let validator_structs: Vec<_> = validators.iter().map(|(v, _)| v.clone()).collect();
    let set = DefaultValidatorSet::new(validator_structs).unwrap();
    let block_hash = b"insufficient quorum";

    // Only 2 validators sign (insufficient for quorum of 3)
    let signing_indices = vec![0, 1];
    let seal = create_aggregated_seal(&validators, &signing_indices, block_hash);

    assert_eq!(seal.signer_count(), 2);

    // Verify the signature is valid (it is, just not enough signers)
    let all_pks: Vec<_> = validators.iter().map(|(_, sk)| sk.public_key()).collect();
    assert!(seal.verify(&all_pks, block_hash).unwrap());

    // But the signer count is less than quorum
    assert!(seal.signer_count() < set.quorum_size());
}

#[test]
fn test_different_messages_fail_verification() {
    let validators = create_validators(4);
    let correct_message = b"correct message";
    let wrong_message = b"wrong message";

    let seal = create_aggregated_seal(&validators, &[0, 1, 2], correct_message);

    let public_keys: Vec<_> = vec![0, 1, 2].iter().map(|&i| validators[i].1.public_key()).collect();

    // Verification with wrong message should fail
    assert!(!seal.verify(&public_keys, wrong_message).unwrap());
}

#[test]
fn test_multiple_rounds_consensus() {
    // Simulate consensus over multiple rounds
    let validators = create_validators(4);
    let sealer = WbftSealer::new();

    for round in 0..3 {
        let block_hash = format!("block at round {}", round);
        let block_bytes = block_hash.as_bytes();

        let prepared_seal = create_aggregated_seal(&validators, &[0, 1, 2], block_bytes);
        let committed_seal = create_aggregated_seal(&validators, &[0, 1, 2], block_bytes);

        let seal_result = SealResult::new(prepared_seal, committed_seal);

        let mut extra = create_unsealed_extra();
        extra.round = round;
        let extra_data = extra.encode();

        let sealed_data = sealer.apply_seals(&extra_data, &seal_result).unwrap();
        assert!(sealer.has_seals(&sealed_data));

        // Verify round is preserved
        let decoded = WbftExtra::decode(&mut &sealed_data[..]).unwrap();
        assert_eq!(decoded.round, round);
    }
}

#[test]
fn test_validator_subset_signing() {
    // Test different subsets of validators signing
    let validators = create_validators(7);
    let block_hash = b"subset signing test";

    // All public keys needed for verification (bitmap uses original indices)
    let all_pks: Vec<_> = validators.iter().map(|(_, sk)| sk.public_key()).collect();

    // Different subsets that form quorum (5)
    let subsets =
        vec![vec![0, 1, 2, 3, 4], vec![0, 1, 2, 3, 5], vec![0, 2, 4, 5, 6], vec![1, 2, 3, 5, 6]];

    for subset in subsets {
        let seal = create_aggregated_seal(&validators, &subset, block_hash);
        assert_eq!(seal.signer_count(), 5);

        // Verify with all public keys - bitmap selects the right ones
        assert!(seal.verify(&all_pks, block_hash).unwrap());
    }
}
