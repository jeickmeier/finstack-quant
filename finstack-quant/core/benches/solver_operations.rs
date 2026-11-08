//! Checked root-finding and system-solver workloads.
//!
//! Fixture construction and independent numerical preflight run only inside the
//! selected Criterion case. Timings include the complete solve and its output.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::cashflow::{irr, xirr};
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::math::solver::BrentSolver;
use finstack_quant_core::math::solver_multi::{
    LevenbergMarquardtSolver, LmSolution, LmTerminationReason,
};
use std::hint::black_box;
use time::Month;

const LM_TOLERANCE: f64 = 1e-10;

struct DenseSystem {
    n_params: usize,
    n_residuals: usize,
    coeffs: Vec<f64>,
    targets: Vec<f64>,
    nonlinear: bool,
}

impl DenseSystem {
    fn full_rank(n_params: usize, n_residuals: usize) -> (Self, Vec<f64>) {
        assert!(n_residuals >= n_params);
        let expected: Vec<f64> = (0..n_params).map(|j| 0.02 + 0.001 * j as f64).collect();
        let mut coeffs = Vec::with_capacity(n_params * n_residuals);
        for i in 0..n_residuals {
            for j in 0..n_params {
                let dense = 0.1 / n_params as f64 * (((i + 1) * (j + 1)) as f64 * 0.314159).sin();
                coeffs.push(if i % n_params == j {
                    1.0 + dense
                } else {
                    dense
                });
            }
        }
        // The leading square block is strictly diagonally dominant, so the
        // Jacobian has full column rank near the known positive solution.
        for i in 0..n_params {
            let row = &coeffs[i * n_params..(i + 1) * n_params];
            let off_diagonal: f64 = row
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, value)| value.abs())
                .sum();
            assert!(row[i].abs() > off_diagonal);
        }
        let targets = coeffs
            .chunks_exact(n_params)
            .map(|row| {
                row.iter()
                    .zip(&expected)
                    .map(|(a, p)| a * (p + 0.25 * p * p))
                    .sum()
            })
            .collect();
        (
            Self {
                n_params,
                n_residuals,
                coeffs,
                targets,
                nonlinear: true,
            },
            expected,
        )
    }

    fn rank_one(n_params: usize, n_residuals: usize) -> Self {
        let mut coeffs = Vec::with_capacity(n_params * n_residuals);
        for i in 0..n_residuals {
            for j in 0..n_params {
                coeffs.push(
                    ((i as f64 + 1.0) * 0.314159).sin() * ((j as f64 + 1.0) * 0.271828).cos(),
                );
            }
        }
        Self {
            n_params,
            n_residuals,
            coeffs,
            targets: (0..n_residuals).map(|i| (i as f64 + 1.0) * 1e-3).collect(),
            nonlinear: false,
        }
    }

    fn residuals(&self, params: &[f64], resid: &mut [f64]) {
        assert_eq!(params.len(), self.n_params);
        assert_eq!(resid.len(), self.n_residuals);
        for (i, out) in resid.iter_mut().enumerate() {
            let row = &self.coeffs[i * self.n_params..(i + 1) * self.n_params];
            *out = -self.targets[i]
                + row
                    .iter()
                    .zip(params)
                    .map(|(a, p)| {
                        let transformed = if self.nonlinear { p + 0.25 * p * p } else { *p };
                        a * transformed
                    })
                    .sum::<f64>();
        }
    }
}

fn date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).expect("valid fixture date")
}

fn xirr_flows(complex: bool) -> Vec<(Date, f64)> {
    if complex {
        vec![
            (date(2023, Month::January, 15), -50_000.0),
            (date(2023, Month::March, 31), -30_000.0),
            (date(2023, Month::June, 15), 10_000.0),
            (date(2023, Month::September, 30), 15_000.0),
            (date(2023, Month::December, 31), 20_000.0),
            (date(2024, Month::June, 15), 45_000.0),
        ]
    } else {
        vec![
            (date(2024, Month::January, 1), -100_000.0),
            (date(2025, Month::January, 1), 110_000.0),
        ]
    }
}

fn dated_npv(flows: &[(Date, f64)], basis: f64, rate: f64) -> f64 {
    let start = flows[0].0;
    flows
        .iter()
        .map(|(on, amount)| amount / (1.0 + rate).powf((*on - start).whole_days() as f64 / basis))
        .sum()
}

