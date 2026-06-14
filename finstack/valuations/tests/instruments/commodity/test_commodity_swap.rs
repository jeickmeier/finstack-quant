//! Integration tests for CommoditySwap pricing.

use finstack_cashflows::CashflowProvider;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, Tenor, TenorUnit};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, PriceCurve};
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::instruments::commodity::commodity_swap::CommoditySwap;
use finstack_valuations::instruments::rates::irs::PayReceive;
use finstack_valuations::instruments::Attributes;
use finstack_valuations::instruments::CommodityUnderlyingParams;
use finstack_valuations::instruments::Instrument;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::Month;

/// Helper to create a test market context with discount and price curves.
fn create_test_market() -> MarketContext {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Create a 5% discount curve for USD-OIS
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (0.5, 0.975), (1.0, 0.95), (5.0, 0.78)])
        .build()
        .expect("discount curve should build");

    // Create a price curve for NG forward prices (slight contango)
    let price_curve = PriceCurve::builder("NG-SPOT-AVG")
        .base_date(base_date)
        .spot_price(3.50)
        .knots([
            (0.0, 3.50),
            (0.25, 3.55),
            (0.5, 3.60),
            (0.75, 3.65),
            (1.0, 3.70),
        ])
        .build()
        .expect("price curve should build");

    MarketContext::new()
        .insert(discount_curve)
        .insert(price_curve)
}

#[test]
fn test_commodity_swap_pricing() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = create_test_market();

    let swap = CommoditySwap::builder()
        .id(InstrumentId::new("NG-TEST"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "NG",
            "MMBTU",
            Currency::USD,
        ))
        .quantity(10000.0)
        .fixed_price(Decimal::try_from(3.50).expect("valid decimal")) // Same as spot
        .floating_index_id(CurveId::new("NG-SPOT-AVG"))
        .side(PayReceive::Pay)
        .start_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
        .maturity(Date::from_calendar_date(2025, Month::June, 30).unwrap())
        .frequency(Tenor::new(1, TenorUnit::Months))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = swap.value(&market, as_of).expect("should price");

    // Verify basic properties
    assert_eq!(npv.currency(), Currency::USD);

    // In contango (forward > spot), pay-fixed swap should have positive NPV
    // because the floating leg receives higher forward prices
    assert!(
        npv.amount() > 0.0,
        "Pay-fixed swap in contango should have positive NPV, got {}",
        npv.amount()
    );
}

#[test]
fn test_commodity_swap_fixed_leg_pv() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = create_test_market();

    let swap = CommoditySwap::builder()
        .id(InstrumentId::new("FIXED-LEG-TEST"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "NG",
            "MMBTU",
            Currency::USD,
        ))
        .quantity(10000.0)
        .fixed_price(Decimal::try_from(3.50).expect("valid decimal"))
        .floating_index_id(CurveId::new("NG-SPOT-AVG"))
        .side(PayReceive::Pay)
        .start_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
        .maturity(Date::from_calendar_date(2025, Month::June, 30).unwrap())
        .frequency(Tenor::new(1, TenorUnit::Months))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let fixed_pv = swap.fixed_leg_pv(&market, as_of).expect("should compute");

    // Fixed leg should have positive PV
    // Each period is 10000 * 3.50 = 35000, with ~6 periods, total ~210000 before discounting
    assert!(fixed_pv > 0.0);
    assert!(fixed_pv < 250000.0); // Reasonable upper bound
}

#[test]
fn test_commodity_swap_payment_schedule() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025, Month::December, 31).unwrap();

    let swap = CommoditySwap::builder()
        .id(InstrumentId::new("SCHEDULE-TEST"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "NG",
            "MMBTU",
            Currency::USD,
        ))
        .quantity(10000.0)
        .fixed_price(Decimal::try_from(3.50).expect("valid decimal"))
        .floating_index_id(CurveId::new("NG-SPOT-AVG"))
        .side(PayReceive::Pay)
        .start_date(start)
        .maturity(end)
        .frequency(Tenor::new(1, TenorUnit::Months))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let schedule = swap
        .payment_schedule(as_of)
        .expect("should generate schedule");

    // Should have approximately 12 monthly payments
    assert!(
        schedule.len() >= 11,
        "Expected at least 11 payments, got {}",
        schedule.len()
    );
    assert!(
        schedule.len() <= 13,
        "Expected at most 13 payments, got {}",
        schedule.len()
    );
}

