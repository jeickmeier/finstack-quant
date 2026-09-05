//! Credit-spread P&L attribution for convertible bonds.
//!
//! A `ConvertibleBond` carries credit risk through a Tsiveriotis–Zhang risky
//! *discount* curve (`credit_curve_id`), not a `HazardCurve`. These tests pin
//! that the attribution credit factor still fires for that curve representation
//! — i.e. a credit-spread move is attributed to `credit_curves_pnl` rather than
//! leaking into the residual.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use std::sync::Arc;
use time::Month;

use finstack_quant_attribution::{
    attribute_pnl, attribute_pnl_metrics_based, AttributionMethod, AttributionRequest,
    ExecutionPolicy, TaylorAttributionConfig,
};
use finstack_quant_cashflows::builder::specs::{CouponType, FixedCouponSpec};
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, ConvertibleBond, DividendAdjustment,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

fn t0() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}
fn t1() -> Date {
    Date::from_calendar_date(2025, Month::January, 2).unwrap()
}

/// OTM (bond-like) convertible referencing a separate risky discount curve as
/// its credit curve — the configuration that exercises the credit factor.
fn convertible_with_credit() -> Arc<dyn Instrument> {
    let conversion = ConversionSpec {
        ratio: Some(10.0),
        price: None,
        policy: ConversionPolicy::Voluntary,
        anti_dilution: AntiDilutionPolicy::None,
        dividend_adjustment: DividendAdjustment::None,
        dilution_events: Vec::new(),
    };
    let fixed_coupon = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: rust_decimal::Decimal::from_f64_retain(0.05).unwrap(),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::semi_annual(),

            day_count: DayCount::Act365F,

            business_day_convention: BusinessDayConvention::Following,

            calendar_id: "weekends_only".to_string(),

            stub: StubKind::None,

            end_of_month: false,

            payment_lag_days: 0,

            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };
    Arc::new(ConvertibleBond {
        id: "CONV-CREDIT-ATTR".to_string().into(),
        notional: Money::new(1000.0, Currency::USD).expect("valid money fixture"),
        issue_date: Date::from_calendar_date(2025, Month::January, 1).unwrap(),
        maturity: Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: Some("USD-CREDIT".into()),
        settlement_days: None,
        recovery_rate: Some(0.0),
        conversion,
        underlying_equity_id: Some("AAPL".to_string()),
        call_put: None,
        soft_call_trigger: None,
        fixed_coupon: Some(fixed_coupon),
        floating_coupon: None,
        instrument_pricing_overrides:
            finstack_quant_valuations::instruments::InstrumentPricingOverrides::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    })
}

/// Market with a risk-free `USD-OIS` curve and a wider risky `USD-CREDIT`
/// discount curve. Only `credit_spread_bp` varies between the two test dates;
/// `USD-OIS`, spot and vol are held fixed so the P&L is purely a credit move.
fn market(credit_spread_bp: f64) -> MarketContext {
    let base = t0();
    let rf = 0.03;
    let credit = rf + credit_spread_bp / 10_000.0;

    // LogLinear so the flat zero rate extrapolates cleanly past the last knot
    // to the 30Y tenors the key-rate attribution samples (Linear DF
    // extrapolation would go negative → NaN zero rate).
    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, (-rf).exp()), (10.0, (-rf * 10.0).exp())])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    let credit_curve = DiscountCurve::builder("USD-CREDIT")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, (-credit).exp()),
            (10.0, (-credit * 10.0).exp()),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    MarketContext::new()
        .insert(ois)
        .insert(credit_curve)
        // Spot well below the $100 conversion price → bond-like, so the credit
        // factor is material rather than swamped by equity optionality.
        .insert_price("AAPL", MarketScalar::Unitless(50.0))
        .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02))
}

