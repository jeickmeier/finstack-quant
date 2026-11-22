use super::*;
use crate::fx_barrier_option::types::FxBarrierOption;
use crate::shared::FxOptionInputs;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::{
    VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_core::types::BarrierType;
use finstack_quant_core::types::PayoutTiming;
use finstack_quant_models::closed_form::BarrierParams;
use finstack_quant_models::closed_form::barrier_rebate;
use finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricerConfig;
use finstack_quant_models::types::OptionType;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::Monitoring;
use finstack_quant_valuations_core::pricer::{InstrumentType, ModelKey, Pricer, PricingError};
use std::sync::Arc;
use time::Month;

#[test]
fn analytical_barrier_error_preserves_category_and_context() {
    let mut inst = FxBarrierOption::example().expect("example");
    let start = inst.monitoring_start_date.expect("example start");
    inst.monitoring = Monitoring::Discrete {
        observation_dates: vec![start, inst.expiry],
    };

    let error = FxBarrierOptionAnalyticalPricer
        .price_dyn(&inst, &MarketContext::new(), start)
        .expect_err("discrete contract must reject analytical model");
    let PricingError::InvalidInput { context, .. } = error else {
        panic!("monitoring/model mismatch must remain an invalid-input error");
    };
    assert_eq!(context.instrument_id.as_deref(), Some(inst.id.as_str()));
    assert_eq!(
        context.instrument_type,
        Some(InstrumentType::FxBarrierOption)
    );
    assert_eq!(context.model, Some(ModelKey::FxBarrierBSContinuous));
    assert!(
        context
            .curve_ids
            .contains(&inst.domestic_discount_curve_id.to_string())
    );
    assert!(context.curve_ids.contains(&inst.vol_surface_id.to_string()));
}

#[test]
fn analytical_barrier_rejects_future_monitoring_before_market_lookup() {
    let mut inst = FxBarrierOption::example().expect("example");
    let as_of = inst.monitoring_start_date.expect("example start");
    inst.monitoring_start_date = Some(inst.expiry);
    let error = FxBarrierOptionAnalyticalPricer
        .price_dyn(&inst, &MarketContext::new(), as_of)
        .expect_err("future monitoring");
    assert!(error.to_string().contains("Future FX barrier monitoring"));
    let PricingError::InvalidInput { context, .. } = error else {
        panic!("expected invalid-input error");
    };
    assert_eq!(context.model, Some(ModelKey::FxBarrierBSContinuous));
    assert_eq!(context.instrument_id.as_deref(), Some(inst.id.as_str()));
    let default_error = inst
        .value(&MarketContext::new(), as_of)
        .expect_err("future monitoring");
    assert!(
        default_error
            .to_string()
            .contains("Future FX barrier monitoring")
    );
}

#[test]
fn mc_expiry_only_down_out_call_matches_vanilla() {
    let mut inst = FxBarrierOption::example().expect("example");
    let as_of = inst.monitoring_start_date.expect("example start");
    inst.expiry = as_of + time::Duration::days(365);
    inst.monitoring_start_date = Some(inst.expiry);
    inst.barrier_type = BarrierType::DownAndOut;
    inst.instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.15);
    let mut market = MarketContext::new().insert_price(
        "EURUSD-SPOT",
        MarketScalar::unitless(1.20).expect("valid scalar"),
    );
    for id in ["USD-OIS", "EUR-OIS"] {
        market = market
            .try_insert(
                DiscountCurve::builder(id)
                    .base_date(as_of)
                    .day_count(finstack_quant_core::dates::DayCount::Act365F)
                    .knots([(0.0, 1.0), (1.0, (-0.03_f64).exp())])
                    .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                    .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                    .build()
                    .expect("flat curve"),
            )
            .expect("insert market curve");
    }
    let pricer = FxBarrierOptionMcPricer {
        config: PathDependentPricerConfig {
            num_paths: 65_536,
            steps_per_year: 1.0,
            min_steps: 1,
            ..Default::default()
        },
    };
    // K > H makes the terminal barrier redundant for a call. Include a
    // barrier already breached today to detect any pre-start monitoring.
    for (strike, barrier) in [(1.15, 1.10), (1.25, 1.21)] {
        inst.strike = strike;
        inst.barrier = barrier;
        let result = pricer.price_dyn(&inst, &market, as_of).expect("mc price");
        let Some(finstack_quant_valuations_core::results::ValuationDetails::MonteCarlo(
            diagnostics,
        )) = result.details
        else {
            panic!("mc diagnostics");
        };
        let expected = inst.notional.amount()
            * (-0.03_f64).exp()
            * finstack_quant_models::closed_form::black_call(1.20, strike, 0.15, 1.0);
        assert!(
            (result.value.amount() - expected).abs() < 5.0 * diagnostics.standard_error,
            "expiry-only barrier must equal vanilla within MC uncertainty: actual={}, expected={expected}, stderr={}",
            result.value.amount(),
            diagnostics.standard_error
        );
    }
}

