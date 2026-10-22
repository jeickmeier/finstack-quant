//! Integration tests for SVI volatility-surface calibration (canonical engine).
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

use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, StepParams, SurfaceExtrapolationPolicy,
    SviSurfaceParams, VolSurfaceParams,
};
use finstack_quant_calibration::quotes::MarketQuote;
use finstack_quant_calibration::quotes::QuoteId;
use finstack_quant_calibration::quotes::VolQuote;
use finstack_quant_calibration::CalibrationConfig;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::types::UnderlyingId;
use finstack_quant_core::FxHashMap;
use finstack_quant_models::types::OptionType;
use finstack_quant_models::volatility::svi::SviParams;
use time::Month;

use crate::calibration::calibration_support as cal_utils;

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
/// The slices are calendar-monotone and pass the target's butterfly scan.
fn true_slices() -> [(f64, SviParams); 3] {
    [
        (
            0.5,
            SviParams {
                a: 0.020,
                b: 0.15,
                rho: 0.3,
                m: 0.10,
                sigma: 0.25,
            },
        ),
        (
            1.5,
            SviParams {
                a: 0.070,
                b: 0.16,
                rho: 0.3,
                m: 0.05,
                sigma: 0.25,
            },
        ),
        (
            3.0,
            SviParams {
                a: 0.190,
                b: 0.17,
                rho: 0.3,
                m: 0.0,
                sigma: 0.25,
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
            let vol = params
                .implied_vol((strike / f).ln(), expiry_years)
                .expect("valid checked inputs");
            quotes.push(MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new(format!("{UNDERLYING}-VOL-{expiry_years}-{mult}")),
                underlying: UnderlyingId::new(UNDERLYING),
                expiry: expiry_date,
                strike,
                vol,
                option_type: OptionType::Call,
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
    let source_market = MarketContext::new().insert(discount);
    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);

    let quotes = svi_option_quotes(base_date);
    cal_utils::extend_market_data(&mut market_data, &quotes);

    // Target expiry grid STRADDLES the calibrated knots (0.5, 1.5, 3.0) so the
    // surface stores genuine cross-expiry interpolation results at 0.9 and 2.2.
    let mut target_expiries = vec![0.9_f64, 2.2];
    // Absolute strikes spanning ITM/ATM/OTM relative to the front forward.
    let mut target_strikes = vec![80.0_f64, 95.0, 103.0, 115.0, 135.0];
    // The published grid must cover and reprice the original dated quotes.
    for quote in &quotes {
        if let MarketQuote::Vol(VolQuote::OptionVol { expiry, strike, .. }) = quote {
            target_expiries.push((*expiry - base_date).whole_days() as f64 / 365.0);
            target_strikes.push(*strike);
        }
    }
    target_expiries.sort_by(f64::total_cmp);
    target_expiries.dedup();
    target_strikes.sort_by(f64::total_cmp);
    target_strikes.dedup();

    let mut quote_sets: FxHashMap<String, Vec<QuoteId>> = FxHashMap::default();
    quote_sets.insert("svi_quotes".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "svi_surface_plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings: CalibrationConfig {
            fail_on_bad_fit: false,
            ..CalibrationConfig::default()
        },
        steps: vec![CalibrationStep {
            id: "svi_step".to_string(),
            quote_set: "svi_quotes".to_string(),
            params: StepParams::SviSurface(SviSurfaceParams {
                vol_surface_id: "SPX-SVI".to_string(),
                base_date,
                underlying_ticker: UNDERLYING.to_string(),
                discount_curve_id: Some(DISCOUNT_ID.into()),
                target_expiries: target_expiries.clone(),
                target_strikes: target_strikes.clone(),
                spot_override: Some(SPOT),
                dividend_yield_override: Some(0.0),
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,
        schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::calibrate(&envelope).expect("SVI surface calibration must not error");
    let report = result
        .result
        .step_reports
        .get("svi_step")
        .expect("step report for 'svi_step' must be present");
    assert!(
        report.success,
        "SVI surface calibration must converge; report: {report:?}"
    );
    assert!(
        report.iterations > true_slices().len(),
        "iteration count must report solver work rather than expiry slice count"
    );

    // The per-expiry SVI parameters behind the gridded surface are exported
    // on the report: one slice per quoted expiry, accounting for every solver
    // iteration, each reproducing the smile its quotes were generated from.
    let slices = &report.fitted_slices;
    assert_eq!(slices.len(), true_slices().len());
    assert_eq!(
        slices.iter().map(|slice| slice.iterations).sum::<usize>(),
        report.iterations
    );
    for (slice, (expiry_years, truth)) in slices.iter().zip(true_slices()) {
        assert!((slice.expiry - expiry_years).abs() < 1.0 / 365.0);
        assert!(slice.max_relative_quote_error.is_none());
        let finstack_quant_calibration::FittedSliceParameters::Svi(fitted) = &slice.parameters
        else {
            panic!("SVI surface slices are SVI fits");
        };
        let fitted_vol = fitted
            .implied_vol(0.0, slice.expiry)
            .expect("fitted ATM vol");
        let true_vol = truth.implied_vol(0.0, expiry_years).expect("true ATM vol");
        assert!(
            (fitted_vol - true_vol).abs() < 1e-3,
            "fitted SVI slice at T={expiry_years} must reproduce the quoted smile: \
             {fitted_vol} vs {true_vol}"
        );
    }

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
            let vol = finstack_quant_models::volatility::get_surface_vol(&surface, t, strike)
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

#[test]
fn calibrated_surfaces_preserve_total_variance_between_published_expiries() {
    let base_date = Date::from_calendar_date(2025, Month::January, 3).expect("base date");
    let expiries = vec![182.0 / 365.0, 1.0];
    let strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];
    let discount = DiscountCurve::builder(DISCOUNT_ID)
        .base_date(base_date)
        .knots([(0.0, 1.0), (5.0, 1.0)])
        .build()
        .expect("zero-rate discount curve");
    let source_market = MarketContext::new().insert(discount);
    let mut quotes = Vec::new();
    for (days, total_variance) in [(182, 0.04_f64), (365, 0.0401)] {
        let expiry = base_date + time::Duration::days(days);
        let vol = (total_variance / (days as f64 / 365.0)).sqrt();
        for &strike in &strikes {
            quotes.push(MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new(format!("flat-{days}-{strike}")),
                underlying: UNDERLYING.into(),
                expiry,
                strike,
                vol,
                option_type: OptionType::Call,
            }));
        }
    }
    let (prior_market, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let steps = [
        StepParams::VolSurface(VolSurfaceParams {
            vol_surface_id: "FLAT-VOL".into(),
            base_date,
            underlying_ticker: UNDERLYING.into(),
            discount_curve_id: Some(DISCOUNT_ID.into()),
            beta: 1.0,
            target_expiries: expiries.clone(),
            target_strikes: strikes.clone(),
            spot_override: Some(SPOT),
            dividend_yield_override: Some(0.0),
            expiry_extrapolation: SurfaceExtrapolationPolicy::Error,
        }),
        StepParams::SviSurface(SviSurfaceParams {
            vol_surface_id: "FLAT-VOL".into(),
            base_date,
            underlying_ticker: UNDERLYING.into(),
            discount_curve_id: Some(DISCOUNT_ID.into()),
            target_expiries: expiries.clone(),
            target_strikes: strikes.clone(),
            spot_override: Some(SPOT),
            dividend_yield_override: Some(0.0),
        }),
    ];
    for params in steps {
        let mut settings = CalibrationConfig::default();
        settings.vol_surface.validation_tolerance = 1e-6;
        let envelope = CalibrationEnvelope {
            schema_url: None,
            schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
            plan: CalibrationPlan {
                id: "delivered-total-variance".into(),
                description: None,
                quote_sets: [("flat".into(), cal_utils::quote_set_ids(&quotes))].into(),
                settings,
                steps: vec![CalibrationStep {
                    id: "surface".into(),
                    quote_set: "flat".into(),
                    params,
                }],
            },
            market_data: market_data.clone(),
            prior_market: prior_market.clone(),
        };
        let result = engine::calibrate(&envelope).expect("flat-smile calibration");
        assert!(result.result.report.success, "{:?}", result.result.report);
        let context = MarketContext::try_from(result.result.final_market).expect("restore context");
        let surface = context.get_surface("FLAT-VOL").expect("delivered surface");
        for &strike in &strikes {
            let mut previous_variance = 0.0;
            for n in 0..=40 {
                let expiry = expiries[0] + (expiries[1] - expiries[0]) * f64::from(n) / 40.0;
                let vol =
                    finstack_quant_models::volatility::get_surface_vol(&surface, expiry, strike)
                        .expect("public surface lookup");
                let variance = vol * vol * expiry;
                assert!(
                    variance + 1e-12 >= previous_variance,
                    "delivered surface loses calendar monotonicity at T={expiry}, K={strike}: \
                     {previous_variance} -> {variance}"
                );
                previous_variance = variance;
            }
        }
    }
}

/// Execute real surface solves across the fit/acceptance/execution-policy matrix.
#[test]
fn surface_acceptance_policy_preserves_fit_and_execution_controls() {
    use finstack_quant_calibration::validation::ValidationMode;
    let base_date = Date::from_calendar_date(2025, Month::January, 3).expect("base date");
    let strikes = vec![70.0, 80.0, 90.0, 100.0, 110.0, 120.0, 140.0];
    let discount = nonflat_discount_curve(base_date);
    let quote_forward = SPOT / discount.df(1.0);
    let truth = finstack_quant_models::volatility::sabr::SabrParameters::new(0.2, 1.0, 1.0, -0.3)
        .expect("SABR truth");
    let source_market = MarketContext::new().insert(discount);
    let quotes: Vec<_> = strikes
        .iter()
        .map(|&strike| {
            let vol = truth
                .implied_vol(quote_forward, strike, 1.0)
                .expect("model quote");
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new(format!("matrix-{strike}")),
                underlying: UNDERLYING.into(),
                expiry: base_date + time::Duration::days(365),
                strike,
                vol,
                option_type: OptionType::Call,
            })
        })
        .collect();
    let (prior_market, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let steps = [
        StepParams::VolSurface(VolSurfaceParams {
            vol_surface_id: "MATRIX".into(),
            base_date,
            underlying_ticker: UNDERLYING.into(),
            discount_curve_id: Some(DISCOUNT_ID.into()),
            beta: 1.0,
            target_expiries: vec![1.0],
            target_strikes: strikes.clone(),
            spot_override: Some(SPOT),
            dividend_yield_override: Some(0.0),
            expiry_extrapolation: SurfaceExtrapolationPolicy::Error,
        }),
        StepParams::SviSurface(SviSurfaceParams {
            vol_surface_id: "MATRIX".into(),
            base_date,
            underlying_ticker: UNDERLYING.into(),
            discount_curve_id: Some(DISCOUNT_ID.into()),
            target_expiries: vec![1.0],
            target_strikes: strikes,
            spot_override: Some(SPOT),
            dividend_yield_override: Some(0.0),
        }),
    ];
    for params in steps {
        for fit_success in [true, false] {
            let mut params = params.clone();
            if !fit_success {
                // Model slices still fit; the coarser delivered grid loses quote accuracy.
                match &mut params {
                    StepParams::VolSurface(p) => p.target_strikes = vec![70.0, 100.0, 140.0],
                    StepParams::SviSurface(p) => {
                        p.target_strikes = vec![70.0, 90.0, 100.0, 120.0, 140.0]
                    }
                    _ => unreachable!(),
                }
            }
            for violation in [true, false] {
                for mode in [ValidationMode::Warn, ValidationMode::Error] {
                    for fail_on_bad_fit in [true, false] {
                        let mut settings = CalibrationConfig {
                            validation_mode: mode.clone(),
                            fail_on_bad_fit,
                            ..Default::default()
                        };
                        settings.vol_surface.validation_tolerance = 0.001;
                        settings.validation.check_arbitrage = false;
                        settings.validation.max_volatility = if violation { 0.1 } else { 5.0 };
                        let envelope = CalibrationEnvelope {
                            schema_url: None,
                            schema:
                                finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
                            plan: CalibrationPlan {
                                id: "surface-policy".into(),
                                description: None,
                                quote_sets: [("quotes".into(), cal_utils::quote_set_ids(&quotes))]
                                    .into(),
                                settings,
                                steps: vec![CalibrationStep {
                                    id: "surface".into(),
                                    quote_set: "quotes".into(),
                                    params: params.clone(),
                                }],
                            },
                            market_data: market_data.clone(),
                            prior_market: prior_market.clone(),
                        };
                        let expected_success =
                            fit_success && !(violation && mode == ValidationMode::Error);
                        let result = engine::calibrate(&envelope);
                        if fail_on_bad_fit && !expected_success {
                            assert!(result.is_err(), "{params:?}, fit={fit_success}, violation={violation}, mode={mode:?}");
                            continue;
                        }
                        let result = result.expect("accepted execution policy");
                        let report = &result.result.step_reports["surface"];
                        assert_eq!(report.success, expected_success, "{params:?}, fit={fit_success}, violation={violation}, mode={mode:?}: {report:?}");
                        assert_eq!(
                            report.metadata.contains_key("validation_warning"),
                            violation && mode == ValidationMode::Warn
                        );
                        assert_eq!(
                            report.validation_passed,
                            !(violation && mode == ValidationMode::Error)
                        );
                        if violation && mode == ValidationMode::Error {
                            assert!(report
                                .validation_error
                                .as_deref()
                                .is_some_and(|s| s.contains("volatility")));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn svi_density_scan_obeys_global_arbitrage_and_report_policy() {
    use finstack_quant_calibration::validation::ValidationMode;
    let base_date = Date::from_calendar_date(2025, Month::January, 3).expect("base date");
    let truth = SviParams {
        a: 0.015,
        b: 0.5,
        rho: 0.6,
        m: 0.2,
        sigma: 0.1,
    };
    truth.validate().expect("necessary SVI conditions");
    assert!(truth.butterfly_violation(-1.5, 1.5, 201, 1e-10).is_some());
    let strikes: Vec<_> = [-0.5_f64, -0.3, -0.1, 0.0, 0.1, 0.2, 0.3, 0.5]
        .iter()
        .map(|k| SPOT * k.exp())
        .collect();
    let quotes: Vec<_> = strikes
        .iter()
        .map(|&strike| {
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new(format!("density-{strike}")),
                underlying: UNDERLYING.into(),
                expiry: base_date + time::Duration::days(365),
                strike,
                vol: truth.implied_vol((strike / SPOT).ln(), 1.0).expect("vol"),
                option_type: OptionType::Call,
            })
        })
        .collect();
    let discount = DiscountCurve::builder(DISCOUNT_ID)
        .base_date(base_date)
        .knots([(0.0, 1.0), (5.0, 1.0)])
        .build()
        .expect("discount");
    let (prior_market, mut market_data) =
        cal_utils::split_market_context(&MarketContext::new().insert(discount));
    cal_utils::extend_market_data(&mut market_data, &quotes);
    for check_arbitrage in [true, false] {
        for mode in [ValidationMode::Warn, ValidationMode::Error] {
            let mut settings = CalibrationConfig {
                validation_mode: mode.clone(),
                fail_on_bad_fit: false,
                ..Default::default()
            };
            settings.vol_surface.validation_tolerance = 0.1;
            settings.validation.check_arbitrage = check_arbitrage;
            let envelope = CalibrationEnvelope {
                schema_url: None,
                schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
                plan: CalibrationPlan {
                    id: "density-policy".into(),
                    description: None,
                    quote_sets: [("quotes".into(), cal_utils::quote_set_ids(&quotes))].into(),
                    settings,
                    steps: vec![CalibrationStep {
                        id: "surface".into(),
                        quote_set: "quotes".into(),
                        params: StepParams::SviSurface(SviSurfaceParams {
                            vol_surface_id: "DENSITY".into(),
                            base_date,
                            underlying_ticker: UNDERLYING.into(),
                            discount_curve_id: Some(DISCOUNT_ID.into()),
                            target_expiries: vec![1.0],
                            target_strikes: strikes.clone(),
                            spot_override: Some(SPOT),
                            dividend_yield_override: Some(0.0),
                        }),
                    }],
                },
                market_data: market_data.clone(),
                prior_market: prior_market.clone(),
            };
            let result = engine::calibrate(&envelope).expect("return diagnostic report");
            let report = &result.result.step_reports["surface"];
            assert_eq!(
                report.success,
                !check_arbitrage || mode == ValidationMode::Warn,
                "{report:?}"
            );
            let detail = if mode == ValidationMode::Warn {
                report.metadata.get("validation_warning")
            } else {
                report.validation_error.as_ref()
            };
            if check_arbitrage {
                assert!(
                    detail.is_some_and(|text| text.contains("Durrleman")),
                    "{report:?}"
                );
            } else {
                assert!(detail.is_none(), "{report:?}");
            }
        }
    }
}
