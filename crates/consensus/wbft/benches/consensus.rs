//! Consensus Latency Benchmarks
//!
//! Measures performance of block sealing and validator set operations
//! for different numbers of validators.

use alloy_primitives::{Address, U256};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use reth_consensus_wbft::{
    bls::{aggregate_signatures, SealerSet, SecretKey},
    header::WbftExtra,
    sealer::{SealResult, WbftSealer},
    validator::{DefaultValidator, DefaultValidatorSet},
    Validator, ValidatorSet, WbftAggregatedSeal,
};

/// Create validators with BLS keys
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

/// Create aggregated seal from validators
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

/// Create unsealed extra data
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

/// Benchmark block sealing time for different validator counts
fn bench_block_sealing(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_sealing");

    for num_validators in [3, 4, 7, 10, 13].iter() {
        let validators = create_validators(*num_validators);
        let validator_structs: Vec<_> = validators.iter().map(|(v, _)| v.clone()).collect();
        let set = DefaultValidatorSet::new(validator_structs).unwrap();
        let quorum = set.quorum_size();

        // Signing indices for quorum
        let signing_indices: Vec<_> = (0..quorum).collect();
        let block_hash = b"benchmark block hash";

        let sealer = WbftSealer::new();

        group.bench_with_input(
            BenchmarkId::from_parameter(num_validators),
            num_validators,
            |b, _| {
                b.iter(|| {
                    // Create seals
                    let prepared_seal = create_aggregated_seal(
                        black_box(&validators),
                        black_box(&signing_indices),
                        black_box(block_hash),
                    );
                    let committed_seal = create_aggregated_seal(
                        black_box(&validators),
                        black_box(&signing_indices),
                        black_box(block_hash),
                    );

                    let seal_result = SealResult::new(prepared_seal, committed_seal);

                    // Apply seals
                    let extra = create_unsealed_extra();
                    let extra_data = extra.encode();
                    let sealed_data = sealer.apply_seals(&extra_data, &seal_result).unwrap();

                    black_box(sealed_data)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark validator set creation and quorum calculation
fn bench_validator_set_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("validator_set");

    for num_validators in [3, 4, 7, 10, 13, 21].iter() {
        let validators = create_validators(*num_validators);
        let validator_structs: Vec<_> = validators.iter().map(|(v, _)| v.clone()).collect();

        group.bench_with_input(
            BenchmarkId::new("creation", num_validators),
            &validator_structs,
            |b, vs| {
                b.iter(|| {
                    let set = DefaultValidatorSet::new(black_box(vs.clone())).unwrap();
                    black_box(set)
                })
            },
        );
    }

    // Quorum calculation
    let validators = create_validators(10);
    let validator_structs: Vec<_> = validators.iter().map(|(v, _)| v.clone()).collect();
    let set = DefaultValidatorSet::new(validator_structs).unwrap();

    group.bench_function("quorum_size", |b| {
        b.iter(|| {
            let quorum = set.quorum_size();
            black_box(quorum)
        })
    });

    group.bench_function("f_calculation", |b| {
        b.iter(|| {
            let f = set.f();
            black_box(f)
        })
    });

    group.finish();
}

/// Benchmark proposer selection
fn bench_proposer_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("proposer_selection");

    for num_validators in [4, 7, 10].iter() {
        let validators = create_validators(*num_validators);
        let validator_structs: Vec<_> = validators.iter().map(|(v, _)| v.clone()).collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(num_validators),
            num_validators,
            |b, _| {
                let mut set = DefaultValidatorSet::new(validator_structs.clone()).unwrap();
                b.iter(|| {
                    set.calc_proposer(black_box(100));
                    let proposer = set.proposer();
                    black_box(proposer.address())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark seal extraction and verification workflow
fn bench_seal_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("seal_extraction");

    for num_validators in [4, 7].iter() {
        let validators = create_validators(*num_validators);
        let validator_structs: Vec<_> = validators.iter().map(|(v, _)| v.clone()).collect();
        let set = DefaultValidatorSet::new(validator_structs).unwrap();
        let quorum = set.quorum_size();

        let signing_indices: Vec<_> = (0..quorum).collect();
        let block_hash = b"benchmark extraction";

        let sealer = WbftSealer::new();

        // Create sealed data
        let prepared_seal = create_aggregated_seal(&validators, &signing_indices, block_hash);
        let committed_seal = create_aggregated_seal(&validators, &signing_indices, block_hash);
        let seal_result = SealResult::new(prepared_seal, committed_seal);

        let extra = create_unsealed_extra();
        let extra_data = extra.encode();
        let sealed_data = sealer.apply_seals(&extra_data, &seal_result).unwrap();

        group.bench_with_input(
            BenchmarkId::from_parameter(num_validators),
            num_validators,
            |b, _| {
                b.iter(|| {
                    // Extract seals
                    let extracted = sealer.extract_seals(black_box(&sealed_data)).unwrap();
                    black_box(extracted)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark complete consensus round (sign + aggregate + verify)
fn bench_consensus_round(c: &mut Criterion) {
    let mut group = c.benchmark_group("consensus_round");

    for num_validators in [4, 7].iter() {
        let validators = create_validators(*num_validators);
        let validator_structs: Vec<_> = validators.iter().map(|(v, _)| v.clone()).collect();
        let set = DefaultValidatorSet::new(validator_structs).unwrap();
        let quorum = set.quorum_size();

        let public_keys: Vec<_> = validators.iter().map(|(_, sk)| sk.public_key()).collect();
        let signing_indices: Vec<_> = (0..quorum).collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(num_validators),
            num_validators,
            |b, _| {
                b.iter(|| {
                    let block_hash = b"consensus round benchmark";

                    // Prepared phase: sign and aggregate
                    let prepared_seal = create_aggregated_seal(
                        black_box(&validators),
                        black_box(&signing_indices),
                        black_box(block_hash),
                    );

                    // Verify prepared seal
                    let _verified = prepared_seal.verify(&public_keys, block_hash);

                    // Committed phase: sign and aggregate
                    let committed_seal = create_aggregated_seal(
                        black_box(&validators),
                        black_box(&signing_indices),
                        black_box(block_hash),
                    );

                    // Verify committed seal
                    let _verified = committed_seal.verify(&public_keys, block_hash);

                    // Create final sealed block
                    let sealer = WbftSealer::new();
                    let seal_result = SealResult::new(prepared_seal, committed_seal);

                    let extra = create_unsealed_extra();
                    let extra_data = extra.encode();
                    let sealed_data = sealer.apply_seals(&extra_data, &seal_result).unwrap();

                    black_box(sealed_data)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark validator lookup by address
fn bench_validator_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("validator_lookup");

    let validators = create_validators(10);
    let validator_structs: Vec<_> = validators.iter().map(|(v, _)| v.clone()).collect();
    let addresses: Vec<_> = validator_structs.iter().map(|v| v.address()).collect();
    let set = DefaultValidatorSet::new(validator_structs).unwrap();

    // Lookup by address
    group.bench_function("by_address", |b| {
        b.iter(|| {
            let validator = set.get_by_address(black_box(&addresses[5]));
            black_box(validator)
        })
    });

    // Lookup by index
    group.bench_function("by_index", |b| {
        b.iter(|| {
            let validator = set.get_by_index(black_box(5));
            black_box(validator)
        })
    });

    // Check if validator
    group.bench_function("is_validator", |b| {
        b.iter(|| {
            let result = set.is_validator(black_box(&addresses[5]));
            black_box(result)
        })
    });

    // Get index
    group.bench_function("get_index", |b| {
        b.iter(|| {
            let index = set.get_index(black_box(&addresses[5]));
            black_box(index)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_block_sealing,
    bench_validator_set_operations,
    bench_proposer_selection,
    bench_seal_extraction,
    bench_consensus_round,
    bench_validator_lookup,
);

criterion_main!(benches);
