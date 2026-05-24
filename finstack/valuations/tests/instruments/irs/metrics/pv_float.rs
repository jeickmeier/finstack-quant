//! PV Float metric tests - Present value of floating leg.

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

fn build_curves(disc_rate: f64, fwd_rate: f64, base_date: Date) -> MarketContext {
    let disc_curve_ois = DiscountCurve::builder("USD_OIS")
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

    // Add a separate discount curve for LIBOR-based instruments
    let disc_curve_libor = DiscountCurve::builder("USD_LIBOR_DISC")
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

    let fwd_curve = ForwardCurve::builder("USD_LIBOR_3M", 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, fwd_rate), (10.0, fwd_rate)])
        .build()
        .unwrap();

    MarketContext::new()
        .insert(disc_curve_ois)
        .insert(disc_curve_libor)
        .insert(fwd_curve)
}

fn create_swap(as_of: Date, end: Date, fixed_rate: rust_decimal::Decimal) -> InterestRateSwap {
    InterestRateSwap {
        id: "IRS_PV_FLOAT_TEST".into(),
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
            discount_curve_id: "USD_LIBOR_DISC".into(), // Use different discount curve for non-OIS swap
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
fn test_pv_float_positive() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_swap(as_of, end, dec!(0.05));
    let market = build_curves(0.05, 0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFloat],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv_float = *result.measures.get("pv_float").unwrap();

    assert!(pv_float > 0.0, "PV float should be positive");
}

#[test]
fn test_pv_float_scales_with_forward_rate() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_swap(as_of, end, dec!(0.05));

    let market_3pct = build_curves(0.05, 0.03, as_of);
    let market_6pct = build_curves(0.05, 0.06, as_of);

    let pv_float_3pct = *swap
        .price_with_metrics(
            &market_3pct,
            as_of,
            &[MetricId::PvFloat],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("pv_float")
        .unwrap();

    let pv_float_6pct = *swap
        .price_with_metrics(
            &market_6pct,
            as_of,
            &[MetricId::PvFloat],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("pv_float")
        .unwrap();

    assert!(
        pv_float_6pct > pv_float_3pct,
        "Higher forward rate should give higher PV: 6%={}, 3%={}",
        pv_float_6pct,
        pv_float_3pct
    );
}

#[test]
fn test_pv_float_with_spread() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let market = build_curves(0.05, 0.05, as_of);

    let swap_no_spread = create_swap(as_of, end, dec!(0.05));

    let mut swap_with_spread = create_swap(as_of, end, dec!(0.05));
    swap_with_spread.float.spread_bp = dec!(50.0);

    let pv_float_no_spread = *swap_no_spread
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFloat],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("pv_float")
        .unwrap();

    let pv_float_with_spread = *swap_with_spread
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFloat],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("pv_float")
        .unwrap();

    assert!(
        pv_float_with_spread > pv_float_no_spread,
        "Spread should increase PV float"
    );
}

#[test]
fn test_pv_float_equals_fixed_at_par() {
    // At par rate, PV fixed ≈ PV float
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_swap(as_of, end, dec!(0.05));
    let market = build_curves(0.05, 0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFixed, MetricId::PvFloat],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv_fixed = *result.measures.get("pv_fixed").unwrap();
    let pv_float = *result.measures.get("pv_float").unwrap();

    // At par rate, PV fixed = PV float by definition (small residual from convention difference)
    assert!(
        (pv_fixed - pv_float).abs() < 100.0,
        "At par rate, PV fixed ≈ PV float: fixed={}, float={}",
        pv_fixed,
        pv_float
    );
}

#[test]
fn test_swap_npv_matches_leg_pvs() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_swap(as_of, end, dec!(0.05));
    let market = build_curves(0.05, 0.05, as_of);

    // Base NPV from the instrument pricer
    let npv = swap.value(&market, as_of).unwrap().amount();

    // Leg PVs from metrics should recombine to the same NPV
    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFixed, MetricId::PvFloat],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv_fixed = *result.measures.get("pv_fixed").unwrap();
    let pv_float = *result.measures.get("pv_float").unwrap();

    let recomposed = match swap.side {
        PayReceive::Receive => pv_fixed - pv_float,
        PayReceive::Pay => pv_float - pv_fixed,
    };

    assert!(
        (npv - recomposed).abs() < 1e-2,
        "NPV from instrument ({}) should match recomposed leg PVs ({}). \
         pv_fixed={}, pv_float={}",
        npv,
        recomposed,
        pv_fixed,
        pv_float
    );
}

#[test]
fn test_pv_float_independent_of_side() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let market = build_curves(0.05, 0.05, as_of);

    let mut swap_receive = create_swap(as_of, end, dec!(0.05));
    swap_receive.side = PayReceive::Receive;

    let mut swap_pay = create_swap(as_of, end, dec!(0.05));
    swap_pay.side = PayReceive::Pay;

    let pv_float_receive = *swap_receive
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFloat],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("pv_float")
        .unwrap();

    let pv_float_pay = *swap_pay
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFloat],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("pv_float")
        .unwrap();

    assert!(
        (pv_float_receive - pv_float_pay).abs() < 1.0,
        "PV float should be independent of side"
    );
}
