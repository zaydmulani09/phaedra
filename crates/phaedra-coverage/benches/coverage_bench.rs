use criterion::{black_box, criterion_group, criterion_main, Criterion};
use phaedra_coverage::{CoverageMap, CoverageTracker};

fn bench_coverage_map_clear(c: &mut Criterion) {
    let mut map = CoverageMap::new();
    for i in 0..1000usize {
        map.data[i] = 1;
    }
    c.bench_function("coverage_map_clear_x10000", |b| {
        b.iter(|| {
            for _ in 0..10000 {
                black_box(&mut map).clear();
            }
        })
    });
}

fn bench_tracker_is_interesting_miss(c: &mut Criterion) {
    // worst case: tracker is empty, map has coverage → all edges are new
    let mut map = CoverageMap::new();
    for i in 0..1000usize {
        map.data[i] = 1;
    }
    c.bench_function("tracker_is_interesting_miss", |b| {
        b.iter(|| {
            let mut tracker = CoverageTracker::new();
            let _ = tracker.is_interesting(black_box(&map));
        })
    });
}

fn bench_tracker_is_interesting_hit(c: &mut Criterion) {
    // best case: all edges already seen, is_interesting returns false
    let mut map = CoverageMap::new();
    for i in 0..1000usize {
        map.data[i] = 1;
    }
    let mut tracker = CoverageTracker::new();
    tracker.is_interesting(&map); // pre-seed tracker with all edges
    c.bench_function("tracker_is_interesting_hit", |b| {
        b.iter(|| {
            let _ = tracker.is_interesting(black_box(&map));
        })
    });
}

fn bench_edge_count(c: &mut Criterion) {
    let mut map = CoverageMap::new();
    for i in 0..65536usize {
        map.data[i] = 1;
    }
    c.bench_function("edge_count_full_map", |b| {
        b.iter(|| {
            let _ = black_box(&map).edge_count();
        })
    });
}

criterion_group!(
    benches,
    bench_coverage_map_clear,
    bench_tracker_is_interesting_miss,
    bench_tracker_is_interesting_hit,
    bench_edge_count,
);
criterion_main!(benches);
