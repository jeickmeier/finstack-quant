//! Numeric fixture test pinning freestanding building-block outputs.
//!
//! Lives in `src/` (rather than `tests/`) so it can reach `pub(crate)`
//! building-block functions directly. Public-API consumers should go
//! through [`crate::performance::Performance`].

use crate::benchmark::{multi_factor_greeks, rolling_greeks};
use crate::dates::Date;
use crate::risk_metrics::{cagr, expected_shortfall, sharpe, sortino, value_at_risk, CagrBasis};
use serde::Deserialize;

const API_INVARIANTS_FIXTURE: &str = include_str!("api_invariants_data.json");

#[derive(Deserialize)]
struct Fixture {
    returns: Vec<f64>,
    benchmark: Vec<f64>,
    factors: Vec<Vec<f64>>,
    dates: Vec<Date>,
    expected: Expected,
}

#[derive(Deserialize)]
struct Expected {
    cagr_factor: f64,
    sharpe: f64,
    sortino: f64,
    value_at_risk: f64,
    expected_shortfall: f64,
    rolling_greeks: ExpectedRollingGreeks,
    multi_factor_greeks: ExpectedMultiFactorGreeks,
}

#[derive(Deserialize)]
struct ExpectedRollingGreeks {
    alphas: Vec<f64>,
    betas: Vec<f64>,
}

#[derive(Deserialize)]
struct ExpectedMultiFactorGreeks {
    alpha: f64,
    betas: Vec<f64>,
    r_squared: f64,
    adjusted_r_squared: f64,
    residual_vol: f64,
}

fn fixture() -> Fixture {
    serde_json::from_str(API_INVARIANTS_FIXTURE).expect("API invariants fixture should parse")
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-12,
        "actual={actual}, expected={expected}"
    );
}

fn assert_vec_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (&actual, &expected) in actual.iter().zip(expected.iter()) {
        assert_close(actual, expected);
    }
}

#[test]
fn rust_core_matches_api_invariants_fixture() {
    let fixture = fixture();
    let expected = &fixture.expected;

    assert_close(
        cagr(&fixture.returns, CagrBasis::factor(252.0)).expect("valid fixture CAGR"),
        expected.cagr_factor,
    );
    assert_close(sharpe(0.12, 0.18, 0.02), expected.sharpe);
    assert_close(
        sortino(&fixture.returns, true, 252.0, 0.0),
        expected.sortino,
    );
    assert_close(
        value_at_risk(&fixture.returns, 0.95),
        expected.value_at_risk,
    );
    assert_close(
        expected_shortfall(&fixture.returns, 0.95),
        expected.expected_shortfall,
    );

    let rolling = rolling_greeks(
        &fixture.returns,
        &fixture.benchmark,
        &fixture.dates,
        5,
        252.0,
        0.0,
    );
    assert_vec_close(&rolling.alphas, &expected.rolling_greeks.alphas);
    assert_vec_close(&rolling.betas, &expected.rolling_greeks.betas);

    let factor_refs: Vec<&[f64]> = fixture.factors.iter().map(Vec::as_slice).collect();
    let multi = multi_factor_greeks(&fixture.returns, &factor_refs, 252.0)
        .expect("valid fixture multi-factor regression");
    assert_close(multi.alpha, expected.multi_factor_greeks.alpha);
    assert_vec_close(&multi.betas, &expected.multi_factor_greeks.betas);
    assert_close(multi.r_squared, expected.multi_factor_greeks.r_squared);
    assert_close(
        multi.adjusted_r_squared,
        expected.multi_factor_greeks.adjusted_r_squared,
    );
    assert_close(
        multi.residual_vol,
        expected.multi_factor_greeks.residual_vol,
    );
}
