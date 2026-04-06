use chiff::{analyze_directory_pair, apply_patch, diff_bytes, Patch};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone)]
struct BenchPair {
    name: String,
    old: Vec<u8>,
    new: Vec<u8>,
    patch: Patch,
}

fn load_mixed_baseline_pairs() -> Vec<BenchPair> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let old_root = root.join("fixtures/corpus/mixed-baseline/old");
    let new_root = root.join("fixtures/corpus/mixed-baseline/new");
    let corpus = analyze_directory_pair(&old_root, &new_root).expect("mixed corpus should load");

    corpus
        .entries
        .into_iter()
        .filter(|entry| entry.status.as_str() == "paired")
        .map(|entry| {
            let old = fs::read(old_root.join(&entry.relative_path)).expect("old file should read");
            let new = fs::read(new_root.join(&entry.relative_path)).expect("new file should read");
            let patch = diff_bytes(&old, &new);
            BenchPair {
                name: entry.relative_path.display().to_string(),
                old,
                new,
                patch,
            }
        })
        .collect()
}

fn bench_mixed_baseline(c: &mut Criterion) {
    let pairs = load_mixed_baseline_pairs();
    let mut diff_group = c.benchmark_group("diff/corpus-mixed-baseline");
    for pair in &pairs {
        diff_group.bench_with_input(BenchmarkId::from_parameter(&pair.name), pair, |b, pair| {
            b.iter(|| diff_bytes(black_box(&pair.old), black_box(&pair.new)))
        });
    }
    diff_group.finish();

    let mut apply_group = c.benchmark_group("apply/corpus-mixed-baseline");
    for pair in &pairs {
        apply_group.bench_with_input(BenchmarkId::from_parameter(&pair.name), pair, |b, pair| {
            b.iter(|| apply_patch(black_box(&pair.old), black_box(&pair.patch)).unwrap())
        });
    }
    apply_group.finish();
}

criterion_group!(benches, bench_mixed_baseline);
criterion_main!(benches);
