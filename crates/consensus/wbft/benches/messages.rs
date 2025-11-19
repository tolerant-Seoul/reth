//! Message Processing Benchmarks
//!
//! Measures performance of message encoding/decoding and
//! WbftExtra header data operations.

use alloy_primitives::{Address, B256, U256};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use reth_consensus_wbft::{
    bls::{aggregate_signatures, SealerSet, SecretKey},
    header::WbftExtra,
    messages::RoundChange,
    types::View,
    WbftAggregatedSeal,
};

/// Create test WbftExtra data with seals
fn create_test_extra(with_seals: bool) -> WbftExtra {
    let mut extra = WbftExtra {
        vanity_data: [0u8; 32],
        randao_reveal: vec![0u8; 96],
        prev_round: 0,
        prev_prepared_seal: None,
        prev_committed_seal: None,
        round: 5,
        prepared_seal: None,
        committed_seal: None,
        gas_tip: U256::from(1_000_000_000u64),
        epoch_info: None,
    };

    if with_seals {
        // Create aggregated seals
        let keys: Vec<_> = (0..4).map(|_| SecretKey::random()).collect();
        let message = b"benchmark seal message";
        let signatures: Vec<_> = keys.iter().map(|sk| sk.sign(message)).collect();
        let agg_sig = aggregate_signatures(&signatures).unwrap();

        let mut bitmap = SealerSet::new(4);
        for i in 0..4 {
            bitmap.set_sealer(i);
        }

        let seal = WbftAggregatedSeal::new(bitmap.clone(), agg_sig.to_bytes());
        extra.prepared_seal = Some(seal.clone());
        extra.committed_seal = Some(seal);
    }

    extra
}

/// Benchmark WbftExtra encoding
fn bench_extra_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("wbft_extra_encode");

    // Without seals
    let extra_no_seals = create_test_extra(false);
    group.bench_function("without_seals", |b| {
        b.iter(|| {
            let encoded = extra_no_seals.encode();
            black_box(encoded)
        })
    });

    // With seals
    let extra_with_seals = create_test_extra(true);
    group.bench_function("with_seals", |b| {
        b.iter(|| {
            let encoded = extra_with_seals.encode();
            black_box(encoded)
        })
    });

    group.finish();
}

/// Benchmark WbftExtra decoding
fn bench_extra_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("wbft_extra_decode");

    // Without seals
    let extra_no_seals = create_test_extra(false);
    let encoded_no_seals = extra_no_seals.encode();
    group.bench_function("without_seals", |b| {
        b.iter(|| {
            let decoded = WbftExtra::decode(&mut &encoded_no_seals[..]).unwrap();
            black_box(decoded)
        })
    });

    // With seals
    let extra_with_seals = create_test_extra(true);
    let encoded_with_seals = extra_with_seals.encode();
    group.bench_function("with_seals", |b| {
        b.iter(|| {
            let decoded = WbftExtra::decode(&mut &encoded_with_seals[..]).unwrap();
            black_box(decoded)
        })
    });

    group.finish();
}

/// Benchmark WbftExtra roundtrip (encode + decode)
fn bench_extra_roundtrip(c: &mut Criterion) {
    let extra = create_test_extra(true);

    c.bench_function("wbft_extra_roundtrip", |b| {
        b.iter(|| {
            let encoded = extra.encode();
            let decoded = WbftExtra::decode(&mut &encoded[..]).unwrap();
            black_box(decoded)
        })
    });
}

/// Benchmark RoundChange message creation
fn bench_round_change_creation(c: &mut Criterion) {
    let sk = SecretKey::random();
    let view = View { sequence: U256::from(100), round: U256::from(5) };
    let address = Address::from([1u8; 20]);
    let message = b"round change message";
    let signature = sk.sign(message);

    c.bench_function("round_change_new", |b| {
        b.iter(|| {
            let rc = RoundChange::new(
                black_box(view.clone()),
                black_box(address),
                black_box(signature.to_bytes()),
            );
            black_box(rc)
        })
    });
}

