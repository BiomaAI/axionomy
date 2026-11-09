use axionomy_tree::Tree;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn key(n: u64) -> Vec<u8> {
    format!("accounts/{n:08}\0assets/cash").into_bytes()
}

fn value(n: u64) -> [u8; 32] {
    let mut value = [0; 32];
    value[24..].copy_from_slice(&n.to_be_bytes());
    value
}

fn benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree");
    for entries in [1_000u64, 100_000] {
        let tree: Tree = (0..entries).map(|n| (key(n), value(n))).collect();
        let middle = key(entries / 2);
        group.bench_with_input(BenchmarkId::new("set_value", entries), &tree, |b, tree| {
            b.iter(|| {
                let mut tree = tree.clone();
                tree.insert(middle.as_slice(), &value(7)[..]);
                black_box(tree)
            });
        });
        group.bench_with_input(
            BenchmarkId::new("set_value_and_hash", entries),
            &tree,
            |b, tree| {
                tree.hash();
                b.iter(|| {
                    let mut tree = tree.clone();
                    tree.insert(middle.as_slice(), &value(7)[..]);
                    black_box(tree.hash())
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("build", entries),
            &entries,
            |b, &entries| {
                b.iter(|| black_box((0..entries).map(|n| (key(n), value(n))).collect::<Tree>()));
            },
        );
    }
    group.finish();
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
