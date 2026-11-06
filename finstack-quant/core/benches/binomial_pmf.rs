//! Prepared and one-shot finite-binomial PMF costs, including a setup control.
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::math::distributions::{BinomialPmf, binomial_pmf_all_into};
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
        let prepared = BinomialPmf::new(n);
        let mut expected = Vec::new();
        let mut actual = Vec::new();
        binomial_pmf_all_into(&mut expected, n, p).expect("ordinary PMF");
        prepared.fill(&mut actual, p).expect("prepared PMF");
        assert_eq!(expected, actual);
        group.bench_with_input(BenchmarkId::new("ordinary", &id), &(n, p), |b, &(n, p)| {
            b.iter(|| {
                binomial_pmf_all_into(&mut expected, black_box(n), black_box(p)).expect("PMF");
                black_box(&expected);
            });
        });
        group.bench_with_input(BenchmarkId::new("prepared", &id), &p, |b, &p| {
            b.iter(|| {
                prepared.fill(&mut actual, black_box(p)).expect("PMF");
                black_box(&actual);
            });
        });
    }
    group.bench_function("prepare_and_fill_125", |b| {
        let mut out = Vec::new();
        b.iter(|| {
            let prepared = BinomialPmf::new(black_box(125));
            prepared.fill(&mut out, black_box(0.2)).expect("PMF");
            black_box(&out);
        });
    });
    group.finish();
}
criterion_group!(benches, bench_binomial);
criterion_main!(benches);
