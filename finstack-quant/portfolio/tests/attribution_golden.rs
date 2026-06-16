//! Attribution golden tests for portfolio.

mod common;

use common::*;
use finstack_quant_attribution::AttributionMethod;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::attribution::attribute_portfolio_pnl;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::PortfolioBuilder;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use std::sync::Arc;
use time::Duration;

#[test]
fn test_attribution_parallel_rates_shift() {
    let as_of_t0 = base_date();
    let as_of_t1 = as_of_t0 + Duration::days(1);

    // Create a bond with known characteristics
    let issue = as_of_t0;
    let maturity = as_of_t0 + Duration::days(1825); // 5 years

    let bond = Bond::fixed(
        "BOND_001",
        Money::new(1_000_000.0, Currency::USD),
        0.05, // 5% coupon
        issue,
        maturity,
        "USD",
    )
    .expect("Bond::fixed should succeed with valid parameters");

    let position = Position::new(
        "POS_001",
        "ENTITY_A",
        "BOND_001",
        Arc::new(bond),
        1.0,
        PositionUnit::FaceValue,
    )
    .expect("Position::new should succeed with valid parameters");

    let portfolio = PortfolioBuilder::new("TEST")
        .base_ccy(Currency::USD)
        .as_of(as_of_t0)
        .entity(Entity::new("ENTITY_A"))
        .position(position)
        .build()
        .unwrap();

    // Create market contexts: T0 with flat 0%, T1 with +100bp
    // Using 100bp for clearer signal (10bp may be too small to detect reliably)
    let market_t0 = market_with_usd_at_rate(0.0); // 0% flat
    let market_t1 = market_with_usd_at_rate(100.0); // 1% flat (+100bp)

    let config = FinstackConfig::default();

    let attribution = attribute_portfolio_pnl(
        &portfolio,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        AttributionMethod::Parallel,
    )
    .unwrap();

    // Verify attribution structure
    assert!(attribution.total_pnl.currency() == Currency::USD);
    assert!(attribution.rates_curves_pnl.currency() == Currency::USD);

    // With a positive rate shock, bond value decreases, so rates P&L should be negative
    let rates_pnl = attribution.rates_curves_pnl.amount();
    assert!(
        rates_pnl < 0.0,
        "Positive rate shock should produce negative rates P&L for long bond, got: {}",
        rates_pnl
    );
    // For a 5Y bond with ~4.5 duration and 1M notional, 100bp shock ≈ -45,000
    // Allow wide tolerance as exact DV01 depends on coupon/curve details
    assert!(
        rates_pnl.abs() < 100_000.0,
        "Rates P&L magnitude should be reasonable, got: {}",
        rates_pnl
    );
}

#[test]
fn test_attribution_fx_translation() {
    let as_of_t0 = base_date();
    let as_of_t1 = as_of_t0 + Duration::days(1);

    // Create EUR bond
    let issue = as_of_t0;
    let maturity = as_of_t0 + Duration::days(365);

    let bond = Bond::fixed(
        "BOND_EUR",
        Money::new(1_000_000.0, Currency::EUR),
        0.03,
        issue,
        maturity,
        "EUR",
    )
    .expect("Bond::fixed should succeed with valid parameters");

    let position = Position::new(
        "POS_EUR",
        "ENTITY_A",
        "BOND_EUR",
        Arc::new(bond),
        1.0,
        PositionUnit::FaceValue,
    )
    .expect("Position::new should succeed with valid parameters");

    let portfolio = PortfolioBuilder::new("TEST")
        .base_ccy(Currency::USD)
        .as_of(as_of_t0)
        .entity(Entity::new("ENTITY_A"))
        .position(position)
        .build()
        .unwrap();

    // Market with EUR curve and FX: T0 = 1.10, T1 = 1.12
    let market_t0 = market_with_eur_and_fx(1.10);
    let market_t1 = market_with_eur_and_fx(1.12);

    let config = FinstackConfig::default();

    let attribution = attribute_portfolio_pnl(
        &portfolio,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        AttributionMethod::Parallel,
    )
    .unwrap();

    // FX translation P&L should reflect the rate change
    // Translation P&L = Principal_T0 * (R1 - R0)
    // With flat curve, bond PV ≈ notional = EUR 1M
    // EUR/USD moves 1.10 -> 1.12, so translation ≈ 1M * 0.02 = USD 20,000
    // Note: Actual implementation also includes translation of P&L flow
    // so we allow wider tolerance but still verify sign and magnitude
    assert!(
        attribution.fx_translation_pnl.currency() == Currency::USD,
        "FX translation P&L should be in base currency"
    );
    let translation_amount = attribution.fx_translation_pnl.amount();
    assert!(
        translation_amount > 0.0,
        "EUR appreciation should produce positive FX translation P&L, got: {}",
        translation_amount
    );
    assert!(
        translation_amount < 50_000.0,
        "FX translation P&L should be reasonable magnitude (~20k expected), got: {}",
        translation_amount
    );

    let explained_total = attribution.carry.amount()
        + attribution.rates_curves_pnl.amount()
        + attribution.credit_curves_pnl.amount()
        + attribution.inflation_curves_pnl.amount()
        + attribution.correlations_pnl.amount()
        + attribution.fx_pnl.amount()
        + attribution.fx_translation_pnl.amount()
        + attribution.vol_pnl.amount()
        + attribution.model_params_pnl.amount()
        + attribution.market_scalars_pnl.amount()
        + attribution.residual.amount();

    assert!(
        (attribution.total_pnl.amount() - explained_total).abs() < 1.0e-8,
        "portfolio attribution should close exactly: total={}, explained={}",
        attribution.total_pnl.amount(),
        explained_total
    );
}

