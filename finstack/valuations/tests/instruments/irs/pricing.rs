//! Interest Rate Swap pricing tests.
//!
//! Tests cover:
//! - Core NPV calculation
//! - Pricing engine integration
//! - Receive vs Pay fixed
//! - Off-market swaps
//! - Theta calculation
//! - Edge cases

use crate::common::test_helpers::{dates, usd_swap_market, usd_swap_market_split};
use crate::finstack_test_utils as test_utils;
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_core::money::Money;
use finstack_valuations::instruments::rates::irs::{InterestRateSwap, PayReceive};
use finstack_valuations::instruments::Instrument;
use rust_decimal_macros::dec;

#[test]
fn test_irs_at_par_npv_zero() {
    // At-the-money swap should have NPV ≈ 0
    let as_of = dates::TODAY;
    let end = dates::five_years_hence();

    // Use consolidated helper for par market (disc = fwd)
    let market = usd_swap_market(as_of, 0.05);

    let swap = InterestRateSwap {
        id: "SWAP_PAR".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Receive,
        fixed: finstack_valuations::instruments::FixedLegSpec {
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
            start: as_of,
            end,
        },
        float: finstack_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
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
    };

    let npv = swap.value(&market, as_of).unwrap();

    assert!(
        npv.amount().abs() < 100.0, // 1bp on $1MM
        "At-par swap NPV should be near zero (within 1bp), got {} ({:.2}bp)",
        npv.amount(),
        npv.amount() / 100.0 // Convert to bp for readability
    );
}

#[test]
fn test_irs_receive_fixed_below_market() {
    // Receive fixed at 3% when market is 5% → negative NPV
    let as_of = dates::TODAY;
    let end = dates::five_years_hence();

    let market = usd_swap_market(as_of, 0.05);

    let swap = InterestRateSwap {
        id: "SWAP_OFF_MARKET".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Receive,
        fixed: finstack_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.03).expect("valid"), // Below market
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
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
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
    };

    let npv = swap.value(&market, as_of).unwrap();

    assert!(
        npv.amount() < 0.0,
        "Receive fixed below market should be negative, got {}",
        npv.amount()
    );
}

#[test]
fn test_irs_receive_fixed_above_market() {
    // Receive fixed at 7% when market is 5% → positive NPV
    let as_of = dates::TODAY;
    let end = dates::five_years_hence();

    let market = usd_swap_market(as_of, 0.05);

    let swap = InterestRateSwap {
        id: "SWAP_ABOVE_MARKET".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Receive,
        fixed: finstack_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.07).expect("valid"), // Above market
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
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
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
    };

    let npv = swap.value(&market, as_of).unwrap();

    assert!(
        npv.amount() > 0.0,
        "Receive fixed above market should be positive, got {}",
        npv.amount()
    );
}

