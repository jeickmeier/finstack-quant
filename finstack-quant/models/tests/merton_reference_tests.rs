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
//! - `cds_par_spread`: [`MertonModel::cds_par_spread`] against QuantLib
//!   `MidPointCdsEngine` on a survival curve whose quarterly nodes are the
//!   model's survival function.
//! - `cds_calibration`: [`MertonModel::from_cds_spread`] must return the asset
//!   volatility a QuantLib par spread was generated from.
//! - `hazard_curve`: [`MertonModel::to_hazard_curve`] survival at its pillars
//!   against the closed-form survival, and between pillars against QuantLib
//!   `HazardRateCurve`.
//! - `report`: [`merton_report`] fields against the references above.
//! - `mc_bond`: the Merton Monte Carlo bond engine against the closed-form
//!   risky bond. That engine lives in the fixed-income crate, which depends on
//!   this one, so the cases are only counted here and are priced by
//!   `valuations/fixed-income/tests/bond/merton_mc_reference.rs`.
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
//! - A null `barrier_growth_rate` is the terminal barrier; a number is the
//!   first-passage barrier growing at that rate.
//! - The CDS par spread uses a quarterly premium grid in year fractions, a
//!   flat continuously compounded discount rate and an ACT/360 premium on an
//!   ACT/365F time axis. QuantLib runs on Actual/360 with 90-day quarters and
//!   its par spread is divided by 365/360.
//! - `merton_report` grades are not checked: the master scale has no external
//!   reference in this suite.
//!
//! # Tolerances
//!
//! Each tolerance is set by the precision of its source, recorded per case and
//! explained in the fixture's `meta.extra.tolerances`. The MATLAB values use
//! half a unit of the last printed digit, the QuantLib table uses QuantLib's
//! own 1e-2, and computed values use the accuracy of their reference. The
//! model-level sections propagate the 5e-9 default-probability tolerance of
//! `black_cox` through each formula; the generator computes that bound per
//! case from the reference values.
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

use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_models::credit::{
    AssetDynamics, MertonBarrierType, MertonModel, MertonReport, MertonReportMeasure,
    MertonReportRequest, MertonReportSource, merton_report,
};
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

/// Structural model inputs shared by the model-level sections.
#[derive(Debug, Deserialize)]
struct ModelInputs {
    asset_value: f64,
    asset_vol: f64,
    debt_barrier: f64,
    rate: f64,
    payout_rate: f64,
    /// `None` is the terminal barrier.
    barrier_growth_rate: Option<f64>,
}

impl ModelInputs {
    fn build(&self) -> MertonModel {
        let barrier = match self.barrier_growth_rate {
            Some(barrier_growth_rate) => MertonBarrierType::FirstPassage {
                barrier_growth_rate,
            },
            None => MertonBarrierType::Terminal,
        };
        MertonModel::new_with_dynamics(
            self.asset_value,
            self.asset_vol,
            self.debt_barrier,
            self.rate,
            self.payout_rate,
            barrier,
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid reference model")
    }
}

/// CDS par spread, decimal per annum.
#[derive(Debug, Deserialize)]
struct CdsExpected {
    par_spread: f64,
    tolerance_abs: f64,
}

/// Asset volatility a CDS quote was generated from.
#[derive(Debug, Deserialize)]
struct CalibrationExpected {
    asset_vol: f64,
    tolerance_abs: f64,
}

/// Survival probabilities at and between the hazard-curve pillars.
#[derive(Debug, Deserialize)]
struct HazardExpected {
    pillar_survival: Vec<f64>,
    pillar_tolerance_abs: f64,
    off_pillar_survival: Vec<f64>,
    off_pillar_tolerance_abs: f64,
}

/// Where a report case takes its model from.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ReportSource {
    Model(ModelInputs),
    Equity {
        equity_value: f64,
        equity_vol: f64,
        short_term_debt: f64,
        long_term_debt: f64,
        rate: f64,
        payout_rate: f64,
        maturity: f64,
    },
}

