use criterion::{black_box, criterion_group, criterion_main, Criterion};
use phaedra_mutator::strategies;
use phaedra_mutator::MutationEngine;
use rand::SeedableRng;

fn bench_bit_flip(c: &mut Criterion) {
    let input = vec![0xAAu8; 256];
    let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
    c.bench_function("bit_flip_256x1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let _ = strategies::bit_flip(black_box(&input), &mut rng);
            }
        })
    });
}

fn bench_block_substitute(c: &mut Criterion) {
    let input = vec![0xAAu8; 256];
    let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
    c.bench_function("block_substitute_256x1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let _ = strategies::block_substitute(black_box(&input), &mut rng);
            }
        })
    });
}

fn bench_havoc(c: &mut Criterion) {
    let input = vec![0xAAu8; 256];
    let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
    c.bench_function("havoc_256x1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let _ = strategies::havoc(black_box(&input), &mut rng);
            }
        })
    });
}

fn bench_recombine(c: &mut Criterion) {
    let input_a = vec![0xAAu8; 256];
    let input_b = vec![0xBBu8; 256];
    let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
    c.bench_function("recombine_256x1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let _ = strategies::recombine(black_box(&input_a), black_box(&input_b), &mut rng);
            }
        })
    });
}

fn bench_engine_mutate(c: &mut Criterion) {
    let input = vec![0u8; 256];
    let corpus: Vec<Vec<u8>> = (0..10).map(|i| vec![i as u8; 64]).collect();
    let mut engine = MutationEngine::with_seed(42);
    c.bench_function("engine_mutate_10seeds_x1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let _ = engine.mutate(black_box(&input), black_box(&corpus));
            }
        })
    });
}

fn bench_engine_mutate_with_tokens(c: &mut Criterion) {
    let input = vec![0u8; 256];
    let corpus: Vec<Vec<u8>> = (0..10).map(|i| vec![i as u8; 64]).collect();
    let mut engine = MutationEngine::with_seed(42);
    for i in 0..20u8 {
        engine.add_token(vec![i; 4]);
    }
    c.bench_function("engine_mutate_20tokens_x1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let _ = engine.mutate(black_box(&input), black_box(&corpus));
            }
        })
    });
}

criterion_group!(
    benches,
    bench_bit_flip,
    bench_block_substitute,
    bench_havoc,
    bench_recombine,
    bench_engine_mutate,
    bench_engine_mutate_with_tokens,
);
criterion_main!(benches);
