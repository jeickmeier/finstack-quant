//! Hot-path benchmarks for `finstack-quant-monte-carlo`.
//!
//! Covers the highest-iteration workloads:
//!
//! - European option pricing (GBM + exact discretization)
//! - LSMC backward induction for American options
//! - LSQ regression (SVD solve per exercise date)
//!
//! Run with:
//! ```sh
//! cargo bench -p finstack-quant-monte-carlo
//! ```

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_monte_carlo::payoff::vanilla::EuropeanCall;
use finstack_quant_monte_carlo::pricer::basis::PolynomialBasis;
use finstack_quant_monte_carlo::pricer::european::EuropeanPricer;
use finstack_quant_monte_carlo::pricer::lsmc::{AmericanPut, LsmcConfig, LsmcPricer};
use finstack_quant_monte_carlo::pricer::lsq::solve_least_squares;
use finstack_quant_monte_carlo::process::gbm::GbmProcess;

// ---------------------------------------------------------------------------
// European pricer: GBM + ExactGbm at various path counts
// ---------------------------------------------------------------------------

fn bench_european_pricer(c: &mut Criterion) {
    let mut group = c.benchmark_group("european_pricer");
    let process = GbmProcess::with_params(0.05, 0.02, 0.20).unwrap();
    let payoff = EuropeanCall::new(100.0, 1.0, 252);
    let df = (-0.05_f64).exp();

    {
        let &num_paths = &10_000;
        group.bench_with_input(BenchmarkId::new("paths", num_paths), &num_paths, |b, &n| {
            let pricer = EuropeanPricer::new(n).with_seed(42).with_parallel(false);
            b.iter(|| {
                pricer
                    .price(&process, 100.0, 1.0, 252, &payoff, Currency::USD, df)
                    .expect("pricing should succeed")
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// LSMC: American put backward induction at various path counts
// ---------------------------------------------------------------------------

fn bench_lsmc_pricer(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsmc_pricer");
    let process = GbmProcess::with_params(0.05, 0.02, 0.20).unwrap();
    let exercise = AmericanPut::new(100.0).expect("valid strike");
    let basis = PolynomialBasis::new(2);

    // Monthly exercise dates over 1 year (12 steps, 12 exercise opportunities)
    let num_steps = 12;
    let exercise_dates: Vec<usize> = (1..=num_steps).collect();

    {
        let &num_paths = &5_000;
        group.bench_with_input(BenchmarkId::new("paths", num_paths), &num_paths, |b, &n| {
            let config = LsmcConfig::new(n, exercise_dates.clone(), num_steps)
                .expect("valid LSMC config")
                .with_seed(42);
            let pricer = LsmcPricer::new(config);
            b.iter(|| {
                pricer
                    .price(
                        &process,
                        100.0,
                        1.0,
                        num_steps,
                        &exercise,
                        &basis,
                        Currency::USD,
                        0.05,
                    )
                    .expect("pricing should succeed")
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// LSQ regression: SVD solve at various observation counts
// ---------------------------------------------------------------------------

fn bench_lsq_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsq_regression");
    let k = 3; // cubic basis: {1, x, x^2}

    for &n in &[500] {
        // Build a deterministic design matrix and response vector
        let mut design = vec![0.0; n * k];
        let mut y = vec![0.0; n];
        for i in 0..n {
            let x = (i as f64) / (n as f64);
            design[i * k] = 1.0;
            design[i * k + 1] = x;
            design[i * k + 2] = x * x;
            y[i] = 1.0 + 2.0 * x + 3.0 * x * x + 0.01 * (i as f64);
        }

        group.bench_with_input(BenchmarkId::new("observations", n), &n, |b, _| {
            b.iter(|| solve_least_squares(&design, &y, n, k).expect("should succeed"));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_european_pricer,
    bench_lsmc_pricer,
    bench_lsq_regression
);
criterion_main!(benches);