#[test]
fn expired_up_and_in_call_returns_intrinsic_when_hit() {
    let mut inst = FxBarrierOption::example().expect("example");
    inst.option_type = OptionType::Call;
    inst.barrier_type = BarrierType::UpAndIn;
    inst.strike = 1.10;
    inst.barrier = 1.20;
    inst.rebate = None;
    inst.observed_barrier_breached = Some(true);

    let per_unit = expired_barrier_value_per_unit(&inst, 1.25).expect("expired value");
    assert!((per_unit - 0.15).abs() < 1e-12);
}

#[test]
fn expired_down_and_out_put_returns_intrinsic_when_not_hit() {
    let mut inst = FxBarrierOption::example().expect("example");
    inst.option_type = OptionType::Put;
    inst.barrier_type = BarrierType::DownAndOut;
    inst.strike = 1.10;
    inst.barrier = 0.90;
    inst.rebate = None;
    inst.observed_barrier_breached = Some(false);

    // Barrier not hit at expiry => down-and-out stays active => intrinsic applies.
    let per_unit = expired_barrier_value_per_unit(&inst, 1.00).expect("expired value");
    assert!((per_unit - 0.10).abs() < 1e-12);
}

#[test]
fn processed_knockout_rebate_is_paid_exactly_once_across_expiry() {
    let mut inst = FxBarrierOption::example().expect("example");
    inst.barrier_type = BarrierType::UpAndOut;
    inst.observed_barrier_breached = Some(true);
    inst.rebate = Some(Money::from((50_000_i64, Currency::USD)));
    inst.rebate_timing = Some(PayoutTiming::AtHit);
    inst.monitoring = Monitoring::Continuous;
    inst.instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.15);
    let before = inst.expiry - time::Duration::days(1);
    let after = inst.expiry + time::Duration::days(1);
    let mut market = MarketContext::new().insert_price(
        "EURUSD-SPOT",
        MarketScalar::unitless(1.25).expect("valid scalar"),
    );
    for id in ["USD-OIS", "EUR-OIS"] {
        market = market
            .try_insert(
                DiscountCurve::builder(id)
                    .base_date(before)
                    .knots([(0.0, 1.0), (1.0, 1.0)])
                    .day_count(finstack_quant_core::dates::DayCount::Act365F)
                    .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                    .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                    .build()
                    .expect("curve"),
            )
            .expect("insert market curve");
    }
    for timing in [PayoutTiming::AtHit, PayoutTiming::AtExpiry] {
        inst.rebate_timing = Some(timing);
        for as_of in [before, inst.expiry, after] {
            let remaining = if timing == PayoutTiming::AtExpiry && as_of <= inst.expiry {
                50_000.0
            } else {
                0.0
            };
            let booked = 50_000.0 - remaining;
            for result in [
                FxBarrierOptionAnalyticalPricer.price_dyn(&inst, &market, as_of),
                FxBarrierOptionMcPricer::new().price_dyn(&inst, &market, as_of),
            ] {
                let pv = result.expect("processed hit value").value.amount();
                assert!((pv - remaining).abs() < 1e-8);
                assert!((booked + pv - 50_000.0).abs() < 1e-8);
            }
        }
    }
    // A same-day processed hit uses the same settled-event state; the
    // boolean does not introduce another unpaid at-hit receivable.
    inst.rebate_timing = Some(PayoutTiming::AtHit);
    assert_eq!(
        expired_barrier_value_per_unit(&inst, 1.25).expect("settled"),
        0.0
    );
    // A knock-in no-hit rebate is only determined at expiry: at-hit timing
    // is rejected, and at-expiry timing matures the rebate.
    inst.barrier_type = BarrierType::UpAndIn;
    inst.observed_barrier_breached = Some(false);
    assert!(inst.validate().is_err());
    assert!(
        expired_barrier_value_per_unit(&inst, 1.15)
            .map(|value| value.is_nan())
            .unwrap_or(true)
    );
    inst.rebate_timing = Some(PayoutTiming::AtExpiry);
    assert!(
        (expired_barrier_value_per_unit(&inst, 1.15).expect("KI rebate") * inst.notional.amount()
            - 50_000.0)
            .abs()
            < 1e-8
    );
}