/// Benchmark RoundChange with prepared block
fn bench_round_change_with_prepared(c: &mut Criterion) {
    let sk = SecretKey::random();
    let view = View { sequence: U256::from(100), round: U256::from(5) };
    let prepared_view = View { sequence: U256::from(100), round: U256::from(4) };
    let address = Address::from([1u8; 20]);
    let prepared_digest = B256::from([0x42u8; 32]);
    let message = b"round change with prepared";
    let signature = sk.sign(message);

    c.bench_function("round_change_with_prepared", |b| {
        b.iter(|| {
            let rc = RoundChange::with_prepared(
                black_box(view.clone()),
                black_box(address),
                black_box(signature.to_bytes()),
                black_box(prepared_digest),
                black_box(prepared_view.clone()),
            );
            black_box(rc)
        })
    });
}

/// Benchmark View operations
fn bench_view_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("view_operations");

    let view1 = View { sequence: U256::from(100), round: U256::from(5) };
    let view2 = View { sequence: U256::from(100), round: U256::from(6) };

    group.bench_function("compare", |b| {
        b.iter(|| {
            let result = view1.cmp(black_box(&view2));
            black_box(result)
        })
    });

    group.bench_function("next_round", |b| {
        b.iter(|| {
            let next = view1.next_round();
            black_box(next)
        })
    });

    group.bench_function("next_sequence", |b| {
        b.iter(|| {
            let next = view1.next_sequence();
            black_box(next)
        })
    });

    group.finish();
}

/// Benchmark WbftAggregatedSeal operations
fn bench_aggregated_seal(c: &mut Criterion) {
    let mut group = c.benchmark_group("aggregated_seal");

    // Create seal
    let keys: Vec<_> = (0..7).map(|_| SecretKey::random()).collect();
    let message = b"benchmark aggregated seal";
    let signatures: Vec<_> = keys.iter().map(|sk| sk.sign(message)).collect();
    let agg_sig = aggregate_signatures(&signatures).unwrap();

    let mut bitmap = SealerSet::new(7);
    for i in 0..7 {
        bitmap.set_sealer(i);
    }

    group.bench_function("new", |b| {
        b.iter(|| {
            let seal =
                WbftAggregatedSeal::new(black_box(bitmap.clone()), black_box(agg_sig.to_bytes()));
            black_box(seal)
        })
    });

    let seal = WbftAggregatedSeal::new(bitmap, agg_sig.to_bytes());

    group.bench_function("signer_count", |b| {
        b.iter(|| {
            let count = seal.signer_count();
            black_box(count)
        })
    });

    // Verify with all public keys
    let public_keys: Vec<_> = keys.iter().map(|sk| sk.public_key()).collect();
    group.bench_function("verify_7_signers", |b| {
        b.iter(|| {
            let result = seal.verify(black_box(&public_keys), black_box(message));
            black_box(result)
        })
    });

    group.finish();
}

/// Benchmark multiple message processing (throughput)
fn bench_message_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_throughput");

    // Create many extra data instances
    let extras: Vec<_> = (0..100).map(|_| create_test_extra(true)).collect();
    let encoded: Vec<_> = extras.iter().map(|e| e.encode()).collect();

    group.bench_function("encode_100_extras", |b| {
        b.iter(|| {
            let results: Vec<_> = extras.iter().map(|e| e.encode()).collect();
            black_box(results)
        })
    });

    group.bench_function("decode_100_extras", |b| {
        b.iter(|| {
            let results: Vec<_> =
                encoded.iter().map(|e| WbftExtra::decode(&mut &e[..]).unwrap()).collect();
            black_box(results)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_extra_encode,
    bench_extra_decode,
    bench_extra_roundtrip,
    bench_round_change_creation,
    bench_round_change_with_prepared,
    bench_view_operations,
    bench_aggregated_seal,
    bench_message_throughput,
);

criterion_main!(benches);
