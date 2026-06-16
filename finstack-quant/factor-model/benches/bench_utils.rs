//! Shared Criterion helpers for factor-model benchmarks.

use criterion::{measurement::WallTime, BenchmarkGroup};

/// Simple helper to reduce repetitive `bench_function` + `iter` boilerplate.
pub fn bench_iter<F>(group: &mut BenchmarkGroup<WallTime>, id: impl Into<String>, mut f: F)
where
    F: FnMut(),
{
    let name = id.into();
    group.bench_function(&name, |b| b.iter(&mut f));
}
