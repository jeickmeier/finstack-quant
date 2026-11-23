use super::target::*;
use crate::api::schema::{SurfaceExtrapolationPolicy, SwaptionVolConvention, SwaptionVolParams};
use crate::config::CalibrationConfig;
use crate::quotes::{MarketQuote, QuoteId, VolQuote};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, DayCount, DayCountContext, StubKind,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolQuoteType;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::types::CurveId;
use finstack_quant_models::volatility::sabr::SabrParameters;
use finstack_quant_valuations_core::market::conventions::ids::SwaptionConventionId;
use finstack_quant_valuations_rates::swaption::contractual_swap_tenor_years;
use std::collections::BTreeMap;
use time::Month;

fn date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).expect("valid date")
}

#[test]
fn published_cube_fit_rejects_discarded_interior_smile() {
    let base = date(2025, Month::January, 1);
    let expiry = date(2026, Month::January, 1);
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (30.0, (-0.03_f64 * 30.0).exp())])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let context = MarketContext::new()
        .try_insert(discount)
        .expect("insert market curve");
    let mut p = params(base);
    p.vol_convention = SwaptionVolConvention::Normal;
    p.sabr_beta = 0.0;
    p.target_expiries = vec![1.0];
    p.target_tenors = vec![1.0, 5.0];
    p.vol_tolerance = Some(1e-5);
    let mut quotes = Vec::new();
    for (tenor, vol) in [(1, 0.005), (3, 0.04), (5, 0.02)] {
        for strike in [0.02, 0.025, 0.03, 0.035, 0.04] {
            quotes.push(MarketQuote::Vol(VolQuote::SwaptionVol {
                id: QuoteId::new(format!("tenor-{tenor}-strike-{strike}")),
                expiry,
                maturity: date(2026 + tenor, Month::January, 5),
                strike,
                vol,
                quote_type: VolQuoteType::Normal,
                convention: SwaptionConventionId::new("USD"),
            }));
        }
    }
    let (cube, report) = solve(&p, &quotes, &context, &CalibrationConfig::default())
        .expect("fit returns published cube diagnostics");
    let fitted = finstack_quant_models::volatility::get_cube_vol(
        &cube,
        1.0,
        3.0,
        0.03,
        finstack_quant_models::volatility::VolatilityConvention::Normal,
    )
    .expect("interior quote is inside the output axes");
    assert!((fitted - 0.0125).abs() < 1e-7);
    assert!(!report.success);
    assert!((report.max_residual - 0.0275).abs() < 1e-7);
    assert_eq!(report.residuals.len(), quotes.len());
    assert!((report.residuals["tenor-3-strike-0.03"] - (fitted - 0.04)).abs() < 1e-12);
}

#[test]
fn swaption_quote_currency_must_match_step_currency() {
    let p = params(date(2025, Month::January, 1));
    let quotes = [MarketQuote::Vol(VolQuote::SwaptionVol {
        id: QuoteId::new("eur-quote"),
        expiry: date(2026, Month::January, 1),
        maturity: date(2031, Month::January, 5),
        strike: 0.03,
        vol: 0.20,
        quote_type: VolQuoteType::BlackLognormal,
        convention: SwaptionConventionId::new("EUR"),
    })];
    let error = solve(
        &p,
        &quotes,
        &MarketContext::new(),
        &CalibrationConfig::default(),
    )
    .expect_err("EUR quotes cannot use a USD step's curve roles");
    assert!(
        error
            .to_string()
            .contains("currency EUR conflicts with step currency USD")
    );
}

fn params(base_date: Date) -> SwaptionVolParams {
    SwaptionVolParams {
        vol_surface_id: "USD-SWPTN".to_string(),
        base_date,
        discount_curve_id: CurveId::from("USD-OIS"),
        forward_id: None,
        currency: Currency::USD,
        vol_convention: SwaptionVolConvention::Lognormal,
        sabr_beta: 0.5,
        target_expiries: vec![1.0, 2.0],
        target_tenors: vec![5.0, 10.0],
        calendar_id: None,
        fixed_day_count: Some(DayCount::Act365F),
        swap_index: Some("USD-SOFR-3M".into()),
        vol_tolerance: None,
        sabr_extrapolation: SurfaceExtrapolationPolicy::Error,
        allow_sabr_missing_bucket_fallback: false,
    }
}