#[test]
fn expired_up_and_out_with_hit_pays_rebate_only() {
    let mut inst = FxBarrierOption::example().expect("example");
    inst.option_type = OptionType::Call;
    inst.barrier_type = BarrierType::UpAndOut;
    inst.strike = 1.10;
    inst.barrier = 1.20;
    // Total rebate = 0.02 USD per EUR on the 1m EUR example notional.
    inst.rebate = Some(Money::from((20_000_i64, Currency::USD)));
    inst.observed_barrier_breached = Some(true);

    inst.rebate_timing = Some(PayoutTiming::AtExpiry);
    // A recorded hit leaves only the explicitly deferred rebate unpaid.
    let per_unit = expired_barrier_value_per_unit(&inst, 1.25).expect("expired value");
    assert!((per_unit - 0.02).abs() < 1e-12);
}

#[test]
fn expired_up_and_in_with_no_hit_pays_rebate_only() {
    let mut inst = FxBarrierOption::example().expect("example");
    inst.option_type = OptionType::Call;
    inst.barrier_type = BarrierType::UpAndIn;
    inst.strike = 1.10;
    inst.barrier = 1.20;
    // Total rebate = 0.02 USD per EUR on the 1m EUR example notional.
    inst.rebate = Some(Money::from((20_000_i64, Currency::USD)));
    inst.rebate_timing = Some(PayoutTiming::AtExpiry);
    inst.observed_barrier_breached = Some(false);

    let per_unit = expired_barrier_value_per_unit(&inst, 1.25).expect("expired value");
    assert!((per_unit - 0.02).abs() < 1e-12);
}

#[test]
fn expired_fx_barrier_requires_observed_state() {
    let mut inst = FxBarrierOption::example().expect("example");
    inst.observed_barrier_breached = None;

    let err = expired_barrier_value_per_unit(&inst, 1.25).expect_err("missing observed state");
    assert!(
        err.to_string().contains("observed_barrier_breached"),
        "unexpected error: {err}"
    );
}

#[test]
fn validation_allows_barrier_equal_to_strike() {
    let mut inst = FxBarrierOption::example().expect("example");
    inst.strike = 1.10;
    inst.barrier = 1.10;

    inst.validate()
        .expect("equal strike/barrier should remain valid");
}

#[test]
fn expired_analytical_value_only_requires_observed_state_and_spot() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let mut option = FxBarrierOption::example().expect("example");
    option.expiry = as_of;
    option.monitoring = Monitoring::Continuous;
    option.option_type = OptionType::Call;
    option.barrier_type = BarrierType::UpAndIn;
    option.rebate = Some(Money::from((20_000_i64, Currency::USD)));
    option.rebate_timing = Some(PayoutTiming::AtExpiry);
    option.observed_barrier_breached = Some(false);

    let market = MarketContext::new().insert_price(
        "EURUSD-SPOT",
        MarketScalar::unitless(1.25).expect("valid scalar"),
    );

    let pv = option
        .value(&market, as_of)
        .expect("expired analytical value");
    assert!(
        (pv.amount() - 20_000.0).abs() < 1e-8,
        "expired FX barrier should settle from observed state and spot only, got {}",
        pv.amount()
    );
}

