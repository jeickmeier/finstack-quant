//! Benchmarks for the expression engine evaluation hot path.
//!
//! `CompiledExpr::eval` is invoked once per period across statement models
//! (hundreds of interdependent formulas), so this measures the steady-state
//! per-eval cost over a multi-node DAG and many-row columns — the path that
//! reuses the pooled arena and node-id-indexed offsets.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::expr::{BinOp, CompiledExpr, EvalOpts, Expr, SimpleContext};
use std::hint::black_box;

/// Build a moderately deep arithmetic expression over `n_cols` columns:
/// `((c0 + c1) * (c2 - c3) + c4) / (c5 + 1) ...` chained to create a
/// multi-node DAG with shared sub-expressions.
fn build_expr(n_cols: usize) -> Expr {
    let mut acc = Expr::column("c0");
    for i in 1..n_cols {
        let col = Expr::column(format!("c{i}"));
        let term = match i % 4 {
            0 => Expr::bin_op(BinOp::Add, acc, col),
            1 => Expr::bin_op(BinOp::Sub, acc, col),
            2 => Expr::bin_op(BinOp::Mul, acc, col),
            _ => Expr::bin_op(
                BinOp::Add,
                acc,
                Expr::bin_op(BinOp::Mul, col, Expr::literal(0.5)),
            ),
        };
        acc = term;
    }
    acc
}

fn benchmark_eval(c: &mut Criterion) {
    let mut group = c.benchmark_group("expr_eval");

    // Representative statement-model shapes: a handful of columns, many periods.
    for &(n_cols, n_rows) in &[(8usize, 64usize), (16, 256), (30, 256)] {
        let names: Vec<String> = (0..n_cols).map(|i| format!("c{i}")).collect();
        let ctx = SimpleContext::new(names.clone()).expect("context");
        let data: Vec<Vec<f64>> = (0..n_cols)
            .map(|i| {
                (0..n_rows)
                    .map(|r| ((i + 1) as f64) * 0.1 + (r as f64) * 0.01)
                    .collect()
            })
            .collect();
        let cols: Vec<&[f64]> = data.iter().map(Vec::as_slice).collect();

        let compiled = CompiledExpr::new(build_expr(n_cols));
        // Warm the lazy plan so the benchmark measures steady-state eval, not
        // one-time DAG construction.
        let _ = compiled
            .eval(&ctx, &cols, EvalOpts::default())
            .expect("warmup eval");

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{n_cols}cols_{n_rows}rows")),
            &(n_cols, n_rows),
            |b, _| {
                b.iter(|| {
                    let res = compiled
                        .eval(black_box(&ctx), black_box(&cols), EvalOpts::default())
                        .expect("eval");
                    black_box(res.values.len())
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_eval);
criterion_main!(benches);