#[test]
fn date_offset_rejects_invalid_swaption_horizon() {
    let params = params(Date::MAX);
    let conventions = default_leg_conventions(&params).expect("leg conventions");
    for expiry_years in [1.0, f64::NAN, f64::INFINITY, f64::MAX] {
        assert!(
            calculate_forward_swap_rate_years(
                &params,
                expiry_years,
                5.0,
                &conventions,
                &MarketContext::new(),
            )
            .is_err()
        );
    }
}

fn settled_swap_dates(
    params: &SwaptionVolParams,
    expiry_years: f64,
    tenor_years: f64,
) -> (Date, Date, Date) {
    let conventions = default_leg_conventions(params).expect("leg conventions");
    let expiry = params
        .base_date
        .add_months((expiry_years * 12.0).round() as i32)
        .expect("valid date shift");
    let start = adjusted_swap_start(expiry, &conventions).expect("adjusted start");
    let maturity = start
        .add_months((tenor_years * 12.0).round() as i32)
        .expect("valid date shift");
    (expiry, start, maturity)
}

#[test]
fn adjusted_swap_start_applies_settlement_and_business_day_conventions() {
    let base_date = date(2024, Month::January, 2);
    let p = params(base_date);
    let conventions = default_leg_conventions(&p).expect("leg conventions");
    let expiry = date(2025, Month::January, 3);

    let start = adjusted_swap_start(expiry, &conventions).expect("adjusted swap start");

    assert_eq!(start, date(2025, Month::January, 7));
}

#[test]
fn quoted_vols_are_decimal_and_must_match_the_plan_convention() {
    let normal = validate_quoted_vol(0.005, VolQuoteType::Normal, SwaptionVolConvention::Normal)
        .expect("normal");
    assert!((normal - 0.005).abs() < 1e-12);

    let lognormal = validate_quoted_vol(
        0.20,
        VolQuoteType::BlackLognormal,
        SwaptionVolConvention::Lognormal,
    )
    .expect("lognormal");
    assert!((lognormal - 0.20).abs() < 1e-12);

    let shifted_convention = SwaptionVolConvention::ShiftedLognormal { shift: 0.02 };
    assert!(
        validate_quoted_vol(
            0.20,
            VolQuoteType::ShiftedBlackLognormal,
            shifted_convention,
        )
        .is_ok()
    );
    assert!(validate_quoted_vol(0.20, VolQuoteType::BlackLognormal, shifted_convention,).is_err());
    assert!(
        validate_quoted_vol(
            0.20,
            VolQuoteType::ShiftedBlackLognormal,
            SwaptionVolConvention::Lognormal,
        )
        .is_err()
    );

    let mismatch = validate_quoted_vol(
        0.20,
        VolQuoteType::BlackLognormal,
        SwaptionVolConvention::Normal,
    )
    .expect_err("mismatched convention");
    assert!(
        mismatch
            .to_string()
            .contains("conflicts with plan convention")
    );
}

