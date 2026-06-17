//! Benchmarks for rolling window operations.
//!
//! Tests performance of rolling window functions with optimized scratch buffer usage.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::expr::{CompiledExpr, EvalOpts, Expr, Function, SimpleContext};
use std::hint::black_box;

fn bench_rolling_median(c: &mut Criterion) {
    let mut group = c.benchmark_group("rolling_median");

    for data_size in [500] {
        {
            let window_size = 10;
            group.bench_with_input(
                BenchmarkId::new(format!("data_{}", data_size), window_size),
                &(data_size, window_size),
                |b, &(data_size, window_size)| {
                    let ctx = SimpleContext::new(["x"]).expect("unique columns");
                    let x: Vec<f64> = (0..data_size)
                        .map(|i| (i as f64) * 0.5 + i as f64 % 7_f64)
                        .collect();
                    let cols: Vec<&[f64]> = vec![&x];

                    let expr = CompiledExpr::new(Expr::call(
                        Function::RollingMedian,
                        vec![Expr::column("x"), Expr::literal(window_size as f64)],
                    ));

                    b.iter(|| {
                        let result =
                            expr.eval(black_box(&ctx), black_box(&cols), EvalOpts::default());
                        let _ = black_box(result);
                    })
                },
            );
        }
    }

    group.finish();
}

fn bench_rolling_mean(c: &mut Criterion) {
    let mut group = c.benchmark_group("rolling_mean");

    for data_size in [500] {
        {
            let window_size = 10;
            group.bench_with_input(
                BenchmarkId::new(format!("data_{}", data_size), window_size),
                &(data_size, window_size),
                |b, &(data_size, window_size)| {
                    let ctx = SimpleContext::new(["x"]).expect("unique columns");
                    let x: Vec<f64> = (0..data_size).map(|i| i as f64).collect();
                    let cols: Vec<&[f64]> = vec![&x];

                    let expr = CompiledExpr::new(Expr::call(
                        Function::RollingMean,
                        vec![Expr::column("x"), Expr::literal(window_size as f64)],
                    ));

                    b.iter(|| {
                        let result =
                            expr.eval(black_box(&ctx), black_box(&cols), EvalOpts::default());
                        let _ = black_box(result);
                    })
                },
            );
        }
    }

    group.finish();
}

fn bench_rolling_std(c: &mut Criterion) {
    let mut group = c.benchmark_group("rolling_std");

    {
        let data_size = 500;
        group.bench_with_input(
            BenchmarkId::from_parameter(data_size),
            &data_size,
            |b, &data_size| {
                let ctx = SimpleContext::new(["x"]).expect("unique columns");
                let x: Vec<f64> = (0..data_size).map(|i| (i as f64) * 0.5).collect();
                let cols: Vec<&[f64]> = vec![&x];

                let expr = CompiledExpr::new(Expr::call(
                    Function::RollingStd,
                    vec![Expr::column("x"), Expr::literal(10.0)],
                ));

                b.iter(|| {
                    let result = expr.eval(black_box(&ctx), black_box(&cols), EvalOpts::default());
                    let _ = black_box(result);
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_rolling_median,
    bench_rolling_mean,
    bench_rolling_std,
);
criterion_main!(benches);
