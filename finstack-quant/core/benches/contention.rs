//! Multi-threaded contention benchmarks.
//!
//! Measures lock-contention scaling on the documented hot-path mutex
//! sites identified by the production-readiness audit:
//!
//! - `money::fx::matrix::FxMatrix::{quotes, observed_quotes}`
//!   (two `Mutex<LruCache>` per matrix instance)
//! - `expr::eval::CompiledExpr::scratch` (`Mutex<ScratchArena>`)
//!
//! Each bench measures throughput at N = 1, 2, 4, 8 worker threads against
//! a single shared instance. The expectation is that throughput plateaus
//! (or regresses) at higher thread counts when contention dominates;
//! that plateau is the signal to either shard the cache or recommend
//! per-thread cloning explicitly in the docs.
//!
//! These benches are intentionally simple — they do not attempt to model
//! a realistic workload mix. Their job is to prove or disprove the
//! contention hypothesis cheaply.

use std::hint::black_box;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::fx::{FxMatrix, FxProvider, FxQuery};

// ---------------------------------------------------------------------------
// FX matrix contention
// ---------------------------------------------------------------------------

/// Constant-rate provider so the bench measures cache-mutex cost,
/// not provider cost.
struct ConstFx;
impl FxProvider for ConstFx {
    fn rate(
        &self,
        _from: Currency,
        _to: Currency,
        _on: Date,
        _policy: finstack_quant_core::money::fx::FxConversionPolicy,
    ) -> finstack_quant_core::Result<f64> {
        Ok(1.10)
    }
}

fn bench_fx_matrix_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("fx_matrix_contention");
    let on = Date::from_calendar_date(2025, time::Month::January, 15).expect("valid date");
    let pairs: Vec<(Currency, Currency)> = vec![
        (Currency::USD, Currency::EUR),
        (Currency::USD, Currency::GBP),
        (Currency::USD, Currency::JPY),
        (Currency::EUR, Currency::GBP),
        (Currency::EUR, Currency::JPY),
        (Currency::GBP, Currency::JPY),
    ];

    for &threads in &[1usize, 2, 4, 8] {
        // Workload: 4096 lookups per thread per iteration.
        // Total work = threads * 4096 lookups; throughput is normalized to
        // total lookups so the y-axis reads as "lookups / second".
        let lookups_per_thread = 4096usize;
        group.throughput(Throughput::Elements((threads * lookups_per_thread) as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(threads),
            &threads,
            |b, &threads| {
                let matrix = Arc::new(FxMatrix::new(Arc::new(ConstFx)));
                b.iter_custom(|iters| {
                    let start = Instant::now();
                    for _ in 0..iters {
                        thread::scope(|s| {
                            for tid in 0..threads {
                                let m = Arc::clone(&matrix);
                                let pairs = pairs.clone();
                                s.spawn(move || {
                                    for i in 0..lookups_per_thread {
                                        let (from, to) = pairs[(i + tid) % pairs.len()];
                                        let q = FxQuery::new(from, to, on);
                                        let _ = black_box(m.rate(q));
                                    }
                                });
                            }
                        });
                    }
                    start.elapsed()
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Expression scratch arena contention
// ---------------------------------------------------------------------------

fn bench_expr_scratch_contention(c: &mut Criterion) {
    use finstack_quant_core::expr::{BinOp, CompiledExpr, EvalOpts, Expr, SimpleContext};

    let mut group = c.benchmark_group("expr_scratch_contention");

    // Simple expression: x + y. Each eval() call goes through the
    // shared scratch Mutex, so this is a worst-case ratio of contention
    // to actual compute (cheap kernel, frequent lock).
    let expr = Expr::bin_op(BinOp::Add, Expr::column("x"), Expr::column("y"));
    let compiled: Arc<CompiledExpr> = Arc::new(CompiledExpr::new(expr));

    let n = 1024usize;
    let xs: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let ys: Vec<f64> = (0..n).map(|i| (i as f64) * 0.5).collect();
    let ctx: Arc<SimpleContext> = Arc::new(SimpleContext::new(["x", "y"]).expect("unique columns"));

    for &threads in &[1usize, 2, 4, 8] {
        let evals_per_thread = 256usize;
        group.throughput(Throughput::Elements((threads * evals_per_thread) as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(threads),
            &threads,
            |b, &threads| {
                b.iter_custom(|iters| {
                    let start = Instant::now();
                    for _ in 0..iters {
                        thread::scope(|s| {
                            for _ in 0..threads {
                                let compiled = Arc::clone(&compiled);
                                let ctx = Arc::clone(&ctx);
                                let xs_ref = xs.as_slice();
                                let ys_ref = ys.as_slice();
                                s.spawn(move || {
                                    let cols: [&[f64]; 2] = [xs_ref, ys_ref];
                                    for _ in 0..evals_per_thread {
                                        let _ = black_box(compiled.eval(
                                            &ctx,
                                            &cols,
                                            EvalOpts::default(),
                                        ));
                                    }
                                });
                            }
                        });
                    }
                    start.elapsed()
                });
            },
        );
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(Duration::from_secs(3));
    targets = bench_fx_matrix_contention, bench_expr_scratch_contention,
}
criterion_main!(benches);
