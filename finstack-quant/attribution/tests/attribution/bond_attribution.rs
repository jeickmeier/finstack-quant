//! Integration test for bond P&L attribution.
//!
//! Tests attribution across carry, curve shifts, and other factors for a
//! simple fixed-rate bond.

use finstack_quant_attribution::{
    attribute_pnl, attribute_pnl_metrics_based, AttributionMethod, AttributionRequest,
    ExecutionPolicy,
};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::create_date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use std::sync::Arc;
use time::Month;

fn flat_curve(id: &str, base_date: finstack_quant_core::dates::Date, rate: f64) -> DiscountCurve {
    let knots: Vec<(f64, f64)> = (0..=20)
        .map(|i| {
            let t = i as f64 * 0.5;
            (t, (-rate * t).exp())
        })
        .collect();

    DiscountCurve::builder(id)
        .base_date(base_date)
        .knots(knots)
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap()
}

#[test]
fn test_bond_attribution_parallel() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    // Create a 5-year bond
    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "US-BOND-001",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"), // 5% coupon
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();

    // Create discount curve at T₀ (flat 4%)
    let curve_t0 = DiscountCurve::builder("USD-OIS")
        .base_date(as_of_t0)
        .knots(vec![(0.0, 1.0), (5.0, 0.82)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Create discount curve at T₁ (rates increased to 5%)
    let curve_t1 = DiscountCurve::builder("USD-OIS")
        .base_date(as_of_t1)
        .knots(vec![(0.0, 1.0), (5.0, 0.78)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let market_t0 = MarketContext::new().insert(curve_t0);
    let market_t1 = MarketContext::new().insert(curve_t1);

    let config = FinstackConfig::default();

    // Run parallel attribution
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);
    let attribution = attribute_pnl(
        &AttributionMethod::Parallel,
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Parallel,
            ..AttributionRequest::new(
                &bond_instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &config,
            )
        },
    )
    .unwrap();

    // Should have some P&L from curve shift
    assert_ne!(attribution.total_pnl.amount(), 0.0);

    // Currency must be USD
    assert_eq!(attribution.total_pnl.currency(), Currency::USD);

    // Directional assertions for market-standard bond behavior:
    //
    // 1. Carry should be non-negative for a coupon-bearing bond
    //    (bond earns coupon income over time, though may be small for 1-day period)
    assert!(
        attribution.carry.amount() >= -0.01,
        "Carry should be non-negative for coupon bond, got {}",
        attribution.carry.amount()
    );

    // 2. Rates increased (DF at 5Y went from 0.82 to 0.78, implying higher rates)
    //    Bond value should decrease when rates increase → negative rates P&L
    assert!(
        attribution.rates_curves_pnl.amount() < 0.0,
        "Rates P&L should be negative when rates increase (bond value decreases), got {}",
        attribution.rates_curves_pnl.amount()
    );

    // 3. Residual should be small for a simple bond with single-factor attribution
    // For parallel attribution, market-standard tolerance is:
    // - Single factor (rates only): < 1%
    // - Multiple interacting factors: < 5%
    // - Large market moves with convexity: < 10%
    let residual_pct = attribution.meta.residual_pct.abs();
    assert!(
        residual_pct < 5.0,
        "Residual percentage should be < 5% for parallel attribution, got {:.2}%",
        residual_pct
    );
}

#[test]
fn test_bond_attribution_structure() {
    // Test that attribution structure is correctly populated
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "US-BOND-001",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of_t0)
        .knots(vec![(0.0, 1.0), (5.0, 0.82)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let market_t0 = MarketContext::new().insert(curve);
    let market_t1 = market_t0.clone();

    let config = FinstackConfig::default();

    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);
    let attribution = attribute_pnl(
        &AttributionMethod::Parallel,
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Parallel,
            ..AttributionRequest::new(
                &bond_instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &config,
            )
        },
    )
    .unwrap();

    // Check structure
    assert_eq!(attribution.meta.instrument_id, "US-BOND-001");
    assert_eq!(attribution.meta.t0, as_of_t0);
    assert_eq!(attribution.meta.t1, as_of_t1);
    assert!(matches!(
        attribution.meta.method,
        AttributionMethod::Parallel
    ));
}

