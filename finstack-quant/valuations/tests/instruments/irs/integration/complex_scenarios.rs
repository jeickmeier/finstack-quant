//! Complex IRS scenario tests.
//!
//! Tests comprehensive and multi-faceted IRS scenarios including:
//! - Off-market swaps with multiple metrics
//! - Basis swaps
//! - Multi-curve environments
//! - Forward-starting swaps

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::irs::{InterestRateSwap, PayReceive};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn build_flat_curves(disc_rate: f64, fwd_rate: f64, base_date: Date) -> MarketContext {
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-disc_rate).exp()),
            (5.0, (-disc_rate * 5.0).exp()),
            (10.0, (-disc_rate * 10.0).exp()),
        ])
        .build()
        .unwrap();

    let fwd_curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, fwd_rate), (10.0, fwd_rate)])
        .build()
        .unwrap();

    let fwd_curve_6m = ForwardCurve::builder("USD-SOFR-6M", 0.5)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, fwd_rate), (10.0, fwd_rate)])
        .build()
        .unwrap();

    // Provide fixings for theta calculations where the rolled as_of date
    // puts the first reset in the past.
    let fixings = finstack_quant_core::market_data::scalars::ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![(base_date, fwd_rate)], // Fixing on base_date at the flat rate
        None,
    )
    .expect("fixings series");

    MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert(fwd_curve_6m)
        .insert_series(fixings)
}

fn create_swap(as_of: Date, end: Date, fixed_rate: f64, side: PayReceive) -> InterestRateSwap {
    InterestRateSwap {
        id: "IRS_INTEGRATION_TEST".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side,
        fixed: finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(fixed_rate).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
        },
        float: finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::ZERO,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            // Use reset_lag_days: 0 for spot-starting swaps to avoid needing historical fixings.
            reset_lag_days: 0,
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
        },
        margin_spec: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

#[test]
fn test_off_market_swap_all_metrics() {
    // Off-market swap with comprehensive metric calculation
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_swap(as_of, end, 0.03, PayReceive::Receive);
    let market = build_flat_curves(0.05, 0.05, as_of);

    let metrics = vec![
        MetricId::Annuity,
        MetricId::Dv01,
        MetricId::ParRate,
        MetricId::PvFixed,
        MetricId::PvFloat,
        MetricId::Theta,
    ];

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Verify all metrics computed
    assert!(result.measures.contains_key("annuity"));
    assert!(result.measures.contains_key("dv01"));
    assert!(result.measures.contains_key("par_rate"));
    assert!(result.measures.contains_key("pv_fixed"));
    assert!(result.measures.contains_key("pv_float"));
    assert!(result.measures.contains_key("theta"));

    // Off-market swap should have negative NPV
    assert!(result.value.amount() < 0.0);
}

#[test]
fn test_forward_starting_swap() {
    // Swap starting in the future
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2025 - 01 - 01); // Forward start
    let end = date!(2030 - 01 - 01);

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.05_f64).exp()),
            (5.0, (-0.05_f64 * 5.0).exp()),
            (10.0, (-0.05_f64 * 10.0).exp()),
        ])
        .build()
        .unwrap();

    let fwd_curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (10.0, 0.05)])
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let swap = InterestRateSwap {
        id: "FORWARD_START".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Receive,
        fixed: finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
            start, // Forward start
            end,
        },
        float: finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 2,
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start,
            end,
        },
        margin_spec: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let npv = swap.value(&market, as_of).unwrap();

    // Forward-starting swap at 5% in flat 5% curve should price near par
    // Small residual from continuous OIS vs simple ACT/360 coupon convention
    assert!(
        npv.amount().abs() < 10_000.0,
        "Forward swap NPV at par: {}",
        npv.amount()
    );
}

#[test]
fn test_swap_with_large_spread() {
    // Swap with large spread on floating leg
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let mut swap = create_swap(as_of, end, 0.05, PayReceive::Receive);
    swap.float.spread_bp = rust_decimal::Decimal::try_from(200.0).expect("valid"); // 200bp spread

    let market = build_flat_curves(0.05, 0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFixed, MetricId::PvFloat],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv_fixed = *result.measures.get("pv_fixed").unwrap();
    let pv_float = *result.measures.get("pv_float").unwrap();

    // Large spread should significantly affect PV float
    assert!(pv_float > pv_fixed * 1.2);
}

#[test]
fn test_swap_seasoned() {
    // Swap traded in January, evaluated in June (truly seasoned swap)
    let start = date!(2024 - 01 - 01); // Trade date
    let as_of = date!(2024 - 06 - 01); // Evaluation date
    let end = date!(2028 - 01 - 01);

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.06_f64).exp()), // Rates have risen
            (5.0, (-0.06_f64 * 5.0).exp()),
        ])
        .build()
        .unwrap();

    // Flat forward curve based at the trade date: seasoned resets strictly
    // before the curve base now error under the default FloatingRateFallback.
    let fwd_curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(start)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.06), (10.0, 0.06)])
        .build()
        .unwrap();

    // Provide fixings for past reset dates (quarterly from Jan through Apr)
    let fixings = finstack_quant_core::market_data::scalars::ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![
            (date!(2024 - 01 - 01), 0.04), // Q1 reset at 4% (old rate)
            (date!(2024 - 04 - 01), 0.05), // Q2 reset at 5% (rate started rising)
        ],
        None,
    )
    .expect("fixings series");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_series(fixings);

    let swap = InterestRateSwap {
        id: "SEASONED_SWAP".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Receive,
        fixed: finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.04).expect("valid"), // Old rate from 2023
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
            start,
            end,
        },
        float: finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            // Use 0 since we're providing fixings anyway and it simplifies the test
            reset_lag_days: 0,
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start,
            end,
        },
        margin_spec: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let npv = swap.value(&market, as_of).unwrap();

    // Seasoned swap should show MTM loss (rates rose, receiving fixed)
    assert!(npv.amount() < 0.0);
}

#[test]
fn test_swap_risk_attribution() {
    // Test that risks are correctly attributed
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_swap(as_of, end, 0.05, PayReceive::Receive);
    let market = build_flat_curves(0.05, 0.06, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01, MetricId::Annuity, MetricId::BucketedDv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();
    let annuity = *result.measures.get("annuity").unwrap();

    // Verify risk attribution is consistent. With a multi-curve setup the
    // float leg also contributes discounting risk. For off-market swaps,
    // DV01 diverges from annuity × notional × 0.0001 due to curve basis.
    let expected_dv01 = annuity * 1_000_000.0 * 0.0001;
    let ratio = dv01.abs() / expected_dv01;
    assert!(
        (ratio - 1.0).abs() < 0.03, // 3% tolerance for multi-curve + off-market effects
        "DV01 {} should be close to annuity-based estimate {}, ratio: {}",
        dv01,
        expected_dv01,
        ratio
    );
}