#[test]
fn analytical_pricer_handles_zero_vol_knock_in_rebate_end_to_end() {
    let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
    let expiry = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let option = FxBarrierOption::builder()
        .id("FXBAR-ZERO-VOL-UPIN".into())
        .strike(1.10)
        .barrier(1.20)
        .rebate(Money::from((20_000_i64, Currency::USD)))
        .rebate_timing(PayoutTiming::AtExpiry)
        .option_type(OptionType::Call)
        .barrier_type(BarrierType::UpAndIn)
        .monitoring_start_date(as_of)
        .expiry(expiry)
        .notional(Money::from((1_000_000_i64, Currency::EUR)))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .monitoring(Monitoring::Continuous)
        .domestic_discount_curve_id("USD-OIS".into())
        .foreign_discount_curve_id("EUR-OIS".into())
        .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
        .vol_surface_id("EURUSD-VOL".into())
        .attributes(finstack_quant_core::types::Attributes::new())
        .build()
        .expect("fx barrier option");

    let market = MarketContext::new()
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 1.0)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("dom curve"),
        )
        .expect("insert market curve")
        .try_insert(
            DiscountCurve::builder("EUR-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 1.0)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("for curve"),
        )
        .expect("insert market curve")
        .insert_surface(
            VolSurface::builder("EURUSD-VOL")
                .expiries(&[0.25, 0.5, 1.0])
                .secondary_values(&[1.0, 1.1, 1.2])
                .secondary_axis(VolSurfaceAxis::Strike)
                .quote_type(VolQuoteType::BlackLognormal)
                .interpolation_mode(VolInterpolationMode::Vol)
                .row(&[0.0, 0.0, 0.0])
                .row(&[0.0, 0.0, 0.0])
                .row(&[0.0, 0.0, 0.0])
                .build()
                .expect("vol surface"),
        )
        .insert_price(
            "EURUSD-SPOT",
            MarketScalar::unitless(1.10).expect("valid scalar"),
        );

    let pv = option.value(&market, as_of).expect("fx barrier pv");
    assert!(
        (pv.amount() - 20_000.0).abs() < 1e-8,
        "zero-vol no-hit knock-in rebate should settle at rebate * notional, got {}",
        pv.amount()
    );
    assert_eq!(pv.currency(), Currency::USD);
}

#[test]
fn analytical_pricer_finite_vol_rebate_delta_matches_closed_form_and_notional() {
    let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
    let expiry = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let notional = 1_000_000.0;
    let rebate = 0.02;

    let build_option = |id: &str, rebate: Option<f64>| {
        FxBarrierOption::builder()
            .id(id.into())
            .strike(1.10)
            .barrier(1.20)
            .rebate_opt(rebate.map(|per_unit| {
                Money::new(per_unit * notional, Currency::USD).expect("valid money fixture")
            }))
            .rebate_timing_opt(rebate.map(|_| PayoutTiming::AtExpiry))
            .option_type(OptionType::Call)
            .barrier_type(BarrierType::UpAndOut)
            .monitoring_start_date(as_of)
            .expiry(expiry)
            .notional(Money::new(notional, Currency::EUR).expect("valid money fixture"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(DayCount::Act365F)
            .monitoring(Monitoring::Continuous)
            .domestic_discount_curve_id("USD-OIS".into())
            .foreign_discount_curve_id("EUR-OIS".into())
            .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
            .vol_surface_id("EURUSD-VOL".into())
            .attributes(finstack_quant_core::types::Attributes::new())
            .build()
            .expect("finite-vol FX barrier option")
    };

    let base_option = build_option("FXBAR-FINITE-VOL-NO-REBATE", None);
    let rebate_option = build_option("FXBAR-FINITE-VOL-REBATE", Some(rebate));
    let domestic_rate: f64 = 0.03;
    let foreign_rate: f64 = 0.01;
    let volatility: f64 = 0.15;
    let market = MarketContext::new()
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (5.0, (-domestic_rate * 5.0).exp())])
                .interp(InterpStyle::LogLinear)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("domestic curve"),
        )
        .expect("insert market curve")
        .try_insert(
            DiscountCurve::builder("EUR-OIS")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (5.0, (-foreign_rate * 5.0).exp())])
                .interp(InterpStyle::LogLinear)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("foreign curve"),
        )
        .expect("insert market curve")
        .insert_surface(
            VolSurface::builder("EURUSD-VOL")
                .expiries(&[0.5, 1.0, 2.0])
                .secondary_values(&[1.0, 1.1, 1.2])
                .secondary_axis(VolSurfaceAxis::Strike)
                .quote_type(VolQuoteType::BlackLognormal)
                .interpolation_mode(VolInterpolationMode::Vol)
                .row(&[volatility, volatility, volatility])
                .row(&[volatility, volatility, volatility])
                .row(&[volatility, volatility, volatility])
                .build()
                .expect("vol surface"),
        )
        .insert_price(
            "EURUSD-SPOT",
            MarketScalar::unitless(1.10).expect("valid scalar"),
        );

    let base_pv = base_option
        .value(&market, as_of)
        .expect("finite-vol FX barrier price without rebate");
    let rebate_pv = rebate_option
        .value(&market, as_of)
        .expect("finite-vol FX barrier price with rebate");

    let spot = resolve_fx_spot(&rebate_option, &market, as_of).expect("spot");
    let FxOptionInputs {
        r_domestic: r_dom,
        r_foreign: r_for,
        sigma,
        ..
    } = collect_fx_barrier_inputs(&rebate_option, &market, as_of, spot)
        .expect("finite-vol FX barrier inputs");
    let t = rebate_option
        .day_count
        .year_fraction(as_of, expiry, DayCountContext::default())
        .expect("year fraction");
    let params = BarrierParams::new(
        spot,
        rebate_option.strike,
        rebate_option.barrier,
        t,
        r_dom,
        r_for,
        sigma,
    );
    let expected_per_unit = barrier_rebate(
        &params,
        rebate,
        BarrierType::UpAndOut,
        PayoutTiming::AtExpiry,
    );
    let continuous_per_unit = barrier_rebate(
        &params,
        rebate,
        BarrierType::UpAndOut,
        PayoutTiming::AtExpiry,
    );
    let actual_delta = rebate_pv.amount() - base_pv.amount();
    let expected_delta = expected_per_unit * notional;

    assert!(
        sigma.is_finite() && sigma > 0.0,
        "test must exercise a finite positive volatility, got {sigma}"
    );
    assert!(
        (expected_per_unit - continuous_per_unit).abs() < 1e-12,
        "at-expiry rebate must match the continuous legacy formula"
    );
    assert!(
        (actual_delta - expected_delta).abs() < 0.02,
        "instrument rebate delta {actual_delta} should equal per-unit rebate {expected_per_unit} scaled by notional {notional}"
    );
    assert_eq!(base_pv.currency(), Currency::USD);
    assert_eq!(rebate_pv.currency(), Currency::USD);
}

