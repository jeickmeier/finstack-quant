//! Credit-spread P&L attribution for convertible bonds.
//!
//! A `ConvertibleBond` carries credit risk through a Tsiveriotis–Zhang risky
//! *discount* curve (`credit_curve_id`), not a `HazardCurve`. These tests pin
//! that the attribution credit factor still fires for that curve representation
//! — i.e. a credit-spread move is attributed to `credit_curves_pnl` rather than
//! leaking into the residual.

use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use std::sync::Arc;
use time::Month;

use finstack_attribution::{
    attribute_pnl_metrics_based, attribute_pnl_taylor, AttributionMethod, ExecutionPolicy,
    TaylorAttributionConfig,
};
use finstack_cashflows::builder::specs::{CouponType, FixedCouponSpec};
use finstack_valuations::instruments::fixed_income::convertible::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, ConvertibleBond, DividendAdjustment,
};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;

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
        freq: Tenor::semi_annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };
    Arc::new(ConvertibleBond {
        id: "CONV-CREDIT-ATTR".to_string().into(),
        notional: Money::new(1000.0, Currency::USD),
        issue_date: Date::from_calendar_date(2025, Month::January, 1).unwrap(),
        maturity: Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: Some("USD-CREDIT".into()),
        settlement_days: None,
        recovery_rate: None,
        conversion,
        underlying_equity_id: Some("AAPL".to_string()),
        call_put: None,
        soft_call_trigger: None,
        fixed_coupon: Some(fixed_coupon),
        floating_coupon: None,
        pricing_overrides: finstack_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    })
}

/// Market with a risk-free `USD-OIS` curve and a wider risky `USD-CREDIT`
/// discount curve. Only `credit_spread_bps` varies between the two test dates;
/// `USD-OIS`, spot and vol are held fixed so the P&L is purely a credit move.
fn market(credit_spread_bps: f64) -> MarketContext {
    let base = t0();
    let rf = 0.03;
    let credit = rf + credit_spread_bps / 10_000.0;

    // LogLinear so the flat zero rate extrapolates cleanly past the last knot
    // to the 30Y tenors the key-rate attribution samples (Linear DF
    // extrapolation would go negative → NaN zero rate).
    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (10.0, (-rf * 10.0).exp())])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    let credit_curve = DiscountCurve::builder("USD-CREDIT")
        .base_date(base)
        .knots([(0.0, 1.0), (10.0, (-credit * 10.0).exp())])
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

    let attribution = attribute_pnl_taylor(
        &conv,
        &market_t0,
        &market_t1,
        t0(),
        t1(),
        &config,
        ExecutionPolicy::Parallel,
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

    let parallel = attribute_pnl_taylor(
        &conv,
        &market_t0,
        &market_t1,
        t0(),
        t1(),
        &config,
        ExecutionPolicy::Parallel,
    )
    .expect("parallel Taylor attribution should succeed");
    let serial = attribute_pnl_taylor(
        &conv,
        &market_t0,
        &market_t1,
        t0(),
        t1(),
        &config,
        ExecutionPolicy::Serial,
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
    let opts = finstack_valuations::instruments::PricingOptions::default();
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
