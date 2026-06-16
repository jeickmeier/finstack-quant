//! Weighted Average Life (WAL) metric tests for bonds.
//!
//! `MetricId::WAL` (`BondWalCalculator`) is registered but was previously
//! unexercised. WAL = Σ(Principalᵢ·tᵢ)/Σ(Principalᵢ) using ACT/365F year
//! fractions from the valuation date. For a bullet bond (single principal flow
//! at maturity) WAL equals the time to maturity, and it shrinks as the
//! valuation date advances toward maturity.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn flat_curve(as_of: Date) -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.60)])
        .build()
        .unwrap();
    MarketContext::new().insert(curve)
}

fn wal(bond: &Bond, market: &MarketContext, as_of: Date) -> f64 {
    *bond
        .price_with_metrics(
            market,
            as_of,
            &[MetricId::WAL],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("wal")
        .unwrap()
}

#[test]
fn test_wal_bullet_equals_time_to_maturity() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let bond = Bond::fixed(
        "WAL-BULLET",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let market = flat_curve(as_of);
    let w = wal(&bond, &market, as_of);

    // Bullet bond: the only principal flow is at maturity, so WAL equals the
    // ACT/365F time to maturity (1826 days / 365 ≈ 5.0027 years).
    assert!(
        (w - 5.0027).abs() < 0.05,
        "Bullet WAL should equal time to maturity (~5.0), got {w}"
    );
}

#[test]
fn test_wal_decreases_as_valuation_date_advances() {
    let issue = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let bond = Bond::fixed(
        "WAL-ADVANCE",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let later = date!(2028 - 01 - 01);
    let wal_issue = wal(&bond, &flat_curve(issue), issue);
    let wal_later = wal(&bond, &flat_curve(later), later);

    assert!(
        wal_later < wal_issue,
        "WAL should shrink as the valuation date advances: at issue {wal_issue}, later {wal_later}"
    );
    // At 2028 there are ~2 years to the 2030 maturity.
    assert!(
        (wal_later - 2.0).abs() < 0.05,
        "WAL at 2028 should be ~2.0 years to maturity, got {wal_later}"
    );
}
