//! Round Change Integration Tests
//!
//! Tests for WBFT round change scenarios including timeouts,
//! multiple round changes, and consensus after round change.

use alloy_primitives::{Address, B256, U256};
use reth_consensus_wbft::{
    bls::{aggregate_signatures, SealerSet, SecretKey},
    header::WbftExtra,
    messages::RoundChange,
    sealer::{SealResult, WbftSealer},
    types::View,
    validator::{DefaultValidator, DefaultValidatorSet},
    Validator, ValidatorSet, WbftAggregatedSeal, WbftMessage,
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

/// Create test extra data with round
fn create_extra_with_round(round: u32) -> WbftExtra {
    WbftExtra {
        vanity_data: [0u8; 32],
        randao_reveal: Vec::new(),
        prev_round: if round > 0 { round - 1 } else { 0 },
        prev_prepared_seal: None,
        prev_committed_seal: None,
        round,
        prepared_seal: None,
        committed_seal: None,
        gas_tip: U256::ZERO,
        epoch_info: None,
    }
}

#[test]
fn test_round_change_message_creation() {
    let validators = create_validators(4);
    let (validator, secret_key) = &validators[0];

    // Create round change message
    let view = View { sequence: U256::from(1), round: U256::from(1) };

    // Sign the view for round change (using a simple message for test)
    let message = b"round_change_test";
    let signature = secret_key.sign(message);

    let round_change = RoundChange::new(view.clone(), validator.address(), signature.to_bytes());

    assert_eq!(round_change.view, view);
    assert_eq!(round_change.sender, validator.address());
    assert!(!round_change.has_prepared());

    // Validate message
    assert!(round_change.validate().is_ok());
}

#[test]
fn test_round_change_with_prepared_block() {
    let validators = create_validators(4);
    let (validator, secret_key) = &validators[0];

    // View for round change
    let view = View { sequence: U256::from(1), round: U256::from(2) };

    // Prepared view from previous round
    let prepared_view = View { sequence: U256::from(1), round: U256::from(1) };

    let prepared_digest = B256::from([0x42; 32]);

    // Sign the view (using a simple message for test)
    let message = b"round_change_with_prepared";
    let signature = secret_key.sign(message);

    let round_change = RoundChange::with_prepared(
        view.clone(),
        validator.address(),
        signature.to_bytes(),
        prepared_digest,
        prepared_view.clone(),
    );

    assert_eq!(round_change.prepared_view, Some(prepared_view));
    assert_eq!(round_change.prepared_digest, Some(prepared_digest));
    assert!(round_change.has_prepared());
    assert!(round_change.validate().is_ok());
}

#[test]
fn test_multiple_round_changes() {
    let validators = create_validators(4);
    let sealer = WbftSealer::new();

    // Simulate multiple round changes before consensus
    let rounds: Vec<u32> = vec![0, 1, 2, 3];

    for round in rounds {
        let block_hash = format!("block at round {}", round);
        let block_bytes = block_hash.as_bytes();

        // Create seals for this round
        let prepared_seal = create_aggregated_seal(&validators, &[0, 1, 2], block_bytes);
        let committed_seal = create_aggregated_seal(&validators, &[0, 1, 2], block_bytes);
        let seal_result = SealResult::new(prepared_seal, committed_seal);

        // Create extra with current round
        let extra = create_extra_with_round(round);
        let extra_data = extra.encode();

        let sealed_data = sealer.apply_seals(&extra_data, &seal_result).unwrap();

        // Verify round is preserved
        let decoded = WbftExtra::decode(&mut &sealed_data[..]).unwrap();
        assert_eq!(decoded.round, round);
    }
}

#[test]
fn test_consensus_after_round_change() {
    let validators = create_validators(4);
    let sealer = WbftSealer::new();

    // Simulate round 0 failure (no consensus)
    // Then round 1 succeeds

    // Round 1 consensus
    let block_hash = b"block after round change";
    let prepared_seal = create_aggregated_seal(&validators, &[0, 1, 2], block_hash);
    let committed_seal = create_aggregated_seal(&validators, &[0, 1, 2], block_hash);
    let seal_result = SealResult::new(prepared_seal, committed_seal);

    // Extra data with round = 1 and prev_round = 0
    let mut extra = create_extra_with_round(1);
    extra.prev_round = 0;

    let extra_data = extra.encode();
    let sealed_data = sealer.apply_seals(&extra_data, &seal_result).unwrap();

    let decoded = WbftExtra::decode(&mut &sealed_data[..]).unwrap();
    assert_eq!(decoded.round, 1);
    assert_eq!(decoded.prev_round, 0);
    assert!(sealer.has_seals(&sealed_data));
}

#[test]
fn test_round_change_proposer_rotation() {
    let validators = create_validators(4);
    let validator_structs: Vec<_> = validators.iter().map(|(v, _)| v.clone()).collect();
    let addresses: Vec<_> = validator_structs.iter().map(|v| v.address()).collect();
    let mut set = DefaultValidatorSet::new(validator_structs).unwrap();

    // Proposer rotates with each round
    set.calc_proposer(0);
    assert_eq!(set.proposer().address(), addresses[0]);

    set.calc_proposer(1);
    assert_eq!(set.proposer().address(), addresses[1]);

    set.calc_proposer(2);
    assert_eq!(set.proposer().address(), addresses[2]);

    set.calc_proposer(3);
    assert_eq!(set.proposer().address(), addresses[3]);
}

#[test]
fn test_round_change_quorum() {
    // 4 validators need 3 round changes for new round
    let validators = create_validators(4);
    let validator_structs: Vec<_> = validators.iter().map(|(v, _)| v.clone()).collect();
    let set = DefaultValidatorSet::new(validator_structs).unwrap();

    // Quorum for round change is same as consensus
    assert_eq!(set.quorum_size(), 3);

    // Create round change messages from quorum
    let view = View { sequence: U256::from(1), round: U256::from(1) };

    let mut round_changes = Vec::new();
    for i in 0..3 {
        let (validator, secret_key) = &validators[i];
        let message = b"round_change_quorum";
        let signature = secret_key.sign(message);
        let rc = RoundChange::new(view.clone(), validator.address(), signature.to_bytes());
        round_changes.push(rc);
    }

    assert_eq!(round_changes.len(), 3);
    assert!(round_changes.len() >= set.quorum_size());
}

#[test]
fn test_prev_round_seals_preserved() {
    let validators = create_validators(4);
    let block_hash = b"block hash";

    // Create seals from round 0
    let prev_seal = create_aggregated_seal(&validators, &[0, 1, 2], block_hash);

    // Create extra with prev round seals
    let mut extra = create_extra_with_round(1);
    extra.prev_prepared_seal = Some(prev_seal.clone());
    extra.prev_committed_seal = Some(prev_seal);

    // Encode and decode
    let encoded = extra.encode();
    let decoded = WbftExtra::decode(&mut &encoded[..]).unwrap();

    assert!(decoded.prev_prepared_seal.is_some());
    assert!(decoded.prev_committed_seal.is_some());
    assert_eq!(decoded.prev_prepared_seal.unwrap().signer_count(), 3);
}

#[test]
fn test_high_round_number() {
    let validators = create_validators(4);
    let sealer = WbftSealer::new();

    // Test with high round number
    let round = 100u32;
    let block_hash = b"high round block";

    let prepared_seal = create_aggregated_seal(&validators, &[0, 1, 2], block_hash);
    let committed_seal = create_aggregated_seal(&validators, &[0, 1, 2], block_hash);
    let seal_result = SealResult::new(prepared_seal, committed_seal);

    let extra = create_extra_with_round(round);
    let extra_data = extra.encode();

    let sealed_data = sealer.apply_seals(&extra_data, &seal_result).unwrap();
    let decoded = WbftExtra::decode(&mut &sealed_data[..]).unwrap();

    assert_eq!(decoded.round, round);
}

#[test]
fn test_round_change_message_validation() {
    let validators = create_validators(4);
    let (validator, secret_key) = &validators[0];

    // Valid round change
    let view = View { sequence: U256::from(1), round: U256::from(1) };

    let message = b"round_change_validation";
    let signature = secret_key.sign(message);

    let rc = RoundChange::new(view.clone(), validator.address(), signature.to_bytes());
    assert!(rc.validate().is_ok());
}

#[test]
fn test_round_change_sequence_preserved() {
    let validators = create_validators(4);

    // Round changes for same sequence, different rounds
    let sequence = U256::from(5);

    for round in 0..3 {
        let view = View { sequence, round: U256::from(round) };

        let (validator, secret_key) = &validators[0];
        let message = b"round_change_sequence";
        let signature = secret_key.sign(message);
        let rc = RoundChange::new(view.clone(), validator.address(), signature.to_bytes());

        assert_eq!(rc.view.sequence, sequence);
        assert_eq!(rc.view.round, U256::from(round));
    }
}
