//! IR Future edge cases and error handling tests.

use super::utils::*;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::rates::ir_future::{FutureContractSpecs, Position};
use time::macros::date;

#[test]
fn test_zero_tau() {
    let start = date!(2024 - 07 - 01);
    let end = date!(2024 - 07 - 02); // One day accrual (minimal but non-zero)
    let as_of = date!(2024 - 01 - 01);

    // Very short accrual period
    let future = create_custom_future(
        "NEAR_ZERO_TAU",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );
    let market = build_standard_market(as_of, 0.05);

    let pv = future.value(&market, as_of).unwrap();

    // Very short tau should give small PV
    assert!(
        pv.amount().abs() < 10000.0,
        "Very short tau should give small PV"
    );
}

#[test]
fn test_zero_face_value() {
    let (_, start, end) = standard_dates();
    let as_of = time::macros::date!(2024 - 01 - 01);
    let market = build_standard_market(as_of, 0.05);

    let specs = FutureContractSpecs {
        face_value: 0.0,
        convexity_adjustment: Some(0.0),
        ..FutureContractSpecs::default()
    };

    let future = create_standard_future(start, end).with_contract_specs(specs);

    let err = future
        .value(&market, as_of)
        .expect_err("zero face value is not a valid exchange contract");
    assert!(err.to_string().contains("face_value"));
}

#[test]
fn test_negative_quoted_price() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    // Negative price (unusual but possible in some markets)
    let future = create_custom_future(
        "NEG_PRICE",
        1_000_000.0,
        start,
        start,
        end,
        -1.0,
        Position::Long,
    );
    let pv = future.value(&market, as_of).unwrap();

    // Should still calculate
    assert!(pv.amount().is_finite());
}

#[test]
fn test_very_large_notional() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    // Billion dollar position
    let notional = 1_000_000_000.0;
    let future = create_custom_future("LARGE", notional, start, start, end, 97.50, Position::Long);
    let pv = future.value(&market, as_of).unwrap();

    assert!(pv.amount().is_finite());

    // Scale-aware bound: PV should be at most a few percent of notional
    // For a 2.5% rate (price 97.50 vs implied 100-5=95), PV is notional × price_diff × DF
    // Maximum reasonable bound: ~5% of notional = $50M for $1B position
    let max_reasonable_pv = notional * 0.05;
    assert!(
        pv.amount().abs() < max_reasonable_pv,
        "PV should be reasonable for notional: pv={:.2}, max={:.2}",
        pv.amount(),
        max_reasonable_pv
    );
}

#[test]
fn test_very_small_notional() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    // $1 position
    let future = create_custom_future("SMALL", 1.0, start, start, end, 97.50, Position::Long);
    let pv = future.value(&market, as_of).unwrap();

    assert!(pv.amount().is_finite());
    assert!(pv.amount().abs() < 1.0);
}

#[test]
fn test_missing_discount_curve() {
    let (as_of, start, end) = standard_dates();

    // Empty market context
    let empty_market = MarketContext::new();

    let future = create_standard_future(start, end);
    let result = future.value(&empty_market, as_of);

    // Should error due to missing curve
    assert!(result.is_err());
}

#[test]
fn test_missing_forward_curve() {
    let (as_of, start, end) = standard_dates();

    // Only discount curve, no forward curve
    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let partial_market = MarketContext::new().insert(disc_curve);

    let future = create_standard_future(start, end);
    let result = future.value(&partial_market, as_of);

    // Should error due to missing forward curve
    assert!(result.is_err());
}

#[test]
fn test_wrong_curve_ids() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    let mut future = create_standard_future(start, end);
    future.discount_curve_id = "NONEXISTENT_DISC".into();

    let result = future.value(&market, as_of);
    assert!(result.is_err());
}

#[test]
fn test_inverted_period_dates() {
    let (as_of, _, _) = standard_dates();
    let start = date!(2024 - 10 - 01);
    let end = date!(2024 - 07 - 01); // End before start

    let future = create_custom_future(
        "INVERTED",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );
    let market = build_standard_market(as_of, 0.05);

    // Should return InvalidDateRange error
    let result = future.value(&market, as_of);

    use finstack_quant_core::{Error, InputError};
    assert!(
        matches!(result, Err(Error::Input(InputError::InvalidDateRange))),
        "Expected InvalidDateRange error, got {:?}",
        result
    );
}

#[test]
fn test_cashflow_expired() {
    use finstack_quant_cashflows::CashflowProvider;

    let as_of = date!(2024 - 07 - 15);
    let expiry = date!(2024 - 07 - 01); // Already expired
    let end = date!(2024 - 10 - 01);

    let future = create_custom_future(
        "EXPIRED_CF",
        1_000_000.0,
        expiry,
        expiry,
        end,
        97.50,
        Position::Long,
    );
    let market = build_standard_market(as_of, 0.05);

    let cashflows = future.dated_cashflows(&market, as_of).unwrap();

    // Should return empty schedule for expired future
    assert!(
        cashflows.is_empty(),
        "Expired future should have no cashflows"
    );
}

#[test]
fn test_cashflow_active() {
    use finstack_quant_cashflows::CashflowProvider;

    let (as_of, start, end) = standard_dates();
    let future = create_standard_future(start, end);
    let market = build_standard_market(as_of, 0.05);

    let cashflows = future.dated_cashflows(&market, as_of).unwrap();

    // Futures are daily settled, so standard discounting schedule is empty
    // The value is returned directly by npv() / value()
    assert!(
        cashflows.is_empty(),
        "Active future should have no discountable cashflows (daily settled)"
    );
}

#[test]
fn test_extreme_rate_difference() {
    let (as_of, start, end) = standard_dates();

    // Market rate 10%, implied rate 0.5% (quoted price 99.5)
    let market = build_standard_market(as_of, 0.10);
    let future = create_custom_future(
        "EXTREME",
        1_000_000.0,
        start,
        start,
        end,
        99.5,
        Position::Long,
    );

    let pv = future.value(&market, as_of).unwrap();

    // Large rate difference should produce large P&L
    assert!(pv.amount().is_finite());
    assert!(
        pv.amount().abs() > 1000.0,
        "Large rate difference should produce meaningful P&L"
    );
}

#[test]
fn test_zero_notional() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    let future = create_custom_future(
        "ZERO_NOTIONAL",
        0.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );
    let pv = future.value(&market, as_of).unwrap();

    // Zero notional should give zero PV
    assert_eq!(pv.amount(), 0.0);
}

#[test]
fn test_price_at_par() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    // Price at par (100.0) => 0% implied rate
    let future = create_custom_future("PAR", 1_000_000.0, start, start, end, 100.0, Position::Long);
    let pv = future.value(&market, as_of).unwrap();

    // Should still produce valid valuation (negative due to positive market rate)
    assert!(pv.amount().is_finite());
}