#[test]
fn validation_rejects_currency_mismatch_and_invalid_numeric_fields() {
    let mut mismatched = FxBarrierOption::example().expect("example");
    mismatched.notional = Money::from((1_000_000_i64, Currency::USD));
    let err = mismatched.validate().expect_err("currency mismatch");
    assert!(err.to_string().contains("Currency mismatch"));

    let mut bad_strike = FxBarrierOption::example().expect("example");
    bad_strike.strike = 0.0;
    assert!(
        bad_strike
            .validate()
            .expect_err("bad strike")
            .to_string()
            .contains("strike")
    );

    let mut bad_barrier = FxBarrierOption::example().expect("example");
    bad_barrier.barrier = f64::NAN;
    assert!(
        bad_barrier
            .validate()
            .expect_err("bad barrier")
            .to_string()
            .contains("barrier")
    );

    let mut bad_notional = FxBarrierOption::example().expect("example");
    bad_notional.notional = Money::from((0_i64, Currency::EUR));
    assert!(
        bad_notional
            .validate()
            .expect_err("bad notional")
            .to_string()
            .contains("notional")
    );
}

#[test]
fn resolve_fx_spot_uses_fx_matrix_when_spot_id_is_absent() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let mut inst = FxBarrierOption::example().expect("example");
    inst.fx_spot_id = None;

    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 1.15)])
        .expect("valid quote");
    let market = MarketContext::new().insert_fx(FxMatrix::new(provider, None));

    let spot = resolve_fx_spot(&inst, &market, as_of).expect("fx matrix spot");
    assert!((spot - 1.15).abs() < 1e-12);
}