#[test]
fn calibrate_normal_decimal_quotes_preserves_atm_vol_in_model_units() {
    let base_date = date(2024, Month::January, 2);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (30.0, 0.20)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve");

    let expiry_years: f64 = 1.0;
    let tenor_years: f64 = 5.0;
    let mut p = params(base_date);
    p.vol_convention = SwaptionVolConvention::Normal;
    p.sabr_beta = 0.0;
    p.vol_tolerance = Some(0.0020);
    let (expiry_date, swap_start, maturity_date) =
        settled_swap_dates(&p, expiry_years, tenor_years);
    let t_exp = DayCount::Act365F
        .year_fraction(base_date, expiry_date, DayCountContext::default())
        .expect("t_exp");
    let t_ten = contractual_swap_tenor_years(swap_start, maturity_date).expect("contractual tenor");

    p.target_expiries = vec![t_exp];
    p.target_tenors = vec![t_ten];

    let leg = default_leg_conventions(&p).expect("leg conventions");
    let fwd = calculate_forward_swap_rate_years(&p, expiry_years, tenor_years, &leg, &ctx)
        .expect("forward");

    let true_alpha = 0.0050;
    let sabr_true = SabrParameters {
        alpha: true_alpha,
        beta: 0.0,
        nu: 0.60,
        rho: -0.20,
        shift: None,
    };

    let strikes = vec![fwd - 0.005, fwd, fwd + 0.005, fwd + 0.010, fwd - 0.010];

    let mut quotes = Vec::new();
    for &k in &strikes {
        let vol_dec = sabr_true
            .implied_vol(fwd, k, t_exp, VolQuoteType::Normal)
            .expect("true vol");
        quotes.push(MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new(format!("USD-SWPTN-VOL-1Yx5Y-{k}")),
            expiry: expiry_date,
            maturity: maturity_date,
            strike: k,
            vol: vol_dec,
            quote_type: VolQuoteType::Normal,
            convention: SwaptionConventionId::new("USD-Annual"),
        }));
    }

    let config = CalibrationConfig::default();
    let (cube, report) = solve(&p, &quotes, &ctx, &config).expect("solve");
    assert!(
        report.iterations > 1,
        "report must retain actual LM work across deterministic starts"
    );
    assert!(
        report
            .metadata
            .get("sabr_winning_starts")
            .is_some_and(|value| value.contains("rho="))
    );
    assert!(report.metadata.contains_key("sabr_parameters_at_bounds"));

    // VolCube stores SABR params; verify calibrated alpha matches ground truth.
    // For beta=0 (normal SABR), alpha IS the ATM normal vol.
    let calibrated = cube.params_at(0, 0).expect("node (0, 0)");
    assert!(
        (calibrated.alpha() - true_alpha).abs() <= 0.0005,
        "alpha mismatch: calibrated={} true={}",
        calibrated.alpha(),
        true_alpha
    );
}

#[test]
fn calibrate_lognormal_decimal_quotes_preserves_atm_vol_in_model_units() {
    calibrate_black_quotes_with_shift(None);
}

#[test]
fn calibrated_shifted_cube_materialization_preserves_quote_coordinates() {
    calibrate_black_quotes_with_shift(Some(0.02));
}

