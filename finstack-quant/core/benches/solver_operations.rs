//! Benchmarks for root-finding solvers.
//!
//! Compares Newton solver with analytic derivatives vs finite differences,
//! and compares different solver strategies (Newton, Brent, Hybrid).

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::cashflow::{irr, xirr, xirr_with_daycount};
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::math::solver::{BrentSolver, NewtonSolver, Solver};
use finstack_quant_core::math::solver_multi::LevenbergMarquardtSolver;
use std::hint::black_box;
use time::Month;

#[derive(Clone)]
struct DenseSystem {
    n_params: usize,
    n_residuals: usize,
    coeffs: Vec<f64>,
    targets: Vec<f64>,
}

impl DenseSystem {
    fn new(n_params: usize, n_residuals: usize) -> Self {
        let mut coeffs = Vec::with_capacity(n_params * n_residuals);
        for i in 0..n_residuals {
            for j in 0..n_params {
                let val = ((i as f64 + 1.0) * 0.314159).sin() * ((j as f64 + 1.0) * 0.271828).cos();
                coeffs.push(val);
            }
        }
        let targets = (0..n_residuals).map(|i| (i as f64 + 1.0) * 1e-3).collect();
        Self {
            n_params,
            n_residuals,
            coeffs,
            targets,
        }
    }

    fn residuals(&self, params: &[f64], resid: &mut [f64]) {
        assert_eq!(params.len(), self.n_params);
        assert!(resid.len() >= self.n_residuals);

        for (i, resid_slot) in resid.iter_mut().enumerate().take(self.n_residuals) {
            let row_start = i * self.n_params;
            let row = &self.coeffs[row_start..row_start + self.n_params];
            let mut acc = -self.targets[i];
            acc += row
                .iter()
                .zip(params.iter())
                .map(|(a, b)| a * b)
                .sum::<f64>();
            *resid_slot = acc;
        }
    }
}

fn benchmark_newton_analytic_vs_fd(c: &mut Criterion) {
    let mut group = c.benchmark_group("newton_solver");

    // Test function: x^3 - 2x - 5 = 0
    let f = |x: f64| x.powi(3) - 2.0 * x - 5.0;
    let f_prime = |x: f64| 3.0 * x.powi(2) - 2.0;

    let solver = NewtonSolver::new();

    group.bench_function("finite_difference", |b| {
        b.iter(|| {
            solver
                .solve(black_box(&f), black_box(2.0))
                .expect("Should converge")
        })
    });

    group.bench_function("analytic_derivative", |b| {
        b.iter(|| {
            solver
                .solve_with_derivative(black_box(&f), black_box(&f_prime), black_box(2.0))
                .expect("Should converge")
        })
    });

    group.finish();
}

fn benchmark_xirr_performance(c: &mut Criterion) {
    // Create realistic XIRR test cases
    let simple_flows = vec![
        (
            Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"),
            -100_000.0,
        ),
        (
            Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            110_000.0,
        ),
    ];

    let complex_flows = vec![
        (
            Date::from_calendar_date(2023, Month::January, 15).expect("Valid date"),
            -50_000.0,
        ),
        (
            Date::from_calendar_date(2023, Month::March, 31).expect("Valid date"),
            -30_000.0,
        ),
        (
            Date::from_calendar_date(2023, Month::June, 15).expect("Valid date"),
            10_000.0,
        ),
        (
            Date::from_calendar_date(2023, Month::September, 30).expect("Valid date"),
            15_000.0,
        ),
        (
            Date::from_calendar_date(2023, Month::December, 31).expect("Valid date"),
            20_000.0,
        ),
        (
            Date::from_calendar_date(2024, Month::June, 15).expect("Valid date"),
            45_000.0,
        ),
    ];

    let mut group = c.benchmark_group("xirr");

    group.bench_with_input(
        BenchmarkId::new("simple", "2_flows"),
        &simple_flows,
        |b, flows| {
            b.iter(|| xirr(black_box(flows), None).expect("Should converge"));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("complex", "6_flows"),
        &complex_flows,
        |b, flows| {
            b.iter(|| xirr(black_box(flows), None).expect("Should converge"));
        },
    );

    group.finish();
}

fn benchmark_xirr_daycount_variants(c: &mut Criterion) {
    let flows = vec![
        (
            Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"),
            -100_000.0,
        ),
        (
            Date::from_calendar_date(2024, Month::July, 1).expect("Valid date"),
            102_500.0,
        ),
    ];

    let mut group = c.benchmark_group("xirr_daycount");

    group.bench_function("act365f", |b| {
        b.iter(|| {
            xirr_with_daycount(black_box(&flows), black_box(DayCount::Act365F), None)
                .expect("Should converge")
        })
    });

    group.bench_function("act360", |b| {
        b.iter(|| {
            xirr_with_daycount(black_box(&flows), black_box(DayCount::Act360), None)
                .expect("Should converge")
        })
    });

    group.finish();
}

fn benchmark_solver_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("solver_comparison");

    // Test function: x^2 - 2 = 0 (simple case where all methods work well)
    let f = |x: f64| x * x - 2.0;
    let f_prime = |x: f64| 2.0 * x;

    let newton = NewtonSolver::new();
    let brent = BrentSolver::new();

    group.bench_function("newton_fd", |b| {
        b.iter(|| {
            newton
                .solve(black_box(&f), black_box(1.0))
                .expect("Should converge")
        })
    });

    group.bench_function("newton_analytic", |b| {
        b.iter(|| {
            newton
                .solve_with_derivative(black_box(&f), black_box(&f_prime), black_box(1.0))
                .expect("Should converge")
        })
    });

    group.bench_function("brent", |b| {
        b.iter(|| {
            brent
                .solve(black_box(&f), black_box(1.0))
                .expect("Should converge")
        })
    });

    group.finish();
}

fn benchmark_irr_periodic(c: &mut Criterion) {
    let mut group = c.benchmark_group("irr_periodic");

    let simple_amounts = vec![-100.0, 110.0];
    let complex_amounts = vec![-1000.0, 300.0, 300.0, 300.0, 300.0];

    group.bench_with_input(
        BenchmarkId::new("simple", "2_periods"),
        &simple_amounts,
        |b, amounts| {
            b.iter(|| irr(black_box(amounts), None).expect("Should converge"));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("complex", "5_periods"),
        &complex_amounts,
        |b, amounts| {
            b.iter(|| irr(black_box(amounts), None).expect("Should converge"));
        },
    );

    group.finish();
}

fn benchmark_lm_global_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("lm_global");
    let cases = [("100x50", 50, 100)];

    for (label, n_params, n_residuals) in cases {
        let system = DenseSystem::new(n_params, n_residuals);
        let initial = vec![0.01; n_params];
        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(1e-10)
            .with_max_iterations(200);

        group.bench_function(BenchmarkId::new("lm_global", label), {
            let system = system.clone();
            let initial = initial.clone();
            let solver = solver.clone();
            move |b| {
                b.iter(|| {
                    let residuals =
                        |params: &[f64], resid: &mut [f64]| system.residuals(params, resid);
                    solver
                        .solve_system_with_dim_stats(residuals, &initial, n_residuals)
                        .expect("LM solve should succeed");
                });
            }
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_newton_analytic_vs_fd,
    benchmark_xirr_performance,
    benchmark_xirr_daycount_variants,
    benchmark_solver_comparison,
    benchmark_irr_periodic,
    benchmark_lm_global_sizes,
);
criterion_main!(benches);
