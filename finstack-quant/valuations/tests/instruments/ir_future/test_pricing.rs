//! IR Future pricing tests covering various scenarios.

use super::utils::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::Position;

#[test]
fn test_basic_npv() {
    let (as_of, start, end) = standard_dates();
    let future = create_standard_future(start, end);
    let market = build_standard_market(as_of, 0.05);

    let pv = future.value(&market, as_of).unwrap();

    // Should have a reasonable PV
    assert!(
        pv.amount().abs() < 100_000.0,
        "PV should be reasonable, got {}",
        pv.amount()
    );
    assert_eq!(pv.currency(), Currency::USD);
}

#[test]
fn test_long_position_pnl() {
    let (as_of, start, end) = standard_dates();

    // Long position with quoted price 97.50 (2.5% implied rate)
    let future = create_custom_future(
        "LONG",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );

    // Market rate 5% > implied 2.5% => negative P&L for long
    let market = build_standard_market(as_of, 0.05);
    let pv = future.value(&market, as_of).unwrap();

    // Long loses when rates rise (price falls)
    assert!(
        pv.amount() < 0.0,
        "Long should lose when market rate > implied rate"
    );
}

#[test]
fn test_short_position_pnl() {
    let (as_of, start, end) = standard_dates();

    // Short position with quoted price 97.50 (2.5% implied rate)
    let future = create_custom_future(
        "SHORT",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Short,
    );

    // Market rate 5% > implied 2.5% => positive P&L for short
    let market = build_standard_market(as_of, 0.05);
    let pv = future.value(&market, as_of).unwrap();

    // Short gains when rates rise (price falls)
    assert!(
        pv.amount() > 0.0,
        "Short should gain when market rate > implied rate"
    );
}

#[test]
fn test_long_vs_short_symmetry() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    let long = create_custom_future(
        "LONG",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );
    let short = create_custom_future(
        "SHORT",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Short,
    );

    let pv_long = long.value(&market, as_of).unwrap().amount();
    let pv_short = short.value(&market, as_of).unwrap().amount();

    // Long and short should be opposite signs and equal magnitude
    assert!(
        (pv_long + pv_short).abs() < 1e-6,
        "Long and short should offset: {} + {} = {}",
        pv_long,
        pv_short,
        pv_long + pv_short
    );
}

#[test]
fn test_multiple_contracts_scaling() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    // Single contract
    let single = create_custom_future(
        "SINGLE",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );
    let pv_single = single.value(&market, as_of).unwrap().amount();

    // Five contracts
    let multiple = create_custom_future(
        "FIVE",
        5_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );
    let pv_multiple = multiple.value(&market, as_of).unwrap().amount();

    // PV is linear in contract size — must hold to float precision
    assert!(
        (pv_multiple - 5.0 * pv_single).abs() < 1e-6,
        "5 contracts must be exactly 5x single: {} vs {}",
        pv_multiple,
        5.0 * pv_single
    );
}

#[test]
fn test_near_term_vs_far_forward() {
    let market_as_of = time::macros::date!(2024 - 01 - 01);
    let market = build_standard_market(market_as_of, 0.05);

    let (_, near_start, near_end) = near_term_dates();
    let (_, far_start, far_end) = far_forward_dates();

    let near_future = create_custom_future(
        "NEAR",
        1_000_000.0,
        near_start,
        near_start,
        near_end,
        97.50,
        Position::Long,
    );
    let far_future = create_custom_future(
        "FAR",
        1_000_000.0,
        far_start,
        far_start,
        far_end,
        97.50,
        Position::Long,
    );

    let pv_near = near_future.value(&market, market_as_of).unwrap().amount();
    let pv_far = far_future.value(&market, market_as_of).unwrap().amount();

    // Both should be valid
    assert!(pv_near.is_finite());
    assert!(pv_far.is_finite());
}

#[test]
fn test_zero_rate_environment() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.001); // Very low but non-zero rate

    // Quoted price 99.9 => 0.1% implied rate
    let future = create_custom_future(
        "LOW_RATE",
        1_000_000.0,
        start,
        start,
        end,
        99.9,
        Position::Long,
    );
    let pv = future.value(&market, as_of).unwrap();

    // Should produce valid valuation
    assert!(pv.amount().is_finite());
}

#[test]
fn test_high_rate_environment() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.10); // 10% rates

    let future = create_custom_future(
        "HIGH_RATE",
        1_000_000.0,
        start,
        start,
        end,
        90.0,
        Position::Long,
    );
    let pv = future.value(&market, as_of).unwrap();

    // Should still produce valid valuation
    assert!(pv.amount().is_finite());
}

#[test]
fn test_discount_curve_id() {
    let (_, start, end) = standard_dates();
    let future = create_standard_future(start, end);

    let disc_id = future
        .market_dependencies()
        .expect("market_dependencies")
        .curves
        .discount_curves
        .first()
        .cloned()
        .expect("IR future should declare a discount curve");
    assert_eq!(disc_id.as_str(), "USD_OIS");
}

#[test]
fn test_different_currency() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.03);

    let mut future = create_standard_future(start, end);
    future.notional = Money::new(1_000_000.0, Currency::EUR);

    let pv = future.value(&market, as_of).unwrap();
    assert_eq!(pv.currency(), Currency::EUR);
}

#[test]
fn test_fractional_notional() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    // 0.5 contracts
    let fractional =
        create_custom_future("FRAC", 500_000.0, start, start, end, 97.50, Position::Long);
    let pv_frac = fractional.value(&market, as_of).unwrap().amount();

    // Full contract
    let full = create_custom_future(
        "FULL",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );
    let pv_full = full.value(&market, as_of).unwrap().amount();

    // PV is linear in contract size — must hold to float precision
    assert!(
        (pv_frac - 0.5 * pv_full).abs() < 1e-6,
        "Half contract must be exactly 0.5x full: {} vs {}",
        pv_frac,
        0.5 * pv_full
    );
}

#[test]
fn test_pricing_with_different_day_counts() {
    use finstack_quant_core::dates::DayCount;

    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    let day_counts = vec![DayCount::Act360, DayCount::Act365F, DayCount::Thirty360];

    for dc in day_counts {
        let mut future = create_standard_future(start, end);
        future.day_count = dc;

        let pv = future.value(&market, as_of).unwrap();
        assert!(pv.amount().is_finite(), "PV should be valid for {:?}", dc);
    }
}
