//! Curve dependency completeness tests.
//!
//! These tests verify that instruments correctly declare all their curve
//! dependencies. When an instrument's canonical dependency declaration returns
//! a set of curves, pricing should succeed with only those curves in the
//! market context.
//!
//! This helps prevent silent failures where an instrument accesses curves
//! that weren't declared as dependencies.

use crate::finstack_quant_test_utils as test_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DateExt, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::variance_swap::VarianceSwap;
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CashflowSpec};
use finstack_quant_valuations::instruments::{Instrument, InstrumentJson, MarketDependencies};
use time::macros::date;

/// Build a discount curve with the given ID and flat rate.
fn build_discount_curve(id: &str, rate: f64) -> DiscountCurve {
    let as_of = date!(2025 - 01 - 01);
    DiscountCurve::builder(id)
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, (-rate).exp()),
            (5.0f64, (-rate * 5.0).exp()),
            (10.0f64, (-rate * 10.0).exp()),
        ])
        .build()
        .expect("Discount curve construction should succeed")
}

/// Build a hazard curve with the given ID and flat hazard rate.
fn build_hazard_curve(id: &str, hazard_rate: f64) -> HazardCurve {
    let as_of = date!(2025 - 01 - 01);
    HazardCurve::builder(id)
        .base_date(as_of)
        .knots(vec![
            (0.0, 1.0),
            (1.0, (-hazard_rate).exp()),
            (5.0, (-hazard_rate * 5.0).exp()),
            (10.0, (-hazard_rate * 10.0).exp()),
        ])
        .build()
        .expect("Hazard curve construction should succeed")
}

/// Create a market context with only the specified curves.
fn build_minimal_market(disc_ids: &[&str], hazard_ids: &[&str]) -> MarketContext {
    let mut market = MarketContext::new();

    for &id in disc_ids {
        market = market.insert(build_discount_curve(id, 0.04));
    }

    for &id in hazard_ids {
        market = market.insert(build_hazard_curve(id, 0.02));
    }

    market
}

/// Test that a Bond can be priced with only its declared dependencies.
#[test]
fn test_bond_curve_dependencies_complete() {
    let as_of = date!(2025 - 01 - 01);
    let issue = as_of;
    let maturity = as_of.add_months(60);

    let bond = Bond::builder()
        .id("BOND-DEPS-TEST".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.04, Tenor::semi_annual(), DayCount::Thirty360)
                .expect("finite test coupon"),
        )
        .discount_curve_id("USD-OIS".into())
        .build()
        .expect("Bond construction should succeed");

    // Get declared dependencies
    let deps = bond
        .market_dependencies()
        .expect("market_dependencies")
        .curves;

    // Extract discount curve IDs
    let disc_ids: Vec<&str> = deps.discount_curves.iter().map(|id| id.as_str()).collect();

    // Build market with only declared dependencies
    let market = build_minimal_market(&disc_ids, &[]);

    // Pricing should succeed
    let result = bond.value(&market, as_of);
    assert!(
        result.is_ok(),
        "Bond pricing with minimal market should succeed, got: {:?}",
        result.err()
    );
}

/// Test that a CDS can be priced with only its declared dependencies.
#[test]
fn test_cds_curve_dependencies_complete() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = as_of.add_months(60);

    let cds = test_utils::cds_buy_protection(
        "CDS-DEPS-TEST",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        maturity,
        "USD-OIS",
        "TEST-CREDIT",
    )
    .expect("CDS construction should succeed");

    // Get declared dependencies
    let deps = cds
        .market_dependencies()
        .expect("market_dependencies")
        .curves;

    // Extract curve IDs
    let disc_ids: Vec<&str> = deps.discount_curves.iter().map(|id| id.as_str()).collect();
    let hazard_ids: Vec<&str> = deps.credit_curves.iter().map(|id| id.as_str()).collect();

    // Build market with only declared dependencies
    let market = build_minimal_market(&disc_ids, &hazard_ids);

    // Pricing should succeed
    let result = cds.value(&market, as_of);
    assert!(
        result.is_ok(),
        "CDS pricing with minimal market should succeed, got: {:?}",
        result.err()
    );
}

/// Test that missing declared dependencies cause pricing to fail.
///
/// This verifies that our dependency declaration is actually being used
/// and not just bypassed by some fallback mechanism.
#[test]
fn test_missing_dependency_fails() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = as_of.add_months(60);

    let cds = test_utils::cds_buy_protection(
        "CDS-MISSING-TEST",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        maturity,
        "USD-OIS",
        "TEST-CREDIT",
    )
    .expect("CDS construction should succeed");

    // Build market with missing hazard curve
    let market = build_minimal_market(&["USD-OIS"], &[]);

    // Pricing should fail due to missing hazard curve
    let result = cds.value(&market, as_of);
    assert!(
        result.is_err(),
        "CDS pricing should fail when hazard curve is missing"
    );
}

/// Test dependency declaration consistency across instruments.
///
/// Verifies that the number of declared dependencies is reasonable
/// and that instruments don't over-declare (which would be inefficient
/// for risk calculations).
#[test]
fn test_dependency_count_reasonable() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = as_of.add_months(60);

    // A simple fixed-rate bond should only need 1 discount curve
    let bond = Bond::builder()
        .id("BOND-COUNT-TEST".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.04, Tenor::semi_annual(), DayCount::Thirty360)
                .expect("finite test coupon"),
        )
        .discount_curve_id("USD-OIS".into())
        .build()
        .expect("Bond construction should succeed");

    let bond_deps = bond
        .market_dependencies()
        .expect("market_dependencies")
        .curves;
    assert_eq!(
        bond_deps.discount_curves.len(),
        1,
        "Fixed-rate bond should declare exactly 1 discount curve"
    );
    assert_eq!(
        bond_deps.credit_curves.len(),
        0,
        "Fixed-rate bond should not declare credit curves"
    );

    // A CDS should need 1 discount + 1 credit curve
    let cds = test_utils::cds_buy_protection(
        "CDS-COUNT-TEST",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        maturity,
        "USD-OIS",
        "TEST-CREDIT",
    )
    .expect("CDS construction should succeed");

    let cds_deps = cds
        .market_dependencies()
        .expect("market_dependencies")
        .curves;
    assert_eq!(
        cds_deps.discount_curves.len(),
        1,
        "CDS should declare exactly 1 discount curve"
    );
    assert_eq!(
        cds_deps.credit_curves.len(),
        1,
        "CDS should declare exactly 1 credit curve"
    );
}

#[test]
fn test_variance_swap_direct_dependencies_match_json_dependencies() {
    let swap = VarianceSwap::example().expect("variance swap example");

    let json_deps =
        MarketDependencies::from_instrument_json(&InstrumentJson::VarianceSwap(swap.clone()))
            .expect("from_instrument_json");
    let direct_deps = swap
        .market_dependencies()
        .expect("direct market_dependencies");

    assert_eq!(
        direct_deps.curves.discount_curves, json_deps.curves.discount_curves,
        "direct dependencies should declare the same discount curves as JSON dependencies"
    );
    assert_eq!(
        direct_deps.series_ids, json_deps.series_ids,
        "direct dependencies should declare the realized variance series IDs"
    );
}
