//! Dense binomial probability mass evaluation with reusable output.
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::math::distributions::binomial_pmf_all_into;
use std::hint::black_box;

fn bench_binomial(c: &mut Criterion) {
    let mut group = c.benchmark_group("binomial_pmf");
    for (n, p) in [
        (25, 0.01),
        (125, 0.2),
        (1000, 0.5),
        (125, 1e-8),
        (125, 1.0 - 1e-8),
    ] {
        let id = format!("n{n}_p{p}");
        let mut pmf = Vec::new();
        group.bench_with_input(BenchmarkId::new("ordinary", &id), &(n, p), |b, &(n, p)| {
            b.iter(|| {
                binomial_pmf_all_into(&mut pmf, black_box(n), black_box(p)).expect("PMF");
                black_box(&pmf);
            });
        });
    }
    group.finish();
}
criterion_group!(benches, bench_binomial);
criterion_main!(benches);
