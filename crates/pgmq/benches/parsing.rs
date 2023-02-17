use criterion::{black_box, criterion_group, criterion_main, Criterion};

use criterion::BenchmarkId;
use pgmq::query::check_input;

fn check_bytes(input: &str) -> bool {}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("check bytes", |b| {
        b.iter(|| check_bytes(black_box("myqueue_123_longername")))
    });
    c.bench_function("check regex", |b| {
        b.iter(|| check_input(black_box("myqueue_123_longername")))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
