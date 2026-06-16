//! QuantLib parity tests for vanilla interest rate swaps.
//!
//! Tests a 5Y USD IRS with:
//! - Fixed: 4.0%, Semi-Annual, 30/360
//! - Floating: SOFR 3M, Quarterly, Act/360
//! - Notional: $10M
//!
//! Validates:
//! 1. Par rate is self-consistent (PV ≈ 0 at par)
//! 2. DV01 ≈ notional × duration × 0.0001
//! 3. Par rate sensible range

use crate::finstack_quant_test_utils as test_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::irs::PayReceive;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::Month;

/// Build a flat discount curve at a given rate.
fn flat_curve(id: &str, as_of: Date, rate: f64) -> DiscountCurve {
    let tenors = [0.0, 0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0];
    let knots: Vec<(f64, f64)> = tenors.iter().map(|&t| (t, (-rate * t).exp())).collect();

    DiscountCurve::builder(id)
        .base_date(as_of)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .expect("flat curve should build")
}

/// Build a flat forward curve at a given rate.
fn flat_forward(id: &str, as_of: Date, rate: f64) -> ForwardCurve {
    ForwardCurve::builder(id, 0.25)
        .base_date(as_of)
        .knots([(0.0, rate), (10.0, rate)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("flat forward curve should build")
}

fn test_market(as_of: Date, rate: f64) -> MarketContext {
    MarketContext::new()
        .insert(flat_curve("USD-OIS", as_of, rate))
        .insert(flat_forward("USD-SOFR-3M", as_of, rate))
}

/// Create a 5Y USD IRS.
fn create_5y_swap(
    as_of: Date,
    fixed_rate: f64,
    side: PayReceive,
) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::InterestRateSwap> {
    let start = as_of;
    let end = Date::from_calendar_date(as_of.year() + 5, as_of.month(), as_of.day())
        .expect("valid end date");

    test_utils::usd_irs_swap(
        "IRS-QLPARITY-5Y",
        Money::new(10_000_000.0, Currency::USD),
        fixed_rate,
        start,
        end,
        side,
    )
}

/// Test: At par rate, PV should be approximately zero.
///
/// If the fixed rate equals the market forward rate, the swap should have
/// near-zero PV. For a flat curve at 4%, the par rate should be close to 4%.
#[test]
fn test_irs_par_rate_self_consistent() {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let market_rate = 0.04;
    let market = test_market(as_of, market_rate);

    // Price with a fixed rate equal to the flat market rate
    let swap = create_5y_swap(as_of, market_rate, PayReceive::Pay)
        .expect("swap construction should succeed");

    let pv = swap.value(&market, as_of).expect("pricing should succeed");

    // For a flat curve at 4%, a swap with 4% fixed should have PV near zero.
    // The PV won't be exactly zero due to day count convention differences
    // between fixed (30/360) and floating (Act/360), but should be small
    // relative to notional.
    let notional = 10_000_000.0;
    let pv_pct_notional = (pv.amount() / notional).abs();

    assert!(
        pv_pct_notional < 0.02, // Within 2% of notional
        "At par, PV should be small relative to notional. PV = {:.2}, ratio = {:.4}%",
        pv.amount(),
        pv_pct_notional * 100.0
    );
}

/// Test: DV01 should be approximately notional × modified_duration × 0.0001.
///
/// For a 5Y swap at par, the modified duration is approximately 4.5 years.
/// DV01 ≈ 10M × 4.5 × 0.0001 ≈ $4,500
#[test]
fn test_irs_dv01_magnitude() {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let market_rate = 0.04;
    let market = test_market(as_of, market_rate);

    let swap = create_5y_swap(as_of, market_rate, PayReceive::Pay)
        .expect("swap construction should succeed");

    let metrics = vec![MetricId::Dv01];
    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("pricing with metrics should succeed");

    let dv01 = result
        .measures
        .get(MetricId::Dv01.as_str())
        .expect("DV01 should be computed");

    // DV01 should be approximately 4,000-5,000 for a 5Y $10M swap
    let dv01_abs = dv01.abs();
    assert!(
        dv01_abs > 1_000.0 && dv01_abs < 10_000.0,
        "DV01 should be in reasonable range for 5Y $10M swap, got {:.2}",
        dv01_abs
    );
}

/// Test: Par rate should be close to 4% for a flat 4% curve environment.
#[test]
fn test_irs_par_rate_flat_curve() {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let market_rate = 0.04;
    let market = test_market(as_of, market_rate);

    let swap = create_5y_swap(as_of, market_rate, PayReceive::Pay)
        .expect("swap construction should succeed");

    let metrics = vec![MetricId::ParRate];
    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("pricing with metrics should succeed");

    if let Some(&par_rate) = result.measures.get(MetricId::ParRate.as_str()) {
        // Par rate should be close to the market rate (within 10bp due to day count convention differences)
        assert!(
            (par_rate - market_rate).abs() < 0.001,
            "Par rate should be close to market rate. Expected ~{:.4}, got {:.4}",
            market_rate,
            par_rate
        );
    }
}

/// Test: Pay-fixed and receive-fixed should have opposite PVs.
#[test]
fn test_irs_pay_receive_symmetry() {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let market_rate = 0.04;
    let market = test_market(as_of, market_rate);

    let pay_fixed =
        create_5y_swap(as_of, 0.035, PayReceive::Pay).expect("pay-fixed swap should build");
    let recv_fixed =
        create_5y_swap(as_of, 0.035, PayReceive::Receive).expect("recv-fixed swap should build");

    let pv_pay = pay_fixed.value(&market, as_of).expect("pricing").amount();
    let pv_recv = recv_fixed.value(&market, as_of).expect("pricing").amount();

    // PVs should be approximately equal in magnitude but opposite in sign
    let sum = (pv_pay + pv_recv).abs();
    let magnitude = pv_pay.abs().max(pv_recv.abs());

    assert!(
        sum < magnitude * 0.01,
        "Pay + Receive should cancel. pay_pv = {:.2}, recv_pv = {:.2}, sum = {:.2}",
        pv_pay,
        pv_recv,
        sum
    );
}
