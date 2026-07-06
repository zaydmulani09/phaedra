use criterion::{black_box, criterion_group, criterion_main, Criterion};
use phaedra_corpus::{fingerprint, CorpusManager};

fn bench_add_seed(c: &mut Criterion) {
    c.bench_function("add_seed_100_fresh_db", |b| {
        b.iter(|| {
            let tmp = tempfile::tempdir().unwrap();
            let db_path = tmp.path().join("bench.db");
            let mut mgr = CorpusManager::open(&db_path).unwrap();
            for i in 0..100u64 {
                let seed = format!("unique_seed_{i:020}").into_bytes();
                let _ = mgr.add_seed(seed, i as usize % 100, "bench");
            }
        })
    });
}

fn bench_pick_seed(c: &mut Criterion) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("bench.db");
    let mut mgr = CorpusManager::open(&db_path).unwrap();
    for i in 0..1000u64 {
        let seed = format!("unique_seed_{i:020}").into_bytes();
        let _ = mgr.add_seed(seed, i as usize % 100, "bench");
    }
    c.bench_function("pick_seed_1000corpus_x1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let _ = mgr.pick();
            }
        })
    });
}

fn bench_all_data(c: &mut Criterion) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("bench.db");
    let mut mgr = CorpusManager::open(&db_path).unwrap();
    for i in 0..500u64 {
        let seed = format!("unique_seed_{i:020}").into_bytes();
        let _ = mgr.add_seed(seed, 0, "bench");
    }
    c.bench_function("all_data_500seeds", |b| {
        b.iter(|| {
            let _ = black_box(mgr.all_data().unwrap());
        })
    });
}

fn bench_fingerprint(c: &mut Criterion) {
    let input = vec![0xAAu8; 256];
    c.bench_function("fingerprint_256_x10000", |b| {
        b.iter(|| {
            for _ in 0..10000 {
                let _ = fingerprint(black_box(&input));
            }
        })
    });
}

criterion_group!(
    benches,
    bench_add_seed,
    bench_pick_seed,
    bench_all_data,
    bench_fingerprint,
);
criterion_main!(benches);
