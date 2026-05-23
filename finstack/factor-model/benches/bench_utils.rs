use criterion::{measurement::WallTime, BenchmarkGroup, Criterion};

/// Simple helper to reduce repetitive `bench_function` + `iter` boilerplate.
pub fn bench_iter<F>(group: &mut BenchmarkGroup<WallTime>, id: impl Into<String>, mut f: F)
where
    F: FnMut(),
{
    let name = id.into();
    group.bench_function(&name, |b| b.iter(&mut f));
}

/// Convenience wrapper for single-function benches without an explicit group.
#[allow(dead_code)]
pub fn bench_with_criterion<F>(c: &mut Criterion, name: impl Into<String>, mut f: F)
where
    F: FnMut(),
{
    let name = name.into();
    c.bench_function(&name, |b| b.iter(&mut f));
}
