//! Tests for parametric (Nelson-Siegel) curve calibration.
//!
//! ## What these tests guard
//!
//! ### Task-12 bug: raw PV residuals (original fix)
//!
//! `ParametricCurveTarget::calculate_residuals` previously returned raw PV
//! values without dividing by the instrument notional (1_000_000). Even a
//! perfectly-converged NS curve produced residuals on the order of tens of
//! currency units, so `success` was always `false`.
//!
//! Sibling targets (`discount.rs`, `hazard.rs`, `inflation.rs`) all divide by
//! `residual_notional`. After the fix, `ParametricCurveTarget` does the same.
//!
//! ### Task-12 follow-up: least-squares success tolerance
//!
//! A parametric (NS/NSS) curve is a LEAST-SQUARES fit: with N > 4 quotes it
//! cannot reprice every instrument exactly. The irreducible residual floor is
//! ~1e-4 per-notional for a well-specified NS fit, which exceeds the default
//! `validation_tolerance = 1e-8` (designed for exact bootstrap root-finding).
//!
//! Production `ParametricCurveTarget::solve` now applies a least-squares floor
//! of `1e-3` to the success tolerance (via `.max(1e-3)`), so a fully-converged
//! NS fit reports `success = true` even with the **default** `CalibrationConfig`.
//!
//! `parametric_ns_calibration_succeeds_with_default_config` is the primary
//! regression guard: it runs with a completely default config (no overrides)
//! and asserts `success == true`.

use finstack_core::dates::{Date, Tenor};
use finstack_core::market_data::term_structures::NsVariant;
use finstack_core::types::IndexId;
use finstack_core::HashMap;
use finstack_valuations::calibration::api::engine;
use finstack_valuations::calibration::api::market_datum::MarketDatum;
use finstack_valuations::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, ParametricCurveParams, StepParams,
};
use finstack_valuations::calibration::CalibrationConfig;
use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_valuations::market::quotes::market_quote::MarketQuote;
use finstack_valuations::market::quotes::rates::RateQuote;
use time::Month;

use crate::finstack_test_utils::calibration as cal_utils;

/// Builds a set of deposit quotes with rates drawn from a known Nelson-Siegel curve.
///
/// The NS zero rate `r(T) = beta0 + (beta1+beta2)*(1-e^{-T/tau})/(T/tau) - beta2*e^{-T/tau}`
/// is used to set each deposit rate. Because deposit pricing is based on a
/// money-market day-count fraction that doesn't exactly equal the Act/365F
/// year fraction used by the NS curve, the best achievable residual (per
/// notional) is `~1e-4` — not `0`. This is fine for the normalization test:
/// the key assertion is that residuals are `O(1e-4)` in per-notional units,
/// NOT `O(1e2)` in raw PV units.
fn build_ns_derived_quotes(_base_date: Date) -> Vec<MarketQuote> {
    let ns_zero_rate = |t: f64| -> f64 {
        let beta0 = 0.04_f64;
        let beta1 = -0.02_f64;
        let beta2 = 0.01_f64;
        let tau = 2.0_f64;
        if t < 1e-9 {
            return beta0 + beta1;
        }
        let x = t / tau;
        let factor = (1.0 - (-x).exp()) / x;
        beta0 + beta1 * factor + beta2 * (factor - (-x).exp())
    };

    // Eight tenors spanning 3M–10Y give a well-determined NS fit.
    let tenors: &[(&str, f64)] = &[
        ("3M", 0.25),
        ("6M", 0.5),
        ("1Y", 1.0),
        ("2Y", 2.0),
        ("3Y", 3.0),
        ("5Y", 5.0),
        ("7Y", 7.0),
        ("10Y", 10.0),
    ];

    tenors
        .iter()
        .map(|(tenor_str, t)| {
            MarketQuote::Rates(RateQuote::Deposit {
                id: QuoteId::new(format!("DEP-{tenor_str}")),
                index: IndexId::new("USD-Deposit"),
                pillar: Pillar::Tenor(Tenor::parse(tenor_str).expect("valid tenor")),
                rate: ns_zero_rate(*t),
            })
        })
        .collect()
}