/// Taylor attribution must explain a convertible-bond credit-spread move.
///
/// REGRESSION: the convertible's credit curve is a `DiscountCurve`. The credit
/// factor previously measured the move only via `measure_par_spread_shift`
/// (hazard-curve only), so `compute_credit_factor` errored and the factor was
/// silently dropped — the entire credit-spread P&L fell into the residual.
#[test]
fn taylor_explains_convertible_credit_spread_move() {
    let conv = convertible_with_credit();
    let market_t0 = market(150.0);
    let market_t1 = market(300.0); // +150bp credit widening
    let config = TaylorAttributionConfig::default();

    let attribution = attribute_pnl(
        &AttributionMethod::Taylor(config),
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Parallel,
            ..AttributionRequest::new(
                &conv,
                &market_t0,
                &market_t1,
                t0(),
                t1(),
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
    )
    .expect("Taylor attribution should succeed");

    assert!(
        attribution.credit_curves_pnl.amount() < 0.0,
        "credit_curves_pnl should be negative for a +150bp widening, got {}",
        attribution.credit_curves_pnl
    );
    // The credit move must be EXPLAINED, not dumped in the residual: the credit
    // factor must dominate the residual it would otherwise have become.
    assert!(
        attribution.credit_curves_pnl.amount().abs() > 5.0 * attribution.residual.amount().abs(),
        "credit P&L ({}) must be attributed, not left in residual ({})",
        attribution.credit_curves_pnl,
        attribution.residual,
    );
}

#[test]
fn taylor_serial_execution_policy_matches_parallel_for_convertible_credit() {
    let conv = convertible_with_credit();
    let market_t0 = market(150.0);
    let market_t1 = market(300.0);
    let config = TaylorAttributionConfig::default();

    let parallel = attribute_pnl(
        &AttributionMethod::Taylor(config.clone()),
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Parallel,
            ..AttributionRequest::new(
                &conv,
                &market_t0,
                &market_t1,
                t0(),
                t1(),
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
    )
    .expect("parallel Taylor attribution should succeed");
    let serial = attribute_pnl(
        &AttributionMethod::Taylor(config),
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &conv,
                &market_t0,
                &market_t1,
                t0(),
                t1(),
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
    )
    .expect("serial Taylor attribution should succeed");

    assert_eq!(parallel.total_pnl, serial.total_pnl);
    assert_eq!(parallel.rates_curves_pnl, serial.rates_curves_pnl);
    assert_eq!(parallel.credit_curves_pnl, serial.credit_curves_pnl);
    assert_eq!(parallel.vol_pnl, serial.vol_pnl);
    assert_eq!(parallel.fx_pnl, serial.fx_pnl);
    assert_eq!(parallel.residual, serial.residual);
    assert_eq!(parallel.meta.num_repricings, serial.meta.num_repricings);
}

/// Metrics-based attribution must likewise explain the convertible credit move
/// (it shares the same par-spread-only measurement path as Taylor).
#[test]
fn metrics_based_explains_convertible_credit_spread_move() {
    let conv = convertible_with_credit();
    let market_t0 = market(150.0);
    let market_t1 = market(300.0);

    let metrics = AttributionMethod::MetricsBased.required_metrics();
    let opts = finstack_quant_valuations::instruments::PricingOptions::default();
    let val_t0 = conv
        .price_with_metrics(&market_t0, t0(), &metrics, opts.clone())
        .unwrap();
    let val_t1 = conv
        .price_with_metrics(&market_t1, t1(), &metrics, opts)
        .unwrap();

    let attribution =
        attribute_pnl_metrics_based(&conv, &market_t0, &market_t1, &val_t0, &val_t1, t0(), t1())
            .unwrap();

    assert!(
        attribution.credit_curves_pnl.amount() < 0.0,
        "metrics-based credit_curves_pnl should be negative for a widening, got {}",
        attribution.credit_curves_pnl
    );
    assert!(
        attribution.credit_curves_pnl.amount().abs() > 5.0 * attribution.residual.amount().abs(),
        "metrics-based credit P&L ({}) must be attributed, not left in residual ({})",
        attribution.credit_curves_pnl,
        attribution.residual,
    );
    // Sanity: the convertible registers a non-zero Cs01 for this curve.
    let cs01 = *val_t0.measures.get(MetricId::Cs01.as_str()).unwrap();
    assert!(
        cs01.abs() > 1e-6,
        "convertible Cs01 should be non-trivial, got {cs01}"
    );
}

#[test]
fn repricing_methods_classify_risky_discount_curve_as_credit() {
    let instrument = convertible_with_credit();
    let opening = market(150.0);
    let closing = market(300.0);
    let expected = instrument
        .value(&closing, t0())
        .unwrap()
        .checked_sub(instrument.value(&opening, t0()).unwrap())
        .unwrap();
    let config = finstack_quant_core::config::FinstackConfig::default();
    for method in [
        AttributionMethod::Parallel,
        AttributionMethod::Waterfall(finstack_quant_attribution::default_waterfall_order()),
    ] {
        for full_cross_attribution in [false, true] {
            let result = attribute_pnl(
                &method,
                &AttributionRequest {
                    full_cross_attribution,
                    strict_validation: false,
                    ..AttributionRequest::new(&instrument, &opening, &closing, t0(), t0(), &config)
                },
            )
            .unwrap();
            assert!(
                (result.credit_curves_pnl.amount() - expected.amount()).abs() < 1e-8,
                "{method:?}: {result:?}"
            );
            assert!(result.rates_curves_pnl.amount().abs() < 1e-8);
            assert!(result.residual.amount().abs() < 1e-8);
        }
    }
}

fn conversion_change_spec(
    method: AttributionMethod,
) -> finstack_quant_attribution::AttributionSpec {
    let opening = convertible_with_credit();
    let mut closing = opening
        .as_any()
        .downcast_ref::<ConvertibleBond>()
        .unwrap()
        .clone();
    let opening_conversion = closing.conversion.clone();
    closing.conversion.ratio = Some(30.0);
    let state = finstack_quant_core::market_data::context::MarketContextState::from(&market(150.0));
    finstack_quant_attribution::AttributionSpec {
        instrument: finstack_quant_valuations::instruments::InstrumentJson::ConvertibleBond(closing),
        market_t0: state.clone(), market_t1: state,
        as_of_t0: t0(), as_of_t1: t0(), method,
        model_params_t0: Some(finstack_quant_valuations::instruments::model_params::ModelParamsSnapshot::Convertible { conversion_spec: opening_conversion }),
        config: None, credit_factor_model: None,
        credit_factor_detail_options: Default::default(), full_cross_attribution: false,
    }
}

#[test]
fn metrics_spec_restores_opening_conversion_and_rounding() {
    let mut spec = conversion_change_spec(AttributionMethod::MetricsBased);
    spec.config = Some(
        serde_json::from_value(serde_json::json!({"rounding_scale": 4, "metrics": ["delta"]}))
            .unwrap(),
    );
    let result = spec.execute().unwrap().attribution;
    let opening = convertible_with_credit()
        .value(&market(150.0), t0())
        .unwrap();
    let closing = spec
        .instrument
        .into_boxed()
        .unwrap()
        .value(&market(150.0), t0())
        .unwrap();
    let expected = closing.checked_sub(opening).unwrap().amount();
    assert!(expected > 100.0);
    assert!((result.total_pnl.amount() - expected).abs() < 1e-8);
    assert!((result.model_params_pnl.amount() - expected).abs() < 1e-8);
    assert!(result.residual.amount().abs() < 1e-8);
    assert_eq!(
        result
            .meta
            .rounding
            .output_scale_by_currency
            .get(&Currency::USD),
        Some(&4)
    );
}

#[test]
fn taylor_market_gamma_uses_opening_conversion_state() {
    let method = AttributionMethod::Taylor(TaylorAttributionConfig {
        include_gamma: true,
        ..Default::default()
    });
    let mut changed = conversion_change_spec(method);
    changed.market_t1 =
        finstack_quant_core::market_data::context::MarketContextState::from(&market(170.0));
    let mut fixed = changed.clone();
    fixed.instrument = finstack_quant_valuations::instruments::InstrumentJson::ConvertibleBond(
        convertible_with_credit()
            .as_any()
            .downcast_ref::<ConvertibleBond>()
            .unwrap()
            .clone(),
    );
    fixed.model_params_t0 = None;
    let changed_result = changed.execute().unwrap().attribution;
    let fixed_result = fixed.execute().unwrap().attribution;
    assert!(
        !changed_result.result_invalid,
        "{:?}",
        changed_result.meta.notes
    );
    assert!(
        (changed_result.credit_curves_pnl.amount() - fixed_result.credit_curves_pnl.amount()).abs()
            < 1e-8
    );
    assert!(
        (changed_result.rates_curves_pnl.amount() - fixed_result.rates_curves_pnl.amount()).abs()
            < 1e-8
    );
}

#[test]
fn requested_reporting_currency_requires_fx() {
    let mut spec = conversion_change_spec(AttributionMethod::Parallel);
    spec.config =
        Some(serde_json::from_value(serde_json::json!({"target_currency": "EUR"})).unwrap());
    let error = spec.execute().unwrap_err();
    assert!(error.to_string().contains("fx"), "{error}");
}