fn calibrate_black_quotes_with_shift(shift: Option<f64>) {
    let base_date = date(2024, Month::January, 2);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (30.0, 0.20)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve");

    let expiry_years: f64 = 1.0;
    let tenor_years: f64 = 5.0;
    let mut p = params(base_date);
    p.vol_convention = match shift {
        Some(shift) => SwaptionVolConvention::ShiftedLognormal { shift },
        None => SwaptionVolConvention::Lognormal,
    };
    let quote_type = if shift.is_some() {
        VolQuoteType::ShiftedBlackLognormal
    } else {
        VolQuoteType::BlackLognormal
    };
    p.sabr_beta = 0.5;
    p.vol_tolerance = Some(0.0020);
    let (expiry_date, swap_start, maturity_date) =
        settled_swap_dates(&p, expiry_years, tenor_years);
    let t_exp = DayCount::Act365F
        .year_fraction(base_date, expiry_date, DayCountContext::default())
        .expect("t_exp");
    let t_ten = contractual_swap_tenor_years(swap_start, maturity_date).expect("contractual tenor");

    p.target_expiries = vec![t_exp];
    p.target_tenors = vec![t_ten];

    let leg = default_leg_conventions(&p).expect("leg conventions");
    let fwd = calculate_forward_swap_rate_years(&p, expiry_years, tenor_years, &leg, &ctx)
        .expect("forward");

    let sabr_true = SabrParameters {
        alpha: 0.020,
        beta: p.sabr_beta,
        nu: 0.30,
        rho: -0.20,
        shift,
    };

    let strikes = vec![fwd - 0.010, fwd - 0.005, fwd, fwd + 0.005, fwd + 0.010];

    let mut quotes = Vec::new();
    for &k in &strikes {
        let vol_dec = sabr_true
            .implied_vol(fwd, k, t_exp, quote_type)
            .expect("true vol");
        quotes.push(MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new(format!("USD-SWPTN-VOL-LN-1Yx5Y-{k}")),
            expiry: expiry_date,
            maturity: maturity_date,
            strike: k,
            vol: vol_dec,
            quote_type: if shift.is_some() {
                VolQuoteType::ShiftedBlackLognormal
            } else {
                VolQuoteType::BlackLognormal
            },
            convention: SwaptionConventionId::new("USD-Annual"),
        }));
    }

    let config = CalibrationConfig {
        solver: crate::solver::SolverConfig::default().with_max_iterations(500),
        ..CalibrationConfig::default()
    };
    let (cube, _report) = solve(&p, &quotes, &ctx, &config).expect("solve");

    let fitted_atm = finstack_quant_models::volatility::get_cube_vol(
        &cube,
        t_exp,
        t_ten,
        fwd,
        finstack_quant_models::volatility::VolatilityConvention::Lognormal,
    )
    .expect("cube vol");
    let true_atm = sabr_true
        .implied_vol(fwd, fwd, t_exp, quote_type)
        .expect("true atm");

    assert!(
        (fitted_atm - true_atm).abs() <= 0.0005,
        "atm mismatch: fitted={} true={}",
        fitted_atm,
        true_atm
    );
    let surface = finstack_quant_models::volatility::materialize_cube_tenor_slice(
        &cube,
        t_ten,
        &strikes,
        finstack_quant_models::volatility::VolatilityConvention::Lognormal,
        finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol,
    )
    .expect("materialized cube");
    match shift {
        Some(expected) => {
            assert_eq!(surface.quote_type(), VolQuoteType::ShiftedBlackLognormal);
            assert_eq!(surface.displacements(), Some(&[expected][..]));
        }
        None => {
            assert_eq!(surface.quote_type(), VolQuoteType::BlackLognormal);
            assert_eq!(surface.displacements(), None);
        }
    }
}