/// Builds wildly inconsistent deposit quotes that no NS curve can fit well.
///
/// Alternating extreme rates (0% and 20%) across the tenor grid create a
/// quote set that violates the smooth monotone shape assumption of the
/// Nelson-Siegel model. LM will converge to some minimum, but the residuals
/// should far exceed 1e-3 (the parametric LS tolerance floor).
fn build_inconsistent_quotes(_base_date: Date) -> Vec<MarketQuote> {
    let tenors: &[(&str, f64)] = &[
        ("3M", 0.0_f64),
        ("6M", 0.2_f64),
        ("1Y", 0.0_f64),
        ("2Y", 0.2_f64),
        ("3Y", 0.0_f64),
        ("5Y", 0.2_f64),
        ("7Y", 0.0_f64),
        ("10Y", 0.2_f64),
    ];

    tenors
        .iter()
        .map(|(tenor_str, rate)| {
            MarketQuote::Rates(RateQuote::Deposit {
                id: QuoteId::new(format!("DEP-{tenor_str}")),
                index: IndexId::new("USD-Deposit"),
                pillar: Pillar::Tenor(Tenor::parse(tenor_str).expect("valid tenor")),
                rate: *rate,
            })
        })
        .collect()
}

/// Run NS parametric calibration and return `(success, max_residual, residuals)`.
///
/// Uses the provided `settings` verbatim — no extra overrides. Pass
/// `CalibrationConfig::default()` for the production-default scenario.
fn run_parametric_ns_with_config(
    base_date: Date,
    curve_id: &str,
    quotes: Vec<MarketQuote>,
    mut settings: CalibrationConfig,
) -> (bool, f64, std::collections::BTreeMap<String, f64>) {
    let mut market_data: Vec<MarketDatum> = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &quotes);

    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("ns_quotes".to_string(), cal_utils::quote_set_ids(&quotes));

    // Do not throw on bad fit — return the report so we can inspect residuals.
    settings.fail_on_bad_fit = false;

    let plan = CalibrationPlan {
        id: "parametric_ns_plan".to_string(),
        description: None,
        quote_sets,
        settings,
        steps: vec![CalibrationStep {
            id: "ns_step".to_string(),
            quote_set: "ns_quotes".to_string(),
            params: StepParams::Parametric(ParametricCurveParams {
                curve_id: curve_id.into(),
                base_date,
                model: NsVariant::Ns,
                discount_curve_id: None,
                initial_params: None,
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,
        schema: "finstack.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: Vec::new(),
    };

    let result = engine::execute(&envelope).expect("calibration engine must not error");
    let report = result
        .result
        .step_reports
        .get("ns_step")
        .expect("step report for 'ns_step' must be present");

    (
        report.success,
        report.max_residual,
        report.residuals.clone(),
    )
}

// ─── Primary regression test: default production config must succeed ────────

/// Primary regression guard for the least-squares tolerance fix.
///
/// This test runs with a **completely default `CalibrationConfig`** (no overrides)
/// and asserts `success == true`.
///
/// | Scenario              | `max_residual`   | `success` |
/// |-----------------------|------------------|-----------|
/// | Before fix (bug)      | `~110` (raw PV)  | `false`   |  ← original Task-12 bug
/// | After fix, default tol| `~1e-4` (per-NL) | `false`   |  ← tol=1e-8, floor missing
/// | After floor fix       | `~1e-4` (per-NL) | `true`    |  ← this test must pass
///
/// The default `validation_tolerance = 1e-8` is designed for exact bootstrap
/// root-finding. A least-squares parametric fit has an irreducible residual
/// floor of ~1e-4; `ParametricCurveTarget::solve` now applies `.max(1e-3)` so
/// the success criterion is appropriate for a LS fit, not a bootstrap.
#[test]
fn parametric_ns_calibration_succeeds_with_default_config() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).expect("base_date");
    let quotes = build_ns_derived_quotes(base_date);

    // Use a completely default CalibrationConfig — no tolerance overrides.
    let (success, max_residual, _residuals) = run_parametric_ns_with_config(
        base_date,
        "USD-NS-DEFAULT",
        quotes,
        CalibrationConfig::default(),
    );

    println!("NS calibration (default config): success={success}, max_residual={max_residual:.4e}");

    // This is the key assertion: a well-converged NS fit must report success
    // even with the production-default CalibrationConfig.
    assert!(
        success,
        "Parametric NS calibration must succeed with default config. \
         max_residual={max_residual:.4e}. \
         If success=false with max_residual≈1e-4, the LS tolerance floor (1e-3) is not applied. \
         If max_residual≈100, PV is not divided by notional (the original Task-12 bug)."
    );

    // Residuals must be in per-notional units (~1e-4), not raw-PV units (~110).
    assert!(
        max_residual < 1e-2,
        "max_residual={max_residual:.4e} must be in per-notional units (<1e-2). \
         A value near 100 indicates un-normalized raw PV residuals (original Task-12 bug)."
    );
}

// ─── Per-quote normalization test ──────────────────────────────────────────

/// Verify all per-quote residuals are in per-notional units.
///
/// Before the original fix, each per-quote residual was a raw PV amount (tens of
/// currency units). After the fix they are all `O(1e-4)` per notional.
#[test]
fn parametric_ns_per_quote_residuals_are_in_per_notional_units() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).expect("base_date");
    let quotes = build_ns_derived_quotes(base_date);

    let (_success, max_residual, residuals) = run_parametric_ns_with_config(
        base_date,
        "USD-NS-PER-QUOTE",
        quotes,
        CalibrationConfig::default(),
    );

    println!("max_residual={max_residual:.4e}");
    for (key, residual) in &residuals {
        println!("  {key}: {residual:.4e}");
    }

    // Each per-quote residual must be in per-notional units — O(1e-4), not O(100).
    // Threshold 1e-2 is 4 orders of magnitude below the bug-era residual.
    for (key, residual) in &residuals {
        let r: f64 = *residual;
        assert!(
            r.abs() < 1e-2,
            "Per-quote residual for '{key}' is {r:.4e} — expected < 1e-2 (per-notional). \
             A value near 100 means PV is not divided by notional (the pre-fix bug)."
        );
    }
}