#[test]
fn test_parallel_bond_attribution_isolates_funding_when_repo_curve_present() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let mut bond = Bond::fixed(
        "US-BOND-FUNDING",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        create_date(2025, Month::January, 15).unwrap(),
        create_date(2030, Month::January, 15).unwrap(),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();
    bond.funding_curve_id = Some(finstack_quant_core::types::CurveId::new("USD-REPO"));

    let market_t0 = MarketContext::new()
        .insert(flat_curve("USD-OIS", as_of_t0, 0.05))
        .insert(flat_curve("USD-REPO", as_of_t0, 0.03));
    let market_t1 = market_t0.clone();

    let instrument: Arc<dyn Instrument> = Arc::new(bond);
    let attribution = attribute_pnl(
        &AttributionMethod::Parallel,
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Parallel,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &FinstackConfig::default(),
            )
        },
    )
    .unwrap();

    let detail = attribution.carry_detail.expect("carry detail");
    let funding = detail.funding_cost.expect("funding overlay").amount();
    assert!(
        funding > 0.0,
        "reprice-path funding_cost must be isolated when a repo curve is present, got {funding}"
    );
    let coupon = detail
        .coupon_income
        .as_ref()
        .map(|l| l.total.amount())
        .unwrap_or(0.0);
    let ptp = detail.pull_to_par.map(|m| m.amount()).unwrap_or(0.0);
    let rd = detail
        .roll_down
        .as_ref()
        .map(|l| l.total.amount())
        .unwrap_or(0.0);
    let total = detail.total.amount();
    // Reprice path: funding is an overlay; price-carry lines partition total.
    assert!(
        (coupon + ptp + rd - total).abs() < 1e-6,
        "price-carry partition: coupon({coupon}) + ptp({ptp}) + rd({rd}) should = total({total})"
    );
}

#[test]
fn test_metrics_based_bond_attribution_populates_carry_decomposition() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let mut bond = Bond::fixed(
        "US-BOND-CARRY",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        create_date(2025, Month::January, 15).unwrap(),
        create_date(2030, Month::January, 15).unwrap(),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();
    // Attach a funding (repo) curve so funding_cost is NON-zero: with
    // funding_cost = 0 both a `− funding` and a `+ funding` partition
    // identity pass, hiding a sign error in the test.
    bond.funding_curve_id = Some(finstack_quant_core::types::CurveId::new("USD-REPO"));

    let market_t0 = MarketContext::new()
        .insert(flat_curve("USD-OIS", as_of_t0, 0.05))
        .insert(flat_curve("USD-REPO", as_of_t0, 0.03));
    let market_t1 = MarketContext::new().insert(flat_curve("USD-OIS", as_of_t1, 0.05));

    let metrics = [
        MetricId::Theta,
        MetricId::CarryTotal,
        MetricId::CouponIncome,
        MetricId::PullToPar,
        MetricId::RollDown,
        MetricId::FundingCost,
    ];
    let val_t0 = bond
        .price_with_metrics(
            &market_t0,
            as_of_t0,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let val_t1 = bond
        .price_with_metrics(
            &market_t1,
            as_of_t1,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let instrument: Arc<dyn Instrument> = Arc::new(bond);
    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .unwrap();

    let detail = attribution.carry_detail.expect("carry detail");
    assert!(detail.coupon_income.is_some());
    assert!(detail.pull_to_par.is_some());
    assert!(detail.roll_down.is_some());
    assert!(detail.funding_cost.is_some());

    let coupon_income = detail.coupon_income.as_ref().unwrap().total.amount();
    let pull_to_par = detail.pull_to_par.unwrap().amount();
    let roll_down = detail.roll_down.as_ref().unwrap().total.amount();
    let funding_cost = detail.funding_cost.unwrap().amount();
    let total = detail.total.amount();

    assert!(
        funding_cost > 0.0,
        "the financing overlay must be exercised"
    );
    assert!(
        (total - (coupon_income + pull_to_par + roll_down)).abs() < 1e-6,
        "gross carry lines must partition gross attribution carry"
    );
    assert!((total - val_t0.measures[MetricId::CarryTotal.as_str()] - funding_cost).abs() < 1e-6);
}

#[test]
fn test_metrics_based_bond_attribution_without_carry_metrics_keeps_legacy_shape() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let bond = Bond::fixed(
        "US-BOND-LEGACY-CARRY",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        create_date(2025, Month::January, 15).unwrap(),
        create_date(2030, Month::January, 15).unwrap(),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();

    let market_t0 = MarketContext::new().insert(flat_curve("USD-OIS", as_of_t0, 0.05));
    let market_t1 = MarketContext::new().insert(flat_curve("USD-OIS", as_of_t1, 0.05));

    let val_t0 = bond
        .price_with_metrics(
            &market_t0,
            as_of_t0,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let val_t1 = bond
        .price_with_metrics(
            &market_t1,
            as_of_t1,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let instrument: Arc<dyn Instrument> = Arc::new(bond);
    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .unwrap();

    let detail = attribution.carry_detail.clone().expect("carry detail");
    assert!(detail.coupon_income.is_none());
    assert!(detail.pull_to_par.is_none());
    // Degenerate (Theta-only) branch now routes carry to roll_down so partition holds.
    assert!(detail.roll_down.is_some());
    assert!(detail.funding_cost.is_none());

    // Partition: roll_down alone should equal total (coupon_income = None, pull_to_par = None).
    let roll_down = detail.roll_down.as_ref().unwrap().total.amount();
    let total = detail.total.amount();
    assert!(
        (roll_down - total).abs() < 1e-6,
        "in the degenerate branch, roll_down ({roll_down}) should equal total ({total})"
    );

    let mut scaled = attribution;
    scaled.scale(0.5);
    let scaled_detail = scaled.carry_detail.expect("scaled carry detail");
    assert!((scaled_detail.total.amount() * 2.0 - detail.total.amount()).abs() < 1e-6);
}