#[test]
fn resolve_fx_spot_requires_valid_spot_source() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let mut no_matrix = FxBarrierOption::example().expect("example");
    no_matrix.fx_spot_id = None;
    let err =
        resolve_fx_spot(&no_matrix, &MarketContext::new(), as_of).expect_err("missing matrix");
    assert!(err.to_string().contains("fx_matrix"));

    let mut price_scalar = FxBarrierOption::example().expect("example");
    price_scalar.fx_spot_id = Some("EURUSD-SPOT".into());
    let price_market = MarketContext::new().insert_price(
        "EURUSD-SPOT",
        MarketScalar::price(Money::new(1.10, Currency::USD).expect("valid money fixture")),
    );
    let spot = resolve_fx_spot(&price_scalar, &price_market, as_of).expect("price scalar spot");
    assert!((spot - 1.10).abs() < 1e-12);

    let bad_market = MarketContext::new().insert_price(
        "EURUSD-SPOT",
        MarketScalar::unitless(0.0).expect("valid scalar"),
    );
    let err = resolve_fx_spot(&price_scalar, &bad_market, as_of).expect_err("bad scalar");
    assert!(err.to_string().contains("spot must be finite and > 0"));
}

/// Regression: MC pricer must honour option_type for puts.
///
/// A deep-ITM down-and-out put with a far barrier priced through the
/// discrete-monitoring MC path must remain close to the continuous
/// analytical benchmark.
/// This catches regressions that evaluate the terminal payoff as a call
/// instead of honoring `option_type`.
#[test]
fn mc_barrier_put_honours_option_type() {
    let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
    let expiry = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    // Deep-ITM down-and-out put: spot=1.10, strike=1.30, barrier=0.80
    // spot is well below strike => put is deep ITM.
    // barrier=0.80 is far below spot=1.10 => very unlikely to knock out.
    // We use a moderate vol so the MC path stays active.
    let mc_option = FxBarrierOption::builder()
        .id("FXBAR-MC-PUT-BUG".into())
        .strike(1.30)
        .barrier(0.80)
        .rebate_opt(None)
        .option_type(OptionType::Put)
        .barrier_type(BarrierType::DownAndOut)
        .monitoring_start_date(as_of)
        .expiry(expiry)
        .notional(Money::from((1_000_000_i64, Currency::EUR)))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .monitoring(Monitoring::Discrete {
            observation_dates: vec![as_of, expiry],
        })
        .domestic_discount_curve_id("USD-OIS".into())
        .foreign_discount_curve_id("EUR-OIS".into())
        .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
        .vol_surface_id("EURUSD-VOL".into())
        .attributes(finstack_quant_core::types::Attributes::new())
        .build()
        .expect("mc put option");

    // Matching continuous-monitoring analytical option.
    let analytical_option = FxBarrierOption::builder()
        .id("FXBAR-ANAL-PUT-BUG".into())
        .strike(1.30)
        .barrier(0.80)
        .rebate_opt(None)
        .option_type(OptionType::Put)
        .barrier_type(BarrierType::DownAndOut)
        .monitoring_start_date(as_of)
        .expiry(expiry)
        .notional(Money::from((1_000_000_i64, Currency::EUR)))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .monitoring(Monitoring::Continuous)
        .domestic_discount_curve_id("USD-OIS".into())
        .foreign_discount_curve_id("EUR-OIS".into())
        .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
        .vol_surface_id("EURUSD-VOL".into())
        .attributes(finstack_quant_core::types::Attributes::new())
        .build()
        .expect("analytical put option");

    let market = MarketContext::new()
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 0.97)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("dom curve"),
        )
        .expect("insert market curve")
        .try_insert(
            DiscountCurve::builder("EUR-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 0.98)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("for curve"),
        )
        .expect("insert market curve")
        .insert_surface(
            VolSurface::builder("EURUSD-VOL")
                .expiries(&[0.25, 0.5, 1.0])
                .secondary_values(&[0.9, 1.1, 1.3])
                .secondary_axis(VolSurfaceAxis::Strike)
                .quote_type(VolQuoteType::BlackLognormal)
                .interpolation_mode(VolInterpolationMode::Vol)
                .row(&[0.10, 0.10, 0.10])
                .row(&[0.10, 0.10, 0.10])
                .row(&[0.10, 0.10, 0.10])
                .build()
                .expect("vol surface"),
        )
        .insert_price(
            "EURUSD-SPOT",
            MarketScalar::unitless(1.10).expect("valid scalar"),
        );

    use finstack_quant_valuations_core::instruments::Instrument;

    let mc_pv = mc_option
        .value(&market, as_of)
        .expect("MC put price")
        .amount();

    let diagnostic_result = FxBarrierOptionMcPricer::new()
        .price_dyn(&mc_option, &market, as_of)
        .expect("MC diagnostic result");
    let Some(finstack_quant_valuations_core::results::ValuationDetails::MonteCarlo(diagnostics)) =
        diagnostic_result.details
    else {
        panic!("MC barrier result must include typed diagnostics");
    };
    assert!(diagnostics.standard_error.is_finite());
    assert!(diagnostics.estimator_paths > 0);
    assert!(diagnostics.simulated_paths >= diagnostics.estimator_paths);
    assert_eq!(
        diagnostics.time_grid.first().copied(),
        Some(0.0),
        "time grid must include valuation time"
    );
    let expected_maturity_time = mc_option
        .day_count
        .year_fraction(as_of, expiry, DayCountContext::default())
        .expect("maturity time");
    assert_eq!(
        diagnostics.time_grid.last().copied(),
        Some(expected_maturity_time),
        "time grid must end at contractual maturity"
    );

    let analytical_pv = analytical_option
        .value(&market, as_of)
        .expect("analytical put price")
        .amount();

    // Both must be positive (deep ITM put, barrier not hit)
    assert!(
        mc_pv > 0.0,
        "MC put price should be positive (deep ITM), got {}",
        mc_pv
    );
    assert!(
        analytical_pv > 0.0,
        "Analytical put price should be positive (deep ITM), got {}",
        analytical_pv
    );

    // MC and analytical must agree within 10% (MC tolerance for 100K paths)
    let rel_err = (mc_pv - analytical_pv).abs() / analytical_pv;
    assert!(
        rel_err < 0.10,
        "MC put price {} differs from analytical {} by {:.1}% (>10%), \
         option_type is likely being ignored in MC path",
        mc_pv,
        analytical_pv,
        rel_err * 100.0
    );
}

