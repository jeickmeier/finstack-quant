//! Tests for parametric (Nelson-Siegel) curve calibration.
//!
//! Regression tests that verify residual normalization in
//! `ParametricCurveTarget::calculate_residuals`.
//!
//! ## The bug
//!
//! `ParametricCurveTarget::calculate_residuals` previously returned raw PV
//! values without dividing by the instrument notional (1_000_000). This meant
//! that even a perfectly-converged NS curve produced residuals on the order of
//! tens of currency units — many orders of magnitude above the default
//! `validation_tolerance = 1e-8` — so `success` was always `false`.
//!
//! Sibling targets (`discount.rs`, `hazard.rs`, `inflation.rs`) all divide by
//! `residual_notional`. After the fix, `ParametricCurveTarget` does the same.
//!
//! ## What these tests assert
//!
//! - Before fix: `max_residual ≈ 110` (raw PV in currency units, ~1e6 notional
//!   scale), `success = false`.
//! - After fix: `max_residual ≈ 1e-4` (normalized PV / notional), well below
//!   the relaxed `validation_tolerance = 1e-3` used here, `success = true`.

use finstack_core::dates::{Date, Tenor};
use finstack_core::market_data::term_structures::NsVariant;
use finstack_core::HashMap;
use finstack_valuations::calibration::api::engine;
use finstack_valuations::calibration::api::market_datum::MarketDatum;
use finstack_valuations::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, ParametricCurveParams, StepParams,
};
use finstack_valuations::calibration::CalibrationConfig;
use finstack_valuations::market::conventions::ids::IndexId;
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

/// Helper: run the NS parametric calibration and return the calibration config
/// used (for documentation) plus the max_residual and success flag.
///
/// `validation_tolerance` is set to `1e-3` — achievable by a properly-normalized
/// NS fit to deposit quotes (best-fit residual ≈ `1e-4` per notional). Without
/// normalization the residual would be `~110` and `success` would be `false`
/// even at `validation_tolerance = 1`.
fn run_parametric_ns(
    base_date: Date,
    curve_id: &str,
    validation_tolerance: f64,
) -> (bool, f64, std::collections::BTreeMap<String, f64>) {
    let quotes = build_ns_derived_quotes(base_date);

    let mut market_data: Vec<MarketDatum> = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &quotes);

    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("ns_quotes".to_string(), cal_utils::quote_set_ids(&quotes));

    let mut settings = CalibrationConfig::default();
    // Relax validation_tolerance so success reflects achievable per-notional fit.
    settings.discount_curve.validation_tolerance = validation_tolerance;
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

/// Regression test: residual normalization allows the calibration to report success.
///
/// The key behavior under test:
///
/// | Scenario          | `max_residual`     | `success` |
/// |-------------------|--------------------|-----------|
/// | Before fix (bug)  | `~110` (raw PV)    | `false`   |
/// | After fix         | `~1e-4` (per-NL)   | `true`    |
///
/// `validation_tolerance = 1e-3` is comfortably above the achievable best-fit
/// residual of `~1e-4` but 5 orders of magnitude below the bug-era residual of
/// `~110`. A result of `success = true` therefore proves normalization is applied.
#[test]
fn parametric_ns_calibration_succeeds_with_normalized_residuals() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).expect("base_date");

    // Use a generous validation_tolerance (1e-3) that is achievable by a properly
    // normalized NS fit (~1e-4) but would be FAR exceeded by raw-PV residuals (~110).
    let (success, max_residual, residuals) = run_parametric_ns(base_date, "USD-NS", 1e-3);

    println!("NS calibration: success={success}, max_residual={max_residual:.4e}");
    println!("Per-quote residuals: {residuals:?}");

    // After fix: per-notional residuals ~1e-4, below tolerance 1e-3 → success = true.
    // Before fix: raw PV residuals ~110, above tolerance 1e-3 → success = false.
    assert!(
        success,
        "Calibration must succeed with normalized residuals at tolerance=1e-3. \
         max_residual={max_residual:.4e}. \
         If max_residual ≈ 100, PV is not being divided by notional (the original bug)."
    );

    // Residuals must be in per-notional units (≈1e-4), not raw-PV units (≈110).
    // A threshold of 1e-2 is deliberately conservative — the key is that it's
    // far below the raw-PV scale of ~100.
    assert!(
        max_residual < 1e-2,
        "max_residual={max_residual:.4e} should be in per-notional units (<1e-2). \
         A value near 100 indicates un-normalized raw PV residuals."
    );
}

/// Verify all per-quote residuals are in per-notional units after the fix.
///
/// Before the fix, each per-quote residual would be a raw PV amount (tens of
/// currency units). After the fix they are all `O(1e-4)` per notional.
#[test]
fn parametric_ns_per_quote_residuals_are_in_per_notional_units() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).expect("base_date");

    let (_, max_residual, residuals) = run_parametric_ns(base_date, "USD-NS-B", 1e-3);

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