#[test]
fn test_commodity_swap_receive_fixed() {
    // Test that receiving fixed gives opposite NPV to paying fixed
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = create_test_market();

    let pay_fixed = CommoditySwap::builder()
        .id(InstrumentId::new("PAY-FIXED"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "NG",
            "MMBTU",
            Currency::USD,
        ))
        .quantity(10000.0)
        .fixed_price(Decimal::try_from(3.50).expect("valid decimal"))
        .floating_index_id(CurveId::new("NG-SPOT-AVG"))
        .side(PayReceive::Pay)
        .start_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
        .maturity(Date::from_calendar_date(2025, Month::June, 30).unwrap())
        .frequency(Tenor::new(1, TenorUnit::Months))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let receive_fixed = CommoditySwap::builder()
        .id(InstrumentId::new("RECEIVE-FIXED"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "NG",
            "MMBTU",
            Currency::USD,
        ))
        .quantity(10000.0)
        .fixed_price(Decimal::try_from(3.50).expect("valid decimal"))
        .floating_index_id(CurveId::new("NG-SPOT-AVG"))
        .side(PayReceive::Receive) // Receive fixed
        .start_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
        .maturity(Date::from_calendar_date(2025, Month::June, 30).unwrap())
        .frequency(Tenor::new(1, TenorUnit::Months))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv_pay = pay_fixed.value(&market, as_of).expect("should price");
    let npv_recv = receive_fixed.value(&market, as_of).expect("should price");

    // NPVs should be opposite (within floating point tolerance)
    let diff = (npv_pay.amount() + npv_recv.amount()).abs();
    assert!(
        diff < 0.01,
        "Expected opposite NPVs, got pay={}, recv={}, diff={}",
        npv_pay.amount(),
        npv_recv.amount(),
        diff
    );
}

// =============================================================================
// Round-trip and Parity Tests
// =============================================================================

/// Round-trip test: Swap at par fixed price should have NPV ≈ 0
///
/// If we set the fixed price equal to the average expected floating price,
/// the swap should have near-zero NPV at inception.
#[test]
fn test_commodity_swap_par_rate_round_trip() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = create_test_market();
    let start_date = as_of;
    let end_date = Date::from_calendar_date(2025, Month::June, 30).unwrap();

    // Step 1: Create a swap with a test fixed price to get the floating leg PV
    let test_swap = CommoditySwap::builder()
        .id(InstrumentId::new("PAR-RATE-TEST"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "NG",
            "MMBTU",
            Currency::USD,
        ))
        .quantity(10000.0)
        .fixed_price(Decimal::try_from(1.0).expect("valid decimal")) // Placeholder
        .floating_index_id(CurveId::new("NG-SPOT-AVG"))
        .side(PayReceive::Pay)
        .start_date(start_date)
        .maturity(end_date)
        .frequency(Tenor::new(1, TenorUnit::Months))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    // Get floating and fixed leg PVs
    let floating_pv = test_swap
        .floating_leg_pv(&market, as_of)
        .expect("floating pv");
    let fixed_leg_pv_per_dollar = test_swap.fixed_leg_pv(&market, as_of).expect("fixed pv");

    // Calculate the par fixed price: F_par = Floating_PV / Fixed_PV_per_dollar
    // where Fixed_PV_per_dollar is the fixed leg PV assuming fixed_price = 1.0
    // But our fixed leg PV uses the actual fixed_price, so we need to adjust
    // Fixed leg PV = ∑ Q × P_fixed × DF, so PV_per_unit = Fixed_PV / P_fixed
    let fixed_pv_per_unit = fixed_leg_pv_per_dollar
        / test_swap
            .fixed_price
            .to_f64()
            .expect("fixed_price converts to f64");
    let par_fixed_price = floating_pv / fixed_pv_per_unit;

    // Step 2: Create swap at par rate
    let par_swap = CommoditySwap::builder()
        .id(InstrumentId::new("PAR-SWAP"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "NG",
            "MMBTU",
            Currency::USD,
        ))
        .quantity(10000.0)
        .fixed_price(Decimal::try_from(par_fixed_price).expect("valid decimal"))
        .floating_index_id(CurveId::new("NG-SPOT-AVG"))
        .side(PayReceive::Pay)
        .start_date(start_date)
        .maturity(end_date)
        .frequency(Tenor::new(1, TenorUnit::Months))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = par_swap.value(&market, as_of).expect("should price");

    // Round-trip test: NPV should be near zero
    // Tolerance: 1bp of floating leg PV
    let tolerance = floating_pv.abs() * 0.0001;
    assert!(
        npv.amount().abs() < tolerance.max(1.0),
        "Par swap round-trip failed: NPV should be ~0, got {} (par_fixed={:.4})",
        npv.amount(),
        par_fixed_price
    );
}

