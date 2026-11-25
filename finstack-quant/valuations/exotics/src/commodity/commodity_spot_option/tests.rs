use super::types::*;
use super::valuation::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_models::monte_carlo::process::schwartz_smith::SchwartzSmithParams;
use finstack_quant_models::types::ExerciseStyle;

use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, PriceCurve};
use finstack_quant_valuations_core::instruments::{Instrument, SettlementType};

#[test]
fn test_default_settlement_type_is_cash() {
    assert_eq!(
        finstack_quant_valuations_core::serde_defaults::settlement_cash(),
        SettlementType::Cash
    );
}

#[test]
fn lognormal_option_rejects_signed_forward_override() {
    let mut option = CommoditySpotOption::example().expect("example");
    option.quoted_forward = Some(-5.0);
    let market = MarketContext::new();
    let as_of = option.expiry - time::Duration::days(30);
    assert!(option.forward_price(&market, as_of).is_err());
}

#[test]
fn settlement_must_be_explicit() {
    let mut value =
        serde_json::to_value(CommoditySpotOption::example().expect("example")).expect("serialize");
    let obj = value
        .as_object_mut()
        .expect("CommoditySpotOption should serialize to an object");
    obj.remove("settlement");
    assert!(serde_json::from_value::<CommoditySpotOption>(value).is_err());
}

#[test]
fn canonical_dependencies_preserve_surface_pairing_and_strike() {
    let mut option = CommoditySpotOption::example().expect("example");
    option.spot_id = Some("WTI-SPOT".into());

    let deps = option.market_dependencies().expect("dependencies");
    assert_eq!(
        deps.curves.discount_curves.as_slice(),
        &[option.discount_curve_id.clone()]
    );
    assert_eq!(
        deps.curves.forward_curves.as_slice(),
        &[option.forward_curve_id.clone()]
    );
    assert_eq!(deps.market_scalar_ids, vec!["WTI-SPOT".to_string()]);
    assert_eq!(deps.volatility_dependencies.len(), 1);
    let volatility = &deps.volatility_dependencies[0];
    assert_eq!(volatility.vol_surface_id, option.vol_surface_id);
    assert_eq!(
        volatility.spot_id.as_ref().map(|id| id.as_str()),
        Some("WTI-SPOT")
    );
    assert_eq!(volatility.reference_strike, Some(option.strike));
}

#[test]
fn commodity_spot_option_default_equals_black76_for_pv_and_raw() {
    let option = CommoditySpotOption::example().expect("example");
    let as_of = Date::from_calendar_date(2025, time::Month::January, 15).expect("valid date");
    let market = MarketContext::new()
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 0.97)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("discount curve"),
        )
        .expect("insert market curve")
        .try_insert(
            PriceCurve::builder("WTI-FORWARD")
                .kind(finstack_quant_core::market_data::term_structures::PriceCurveKind::Price)
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
                .base_date(as_of)
                .knots([(0.0, 75.0), (1.0, 76.0)])
                .build()
                .expect("price curve"),
        )
        .expect("insert market curve")
        .insert_surface(
            VolSurface::builder("WTI-VOL")
                .secondary_axis(finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Strike)
                .quote_type(
                    finstack_quant_core::market_data::surfaces::VolQuoteType::BlackLognormal,
                )
                .interpolation_mode(
                    finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol,
                )
                .expiries(&[0.25, 0.5, 1.0])
                .secondary_values(&[60.0, 75.0, 90.0])
                .row(&[0.25, 0.25, 0.25])
                .row(&[0.25, 0.25, 0.25])
                .row(&[0.25, 0.25, 0.25])
                .build()
                .expect("vol surface"),
        );
    let registry = crate::EXOTICS.pricers();

    let default = option
        .price_with_metrics(
            &market,
            as_of,
            &[],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .expect("default commodity-option price");
    let black76 = registry
        .price_with_metrics(
            &option,
            finstack_quant_valuations_core::pricer::ModelKey::Black76,
            &market,
            as_of,
            &[],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .expect("Black76 commodity-option price");
    let default_raw = option
        .value_raw(&market, as_of)
        .expect("default commodity-option raw price");
    let black76_raw = registry
        .price_raw(
            &option,
            finstack_quant_valuations_core::pricer::ModelKey::Black76,
            &market,
            as_of,
        )
        .expect("Black76 commodity-option raw price");

    assert_eq!(default.value, black76.value);
    assert_eq!(default_raw, black76_raw);
}

#[test]
fn bermudan_exercise_requires_schedule() {
    let mut option = CommoditySpotOption::example().expect("example");
    option.exercise_style = ExerciseStyle::Bermudan;
    option.spot_id = Some("WTI-SPOT".into());
    let as_of = Date::from_calendar_date(2025, time::Month::January, 15).expect("valid");
    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 0.97)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("discount curve"),
        )
        .expect("insert market curve")
        .try_insert(
            PriceCurve::builder("WTI-FORWARD")
                .kind(finstack_quant_core::market_data::term_structures::PriceCurveKind::Price)
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
                .base_date(as_of)
                .knots([(0.0, 75.0), (1.0, 76.0)])
                .build()
                .expect("price curve"),
        )
        .expect("insert market curve")
        .insert_surface(
            VolSurface::builder("WTI-VOL")
                .secondary_axis(finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Strike)
                .quote_type(
                    finstack_quant_core::market_data::surfaces::VolQuoteType::BlackLognormal,
                )
                .interpolation_mode(
                    finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol,
                )
                .expiries(&[0.25, 0.5, 1.0])
                .secondary_values(&[60.0, 75.0, 90.0])
                .row(&[0.25, 0.25, 0.25])
                .row(&[0.25, 0.25, 0.25])
                .row(&[0.25, 0.25, 0.25])
                .build()
                .expect("vol surface"),
        );
    let err = option
        .value(&market, as_of)
        .expect_err("Bermudan commodity options should require an exercise schedule");
    assert!(err.to_string().contains("exercise_dates"));
}

