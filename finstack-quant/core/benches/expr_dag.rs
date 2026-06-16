//! Benchmarks for DAG-based expression evaluation.
//!
//! Tests performance of complex expression graphs with shared sub-expressions
//! and arena-based allocation.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::expr::{BinOp, CompiledExpr, EvalOpts, Expr, Function, SimpleContext};
use std::hint::black_box;

/// Build a complex DAG with `n` interdependent nodes.
/// Creates expressions like: x + y, x * 2, (x + y) * x, etc.
fn build_complex_dag(n: usize) -> Expr {
    let mut exprs: Vec<Expr> = vec![Expr::column("x"), Expr::column("y")];

    for i in 0..n {
        let left_idx = i % exprs.len();
        let right_idx = (i + 1) % exprs.len();

        let new_expr = if i % 3 == 0 {
            Expr::bin_op(
                BinOp::Add,
                exprs[left_idx].clone(),
                exprs[right_idx].clone(),
            )
        } else if i % 3 == 1 {
            Expr::bin_op(BinOp::Mul, exprs[left_idx].clone(), Expr::literal(2.0))
        } else {
            Expr::call(
                Function::RollingSum,
                vec![exprs[left_idx].clone(), Expr::literal(3.0)],
            )
        };

        exprs.push(new_expr);
    }

    // Return complex expression combining many sub-expressions
    let mid = exprs.len() / 2;
    Expr::bin_op(BinOp::Add, exprs[mid].clone(), exprs[mid + 1].clone())
}

fn bench_dag_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_evaluation");

    // Set up test data
    let ctx = SimpleContext::new(["x", "y"]).expect("unique columns");
    let x: Vec<f64> = (0..100).map(|i| i as f64).collect();
    let y: Vec<f64> = (0..100).map(|i| (i as f64) * 0.5).collect();
    let cols: Vec<&[f64]> = vec![&x, &y];

    for size in [10, 50, 100] {
        group.bench_with_input(BenchmarkId::new("complex_dag", size), &size, |b, &size| {
            let expr = build_complex_dag(size);
            let compiled = CompiledExpr::new(expr);
            b.iter(|| {
                let result = compiled.eval(black_box(&ctx), black_box(&cols), EvalOpts::default());
                black_box(result.is_ok());
            })
        });
    }

    group.finish();
}

fn bench_dag_with_planning(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_with_planning");

    // Set up test data
    let ctx = SimpleContext::new(["x", "y"]).expect("unique columns");
    let x: Vec<f64> = (0..100).map(|i| i as f64).collect();
    let y: Vec<f64> = (0..100).map(|i| (i as f64) * 0.5).collect();
    let cols: Vec<&[f64]> = vec![&x, &y];

    for size in [10, 50, 100] {
        group.bench_with_input(BenchmarkId::new("with_plan", size), &size, |b, &size| {
            let expr = build_complex_dag(size);
            let meta = finstack_quant_core::config::results_meta(
                &finstack_quant_core::config::FinstackConfig::default(),
            );
            let compiled = CompiledExpr::with_planning(expr, meta).unwrap();
            b.iter(|| {
                let result = compiled.eval(black_box(&ctx), black_box(&cols), EvalOpts::default());
                black_box(result.is_ok());
            })
        });
    }

    group.finish();
}

fn bench_dag_cache_enabled(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_cache_enabled");

    // Set up test data
    let ctx = SimpleContext::new(["x", "y"]).expect("unique columns");
    let x: Vec<f64> = (0..100).map(|i| i as f64).collect();
    let y: Vec<f64> = (0..100).map(|i| (i as f64) * 0.5).collect();
    let cols: Vec<&[f64]> = vec![&x, &y];

    for size in [50, 100] {
        group.bench_with_input(BenchmarkId::new("with_cache", size), &size, |b, &size| {
            let expr = build_complex_dag(size);
            let meta = finstack_quant_core::config::results_meta(
                &finstack_quant_core::config::FinstackConfig::default(),
            );
            let compiled = CompiledExpr::with_planning(expr, meta)
                .unwrap()
                .with_cache(10);
            b.iter(|| {
                let result = compiled.eval(black_box(&ctx), black_box(&cols), EvalOpts::default());
                black_box(result.is_ok());
            })
        });
    }

    group.finish();
}

fn bench_dag_row_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_row_scaling");
    let expr = build_complex_dag(100);
    let meta = finstack_quant_core::config::results_meta(
        &finstack_quant_core::config::FinstackConfig::default(),
    );
    let compiled = CompiledExpr::with_planning(expr, meta)
        .unwrap()
        .with_cache(10);
    let ctx = SimpleContext::new(["x", "y"]).expect("unique columns");

    for rows in [100usize, 1_000, 10_000] {
        let x: Vec<f64> = (0..rows).map(|i| i as f64).collect();
        let y: Vec<f64> = (0..rows).map(|i| (i as f64) * 0.5).collect();
        let cols: Vec<&[f64]> = vec![&x, &y];

        group.bench_with_input(
            BenchmarkId::new("planned_cached_rows", rows),
            &rows,
            |b, _| {
                b.iter(|| {
                    let result = compiled
                        .eval(black_box(&ctx), black_box(&cols), EvalOpts::default())
                        .unwrap();
                    black_box(result);
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_dag_evaluation,
    bench_dag_with_planning,
    bench_dag_cache_enabled,
    bench_dag_row_scaling,
);
criterion_main!(benches);