#[test]
fn mc_inputs_pass_domestic_discount_factor_not_year_fraction() {
    let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
    let expiry = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

    let option = FxBarrierOption::builder()
        .id("FXBAR-MC-DF".into())
        .strike(1.10)
        .barrier(1.30)
        .rebate_opt(None)
        .option_type(OptionType::Call)
        .barrier_type(BarrierType::UpAndOut)
        .monitoring_start_date(as_of)
        .expiry(expiry)
        .notional(Money::from((1_000_000_i64, Currency::EUR)))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .monitoring(Monitoring::Discrete {
            observation_dates: vec![as_of, expiry],
        })
        .domestic_discount_curve_id("USD-OIS".into())
        .foreign_discount_curve_id("EUR-OIS".into())
        .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
        .vol_surface_id("EURUSD-VOL".into())
        .attributes(finstack_quant_core::types::Attributes::new())
        .build()
        .expect("fx barrier option");

    let domestic_df_at_two_years = (-0.03_f64 * 2.0).exp();
    let market = MarketContext::new()
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (2.0, domestic_df_at_two_years)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("dom curve"),
        )
        .expect("insert market curve")
        .try_insert(
            DiscountCurve::builder("EUR-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (2.0, (-0.01_f64 * 2.0).exp())])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("for curve"),
        )
        .expect("insert market curve")
        .insert_surface(
            VolSurface::builder("EURUSD-VOL")
                .expiries(&[2.0])
                .secondary_values(&[1.10])
                .secondary_axis(VolSurfaceAxis::Strike)
                .quote_type(VolQuoteType::BlackLognormal)
                .interpolation_mode(VolInterpolationMode::Vol)
                .row(&[0.12])
                .build()
                .expect("vol surface"),
        )
        .insert_price(
            "EURUSD-SPOT",
            MarketScalar::unitless(1.10).expect("valid scalar"),
        );

    let expected_domestic_df = market
        .get_discount("USD-OIS")
        .expect("dom curve")
        .df_between_dates(as_of, expiry)
        .expect("domestic df");
    let spot = resolve_fx_spot(&option, &market, as_of).expect("spot");
    let discount_factor = collect_fx_barrier_inputs(&option, &market, as_of, spot)
        .expect("inputs")
        .df_domestic;

    assert!(
        (discount_factor - expected_domestic_df).abs() < 1e-12,
        "MC discount factor must be domestic DF {expected_domestic_df}, got {discount_factor}"
    );
}