/// W-13: `implied_carry` must guard against non-finite `forward` / `spot`.
/// A NaN or infinite input passes the `<= 0.0` checks (NaN comparisons are
/// always false), so without an `is_finite()` guard a non-finite cost-of-
/// carry `q` would flow straight into the binomial tree. The guard makes
/// `implied_carry` fall back to the risk-free rate (zero carry).
#[test]
fn w13_implied_carry_guards_against_non_finite_inputs() {
    let r = 0.05;
    let t = 0.5;

    // NaN forward → fall back to r.
    let q_nan_fwd = implied_carry(100.0, f64::NAN, r, t);
    assert_eq!(
        q_nan_fwd, r,
        "NaN forward must fall back to the risk-free rate, got {q_nan_fwd}"
    );
    assert!(q_nan_fwd.is_finite());

    // NaN spot → fall back to r.
    let q_nan_spot = implied_carry(f64::NAN, 100.0, r, t);
    assert_eq!(
        q_nan_spot, r,
        "NaN spot must fall back to the risk-free rate"
    );

    // Infinite forward → fall back to r.
    let q_inf_fwd = implied_carry(100.0, f64::INFINITY, r, t);
    assert_eq!(q_inf_fwd, r, "infinite forward must fall back to r");
    assert!(q_inf_fwd.is_finite());

    // Sanity: a well-formed input still yields the true cost-of-carry.
    let q_ok = implied_carry(100.0, 105.0, r, t);
    let expected = r - (105.0_f64 / 100.0).ln() / t;
    assert!(
        (q_ok - expected).abs() < 1e-12,
        "well-formed inputs must yield the true implied carry"
    );
}
#[test]
fn schwartz_smith_risk_retains_custom_registry_and_rejects_black_vega() {
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_valuations_core::instruments::PricingOptions;
    use finstack_quant_valuations_core::metrics::MetricId;
    use finstack_quant_valuations_core::pricer::{ModelKey, PricerRegistry};
    use std::sync::Arc;
    let mut option = CommoditySpotOption::example().unwrap();
    option.quoted_forward = Some(76.0);
    option.spot_id = Some("WTI-SPOT".into());
    let as_of = Date::from_calendar_date(2025, time::Month::January, 15).unwrap();
    let market = MarketContext::new()
        .insert_price(
            "WTI-SPOT",
            MarketScalar::unitless(75.0).expect("valid scalar"),
        )
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (5.0, 0.9)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .unwrap(),
        )
        .expect("insert market curve");
    let mut registry = PricerRegistry::new();
    registry
        .register(super::pricer::CommoditySpotOptionMcPricer::new(
            CommodityMcParams {
                params: SchwartzSmithParams::new(1.0, 0.1, 0.0, 0.05, 0.2).unwrap(),
                num_paths: 512,
                num_steps: 4,
                seed: Some(71),
            },
        ))
        .unwrap();
    let registry = Arc::new(registry);
    let options = PricingOptions::default()
        .with_model(ModelKey::MonteCarloSchwartzSmith)
        .with_registry(Arc::clone(&registry));
    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::Gamma],
            options.clone(),
        )
        .unwrap();
    assert!(result.measures["delta"].is_finite());
    assert!(result.measures["gamma"].is_finite());
    for metric in [MetricId::Vega, MetricId::Vanna, MetricId::Volga] {
        let error = option
            .price_with_metrics(&market, as_of, &[metric], options.clone())
            .unwrap_err();
        assert!(error.to_string().contains("sigma_x/sigma_y"), "{error}");
    }
}
#[test]
fn resolved_spot_claim_keeps_cash_or_physical_delivery_until_payment() {
    use time::macros::date;
    let as_of = date!(2025 - 06 - 16);
    let payment = date!(2025 - 06 - 20);
    let mut option = CommoditySpotOption::example().unwrap();
    option.strike = 90.0;
    option.quantity = 1.0;
    option.exercise = Some(CommoditySpotOptionExercise {
        date: option.expiry,
        spot: 100.0,
        settlement_date: payment,
        exercised: true,
    });
    let market = MarketContext::new()
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 1.0)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .unwrap(),
        )
        .expect("insert market curve")
        .try_insert(
            PriceCurve::builder("WTI-FORWARD")
                .kind(finstack_quant_core::market_data::term_structures::PriceCurveKind::Price)
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
                .base_date(as_of)
                .knots([(0.0, 80.0), (1.0, 80.0)])
                .build()
                .unwrap(),
        )
        .expect("insert market curve");
    assert!((option.value(&market, as_of).unwrap().amount() - 10.0).abs() < 1e-12);
    option.settlement = SettlementType::Physical;
    assert!((option.value(&market, as_of).unwrap().amount() + 10.0).abs() < 1e-12);
    let rebuilt: CommoditySpotOption =
        serde_json::from_value(serde_json::to_value(&option).unwrap()).unwrap();
    assert_eq!(
        rebuilt.value(&market, as_of).unwrap(),
        option.value(&market, as_of).unwrap()
    );
    assert_eq!(
        rebuilt
            .value(&market, payment.next_day().unwrap())
            .unwrap()
            .amount(),
        0.0
    );
    option.exercise = None;
    assert!(option.value(&market, as_of).is_err());
}

