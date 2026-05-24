//! PV Fixed metric tests - Present value of fixed leg.

use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_core::money::Money;
use finstack_valuations::instruments::rates::irs::{InterestRateSwap, PayReceive};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use rust_decimal_macros::dec;
use time::macros::date;

fn build_flat_discount_curve(rate: f64, base_date: Date) -> DiscountCurve {
    let mut builder = DiscountCurve::builder("USD_OIS")
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
    let disc_curve = build_flat_discount_curve(rate, base_date);
    let fwd_curve = ForwardCurve::builder("USD_LIBOR_3M", 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (10.0, rate)])
        .build()
        .unwrap();

    MarketContext::new().insert(disc_curve).insert(fwd_curve)
}

fn create_swap(as_of: Date, end: Date, fixed_rate: rust_decimal::Decimal) -> InterestRateSwap {
    InterestRateSwap {
        id: "IRS_PV_FIXED_TEST".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Receive,
        fixed: finstack_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: fixed_rate,
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
fn test_pv_fixed_positive() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_swap(as_of, end, dec!(0.05));
    let market = build_market(0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFixed],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv_fixed = *result.measures.get("pv_fixed").unwrap();

    assert!(pv_fixed > 0.0, "PV fixed should be positive");
}

#[test]
fn test_pv_fixed_scales_with_rate() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let market = build_market(0.05, as_of);

    let swap_3pct = create_swap(as_of, end, dec!(0.03));
    let swap_6pct = create_swap(as_of, end, dec!(0.06));

    let pv_fixed_3pct = *swap_3pct
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFixed],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("pv_fixed")
        .unwrap();

    let pv_fixed_6pct = *swap_6pct
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFixed],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("pv_fixed")
        .unwrap();

    // 6% rate should give exactly 2x PV of 3% rate: PV = rate × annuity, annuity is same for both
    let ratio = pv_fixed_6pct / pv_fixed_3pct;
    assert!(
        (ratio - 2.0).abs() < 1e-6,
        "PV must scale linearly with rate: ratio={}",
        ratio
    );
}

#[test]
fn test_pv_fixed_scales_with_notional() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let market = build_market(0.05, as_of);

    let swap_1m = create_swap(as_of, end, dec!(0.05));

    let mut swap_5m = create_swap(as_of, end, dec!(0.05));
    swap_5m.notional = Money::new(5_000_000.0, Currency::USD);

    let pv_fixed_1m = *swap_1m
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFixed],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("pv_fixed")
        .unwrap();

    let pv_fixed_5m = *swap_5m
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFixed],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("pv_fixed")
        .unwrap();

    // PV = rate × annuity × notional — annuity is identical for both swaps, so ratio is exact
    let ratio = pv_fixed_5m / pv_fixed_1m;
    assert!(
        (ratio - 5.0).abs() < 1e-6,
        "PV must scale exactly with notional: ratio={}",
        ratio
    );
}

#[test]
fn test_pv_fixed_reasonable_magnitude() {
    // $1MM notional, 5% rate, 5 years
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_swap(as_of, end, dec!(0.05));
    let market = build_market(0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFixed],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv_fixed = *result.measures.get("pv_fixed").unwrap();

    // Rough estimate: $1MM × 5% × 4.3 annuity ≈ $215,000
    assert!(
        pv_fixed > 100_000.0 && pv_fixed < 300_000.0,
        "PV fixed should be reasonable: got {}",
        pv_fixed
    );
}

#[test]
fn test_pv_fixed_independent_of_side() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let market = build_market(0.05, as_of);

    let mut swap_receive = create_swap(as_of, end, dec!(0.05));
    swap_receive.side = PayReceive::Receive;

    let mut swap_pay = create_swap(as_of, end, dec!(0.05));
    swap_pay.side = PayReceive::Pay;

    let pv_fixed_receive = *swap_receive
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFixed],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("pv_fixed")
        .unwrap();

    let pv_fixed_pay = *swap_pay
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFixed],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("pv_fixed")
        .unwrap();

    assert!(
        (pv_fixed_receive - pv_fixed_pay).abs() < 1.0,
        "PV fixed should be independent of side"
    );
}
