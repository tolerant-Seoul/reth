//! BLS Signature Benchmarks
//!
//! Measures performance of BLS signature operations including
//! key generation, signing, verification, and aggregation.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use reth_consensus_wbft::bls::{aggregate_signatures, verify_aggregated, SealerSet, SecretKey};

/// Benchmark individual signature generation
fn bench_sign(c: &mut Criterion) {
    let sk = SecretKey::random();
    let message = b"benchmark message for signing";

    c.bench_function("bls_sign", |b| {
        b.iter(|| {
            let sig = sk.sign(black_box(message));
            black_box(sig)
        })
    });
}

/// Benchmark individual signature verification
fn bench_verify(c: &mut Criterion) {
    let sk = SecretKey::random();
    let pk = sk.public_key();
    let message = b"benchmark message for verification";
    let signature = sk.sign(message);

    c.bench_function("bls_verify", |b| {
        b.iter(|| {
            let result = pk.verify(black_box(message), black_box(&signature));
            black_box(result)
        })
    });
}

/// Benchmark key generation
fn bench_key_generation(c: &mut Criterion) {
    c.bench_function("bls_key_generation", |b| {
        b.iter(|| {
            let sk = SecretKey::random();
            let pk = sk.public_key();
            black_box((sk, pk))
        })
    });
}

/// Benchmark signature aggregation for different numbers of signatures
fn bench_aggregate(c: &mut Criterion) {
    let mut group = c.benchmark_group("bls_aggregate");

    for num_signers in [3, 4, 7, 10, 13].iter() {
        let message = b"benchmark message for aggregation";

        // Create signatures
        let signatures: Vec<_> = (0..*num_signers)
            .map(|_| {
                let sk = SecretKey::random();
                sk.sign(message)
            })
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(num_signers), num_signers, |b, _| {
            b.iter(|| {
                let agg = aggregate_signatures(black_box(&signatures)).unwrap();
                black_box(agg)
            })
        });
    }

    group.finish();
}

/// Benchmark aggregated signature verification for different numbers of signers
fn bench_aggregate_verify(c: &mut Criterion) {
    let mut group = c.benchmark_group("bls_aggregate_verify");

    for num_signers in [3, 4, 7, 10, 13].iter() {
        let message = b"benchmark message for aggregate verification";

        // Create keys and signatures
        let keys: Vec<_> = (0..*num_signers).map(|_| SecretKey::random()).collect();

        let public_keys: Vec<_> = keys.iter().map(|sk| sk.public_key()).collect();

        let signatures: Vec<_> = keys.iter().map(|sk| sk.sign(message)).collect();

        let aggregated = aggregate_signatures(&signatures).unwrap();

        let pk_refs: Vec<_> = public_keys.iter().collect();

        group.bench_with_input(BenchmarkId::from_parameter(num_signers), num_signers, |b, _| {
            b.iter(|| {
                let result = verify_aggregated(
                    black_box(&pk_refs),
                    black_box(message),
                    black_box(&aggregated),
                );
                black_box(result)
            })
        });
    }

    group.finish();
}

/// Benchmark SealerSet bitmap operations
fn bench_sealer_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("sealer_set");

    // Benchmark creating SealerSet
    group.bench_function("create_100", |b| {
        b.iter(|| {
            let set = SealerSet::new(black_box(100));
            black_box(set)
        })
    });

    // Benchmark setting sealers
    group.bench_function("set_sealer", |b| {
        b.iter(|| {
            let mut set = SealerSet::new(100);
            set.set_sealer(black_box(50));
            black_box(set)
        })
    });

    // Benchmark checking sealer
    let mut set = SealerSet::new(100);
    for i in 0..50 {
        set.set_sealer(i);
    }
    group.bench_function("is_sealer", |b| {
        b.iter(|| {
            let result = set.is_sealer(black_box(25));
            black_box(result)
        })
    });

    // Benchmark getting all sealers
    group.bench_function("get_sealers_50", |b| {
        b.iter(|| {
            let sealers = set.get_sealers();
            black_box(sealers)
        })
    });

    // Benchmark count
    group.bench_function("count_50", |b| {
        b.iter(|| {
            let count = set.count();
            black_box(count)
        })
    });

    group.finish();
}

/// Benchmark complete signing and verification workflow
fn bench_full_workflow(c: &mut Criterion) {
    let mut group = c.benchmark_group("bls_full_workflow");

    for num_signers in [4, 7].iter() {
        let message = b"benchmark full workflow message";

        group.bench_with_input(
            BenchmarkId::new("sign_aggregate_verify", num_signers),
            num_signers,
            |b, &n| {
                // Pre-generate keys
                let keys: Vec<_> = (0..n).map(|_| SecretKey::random()).collect();
                let public_keys: Vec<_> = keys.iter().map(|sk| sk.public_key()).collect();

                b.iter(|| {
                    // Sign
                    let signatures: Vec<_> =
                        keys.iter().map(|sk| sk.sign(black_box(message))).collect();

                    // Aggregate
                    let aggregated = aggregate_signatures(&signatures).unwrap();

                    // Verify
                    let pk_refs: Vec<_> = public_keys.iter().collect();
                    let result = verify_aggregated(&pk_refs, message, &aggregated);

                    black_box(result)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_sign,
    bench_verify,
    bench_key_generation,
    bench_aggregate,
    bench_aggregate_verify,
    bench_sealer_set,
    bench_full_workflow,
);

criterion_main!(benches);