#[test]
fn spot_early_exercise_never_substitutes_a_future_for_spot() {
    let mut option = CommoditySpotOption::example().unwrap();
    option.exercise_style = ExerciseStyle::American;
    option.spot_id = None;
    assert!(
        option
            .validate()
            .unwrap_err()
            .to_string()
            .contains("spot_id")
    );
}

#[test]
fn spot_currency_is_checked_at_expiry_and_in_analytic_and_mc_pricing() {
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use time::macros::date;
    let mut option = CommoditySpotOption::example().expect("option");
    option.spot_id = Some("WTI-SPOT".into());
    option.quoted_forward = Some(80.0);
    let as_of = date!(2025 - 01 - 15);
    let market = MarketContext::new()
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 0.97)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("discount curve"),
        )
        .expect("insert market curve")
        .insert_surface(
            VolSurface::builder("WTI-VOL")
                .secondary_axis(finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Strike)
                .quote_type(
                    finstack_quant_core::market_data::surfaces::VolQuoteType::BlackLognormal,
                )
                .interpolation_mode(
                    finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol,
                )
                .expiries(&[0.25, 0.5, 1.0])
                .secondary_values(&[60.0, 75.0, 90.0])
                .row(&[0.25, 0.25, 0.25])
                .row(&[0.25, 0.25, 0.25])
                .row(&[0.25, 0.25, 0.25])
                .build()
                .expect("surface"),
        );
    let mc = CommodityMcParams {
        params: SchwartzSmithParams::new(1.0, 0.1, 0.0, 0.05, 0.2).expect("model"),
        num_paths: 64,
        num_steps: 4,
        seed: Some(71),
    };
    let mut american = option.clone();
    american.exercise_style = ExerciseStyle::American;
    let numeric = market.clone().insert_price(
        "WTI-SPOT",
        MarketScalar::unitless(80.0).expect("valid scalar"),
    );
    let matching = market.clone().insert_price(
        "WTI-SPOT",
        MarketScalar::price(Money::new(80.0, Currency::USD).expect("USD spot")),
    );
    assert_eq!(
        option
            .value(&numeric, option.expiry)
            .expect("numeric expiry"),
        option.value(&matching, option.expiry).expect("USD expiry")
    );
    assert_eq!(
        american.value(&numeric, as_of).expect("numeric American"),
        american.value(&matching, as_of).expect("USD American")
    );
    assert_eq!(
        option.price_mc(&mc, &numeric, as_of).expect("numeric MC").0,
        option.price_mc(&mc, &matching, as_of).expect("USD MC").0
    );
    let mismatched = market.insert_price(
        "WTI-SPOT",
        MarketScalar::price(Money::new(80.0, Currency::EUR).expect("EUR spot")),
    );
    for error in [
        option
            .value(&mismatched, option.expiry)
            .expect_err("expiry currency"),
        american
            .value(&mismatched, as_of)
            .expect_err("American currency"),
        option
            .price_mc(&mc, &mismatched, as_of)
            .expect_err("MC currency"),
    ] {
        assert!(
            matches!(
                error,
                finstack_quant_core::Error::CurrencyMismatch {
                    expected: Currency::USD,
                    actual: Currency::EUR,
                }
            ),
            "{error}"
        );
    }
}

