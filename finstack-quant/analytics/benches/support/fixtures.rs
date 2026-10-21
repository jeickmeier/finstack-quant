//! Deterministic fixtures shared by analytics Criterion targets.
//!
//! No RNG crate and no clock. A seeded generator is required: these inputs
//! feed the same code paths the correctness tests pin.

use finstack_quant_analytics::Performance;
use finstack_quant_core::dates::{Date, PeriodKind};
use time::Month;

/// Splitmix64-style iteration mapped into `(-0.02, 0.02)`.
pub fn synthetic_returns(n: usize, seed: u64) -> Vec<f64> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    (0..n)
        .map(|_| {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            let u = ((z ^ (z >> 31)) as f64) / (u64::MAX as f64);
            (u - 0.5) * 0.04
        })
        .collect()
}

/// Consecutive calendar days from 2020-01-01.
pub fn synthetic_dates(n: usize) -> Vec<Date> {
    let start = Date::from_calendar_date(2020, Month::January, 1).expect("valid");
    let mut dates = Vec::with_capacity(n);
    let mut d = start;
    for _ in 0..n {
        dates.push(d);
        d = d.next_day().expect("next day");
    }
    dates
}

/// Single-ticker daily `Performance` from synthetic returns.
pub fn perf_from_returns(n: usize, seed: u64) -> Performance {
    let returns = synthetic_returns(n, seed);
    let dates = synthetic_dates(n);
    Performance::from_returns(
        dates,
        vec![returns],
        vec!["X".to_string()],
        None,
        PeriodKind::Daily,
    )
    .expect("performance from returns")
}

/// Multi-ticker panel. Column 0 is the benchmark.
pub fn perf_panel(n_obs: usize, n_tickers: usize, seed: u64) -> Performance {
    assert!(n_tickers >= 2, "panel must have at least benchmark + 1");
    let dates = synthetic_dates(n_obs);
    let mut columns: Vec<Vec<f64>> = Vec::with_capacity(n_tickers);
    let mut names: Vec<String> = Vec::with_capacity(n_tickers);
    for i in 0..n_tickers {
        columns.push(synthetic_returns(n_obs, seed.wrapping_add(i as u64)));
        names.push(format!("T{i}"));
    }
    Performance::from_returns(dates, columns, names, Some("T0"), PeriodKind::Daily)
        .expect("performance panel")
}

/// Symmetric unit-diagonal matrix that is indefinite for every `n >= 3`.
///
/// The decaying off-diagonal kernel used by the crate's Higham unit test is
/// actually positive definite (smallest eigenvalue ≈ 0.20 at `n = 50`). This
/// fixture keeps that bulk and overwrites the leading 3×3 with the classic
/// indefinite correlation-shaped block so Cholesky fails and Higham iterates.
#[allow(dead_code)]
pub fn near_correlation_needs_repair(n: usize) -> Vec<f64> {
    let mut input = vec![0.0; n * n];
    for i in 0..n {
        input[i * n + i] = 1.0;
        for j in (i + 1)..n {
            let rho = 0.2 + 0.6 * ((i as f64 + 1.0) / (j as f64 + 1.0));
            let rho = rho.clamp(-0.9, 0.9);
            input[i * n + j] = rho;
            input[j * n + i] = rho;
        }
    }
    if n >= 3 {
        // Leading 3×3 in row-major layout: (0,1)/(1,0)/(0,2)/(2,0) = 0.9,
        // (1,2)/(2,1) = −0.9.
        input[1] = 0.9;
        input[n] = 0.9;
        input[2] = 0.9;
        input[2 * n] = 0.9;
        input[n + 2] = -0.9;
        input[2 * n + 1] = -0.9;
    }
    input
}

/// Full-rank exposure panel plus returns/weights for constrained LS.
#[allow(dead_code)]
pub fn constrained_ls_inputs(n_assets: usize, n_factors: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut exposures = Vec::with_capacity(n_assets * n_factors);
    let mut returns = Vec::with_capacity(n_assets);
    let mut weights = Vec::with_capacity(n_assets);
    let w = 1.0 / n_assets as f64;
    for i in 0..n_assets {
        for j in 0..n_factors {
            let base = if i % n_factors == j { 1.0 } else { 0.05 };
            exposures.push(base + 0.01 * ((i * 17 + j * 13) % 7) as f64);
        }
        returns.push(0.01 + 0.001 * (i % 11) as f64);
        weights.push(w);
    }
    (exposures, returns, weights)
}