/// Test that cashflows sum (discounted) equals NPV
///
/// This validates internal consistency of the cashflow generation.
#[test]
fn test_commodity_swap_cashflow_npv_consistency() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = create_test_market();

    let swap = CommoditySwap::builder()
        .id(InstrumentId::new("CF-NPV-TEST"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "NG",
            "MMBTU",
            Currency::USD,
        ))
        .quantity(10000.0)
        .fixed_price(Decimal::try_from(3.55).expect("valid decimal"))
        .floating_index_id(CurveId::new("NG-SPOT-AVG"))
        .side(PayReceive::Pay)
        .start_date(as_of)
        .maturity(Date::from_calendar_date(2025, Month::June, 30).unwrap())
        .frequency(Tenor::new(1, TenorUnit::Months))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    // Get NPV
    let npv = swap.value(&market, as_of).expect("should price");

    // Get cashflows and compute discounted sum
    let cashflows = swap
        .dated_cashflows(&market, as_of)
        .expect("should get cashflows");
    let disc = market.get_discount("USD-OIS").expect("discount curve");

    let mut cf_pv_sum = 0.0;
    for (date, cf) in &cashflows {
        let df = disc
            .df_between_dates(as_of, *date)
            .expect("df between dates");
        cf_pv_sum += cf.amount() * df;
    }

    // NPV should equal discounted cashflow sum
    // Tolerance: 0.1% of NPV
    let tolerance = npv.amount().abs() * 0.001;
    let error = (npv.amount() - cf_pv_sum).abs();

    assert!(
        error < tolerance.max(1.0),
        "Cashflow-NPV consistency failed: NPV={:.6}, CF_PV_sum={:.6}, error={:.6}",
        npv.amount(),
        cf_pv_sum,
        error
    );
}

/// Test delta aggregation: sum of period deltas equals total delta
#[test]
fn test_commodity_swap_delta_analytical() {
    use finstack_valuations::metrics::MetricId;
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = create_test_market();

    let quantity = 10000.0;

    let swap = CommoditySwap::builder()
        .id(InstrumentId::new("DELTA-TEST"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "NG",
            "MMBTU",
            Currency::USD,
        ))
        .quantity(quantity)
        .fixed_price(Decimal::try_from(3.55).expect("valid decimal"))
        .floating_index_id(CurveId::new("NG-SPOT-AVG"))
        .side(PayReceive::Pay)
        .start_date(as_of)
        .maturity(Date::from_calendar_date(2025, Month::June, 30).unwrap())
        .frequency(Tenor::new(1, TenorUnit::Months))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    // Get delta from metric calculation
    use finstack_valuations::instruments::Instrument;
    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("should price with delta");

    let delta = result
        .measures
        .get(&MetricId::Delta)
        .expect("should have delta");

    // Analytical delta for pay-fixed swap: ∑ Q × DF(payment_date)
    let disc = market.get_discount("USD-OIS").expect("discount curve");
    let schedule = swap.payment_schedule(as_of).expect("schedule");

    let mut expected_delta = 0.0;
    for payment_date in schedule {
        if payment_date >= as_of {
            let df = disc
                .df_between_dates(as_of, payment_date)
                .expect("df between dates");
            expected_delta += quantity * df; // pay_fixed = +1 sign
        }
    }

    // Tolerance: 1% of delta (delta calculation may have some discretization)
    let tolerance = expected_delta.abs() * 0.01;
    let error = (delta - expected_delta).abs();

    assert!(
        error < tolerance,
        "Delta analytical parity failed: expected {:.2}, got {:.2}, error={:.2}",
        expected_delta,
        delta,
        error
    );
}