// Deliberately independent, fixed-work bisection for these monotone fixtures;
// this reference never uses the production IRR solver or day-count dispatch.
fn reference_root(objective: impl Fn(f64) -> f64) -> f64 {
    let mut lo = 0.0;
    let mut hi = 1.0;
    assert!(objective(lo) > 0.0);
    assert!(objective(hi) < 0.0);
    for _ in 0..100 {
        let mid = (lo + hi) * 0.5;
        if objective(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) * 0.5
}

fn check_root(root: f64, expected: f64, residual: f64, scale: f64) {
    assert!(root.is_finite() && root > -1.0);
    assert!(
        (root - expected).abs() <= 1e-9,
        "root={root}, expected={expected}"
    );
    assert!(residual.is_finite() && residual.abs() <= scale * 1e-9);
}

fn checked_lm(system: &DenseSystem, solution: &LmSolution, expected: LmTerminationReason) {
    assert_eq!(solution.params.len(), system.n_params);
    assert!(solution.params.iter().all(|value| value.is_finite()));
    assert_eq!(solution.stats.termination_reason, expected);
    assert!(solution.stats.final_residual_norm.is_finite());
    assert!(solution.stats.final_step_norm.is_finite());
    assert!(solution.stats.lambda_final.is_finite() && solution.stats.lambda_final > 0.0);
    assert!(solution.stats.iterations > 0);
    assert!(solution.stats.jacobian_evals > 0);
    assert!(
        solution.stats.residual_evals
            >= 1 + 2 * system.n_params * solution.stats.jacobian_evals + solution.stats.iterations
    );
    let mut residuals = vec![0.0; system.n_residuals];
    system.residuals(&solution.params, &mut residuals);
    let norm = residuals.iter().map(|r| r * r).sum::<f64>().sqrt();
    assert!(norm.is_finite());
    assert!((norm - solution.stats.final_residual_norm).abs() <= 1e-12);
}

fn benchmark_xirr_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("xirr");
    for (label, flow_label, complex) in [("simple", "2_flows", false), ("complex", "6_flows", true)]
    {
        group.bench_function(BenchmarkId::new(label, flow_label), |b| {
            let flows = xirr_flows(complex);
            assert_eq!(flows.len(), if complex { 6 } else { 2 });
            let objective = |rate| dated_npv(&flows, 365.0, rate);
            let expected = reference_root(objective);
            let checked = xirr(&flows, DayCount::Act365F, None).expect("XIRR preflight converges");
            check_root(
                checked,
                expected,
                objective(checked),
                flows.iter().map(|(_, amount)| amount.abs()).sum(),
            );
            b.iter(|| {
                let root =
                    xirr(black_box(&flows), DayCount::Act365F, None).expect("XIRR converges");
                assert!(root.is_finite());
                black_box(root)
            });
        });
    }
    group.finish();
}

fn benchmark_xirr_daycount_variants(c: &mut Criterion) {
    let mut group = c.benchmark_group("xirr_daycount");
    for (label, day_count, basis) in [
        ("act365f", DayCount::Act365F, 365.0),
        ("act360", DayCount::Act360, 360.0),
    ] {
        group.bench_function(label, |b| {
            let flows = vec![
                (date(2024, Month::January, 1), -100_000.0),
                (date(2024, Month::July, 1), 102_500.0),
            ];
            let days = (flows[1].0 - flows[0].0).whole_days() as f64;
            assert_eq!(days, 182.0);
            let expected = (102_500.0_f64 / 100_000.0).powf(basis / days) - 1.0;
            let checked =
                xirr(&flows, day_count, None).expect("XIRR day-count preflight converges");
            check_root(
                checked,
                expected,
                dated_npv(&flows, basis, checked),
                202_500.0,
            );
            b.iter(|| {
                let root =
                    xirr(black_box(&flows), black_box(day_count), None).expect("XIRR converges");
                assert!(root.is_finite());
                black_box(root)
            });
        });
    }
    group.finish();
}