/// Item 3: a failed SABR expiry/tenor bucket must fail the whole surface report.
///
/// One bucket (1Y×5Y) is given five well-behaved strikes and calibrates cleanly.
/// A second bucket (2Y×5Y) is given only two strikes — below the three-strike SABR
/// minimum — so it lands in `bucket_errors` and is `continue`d. The target grid
/// points only at the good bucket, so the `VolCube` still builds. Pre-fix the report
/// was assembled from the surviving bucket's residuals alone and reported
/// `success = true`; post-fix the non-empty `bucket_errors` must force
/// `success = false`.
#[test]
fn item3_failed_sabr_bucket_fails_surface_report() {
    let base_date = date(2024, Month::January, 2);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (30.0, 0.20)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve");

    // --- Good bucket: 1Y x 5Y, five strikes. ---
    let good_expiry_years = 1.0_f64;
    let tenor_years = 5.0_f64;
    let mut p = params(base_date);
    p.vol_convention = SwaptionVolConvention::Lognormal;
    p.sabr_beta = 0.5;
    p.vol_tolerance = Some(0.0020);
    let (good_expiry_date, good_swap_start, good_maturity_date) =
        settled_swap_dates(&p, good_expiry_years, tenor_years);
    let good_t_exp = DayCount::Act365F
        .year_fraction(base_date, good_expiry_date, DayCountContext::default())
        .expect("t_exp");
    let good_t_ten = contractual_swap_tenor_years(good_swap_start, good_maturity_date)
        .expect("contractual tenor");

    // Target only the good bucket so the cube can be built from it alone.
    p.target_expiries = vec![good_t_exp];
    p.target_tenors = vec![good_t_ten];

    let leg = default_leg_conventions(&p).expect("leg conventions");
    let good_fwd =
        calculate_forward_swap_rate_years(&p, good_expiry_years, tenor_years, &leg, &ctx)
            .expect("forward");
    let sabr_true = SabrParameters {
        alpha: 0.020,
        beta: p.sabr_beta,
        nu: 0.30,
        rho: -0.20,
        shift: None,
    };

    let mut quotes = Vec::new();
    for &k in &[
        good_fwd - 0.010,
        good_fwd - 0.005,
        good_fwd,
        good_fwd + 0.005,
        good_fwd + 0.010,
    ] {
        let vol_dec = sabr_true
            .implied_vol(good_fwd, k, good_t_exp, VolQuoteType::BlackLognormal)
            .expect("true vol");
        quotes.push(MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new(format!("GOOD-1Yx5Y-{k}")),
            expiry: good_expiry_date,
            maturity: good_maturity_date,
            strike: k,
            vol: vol_dec,
            quote_type: VolQuoteType::BlackLognormal,
            convention: SwaptionConventionId::new("USD-Annual"),
        }));
    }

    // --- Bad bucket: 2Y x 5Y, only TWO strikes (< 3 required for SABR). ---
    let (bad_expiry_date, _, bad_maturity_date) = settled_swap_dates(&p, 2.0, tenor_years);
    for &k in &[good_fwd, good_fwd + 0.005] {
        quotes.push(MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new(format!("BAD-2Yx5Y-{k}")),
            expiry: bad_expiry_date,
            maturity: bad_maturity_date,
            strike: k,
            vol: 0.20,
            quote_type: VolQuoteType::BlackLognormal,
            convention: SwaptionConventionId::new("USD-Annual"),
        }));
    }

    let config = CalibrationConfig {
        solver: crate::solver::SolverConfig::default().with_max_iterations(500),
        ..CalibrationConfig::default()
    };

    let (_cube, report) = solve(&p, &quotes, &ctx, &config).expect("solve builds the cube");

    assert!(
        !report.success,
        "Item 3: a surface with a failed SABR bucket must NOT report success \
         (convergence_reason: {})",
        report.convergence_reason,
    );
    assert!(
        !report.validation_passed,
        "validation_passed must be false when a bucket failed"
    );
    assert_eq!(
        report.metadata.get("failed_buckets").map(String::as_str),
        Some("1"),
        "exactly one bucket should be recorded as failed"
    );
    assert!(
        report
            .validation_error
            .as_deref()
            .is_some_and(|e| e.contains("did not calibrate")),
        "validation_error should describe the failed bucket(s): {:?}",
        report.validation_error,
    );
}

#[test]
fn shifted_lognormal_uses_explicit_shift_and_does_not_auto_shift() {
    let base_date = date(2024, Month::January, 2);
    // Negative forwards via an explicit forward curve (discount curve remains standard).
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (30.0, 0.20)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let fwd_curve = finstack_quant_core::market_data::term_structures::ForwardCurve::flat(
        "USD-FWD",
        base_date,
        -0.01,
        DayCount::Act365F,
    )
    .expect("forward curve");
    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(fwd_curve)
        .expect("insert market curve");

    let expiry_years: f64 = 1.0;
    let tenor_years: f64 = 5.0;
    let mut p = params(base_date);
    p.forward_id = Some("USD-FWD".to_string());
    p.vol_convention = SwaptionVolConvention::ShiftedLognormal { shift: 1e-6 };
    p.sabr_beta = 0.5;
    let (expiry_date, swap_start, maturity_date) =
        settled_swap_dates(&p, expiry_years, tenor_years);
    let t_exp = DayCount::Act365F
        .year_fraction(base_date, expiry_date, DayCountContext::default())
        .expect("t_exp");
    let t_ten = contractual_swap_tenor_years(swap_start, maturity_date).expect("contractual tenor");

    p.target_expiries = vec![t_exp];
    p.target_tenors = vec![t_ten];

    let leg = default_leg_conventions(&p).expect("leg conventions");
    let fwd = calculate_forward_swap_rate_years(&p, expiry_years, tenor_years, &leg, &ctx)
        .expect("forward");
    assert!(fwd < 0.0, "expected negative forward for test; got {}", fwd);

    let strikes = vec![fwd - 0.002, fwd, fwd + 0.002, fwd + 0.005, fwd - 0.005];
    let mut quotes = Vec::new();
    for &k in &strikes {
        // Explicitly displaced Black quote; exact vol does not matter here.
        quotes.push(MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new(format!("USD-SWPTN-VOL-SLN-1Yx5Y-{k}")),
            expiry: expiry_date,
            maturity: maturity_date,
            strike: k,
            vol: 0.20,
            quote_type: VolQuoteType::ShiftedBlackLognormal,
            convention: SwaptionConventionId::new("USD-Annual"),
        }));
    }

    let config = CalibrationConfig::default();
    let err = solve(&p, &quotes, &ctx, &config)
        .expect_err("insufficient explicit shift should not be auto-adjusted");
    assert!(
        err.to_string().contains("Swaption SABR params missing"),
        "unexpected error: {}",
        err
    );

    // Plain Black inputs cannot acquire a hidden auto-shift merely
    // because their forwards are outside the lognormal domain.
    p.vol_convention = SwaptionVolConvention::Lognormal;
    for quote in &mut quotes {
        if let MarketQuote::Vol(VolQuote::SwaptionVol { quote_type, .. }) = quote {
            *quote_type = VolQuoteType::BlackLognormal;
        }
    }
    assert!(solve(&p, &quotes, &ctx, &config).is_err());
}