#[test]
fn test_irs_pay_vs_receive_opposite_signs() {
    // Pay and receive should have opposite NPVs
    let as_of = dates::TODAY;
    let end = dates::five_years_hence();

    // Off-market: discount at 5%, forward at 6%
    let market = usd_swap_market_split(as_of, 0.05, 0.06);

    let fixed_leg = finstack_valuations::instruments::FixedLegSpec {
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
        start: as_of,
        end,
    };

    let float_leg = finstack_valuations::instruments::FloatLegSpec {
        discount_curve_id: "USD-OIS".into(),
        forward_curve_id: "USD-SOFR-3M".into(),
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
    };

    let swap_receive = InterestRateSwap {
        id: "SWAP_RECEIVE".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Receive,
        fixed: fixed_leg.clone(),
        float: float_leg.clone(),
        margin_spec: None,
        pricing_overrides: finstack_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let swap_pay = InterestRateSwap {
        id: "SWAP_PAY".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Pay,
        fixed: fixed_leg,
        float: float_leg,
        margin_spec: None,
        pricing_overrides: finstack_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let npv_receive = swap_receive.value(&market, as_of).unwrap();
    let npv_pay = swap_pay.value(&market, as_of).unwrap();

    // Should have opposite signs
    assert!(
        npv_receive.amount() * npv_pay.amount() < 0.0,
        "Receive and pay should have opposite signs: receive={}, pay={}",
        npv_receive.amount(),
        npv_pay.amount()
    );

    // Should be approximately equal in magnitude
    assert!(
        (npv_receive.amount().abs() - npv_pay.amount().abs()).abs() < 10.0,
        "Magnitudes should be similar: |receive|={}, |pay|={}",
        npv_receive.amount().abs(),
        npv_pay.amount().abs()
    );
}

#[test]
fn test_irs_npv_scales_with_notional() {
    let as_of = dates::TODAY;
    let end = dates::five_years_hence();

    let market = usd_swap_market_split(as_of, 0.05, 0.06);

    let swap_1m = test_utils::usd_irs_swap(
        "SWAP_1M",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let swap_10m = test_utils::usd_irs_swap(
        "SWAP_10M",
        Money::new(10_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let npv_1m = swap_1m.value(&market, as_of).unwrap();
    let npv_10m = swap_10m.value(&market, as_of).unwrap();

    // NPV is linear in notional (all cashflows scale proportionally)
    let ratio = npv_10m.amount() / npv_1m.amount();
    assert!(
        (ratio - 10.0).abs() < 1e-6,
        "NPV must scale exactly with notional: ratio={}",
        ratio
    );
}

#[test]
fn test_irs_with_spread() {
    let as_of = dates::TODAY;
    let end = dates::five_years_hence();

    let market = usd_swap_market(as_of, 0.05);

    // Swap with 50bp spread on floating leg
    let mut swap = test_utils::usd_irs_swap(
        "SWAP_SPREAD",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();
    swap.float.spread_bp = dec!(50.0);

    let npv = swap.value(&market, as_of).unwrap();

    // Paying higher floating rate → negative NPV
    assert!(
        npv.amount() < 0.0,
        "Receive fixed with spread on float should be negative, got {}",
        npv.amount()
    );
}

#[test]
fn test_irs_short_maturity() {
    // 1-year swap
    let as_of = dates::TODAY;
    let end = dates::one_year_hence();

    let market = usd_swap_market_split(as_of, 0.05, 0.06);

    let swap = test_utils::usd_irs_swap(
        "SWAP_1Y",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let npv = swap.value(&market, as_of).unwrap();

    // Should have reasonable NPV
    assert!(
        npv.amount().abs() < 50_000.0,
        "1Y swap NPV should be small, got {}",
        npv.amount()
    );
}

#[test]
fn test_irs_zero_rate() {
    // Very low rates edge case
    let as_of = dates::TODAY;
    let end = dates::five_years_hence();

    // Use very small rate instead of exactly 0 to avoid numerical issues
    let market = usd_swap_market(as_of, 0.0001);

    let swap = test_utils::usd_irs_swap(
        "SWAP_ZERO",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let npv = swap.value(&market, as_of);

    assert!(npv.is_ok(), "Should handle very low rates");
}

#[test]
fn test_irs_theta_calculation() {
    use finstack_valuations::metrics::MetricId;

    let as_of = dates::TODAY;
    let end = dates::five_years_hence();

    let market = usd_swap_market(as_of, 0.05);

    let swap = test_utils::usd_irs_swap(
        "SWAP_THETA",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta = *result.measures.get("theta").unwrap();

    // Theta should be defined
    assert!(
        theta.abs() < 100_000.0,
        "Theta should be reasonable, got {}",
        theta
    );
}

#[test]
fn test_irs_forward_starting() {
    // Swap starting in the future
    let as_of = dates::TODAY;
    let start = dates::one_year_hence();
    let end = dates::years_hence(6); // 5Y swap starting in 1Y

    let market = usd_swap_market(as_of, 0.05);

    let swap = test_utils::usd_irs_swap(
        "SWAP_FORWARD",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        start,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let npv = swap.value(&market, as_of);

    assert!(npv.is_ok(), "Should price forward-starting swap");
}

#[test]
fn test_irs_npv_currency_matches() {
    let as_of = dates::TODAY;
    let end = dates::five_years_hence();

    let market = usd_swap_market(as_of, 0.05);

    let swap = test_utils::usd_irs_swap(
        "SWAP_CCY",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let npv = swap.value(&market, as_of).unwrap();

    assert_eq!(
        npv.currency(),
        Currency::USD,
        "NPV currency should match swap currency"
    );
}
