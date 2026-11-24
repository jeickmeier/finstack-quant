//! External reference tests for the Merton structural and jump-diffusion models.
//!
//! Expected values are pinned in `golden/data/merton_reference.json`, written
//! by `golden/data/gen_merton_golden.py`. That script never calls this crate:
//! each value is transcribed from a published source or computed by an
//! independent implementation. Sections:
//!
//! - `kmv_published`: the five-firm single-point example in the MATLAB
//!   `mertonmodel` documentation, against [`MertonModel::from_equity`] and the
//!   physical-drift distance to default and default probability.
//! - `equity_inversion`: Hull (2022) Example 24.3 plus payout and multi-year
//!   cases, against an independent Newton solve of the two Merton equations.
//! - `black_cox`: first-passage default probability against QuantLib
//!   `AnalyticBinaryBarrierEngine`.
//! - `jump_bates_limit`, `jump_quantlib_table`, `jump_series`: Merton (1976)
//!   European prices from [`merton_jump_cos_price`] against QuantLib's Bates
//!   engine in the near-degenerate limit, QuantLib's own jump-diffusion test
//!   table, and Merton's Poisson series.
//!
//! # Convention mapping
//!
//! - MATLAB `Liability` is the debt barrier; its `Drift` is the physical asset
//!   return passed to [`MertonModel::distance_to_default_with_drift`].
//! - QuantLib has no structural credit model. The Black-Cox reference is a
//!   down-and-in cash-or-nothing call plus put paid at expiry, undiscounted;
//!   a barrier growing at `g` is priced as a flat barrier with dividend yield
//!   `payout_rate + g`.
//! - The QuantLib table uses Haug's inputs (total volatility and jump share of
//!   variance, zero mean jump). The fixture stores the mapped `sigma`,
//!   `mu_jump` and `sigma_jump`.
//!
//! # Tolerances
//!
//! Each tolerance is set by the precision of its source, recorded per case and
//! explained in the fixture's `meta.extra.tolerances`. The MATLAB values use
//! half a unit of the last printed digit, the QuantLib table uses QuantLib's
//! own 1e-2, and computed values use the accuracy of their reference.
//!
//! Only the zero-payout structural cases are externally anchored. The payout
//! cases of `equity_inversion` follow this crate's documented payout
//! convention and so pin solver accuracy, not the convention.
//!
//! # References
//!
//! - MathWorks, `mertonmodel`, <https://www.mathworks.com/help/risk/mertonmodel.html>
//! - Hull (2022), Example 24.3, as cited by the replication at
//!   <https://mlozanoqf.github.io/tutorial_arf/02-merton.html>.
//! - QuantLib, <https://www.quantlib.org/>: `analyticbinarybarrierengine.cpp`,
//!   `test-suite/batesmodel.cpp`, `test-suite/jumpdiffusion.cpp`.
//! - Black, F. & Cox, J. (1976). "Valuing Corporate Securities: Some Effects
//!   of Bond Indenture Provisions." *Journal of Finance*, 31(2).
//! - Merton, R. (1976). "Option pricing when underlying stock returns are
//!   discontinuous." *Journal of Financial Economics*, 3.

