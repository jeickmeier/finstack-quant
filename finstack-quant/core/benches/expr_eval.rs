//! Compile-excluded evaluation of an arithmetic expression over prepared columns.
//!
//! The measured call includes the evaluator's arena/offset allocations, kernels,
//! root-result copy and output destruction. These synthetic many-row cases do
//! not represent the statements crate's separate period-aware evaluator.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::expr::{BinOp, CompiledExpr, Expr, SimpleContext};
use std::hint::black_box;

/// Build an arithmetic chain over distinct columns, sharing the 0.5 literal.
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

    for &(n_cols, n_rows) in &[(8usize, 64usize), (16, 256), (30, 256)] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{n_cols}cols_{n_rows}rows")),
            &(n_cols, n_rows),
            |b, _| {
                // Criterion selects this case before any fixture or preflight work.
                let names: Vec<String> = (0..n_cols).map(|i| format!("c{i}")).collect();
                let ctx = SimpleContext::new(names).expect("context");
                let data: Vec<Vec<f64>> = (0..n_cols)
                    .map(|i| {
                        (0..n_rows)
                            .map(|r| ((i + 1) as f64) * 0.1 + (r as f64) * 0.01)
                            .collect()
                    })
                    .collect();
                let cols: Vec<&[f64]> = data.iter().map(Vec::as_slice).collect();
                let compiled =
                    CompiledExpr::new(build_expr(n_cols)).expect("elementwise expression compiles");

                let checked = compiled.eval(&ctx, &cols).expect("preflight eval");
                assert_eq!(checked.len(), n_rows);
                for (row, &actual) in checked.iter().enumerate() {
                    // Scalar reference written independently of the AST/evaluator.
                    let expected = data.iter().enumerate().skip(1).fold(
                        data[0][row],
                        |acc, (column, values)| match column % 4 {
                            0 => acc + values[row],
                            1 => acc - values[row],
                            2 => acc * values[row],
                            _ => acc + values[row] * 0.5,
                        },
                    );
                    assert!(actual.is_finite());
                    assert_eq!(actual.to_bits(), expected.to_bits(), "row {row}");
                }
                drop(checked);

                b.iter(|| {
                    let res = compiled
                        .eval(black_box(&ctx), black_box(&cols))
                        .expect("eval");
                    black_box(res)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_eval);
criterion_main!(benches);