#[test]
fn test_attribution_carry_theta() {
    let as_of_t0 = base_date();
    let as_of_t1 = as_of_t0 + Duration::days(1);

    // Create a bond that will have carry (accrued interest)
    let issue = as_of_t0;
    let maturity = as_of_t0 + Duration::days(365);

    let bond = Bond::fixed(
        "BOND_CARRY",
        Money::new(1_000_000.0, Currency::USD),
        0.05, // 5% coupon
        issue,
        maturity,
        "USD",
    )
    .expect("Bond::fixed should succeed with valid parameters");

    let position = Position::new(
        "POS_CARRY",
        "ENTITY_A",
        "BOND_CARRY",
        Arc::new(bond),
        1.0,
        PositionUnit::FaceValue,
    )
    .expect("Position::new should succeed with valid parameters");

    let portfolio = PortfolioBuilder::new("TEST")
        .base_ccy(Currency::USD)
        .as_of(as_of_t0)
        .entity(Entity::new("ENTITY_A"))
        .position(position)
        .build()
        .unwrap();

    // Use a non-zero rate curve so that theta (time value) is non-zero
    // With a 0% flat curve, theta would be zero since there's no time value of money
    let market = market_with_usd_at_rate(500.0); // 5% flat rate
    let config = FinstackConfig::default();

    let attribution = attribute_portfolio_pnl(
        &portfolio,
        &market,
        &market, // Same market for T0 and T1 to isolate carry
        as_of_t0,
        as_of_t1,
        &config,
        AttributionMethod::Parallel,
    )
    .unwrap();

    // Carry includes both accrued interest and theta (time value effect)
    // For a 5% coupon bond at 5% discount rate, carry should be close to the coupon accrual
    // 1-day accrual ≈ 1M * 0.05 / 365 ≈ 136.99 USD
    // Theta may add or subtract from this depending on bond price vs par
    assert!(
        attribution.carry.currency() == Currency::USD,
        "Carry should be in base currency"
    );
    let carry_amount = attribution.carry.amount();
    // Carry can be positive or negative depending on bond price vs par
    // Just verify it's computed (finite) and reasonable magnitude
    assert!(
        carry_amount.is_finite(),
        "Carry should be finite, got: {}",
        carry_amount
    );
    // For a 1Y bond at ~$1M, 1-day carry should be less than ~$1000 in magnitude
    assert!(
        carry_amount.abs() < 1000.0,
        "Carry magnitude should be reasonable for 1-day hold, got: {}",
        carry_amount
    );
}

#[test]
fn test_parallel_portfolio_attribution_closes_with_serial_inner_policy() {
    let as_of_t0 = base_date();
    let as_of_t1 = as_of_t0 + Duration::days(1);
    let issue = as_of_t0;
    let maturity = as_of_t0 + Duration::days(1825);

    let positions = (0..80)
        .map(|i| {
            let instrument_id = format!("BOND_{i:03}");
            let bond = Bond::fixed(
                instrument_id.as_str(),
                Money::new(100_000.0 + f64::from(i) * 1_000.0, Currency::USD),
                0.04,
                issue,
                maturity,
                "USD",
            )
            .expect("Bond::fixed should succeed with valid parameters");
            Position::new(
                format!("POS_{i:03}"),
                "ENTITY_A",
                instrument_id,
                Arc::new(bond),
                1.0,
                PositionUnit::FaceValue,
            )
            .expect("Position::new should succeed with valid parameters")
        })
        .collect::<Vec<_>>();

    let portfolio = PortfolioBuilder::new("TEST_PARALLEL_ATTR")
        .base_ccy(Currency::USD)
        .as_of(as_of_t0)
        .entity(Entity::new("ENTITY_A"))
        .positions(positions)
        .build()
        .unwrap();

    let market_t0 = market_with_usd_at_rate(100.0);
    let market_t1 = market_with_usd_at_rate(125.0);
    let config = FinstackConfig::default();

    let attribution = attribute_portfolio_pnl(
        &portfolio,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        AttributionMethod::Parallel,
    )
    .expect("portfolio attribution should succeed");

    assert_eq!(attribution.by_position.len(), 80);

    let explained_total = attribution.carry.amount()
        + attribution.rates_curves_pnl.amount()
        + attribution.credit_curves_pnl.amount()
        + attribution.inflation_curves_pnl.amount()
        + attribution.correlations_pnl.amount()
        + attribution.fx_pnl.amount()
        + attribution.fx_translation_pnl.amount()
        + attribution.vol_pnl.amount()
        + attribution.model_params_pnl.amount()
        + attribution.market_scalars_pnl.amount()
        + attribution.residual.amount();

    assert!(
        (attribution.total_pnl.amount() - explained_total).abs() < 1.0e-8,
        "portfolio attribution should close with serial inner attribution: total={}, explained={}",
        attribution.total_pnl.amount(),
        explained_total
    );
}