#[test]
fn scalar_spot_still_requires_a_finite_positive_amount() {
    use finstack_quant_core::market_data::scalars::MarketScalar;
    let mut option = CommoditySpotOption::example().expect("option");
    option.spot_id = Some("WTI-SPOT".into());
    for value in [0.0, -1.0] {
        let market = MarketContext::new().insert_price(
            "WTI-SPOT",
            MarketScalar::unitless(value).expect("valid scalar"),
        );
        assert!(option.required_spot(&market).is_err());
    }
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(MarketScalar::unitless(bad).is_err());
    }
}

#[test]
fn low_vol_analytical_greeks_succeed_without_negative_vol_bump() {
    let as_of = time::macros::date!(2025 - 01 - 02);
    let mut option = CommoditySpotOption::example().expect("example");
    option.strike = 75.0;
    option
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.005);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.96)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("disc");
    let fwd = PriceCurve::builder("WTI-FORWARD")
        .kind(finstack_quant_core::market_data::term_structures::PriceCurveKind::Price)
        .base_date(as_of)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([(0.0, 76.0), (1.0, 76.0)])
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
        .build()
        .expect("fwd");
    let surf = VolSurface::from_grid(
        "WTI-VOL",
        &[0.1, 1.0],
        &[50.0, 100.0],
        &[0.005; 4],
        finstack_quant_core::market_data::surfaces::VolGridOpts {
            secondary_axis: finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Strike,
            quote_type: finstack_quant_core::market_data::surfaces::VolQuoteType::BlackLognormal,
            interpolation_mode:
                finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol,
        },
        None,
    )
    .expect("surf");
    let market = MarketContext::new()
        .try_insert(disc)
        .expect("disc")
        .try_insert(fwd)
        .expect("fwd")
        .insert_surface(surf);

    let metrics = [
        finstack_quant_valuations_core::metrics::MetricId::Delta,
        finstack_quant_valuations_core::metrics::MetricId::Gamma,
        finstack_quant_valuations_core::metrics::MetricId::Vega,
        finstack_quant_valuations_core::metrics::MetricId::Vanna,
        finstack_quant_valuations_core::metrics::MetricId::Volga,
    ];
    let res = option
        .price_with_metrics(&market, as_of, &metrics, Default::default())
        .expect("low-vol Black-76 analytical Greeks must succeed");
    for id in metrics {
        let val = res.metric(id.clone()).expect("metric");
        assert!(val.is_finite(), "{id:?} not finite: {val}");
    }
}