fn benchmark_brent(c: &mut Criterion) {
    let mut group = c.benchmark_group("solver_comparison");
    group.bench_function("brent", |b| {
        let f = |x: f64| x * x - 2.0;
        let solver = BrentSolver::new();
        let checked = solver.solve(f, 1.0).expect("Brent preflight converges");
        check_root(checked, 2.0_f64.sqrt(), f(checked), 2.0);
        b.iter(|| {
            let root = solver
                .solve(black_box(&f), black_box(1.0))
                .expect("Brent converges");
            assert!(root.is_finite());
            black_box(root)
        });
    });
    group.finish();
}

fn benchmark_irr_periodic(c: &mut Criterion) {
    let mut group = c.benchmark_group("irr_periodic");
    for (label, flow_label, complex) in [
        ("simple", "2_periods", false),
        ("complex", "5_periods", true),
    ] {
        group.bench_function(BenchmarkId::new(label, flow_label), |b| {
            let amounts = if complex {
                vec![-1000.0, 300.0, 300.0, 300.0, 300.0]
            } else {
                vec![-100.0, 110.0]
            };
            assert_eq!(amounts.len(), if complex { 5 } else { 2 });
            let objective = |rate: f64| {
                amounts
                    .iter()
                    .enumerate()
                    .map(|(i, amount)| amount / (1.0 + rate).powi(i as i32))
                    .sum()
            };
            let expected = reference_root(objective);
            let checked = irr(&amounts, None).expect("IRR preflight converges");
            check_root(
                checked,
                expected,
                objective(checked),
                amounts.iter().map(|amount| amount.abs()).sum(),
            );
            b.iter(|| {
                let root = irr(black_box(&amounts), None).expect("IRR converges");
                assert!(root.is_finite());
                black_box(root)
            });
        });
    }
    group.finish();
}

fn benchmark_lm_global_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("lm_global");
    group.bench_function(BenchmarkId::new("nonlinear_full_rank", "100x50"), |b| {
        let (system, expected) = DenseSystem::full_rank(50, 100);
        let initial = vec![0.01; system.n_params];
        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(LM_TOLERANCE)
            .with_max_iterations(200);
        let solve = || {
            solver
                .solve_system(
                    |p, r| system.residuals(p, r),
                    black_box(&initial),
                    system.n_residuals,
                )
                .expect("full-rank LM solve succeeds")
        };
        let checked = solve();
        checked_lm(
            &system,
            &checked,
            LmTerminationReason::ConvergedResidualNorm,
        );
        assert!(checked.stats.final_residual_norm < LM_TOLERANCE);
        for (actual, expected) in checked.params.iter().zip(&expected) {
            assert!((actual - expected).abs() < 1e-8);
        }
        drop(checked);
        b.iter(|| {
            let solution = solve();
            assert_eq!(
                solution.stats.termination_reason,
                LmTerminationReason::ConvergedResidualNorm
            );
            assert!(solution.stats.final_residual_norm < LM_TOLERANCE);
            black_box(solution)
        });
    });
    group.bench_function(
        BenchmarkId::new("termination", "rank_one_max1_100x50"),
        |b| {
            // Retains the former separable coefficient fixture as an explicit
            // one-trial nonconvergence control, not a claimed dense successful fit.
            let system = DenseSystem::rank_one(50, 100);
            let initial = vec![0.01; system.n_params];
            let solver = LevenbergMarquardtSolver::new()
                .with_tolerance(LM_TOLERANCE)
                .with_max_iterations(1);
            let solve = || {
                solver
                    .solve_system(
                        |p, r| system.residuals(p, r),
                        black_box(&initial),
                        system.n_residuals,
                    )
                    .expect("rank-one termination control returns diagnostics")
            };
            let checked = solve();
            checked_lm(&system, &checked, LmTerminationReason::MaxIterations);
            assert_eq!(checked.stats.iterations, 1);
            assert_eq!(checked.stats.jacobian_evals, 1);
            assert_eq!(checked.stats.residual_evals, 102);
            assert!(checked.stats.final_residual_norm > 1e-6);
            drop(checked);
            b.iter(|| {
                let solution = solve();
                assert_eq!(
                    solution.stats.termination_reason,
                    LmTerminationReason::MaxIterations
                );
                assert!(solution.stats.final_residual_norm.is_finite());
                black_box(solution)
            });
        },
    );
    group.finish();
}

criterion_group!(
    benches,
    benchmark_xirr_performance,
    benchmark_xirr_daycount_variants,
    benchmark_brent,
    benchmark_irr_periodic,
    benchmark_lm_global_sizes,
);
criterion_main!(benches);