// ─── Negative test: genuinely bad fit still reports failure ────────────────

/// A quote set that no NS curve can fit well must still report `success = false`.
///
/// This guards against the relaxed tolerance floor (1e-3) rubber-stamping
/// every calibration: a genuinely inconsistent quote set should produce
/// residuals far above 1e-3 and thus `success = false`.
#[test]
fn parametric_ns_calibration_fails_for_inconsistent_quotes() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).expect("base_date");
    let quotes = build_inconsistent_quotes(base_date);

    let (success, max_residual, _residuals) = run_parametric_ns_with_config(
        base_date,
        "USD-NS-BAD",
        quotes,
        CalibrationConfig::default(),
    );

    println!("NS bad-fit calibration: success={success}, max_residual={max_residual:.4e}");

    // Wildly inconsistent quotes (alternating 0% / 20%) cannot be fit by any
    // smooth NS curve; residuals must far exceed the 1e-3 tolerance floor.
    assert!(
        !success,
        "NS calibration on inconsistent quotes should report success=false. \
         max_residual={max_residual:.4e}. \
         If success=true, the tolerance floor may be too lenient."
    );

    assert!(
        max_residual > 1e-3,
        "Inconsistent-quote residual {max_residual:.4e} must exceed the 1e-3 LS tolerance floor."
    );
}
