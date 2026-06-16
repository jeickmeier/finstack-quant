//! Integration tests for SVI volatility-surface calibration (v2 engine).
//!
//! ## What these tests guard
//!
//! ### Task-18 bug (M11): cross-expiry interpolation calendar safety
//!
//! `interpolate_svi_vol` interpolates total variance `w` between two
//! calibrated SVI expiry slices. The pre-fix code evaluated *both* slices at
//! a single log-moneyness `k` computed once from the **target** expiry's
//! forward (`k = ln(K / F(T_target))`).
//!
//! Each SVI slice, however, is calibrated in *its own* forward-moneyness
//! coordinate `k_i = ln(K / F(T_i))`. With a non-flat discount curve
//! `F(T_1) != F(T_2) != F(T_target)`, so evaluating both slices at the
//! target's `k` compares total variances at *different absolute strikes* —
//! which can make the interpolated `w` **decrease** in `T` at a fixed
//! absolute strike, a calendar-spread arbitrage.
//!
//! The fix recomputes `k_i` per slice before evaluating its total variance.
//!
//! `svi_surface_grid_is_calendar_monotone_under_nonflat_curve` calibrates a
//! genuine 3-expiry SVI surface against a steep (non-flat) discount curve and
//! asserts the produced grid is calendar-monotone at every strike. The
//! `target_expiries` straddle the calibrated knots, so the grid nodes store
//! `interpolate_svi_vol` outputs directly — reading them back exercises the
//! cross-expiry interpolation, not surface bilinear interpolation.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::volatility::svi::SviParams;
use finstack_quant_core::types::UnderlyingId;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::calibration::api::engine;
use finstack_quant_valuations::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, StepParams, SviSurfaceParams,
};
use finstack_quant_valuations::calibration::CalibrationConfig;
use finstack_quant_valuations::instruments::OptionType;
use finstack_quant_valuations::market::conventions::ids::OptionConventionId;
use finstack_quant_valuations::market::quotes::ids::QuoteId;
use finstack_quant_valuations::market::quotes::market_quote::MarketQuote;
use finstack_quant_valuations::market::quotes::vol::VolQuote;
use time::Month;

use crate::finstack_quant_test_utils::calibration as cal_utils;

const SPOT: f64 = 100.0;
const UNDERLYING: &str = "SPX";
const DISCOUNT_ID: &str = "USD-OIS";

/// Continuously-compounded zero rate of the test discount curve.
///
/// Deliberately steep and rising so the forward `F(T) = SPOT * exp(r(T)*T)`
/// is genuinely T-dependent — that non-flatness is what makes the per-slice
/// forward-moneyness recomputation matter.
fn zero_rate(t: f64) -> f64 {
    0.01 + 0.09 * t
}

/// Forward used internally by SVI calibration: `F(T) = SPOT / DF(T)` with
/// `DF(T) = exp(-r(T)*T)`, i.e. `F(T) = SPOT * exp(r(T)*T)`.
fn forward(t: f64) -> f64 {
    SPOT * (zero_rate(t) * t).exp()
}

/// Non-flat USD discount curve. Knot DFs are `exp(-r(T)*T)` so the forward
/// curve seen by `SviSurfaceTarget::solve` matches [`forward`].
fn nonflat_discount_curve(base_date: Date) -> DiscountCurve {
    let knot_times = [0.0_f64, 0.5, 0.9, 1.5, 2.2, 3.0, 4.0];
    let knots: Vec<(f64, f64)> = knot_times
        .iter()
        .map(|&t| (t, (-zero_rate(t) * t).exp()))
        .collect();
    DiscountCurve::builder(DISCOUNT_ID)
        .base_date(base_date)
        .knots(knots)
        .build()
        .expect("non-flat discount curve")
}

/// One calibrated SVI expiry slice: its expiry (years) and true parameters.
///
/// The three slices are calendar-monotone at every fixed *absolute* strike on
/// the test grid and are individually arbitrage-valid (`SviParams::validate`).
fn true_slices() -> [(f64, SviParams); 3] {
    [
        (
            0.5,
            SviParams {
                a: 0.015,
                b: 0.50,
                rho: 0.6,
                m: 0.20,
                sigma: 0.10,
            },
        ),
        (
            1.5,
            SviParams {
                a: 0.060,
                b: 0.525,
                rho: 0.6,
                m: 0.08,
                sigma: 0.10,
            },
        ),
        (
            3.0,
            SviParams {
                a: 0.180,
                b: 0.55,
                rho: 0.6,
                m: 0.0,
                sigma: 0.10,
            },
        ),
    ]
}