#[test]
fn forward_swap_rate_single_curve_telescopes_when_float_payments_are_unadjusted() {
    let base_date = date(2024, Month::January, 2);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (30.0, 0.20)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve");

    let p = params(base_date);
    let mut leg = default_leg_conventions(&p).expect("leg conventions");
    leg.float_business_day_convention = BusinessDayConvention::Unadjusted;

    let expiry_years: f64 = 1.0;
    let tenor_years: f64 = 5.0;
    let (_, swap_start, maturity_date) = settled_swap_dates(&p, expiry_years, tenor_years);

    let disc_ref = ctx
        .get_discount(p.discount_curve_id.as_ref())
        .expect("disc");
    let pv01 =
        calculate_pv01_proper(swap_start, maturity_date, &leg, disc_ref.as_ref()).expect("pv01");
    let t_start = disc_ref
        .day_count()
        .year_fraction(disc_ref.base_date(), swap_start, DayCountContext::default())
        .expect("t_start");
    let t_end = disc_ref
        .day_count()
        .year_fraction(
            disc_ref.base_date(),
            maturity_date,
            DayCountContext::default(),
        )
        .expect("t_end");
    let expected =
        (disc_ref.df(t_start).expect("valid") - disc_ref.df(t_end).expect("valid")) / pv01;

    let actual = calculate_forward_swap_rate_years(&p, expiry_years, tenor_years, &leg, &ctx)
        .expect("forward");

    assert!(
        (actual - expected).abs() < 1e-12,
        "forward mismatch: actual={} expected={}",
        actual,
        expected
    );
}

