//! Merton Monte Carlo bond engine against the closed-form risky bond.
//!
//! The cases are the `mc_bond` section of the models crate's
//! `tests/golden/data/merton_reference.json`, written by
//! `gen_merton_golden.py` there. That script never calls finstack: the
//! expected price is the closed-form exogenous-recovery risky bond on a
//! survival function taken from N(-d2) (terminal barrier) or QuantLib's
//! Black-Cox hitting probability (first passage). The fixture lives beside the
//! other Merton references; the test lives here because the engine does.
//!
//! # Convention mapping
//!
//! - Prices are dirty, in percent of par. The reference pays each cash coupon
//!   if the firm has survived to its date, `recovery x notional` at the end of
//!   the monitoring step in which default is recognised, and notional at
//!   maturity on survival, all discounted at the flat risk-free rate.
//! - A null `barrier_growth_rate` is the terminal barrier. First-passage cases
//!   use the engine's default Brownian-bridge crossing test, which is exact in
//!   distribution for an exponential barrier, so the estimator is unbiased.
//!
//! # Tolerance
//!
//! Three standard errors. The standard error is not the simulation's own: it
//! is the exact standard deviation of the closed-form outcome distribution
//! over `sqrt(num_paths)`, fixed in the fixture together with the seed and
//! path count before any run. A miss is a finding; do not re-seed.

use std::path::Path;

use finstack_quant_models::credit::{AssetDynamics, MertonBarrierType, MertonModel};
use finstack_quant_test_utils::golden::{GoldenSuite, load_suite_from_path};
use finstack_quant_valuations_fixed_income::MertonMcConfig;
use finstack_quant_valuations_fixed_income::bond::pricing::merton_mc::{
    MertonMcEngine, MertonMcRun,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct McExpected {
    dirty_price_pct: f64,
    tolerance_abs: f64,
    default_rate: f64,
    default_rate_tolerance_abs: f64,
}

#[derive(Debug, Deserialize)]
struct McBondCase {
    id: String,
    asset_value: f64,
    asset_vol: f64,
    debt_barrier: f64,
    rate: f64,
    payout_rate: f64,
    /// `None` is the terminal barrier.
    barrier_growth_rate: Option<f64>,
    notional: f64,
    coupon_rate: f64,
    coupon_frequency: usize,
    maturity: f64,
    recovery: f64,
    steps_per_year: usize,
    num_paths: usize,
    seed: u64,
    antithetic: bool,
    expected: McExpected,
}

#[test]
fn merton_mc_bond_matches_closed_form_risky_bond() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../models/tests/golden/data/merton_reference.json");
    let suite: GoldenSuite<serde_json::Value> =
        load_suite_from_path(path).expect("Failed to load Merton reference suite");
    assert_eq!(suite.meta.suite_id, "merton_reference");

    let cases: Vec<McBondCase> = suite
        .cases
        .into_iter()
        .filter(|case| case["model"] == "mc_bond")
        .map(|case| serde_json::from_value(case).expect("mc_bond case deserializes"))
        .collect();
    // A section that silently stops matching must not pass as "no failures".
    assert_eq!(cases.len(), 4, "unexpected mc_bond case count");

    let mut failures: Vec<String> = Vec::new();
    for case in &cases {
        let barrier = match case.barrier_growth_rate {
            Some(barrier_growth_rate) => MertonBarrierType::FirstPassage {
                barrier_growth_rate,
            },
            None => MertonBarrierType::Terminal,
        };
        let merton = MertonModel::new_with_dynamics(
            case.asset_value,
            case.asset_vol,
            case.debt_barrier,
            case.rate,
            case.payout_rate,
            barrier,
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid reference model");
        let config = MertonMcConfig::new(merton, case.recovery)
            .expect("valid recovery")
            .steps_per_year(case.steps_per_year);
        let run = MertonMcRun {
            num_paths: case.num_paths,
            seed: case.seed,
            antithetic: case.antithetic,
        };
        let result = MertonMcEngine::price(
            case.notional,
            case.coupon_rate,
            case.maturity,
            case.coupon_frequency,
            &config,
            &run,
            case.rate,
        )
        .expect("reference bond prices");

        let checks = [
            (
                "dirty_price_pct",
                result.dirty_price_pct,
                case.expected.dirty_price_pct,
                case.expected.tolerance_abs,
            ),
            (
                "default_rate",
                result.path_statistics.default_rate,
                case.expected.default_rate,
                case.expected.default_rate_tolerance_abs,
            ),
        ];
        for (label, actual, expected, tolerance) in checks {
            let err = (actual - expected).abs();
            // NaN-aware: a NaN error must register as a failure.
            if err.is_nan() || err > tolerance {
                failures.push(format!(
                    "[{}] {label}: expected {expected:.12}, got {actual:.12} \
                     (abs err {err:.3e} > {tolerance:.3e}; engine standard error {:.3e})",
                    case.id, result.standard_error
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} Merton MC reference mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