/// Build option-vol quotes by evaluating each true SVI slice in *its own*
/// forward-moneyness, so per-expiry `calibrate_svi` recovers the slice.
fn svi_option_quotes(base_date: Date) -> Vec<MarketQuote> {
    // Strike multipliers (relative to each slice's own forward) span a
    // generous ITM/ATM/OTM range so the per-expiry SVI fit is well-posed.
    let moneyness = [0.70_f64, 0.82, 0.92, 1.0, 1.10, 1.22, 1.40];
    let mut quotes = Vec::new();
    for (expiry_years, params) in true_slices() {
        params
            .validate()
            .expect("true SVI slice must be arbitrage-valid");
        let f = forward(expiry_years);
        // Calendar date for this expiry, derived from the Act/365F year frac.
        let expiry_date = base_date + time::Duration::days((expiry_years * 365.0).round() as i64);
        for &mult in &moneyness {
            let strike = mult * f;
            let vol = params.implied_vol((strike / f).ln(), expiry_years);
            quotes.push(MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new(format!("{UNDERLYING}-VOL-{expiry_years}-{mult}")),
                underlying: UnderlyingId::new(UNDERLYING),
                expiry: expiry_date,
                strike,
                vol,
                option_type: OptionType::Call,
                convention: OptionConventionId::new("USD-EQ"),
            }));
        }
    }
    quotes
}

#[test]
fn svi_surface_grid_is_calendar_monotone_under_nonflat_curve() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).expect("base_date");

    // Sanity-check the fixture: the discount curve is genuinely non-flat, so
    // forwards differ materially across the calibrated expiries.
    let f_short = forward(0.5);
    let f_long = forward(3.0);
    assert!(
        f_long > f_short * 1.5,
        "test fixture must use a steep non-flat curve: F(0.5)={f_short:.2}, F(3.0)={f_long:.2}"
    );

    let discount = nonflat_discount_curve(base_date);
    let initial_market = MarketContext::new().insert(discount);
    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);

    let quotes = svi_option_quotes(base_date);
    cal_utils::extend_market_data(&mut market_data, &quotes);

    // Target expiry grid STRADDLES the calibrated knots (0.5, 1.5, 3.0) so the
    // surface stores genuine cross-expiry interpolation results at 0.9 and 2.2.
    let target_expiries = vec![0.5_f64, 0.9, 1.5, 2.2, 3.0];
    // Absolute strikes spanning ITM/ATM/OTM relative to the front forward.
    let target_strikes = vec![80.0_f64, 95.0, 103.0, 115.0, 135.0];

    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("svi_quotes".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "svi_surface_plan".to_string(),
        description: None,
        quote_sets,
        settings: CalibrationConfig::default(),
        steps: vec![CalibrationStep {
            id: "svi_step".to_string(),
            quote_set: "svi_quotes".to_string(),
            params: StepParams::SviSurface(SviSurfaceParams {
                surface_id: "SPX-SVI".to_string(),
                base_date,
                underlying_ticker: UNDERLYING.to_string(),
                discount_curve_id: Some(DISCOUNT_ID.into()),
                target_expiries: target_expiries.clone(),
                target_strikes: target_strikes.clone(),
                spot_override: Some(SPOT),
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,
        schema: "finstack_quant.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::execute(&envelope).expect("SVI surface calibration must not error");
    let report = result
        .result
        .step_reports
        .get("svi_step")
        .expect("step report for 'svi_step' must be present");
    assert!(
        report.success,
        "SVI surface calibration must converge; report: {report:?}"
    );

    let context =
        MarketContext::try_from(result.result.final_market).expect("restore market context");
    let surface = context
        .get_surface("SPX-SVI")
        .expect("calibrated SVI surface must be present");

    // Calendar-spread no-arbitrage: total variance w = vol^2 * T must be
    // non-decreasing in T at every fixed absolute strike. With the pre-fix
    // single-k interpolation this is violated off-ATM (e.g. at strike 135).
    let mut max_violation = 0.0_f64;
    let mut violation_detail = String::new();
    for &strike in &target_strikes {
        let mut prev_w = f64::NEG_INFINITY;
        let mut prev_t = 0.0_f64;
        for &t in &target_expiries {
            let vol = surface
                .value_checked(t, strike)
                .unwrap_or_else(|e| panic!("surface lookup at (T={t}, K={strike}) failed: {e}"));
            assert!(
                vol.is_finite() && vol > 0.0,
                "surface vol at (T={t}, K={strike}) must be positive and finite: {vol}"
            );
            let w = vol * vol * t;
            if prev_w.is_finite() && w < prev_w {
                let drop = prev_w - w;
                if drop > max_violation {
                    max_violation = drop;
                    violation_detail = format!(
                        "strike={strike:.1}: w(T={prev_t:.2})={prev_w:.6} > w(T={t:.2})={w:.6}"
                    );
                }
            }
            prev_w = w;
            prev_t = t;
        }
    }

    assert!(
        max_violation <= 1e-6,
        "SVI surface has a calendar-spread arbitrage (interpolated total \
         variance decreases in T at a fixed strike): {violation_detail} \
         (worst drop {max_violation:.3e}). The cross-expiry interpolation \
         must recompute log-moneyness per slice."
    );
}