use finstack_quant_models::credit::{AssetDynamics, MertonBarrierType, MertonModel};
use finstack_quant_models::fourier::cos::merton_jump_cos_price;
use finstack_quant_models::types::{OptionContractParams, OptionType};
use finstack_quant_test_utils::golden::{GoldenSuite, load_suite_from_path};
use finstack_quant_test_utils::golden_path;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Published KMV outputs, printed to four decimals (asset value to five
/// significant figures).
#[derive(Debug, Deserialize)]
struct KmvExpected {
    asset_value: f64,
    asset_value_tolerance_abs: f64,
    asset_vol: f64,
    distance_to_default: f64,
    default_probability: f64,
    tolerance_abs: f64,
}

/// Independently solved asset value, asset volatility and risk-neutral
/// credit measures.
#[derive(Debug, Deserialize)]
struct InversionExpected {
    asset_value: f64,
    asset_vol: f64,
    distance_to_default: f64,
    default_probability: f64,
    debt_spread: f64,
    tolerance_rel: f64,
}

/// First-passage default probability.
#[derive(Debug, Deserialize)]
struct FirstPassageExpected {
    default_probability: f64,
    tolerance_abs: f64,
}

/// Jump-diffusion prices; a table row carries a call only.
#[derive(Debug, Deserialize)]
struct JumpExpected {
    call: f64,
    #[serde(default)]
    put: Option<f64>,
    #[serde(default)]
    tolerance_rel: Option<f64>,
    #[serde(default)]
    tolerance_abs: Option<f64>,
}

/// Contract and process inputs shared by the three jump sections.
#[derive(Debug, Deserialize)]
struct JumpCase {
    id: String,
    spot: f64,
    strike: f64,
    rate: f64,
    dividend_yield: f64,
    t: f64,
    sigma: f64,
    mu_jump: f64,
    sigma_jump: f64,
    lambda: f64,
    expected: JumpExpected,
}

/// One reference case, tagged by section.
#[derive(Debug, Deserialize)]
#[serde(tag = "model", rename_all = "snake_case")]
enum MertonCase {
    KmvPublished {
        id: String,
        equity: f64,
        equity_vol: f64,
        liability: f64,
        rate: f64,
        drift: f64,
        t: f64,
        expected: KmvExpected,
    },
    EquityInversion {
        id: String,
        equity: f64,
        equity_vol: f64,
        debt: f64,
        rate: f64,
        payout_rate: f64,
        t: f64,
        expected: InversionExpected,
    },
    BlackCox {
        id: String,
        asset_value: f64,
        debt_barrier: f64,
        rate: f64,
        payout_rate: f64,
        asset_vol: f64,
        barrier_growth_rate: f64,
        t: f64,
        expected: FirstPassageExpected,
    },
    JumpBatesLimit(JumpCase),
    JumpQuantlibTable(JumpCase),
    JumpSeries(JumpCase),
}

/// Records a failure when `actual` misses `expected` by more than the
/// absolute tolerance, or by more than the relative one when `relative`.
fn check(
    failures: &mut Vec<String>,
    id: &str,
    label: &str,
    actual: f64,
    expected: f64,
    tolerance: f64,
    relative: bool,
) {
    let err = if relative {
        ((actual - expected) / expected).abs()
    } else {
        (actual - expected).abs()
    };
    // NaN-aware: a NaN error must register as a failure.
    if err.is_nan() || err > tolerance {
        let kind = if relative { "rel" } else { "abs" };
        failures.push(format!(
            "[{id}] {label}: expected {expected:.15e}, got {actual:.15e} ({kind} err {err:.3e} > {tolerance:.1e})"
        ));
    }
}

fn check_jump(failures: &mut Vec<String>, case: &JumpCase) {
    let price = |option_type| {
        let contract = OptionContractParams {
            spot: case.spot,
            strike: case.strike,
            rate: case.rate,
            time_to_expiry: case.t,
            dividend_yield: case.dividend_yield,
            option_type,
        };
        merton_jump_cos_price(
            &contract,
            case.sigma,
            case.mu_jump,
            case.sigma_jump,
            case.lambda,
            None,
        )
        .expect("valid jump-diffusion inputs")
    };
    let (tolerance, relative) = match (case.expected.tolerance_rel, case.expected.tolerance_abs) {
        (Some(tolerance), None) => (tolerance, true),
        (None, Some(tolerance)) => (tolerance, false),
        other => panic!("[{}] needs exactly one tolerance, got {other:?}", case.id),
    };
    let call = price(OptionType::Call);
    check(
        failures,
        &case.id,
        "call",
        call,
        case.expected.call,
        tolerance,
        relative,
    );
    if let Some(expected_put) = case.expected.put {
        let put = price(OptionType::Put);
        check(
            failures,
            &case.id,
            "put",
            put,
            expected_put,
            tolerance,
            relative,
        );
    }
}

#[test]
fn merton_models_match_external_references() {
    let suite: GoldenSuite<MertonCase> =
        load_suite_from_path(golden_path!("data/merton_reference.json"))
            .expect("Failed to load Merton reference suite");
    assert_eq!(suite.meta.suite_id, "merton_reference");

    let mut failures: Vec<String> = Vec::new();
    let mut counts = BTreeMap::<&'static str, usize>::new();

    for case in &suite.cases {
        match case {
            MertonCase::KmvPublished {
                id,
                equity,
                equity_vol,
                liability,
                rate,
                drift,
                t,
                expected,
            } => {
                *counts.entry("kmv_published").or_default() += 1;
                let model =
                    MertonModel::from_equity(*equity, *equity_vol, *liability, *rate, 0.0, *t)
                        .expect("published KMV inputs invert");
                let dd = model
                    .distance_to_default_with_drift(*drift, *t)
                    .expect("geometric Brownian dynamics support a physical drift");
                let pd = model
                    .default_probability_with_drift(*drift, *t)
                    .expect("geometric Brownian dynamics support a physical drift");
                let tol = expected.tolerance_abs;
                check(
                    &mut failures,
                    id,
                    "asset_value",
                    model.asset_value(),
                    expected.asset_value,
                    expected.asset_value_tolerance_abs,
                    false,
                );
                check(
                    &mut failures,
                    id,
                    "asset_vol",
                    model.asset_vol(),
                    expected.asset_vol,
                    tol,
                    false,
                );
                check(
                    &mut failures,
                    id,
                    "distance_to_default",
                    dd,
                    expected.distance_to_default,
                    tol,
                    false,
                );
                check(
                    &mut failures,
                    id,
                    "default_probability",
                    pd,
                    expected.default_probability,
                    tol,
                    false,
                );
            }
            MertonCase::EquityInversion {
                id,
                equity,
                equity_vol,
                debt,
                rate,
                payout_rate,
                t,
                expected,
            } => {
                *counts.entry("equity_inversion").or_default() += 1;
                let model =
                    MertonModel::from_equity(*equity, *equity_vol, *debt, *rate, *payout_rate, *t)
                        .expect("reference equity inputs invert");
                let pd = model.default_probability(*t).expect("valid model horizon");
                let spread = model.debt_spread(*t).expect("terminal barrier debt spread");
                let tol = expected.tolerance_rel;
                check(
                    &mut failures,
                    id,
                    "asset_value",
                    model.asset_value(),
                    expected.asset_value,
                    tol,
                    true,
                );
                check(
                    &mut failures,
                    id,
                    "asset_vol",
                    model.asset_vol(),
                    expected.asset_vol,
                    tol,
                    true,
                );
                check(
                    &mut failures,
                    id,
                    "distance_to_default",
                    model.distance_to_default(*t),
                    expected.distance_to_default,
                    tol,
                    true,
                );
                check(
                    &mut failures,
                    id,
                    "default_probability",
                    pd,
                    expected.default_probability,
                    tol,
                    true,
                );
                check(
                    &mut failures,
                    id,
                    "debt_spread",
                    spread,
                    expected.debt_spread,
                    tol,
                    true,
                );
            }
            MertonCase::BlackCox {
                id,
                asset_value,
                debt_barrier,
                rate,
                payout_rate,
                asset_vol,
                barrier_growth_rate,
                t,
                expected,
            } => {
                *counts.entry("black_cox").or_default() += 1;
                let model = MertonModel::new_with_dynamics(
                    *asset_value,
                    *asset_vol,
                    *debt_barrier,
                    *rate,
                    *payout_rate,
                    MertonBarrierType::FirstPassage {
                        barrier_growth_rate: *barrier_growth_rate,
                    },
                    AssetDynamics::GeometricBrownian,
                )
                .expect("valid first-passage model");
                let pd = model.default_probability(*t).expect("valid model horizon");
                check(
                    &mut failures,
                    id,
                    "default_probability",
                    pd,
                    expected.default_probability,
                    expected.tolerance_abs,
                    false,
                );
            }
            MertonCase::JumpBatesLimit(jump) => {
                *counts.entry("jump_bates_limit").or_default() += 1;
                check_jump(&mut failures, jump);
            }
            MertonCase::JumpQuantlibTable(jump) => {
                *counts.entry("jump_quantlib_table").or_default() += 1;
                check_jump(&mut failures, jump);
            }
            MertonCase::JumpSeries(jump) => {
                *counts.entry("jump_series").or_default() += 1;
                check_jump(&mut failures, jump);
            }
        }
    }

    // A section that silently stops deserializing must not pass as "no failures".
    let expected_counts = BTreeMap::from([
        ("black_cox", 8),
        ("equity_inversion", 4),
        ("jump_bates_limit", 9),
        ("jump_quantlib_table", 27),
        ("jump_series", 8),
        ("kmv_published", 5),
    ]);
    assert_eq!(
        counts, expected_counts,
        "unexpected per-section case counts"
    );
    assert!(
        failures.is_empty(),
        "{} Merton reference mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
