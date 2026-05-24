//! DV01 metric tests.
//!
//! Tests dollar value of a basis point: DV01 = Annuity × Notional × 0.0001
//! Sign depends on swap side (Receive vs Pay).

use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_core::money::Money;
use finstack_valuations::instruments::rates::irs::{InterestRateSwap, PayReceive};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

fn build_flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    let mut builder = DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ]);

    // For zero or negative rates, DFs may be flat or increasing
    if rate.abs() < 1e-10 || rate < 0.0 {
        builder = builder.validation(
            finstack_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        );
    }

    builder.build().unwrap()
}

fn build_market(rate: f64, base_date: Date) -> MarketContext {
    let disc_curve = build_flat_discount_curve(rate, base_date, "USD_OIS");
    let fwd_curve = ForwardCurve::builder("USD_LIBOR_3M", 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (10.0, rate)])
        .build()
        .unwrap();

    MarketContext::new().insert(disc_curve).insert(fwd_curve)
}

fn create_standard_swap(as_of: Date, end: Date, side: PayReceive) -> InterestRateSwap {
    InterestRateSwap {
        id: "IRS_DV01_TEST".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side,
        fixed: finstack_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
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
            start: as_of,
            end,
        },
        float: finstack_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
        },
        margin_spec: None,
        pricing_overrides: finstack_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    }
}

#[test]
fn test_dv01_formula_consistency() {
    // DV01 = Annuity × Notional × 0.0001
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_standard_swap(as_of, end, PayReceive::Receive);
    let market = build_market(0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity, MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let annuity = *result.measures.get("annuity").unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();

    let expected_dv01 = annuity * 1_000_000.0 * 0.0001;

    // Identity holds at par (market rate = fixed rate = 5%) on matched disc/fwd curves:
    // floating sensitivity cancels, leaving DV01 ≈ annuity × notional × 1bp.
    assert!(
        (dv01.abs() - expected_dv01.abs()).abs() < 0.01,
        "DV01={} should match annuity×notional×1bp formula (at-par, matched curves), got {}",
        dv01,
        expected_dv01
    );
}

#[test]
fn test_dv01_five_year_swap() {
    // $1MM 5Y swap should have DV01 around $430
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_standard_swap(as_of, end, PayReceive::Receive);
    let market = build_market(0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    assert!(
        dv01.abs() > 400.0 && dv01.abs() < 455.0,
        "$1MM 5Y swap DV01 should be ~$430, got {}",
        dv01
    );
}

#[test]
fn test_dv01_scales_with_notional() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let market = build_market(0.05, as_of);

    let swap_1m = create_standard_swap(as_of, end, PayReceive::Receive);

    let mut swap_10m = create_standard_swap(as_of, end, PayReceive::Receive);
    swap_10m.notional = Money::new(10_000_000.0, Currency::USD);

    let dv01_1m = *swap_1m
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap();

    let dv01_10m = *swap_10m
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap();

    let ratio = dv01_10m.abs() / dv01_1m.abs();

    assert!(
        (ratio - 10.0).abs() < 0.01,
        "DV01 should scale linearly with notional: ratio={}",
        ratio
    );
}

#[test]
fn test_dv01_receive_vs_pay_opposite_signs() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let market = build_market(0.05, as_of);

    let swap_receive = create_standard_swap(as_of, end, PayReceive::Receive);
    let swap_pay = create_standard_swap(as_of, end, PayReceive::Pay);

    let dv01_receive = *swap_receive
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap();

    let dv01_pay = *swap_pay
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap();

    assert!(
        dv01_receive * dv01_pay < 0.0,
        "Receive and pay DV01 should have opposite signs: receive={}, pay={}",
        dv01_receive,
        dv01_pay
    );

    // Same swap, opposite direction: DV01 magnitudes must be exactly equal
    assert!(
        (dv01_receive.abs() - dv01_pay.abs()).abs() < 1e-6,
        "DV01 magnitudes must be equal: |receive|={}, |pay|={}",
        dv01_receive.abs(),
        dv01_pay.abs()
    );
}

#[test]
fn test_dv01_longer_maturity_higher_dv01() {
    let as_of = date!(2024 - 01 - 01);

    let market = build_market(0.05, as_of);

    let swap_2y = create_standard_swap(as_of, date!(2026 - 01 - 01), PayReceive::Receive);
    let swap_5y = create_standard_swap(as_of, date!(2029 - 01 - 01), PayReceive::Receive);
    let swap_10y = create_standard_swap(as_of, date!(2034 - 01 - 01), PayReceive::Receive);

    let dv01_2y = *swap_2y
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap();

    let dv01_5y = *swap_5y
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap();

    let dv01_10y = *swap_10y
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap();

    assert!(dv01_2y.abs() < dv01_5y.abs());
    assert!(dv01_5y.abs() < dv01_10y.abs());
}

#[test]
fn test_dv01_short_swap() {
    // 1Y swap has lower DV01
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2025 - 01 - 01);

    let swap = create_standard_swap(as_of, end, PayReceive::Receive);
    let market = build_market(0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    assert!(
        dv01.abs() < 150.0,
        "1Y swap DV01 should be small, got {}",
        dv01
    );
}

#[test]
fn test_dv01_higher_rates_lower_dv01() {
    // Higher discount rates → lower annuity → lower DV01
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_standard_swap(as_of, end, PayReceive::Receive);
    let market_3pct = build_market(0.03, as_of);
    let market_7pct = build_market(0.07, as_of);

    let dv01_3pct = *swap
        .price_with_metrics(
            &market_3pct,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap();

    let dv01_7pct = *swap
        .price_with_metrics(
            &market_7pct,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap();

    assert!(
        dv01_7pct.abs() < dv01_3pct.abs(),
        "Higher rate should give lower DV01: 7%={}, 3%={}",
        dv01_7pct,
        dv01_3pct
    );
}

#[test]
fn test_dv01_receive_fixed_negative() {
    // Receive fixed has negative DV01 (QuantLib/market standard)
    // Rationale: Receiving fixed loses value when rates RISE
    // (floating leg payments increase while fixed receipts stay constant)
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_standard_swap(as_of, end, PayReceive::Receive);
    let market = build_market(0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    assert!(
        dv01 < 0.0,
        "Receive fixed DV01 should be negative (market standard), got {}",
        dv01
    );
}

#[test]
fn test_dv01_pay_fixed_positive() {
    // Pay fixed has positive DV01 (QuantLib/market standard)
    // Rationale: Paying fixed GAINS value when rates RISE
    // (floating leg receipts increase while fixed payments stay constant)
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_standard_swap(as_of, end, PayReceive::Pay);
    let market = build_market(0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    assert!(
        dv01 > 0.0,
        "Pay fixed DV01 should be positive (market standard), got {}",
        dv01
    );
}

#[test]
fn test_dv01_typical_range() {
    // Verify DV01 is in reasonable range for typical swap
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_standard_swap(as_of, end, PayReceive::Receive);
    let market = build_market(0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    // For $1MM, 5Y swap, DV01 should be in reasonable range
    assert!(
        dv01.abs() > 100.0 && dv01.abs() < 1000.0,
        "DV01 {} outside typical range for $1MM 5Y swap",
        dv01
    );
}