#[test]
fn forward_swap_rate_multi_curve_uses_df_implied_period_rates() {
    let base_date = date(2024, Month::January, 2);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (5.0, 0.75)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let forward = finstack_quant_core::market_data::term_structures::ForwardCurve::from_zero_rates(
        "USD-FWD",
        base_date,
        &[(0.0, 0.01), (1.0, 0.03), (2.0, 0.08), (3.0, 0.02)],
        finstack_quant_core::math::Compounding::Continuous,
        DayCount::Act365F,
        finstack_quant_core::math::interp::InterpStyle::LogLinear,
        finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
    )
    .expect("forward curve");
    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(forward)
        .expect("insert market curve");

    let mut p = params(base_date);
    p.forward_id = Some("USD-FWD".to_string());
    let leg = default_leg_conventions(&p).expect("leg conventions");
    let expiry_years = 1.0;
    let tenor_years = 1.0;
    let (_, swap_start, swap_end) = settled_swap_dates(&p, expiry_years, tenor_years);
    let disc = ctx.get_discount("USD-OIS").expect("discount curve");
    let fwd = ctx.get_forward("USD-FWD").expect("forward curve");
    let pv01 = calculate_pv01_proper(swap_start, swap_end, &leg, disc.as_ref()).expect("pv01");
    let schedule = finstack_quant_cashflows::builder::build_periods(
        finstack_quant_cashflows::builder::BuildPeriodsParams {
            start: swap_start,
            end: swap_end,
            frequency: leg.float_frequency,
            stub: StubKind::ShortBack,
            business_day_convention: leg.float_business_day_convention,
            calendar_id: leg.calendar_id,
            end_of_month: false,
            day_count: leg.float_day_count,
            payment_lag_days: leg.float_payment_lag_days,
            reset_lag_days: Some(leg.float_reset_lag_days),
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        },
    )
    .expect("floating schedule");
    let mut expected_float_pv = 0.0;
    let mut legacy_float_pv = 0.0;
    for period in schedule {
        let accrual = leg
            .float_day_count
            .year_fraction(
                period.accrual_start,
                period.accrual_end,
                DayCountContext::default(),
            )
            .expect("accrual");
        let t_start = fwd
            .day_count()
            .year_fraction(
                fwd.base_date(),
                period.accrual_start,
                DayCountContext::default(),
            )
            .expect("forward start");
        let t_end = fwd
            .day_count()
            .year_fraction(
                fwd.base_date(),
                period.accrual_end,
                DayCountContext::default(),
            )
            .expect("forward end");
        let t_pay = disc
            .day_count()
            .year_fraction(
                disc.base_date(),
                period.payment_date,
                DayCountContext::default(),
            )
            .expect("payment time");
        let discount = disc.df(t_pay).expect("valid");
        expected_float_pv += (fwd.df(t_start).expect("projection start DF")
            / fwd.df(t_end).expect("projection end DF")
            - 1.0)
            * discount;
        let continuous_rate = (fwd.df(t_start).expect("start factor").ln()
            - fwd.df(t_end).expect("end factor").ln())
            / (t_end - t_start);
        legacy_float_pv += continuous_rate * accrual * discount;
    }
    let expected = expected_float_pv / pv01;
    let legacy = legacy_float_pv / pv01;
    assert!(
        (expected - legacy).abs() > 1e-8,
        "test curve must distinguish simple and continuously compounded rates"
    );

    let actual = calculate_forward_swap_rate_years(&p, expiry_years, tenor_years, &leg, &ctx)
        .expect("multi-curve forward swap rate");
    assert!(
        (actual - expected).abs() < 1e-12,
        "multi-curve forward mismatch: actual={actual}, expected={expected}, legacy={legacy}"
    );
}

#[test]
fn sabr_param_bilinear_interpolation_interpolates_in_log_space_for_positive_params() {
    let mut grid: BTreeMap<(u64, u64), SabrParameters> = BTreeMap::new();
    let p00 = SabrParameters {
        alpha: 0.01,
        beta: 0.5,
        nu: 0.20,
        rho: -0.20,
        shift: Some(0.0),
    };
    let p10 = SabrParameters {
        alpha: 0.02,
        beta: 0.5,
        nu: 0.40,
        rho: 0.00,
        shift: Some(0.0),
    };
    let p01 = SabrParameters {
        alpha: 0.02,
        beta: 0.5,
        nu: 0.40,
        rho: -0.40,
        shift: Some(0.0),
    };
    let p11 = SabrParameters {
        alpha: 0.04,
        beta: 0.5,
        nu: 0.80,
        rho: 0.20,
        shift: Some(0.0),
    };

    grid.insert((to_basis_points(1.0), to_basis_points(5.0)), p00);
    grid.insert((to_basis_points(2.0), to_basis_points(5.0)), p10);
    grid.insert((to_basis_points(1.0), to_basis_points(10.0)), p01);
    grid.insert((to_basis_points(2.0), to_basis_points(10.0)), p11);

    let mid = interpolate_sabr_params_bilinear(
        1.5,
        7.5,
        &grid,
        (&[1.0, 2.0], &[5.0, 10.0]),
        SurfaceExtrapolationPolicy::Error,
        false,
    )
    .expect("interpolated params");

    assert!(mid.alpha.is_finite() && mid.alpha > 0.0);
    assert!(mid.nu.is_finite() && mid.nu > 0.0);
    assert!(mid.rho > -1.0 && mid.rho < 1.0);
    assert!((mid.beta - 0.5).abs() < 1e-12);
}