/// One `MertonReport` field compared with its reference.
#[derive(Debug, Deserialize)]
struct ReportCheck {
    field: String,
    index: Option<usize>,
    value: f64,
    tolerance: f64,
    relative: bool,
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
    CdsParSpread {
        id: String,
        #[serde(flatten)]
        firm: ModelInputs,
        maturity: f64,
        recovery: f64,
        expected: CdsExpected,
    },
    CdsCalibration {
        id: String,
        cds_spread_bp: f64,
        recovery: f64,
        debt_barrier: f64,
        rate: f64,
        maturity: f64,
        asset_value: f64,
        payout_rate: f64,
        expected: CalibrationExpected,
    },
    HazardCurve {
        id: String,
        #[serde(flatten)]
        firm: ModelInputs,
        tenors: Vec<f64>,
        recovery: f64,
        off_pillar_times: Vec<f64>,
        expected: HazardExpected,
    },
    /// Priced by the fixed-income crate's `merton_mc_reference` test.
    McBond {},
    Report {
        id: String,
        source: ReportSource,
        horizons: Vec<f64>,
        maturity: f64,
        recovery: f64,
        checks: Vec<ReportCheck>,
        rejected: Vec<String>,
    },
}

/// The value of a report field named by a fixture check, or why it is absent.
fn report_value(report: &MertonReport, field: &str, index: Option<usize>) -> Result<f64, String> {
    let at = |values: &[f64]| {
        index
            .and_then(|i| values.get(i).copied())
            .ok_or_else(|| format!("no element {index:?}"))
    };
    let measure = |values: &Option<Vec<f64>>, measure: MertonReportMeasure| match values {
        Some(values) => at(values),
        None => Err(format!("rejected: {:?}", report.errors.get(&measure))),
    };
    match field {
        "default_point" => report
            .default_point
            .ok_or_else(|| "no default point".into()),
        "model.debt_barrier" => Ok(report.model.debt_barrier()),
        "model.asset_value" => Ok(report.model.asset_value()),
        "model.asset_vol" => Ok(report.model.asset_vol()),
        "default_probabilities" => at(&report.default_probabilities),
        "distance_to_default" => at(&report.distance_to_default),
        "implied_spread" => measure(&report.implied_spread, MertonReportMeasure::ImpliedSpread),
        "debt_spread" => measure(&report.debt_spread, MertonReportMeasure::DebtSpread),
        "cds_par_spread" => report.cds_par_spread.ok_or_else(|| {
            format!(
                "rejected: {:?}",
                report.errors.get(&MertonReportMeasure::CdsParSpread)
            )
        }),
        "implied_equity.value" | "implied_equity.vol" => report
            .implied_equity
            .map(|equity| {
                if field.ends_with("value") {
                    equity.value
                } else {
                    equity.vol
                }
            })
            .ok_or_else(|| {
                format!(
                    "rejected: {:?}",
                    report.errors.get(&MertonReportMeasure::ImpliedEquity)
                )
            }),
        other => Err(format!("unknown report field {other}")),
    }
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
            MertonCase::CdsParSpread {
                id,
                firm,
                maturity,
                recovery,
                expected,
            } => {
                *counts.entry("cds_par_spread").or_default() += 1;
                match firm.build().cds_par_spread(*maturity, *recovery) {
                    Ok(spread) => check(
                        &mut failures,
                        id,
                        "par_spread",
                        spread,
                        expected.par_spread,
                        expected.tolerance_abs,
                        false,
                    ),
                    Err(error) => failures.push(format!("[{id}] cds_par_spread: {error}")),
                }
            }
            MertonCase::CdsCalibration {
                id,
                cds_spread_bp,
                recovery,
                debt_barrier,
                rate,
                maturity,
                asset_value,
                payout_rate,
                expected,
            } => {
                *counts.entry("cds_calibration").or_default() += 1;
                match MertonModel::from_cds_spread(
                    *cds_spread_bp,
                    *recovery,
                    *debt_barrier,
                    *rate,
                    *maturity,
                    *asset_value,
                    *payout_rate,
                ) {
                    Ok(model) => check(
                        &mut failures,
                        id,
                        "asset_vol",
                        model.asset_vol(),
                        expected.asset_vol,
                        expected.tolerance_abs,
                        false,
                    ),
                    Err(error) => failures.push(format!("[{id}] from_cds_spread: {error}")),
                }
            }
            MertonCase::HazardCurve {
                id,
                firm,
                tenors,
                recovery,
                off_pillar_times,
                expected,
            } => {
                *counts.entry("hazard_curve").or_default() += 1;
                // Survival is queried by year fraction, so the anchor is immaterial.
                let base =
                    Date::from_calendar_date(2026, time::Month::January, 15).expect("valid date");
                let curve = match firm.build().to_hazard_curve(
                    "MERTON-REFERENCE",
                    base,
                    tenors,
                    *recovery,
                    DayCount::Act365F,
                ) {
                    Ok(curve) => curve,
                    Err(error) => {
                        failures.push(format!("[{id}] to_hazard_curve: {error}"));
                        continue;
                    }
                };
                assert_eq!(tenors.len(), expected.pillar_survival.len(), "[{id}]");
                assert_eq!(
                    off_pillar_times.len(),
                    expected.off_pillar_survival.len(),
                    "[{id}]"
                );
                let points = tenors
                    .iter()
                    .zip(&expected.pillar_survival)
                    .map(|(t, s)| (t, s, expected.pillar_tolerance_abs, "pillar"))
                    .chain(
                        off_pillar_times
                            .iter()
                            .zip(&expected.off_pillar_survival)
                            .map(|(t, s)| (t, s, expected.off_pillar_tolerance_abs, "off-pillar")),
                    );
                for (t, survival, tolerance, kind) in points {
                    check(
                        &mut failures,
                        id,
                        &format!("{kind} survival at {t}y"),
                        curve.sp(*t).expect("finite positive time"),
                        *survival,
                        tolerance,
                        false,
                    );
                }
            }
            MertonCase::McBond {} => {
                *counts.entry("mc_bond").or_default() += 1;
            }
            MertonCase::Report {
                id,
                source,
                horizons,
                maturity,
                recovery,
                checks,
                rejected,
            } => {
                *counts.entry("report").or_default() += 1;
                let source = match source {
                    ReportSource::Model(model) => MertonReportSource::Model {
                        model: model.build(),
                    },
                    ReportSource::Equity {
                        equity_value,
                        equity_vol,
                        short_term_debt,
                        long_term_debt,
                        rate,
                        payout_rate,
                        maturity,
                    } => MertonReportSource::Equity {
                        equity_value: *equity_value,
                        equity_vol: *equity_vol,
                        short_term_debt: *short_term_debt,
                        long_term_debt: *long_term_debt,
                        risk_free_rate: *rate,
                        payout_rate: *payout_rate,
                        maturity: *maturity,
                    },
                };
                let report = match merton_report(&MertonReportRequest {
                    source,
                    horizons: horizons.clone(),
                    maturity: *maturity,
                    recovery: *recovery,
                }) {
                    Ok(report) => report,
                    Err(error) => {
                        failures.push(format!("[{id}] merton_report: {error}"));
                        continue;
                    }
                };
                for item in checks {
                    let label = match item.index {
                        Some(index) => format!("{}[{index}]", item.field),
                        None => item.field.clone(),
                    };
                    match report_value(&report, &item.field, item.index) {
                        Ok(actual) => check(
                            &mut failures,
                            id,
                            &label,
                            actual,
                            item.value,
                            item.tolerance,
                            item.relative,
                        ),
                        Err(reason) => failures.push(format!("[{id}] {label}: {reason}")),
                    }
                }
                // The rejected measures, and only those, carry an error.
                let reported: Vec<&str> = report.errors.keys().map(|m| m.as_str()).collect();
                let mut wanted: Vec<&str> = rejected.iter().map(String::as_str).collect();
                wanted.sort_unstable();
                if reported != wanted {
                    failures.push(format!(
                        "[{id}] rejected measures: expected {wanted:?}, got {:?}",
                        report.errors
                    ));
                }
            }
        }
    }

    // A section that silently stops deserializing must not pass as "no failures".
    let expected_counts = BTreeMap::from([
        ("black_cox", 8),
        ("cds_calibration", 2),
        ("cds_par_spread", 6),
        ("equity_inversion", 4),
        ("hazard_curve", 3),
        ("jump_bates_limit", 9),
        ("jump_quantlib_table", 27),
        ("jump_series", 8),
        ("kmv_published", 5),
        ("mc_bond", 4),
        ("report", 3),
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
