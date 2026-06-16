//! Unit tests for pool characteristic metrics.
//!
//! Tests cover:
//! - WAC/WAS calculations
//! - Default rate aggregation
//! - Basic pool stats output consistency

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CreditRating;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_pool_stats, AssetPool, CdrCalculator, CloWarfCalculator, CprCalculator, DealType,
    PoolAsset, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use finstack_quant_valuations::metrics::{MetricCalculator, MetricContext};
use std::sync::Arc;
use time::Month;

fn maturity_date() -> finstack_quant_core::dates::Date {
    finstack_quant_core::dates::Date::from_calendar_date(2030, Month::December, 31).unwrap()
}

fn as_of() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn tranche_structure() -> TrancheStructure {
    let tranche = Tranche::new(
        "A",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(8_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.04 },
        maturity_date(),
    )
    .unwrap();
    TrancheStructure::new(vec![tranche]).unwrap()
}

fn metric_context(instrument: StructuredCredit) -> MetricContext {
    MetricContext::new(
        Arc::new(instrument),
        Arc::new(MarketContext::new()),
        as_of(),
        Money::new(0.0, Currency::USD),
        MetricContext::default_config(),
    )
}

#[test]
fn test_pool_stats_weighted_spread_and_coupon() {
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(10_000_000.0, Currency::USD),
        "SOFR-3M",
        400.0,
        maturity_date(),
        DayCount::Act360,
    ));
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "B1",
        Money::new(20_000_000.0, Currency::USD),
        0.06,
        maturity_date(),
        DayCount::Thirty360,
    ));

    let stats = calculate_pool_stats(&pool, maturity_date());

    // Fixed-rate B1 has no explicit spread component, so it is excluded
    // from WAS (no rate × 10⁴ fallback): only L1's 400bps contributes.
    let expected_was = 400.0;
    assert!(
        (stats.weighted_avg_spread - expected_was).abs() < 1e-6,
        "Weighted avg spread should match balance-weighted spreads over \
         assets that carry a spread"
    );
    assert!(
        stats.weighted_avg_coupon > 0.0,
        "Weighted avg coupon should be positive for fixed/floating assets"
    );
}

#[test]
fn test_pool_stats_default_rate() {
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    let mut defaulted = PoolAsset::fixed_rate_bond(
        "D1",
        Money::new(5_000_000.0, Currency::USD),
        0.05,
        maturity_date(),
        DayCount::Thirty360,
    );
    defaulted.is_defaulted = true;

    pool.assets.push(defaulted);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "D2",
        Money::new(10_000_000.0, Currency::USD),
        0.05,
        maturity_date(),
        DayCount::Thirty360,
    ));

    let stats = calculate_pool_stats(&pool, maturity_date());
    assert!(
        (stats.cumulative_default_rate - 33.333333).abs() < 1e-3,
        "Default rate should reflect defaulted balance share"
    );
}

#[test]
fn test_clo_warf_calculator_matches_weighted_average_factors() {
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(
        PoolAsset::fixed_rate_bond(
            "B1",
            Money::new(5_000_000.0, Currency::USD),
            0.06,
            maturity_date(),
            DayCount::Thirty360,
        )
        .with_rating(CreditRating::BB),
    );
    pool.assets.push(
        PoolAsset::fixed_rate_bond(
            "B2",
            Money::new(3_000_000.0, Currency::USD),
            0.08,
            maturity_date(),
            DayCount::Thirty360,
        )
        .with_rating(CreditRating::B),
    );

    let instrument = StructuredCredit::new_clo(
        "TEST_CLO_WARF",
        pool,
        tranche_structure(),
        as_of(),
        maturity_date(),
        "USD-OIS",
    );
    let mut context = metric_context(instrument);

    let warf = CloWarfCalculator.calculate(&mut context).unwrap();
    let expected = (5_000_000.0 * 1350.0 + 3_000_000.0 * 2720.0) / 8_000_000.0;

    assert!(
        (warf - expected).abs() < 1e-10,
        "Expected WARF {expected}, got {warf}"
    );
}

#[test]
fn test_clo_warf_calculator_uses_default_factor_for_missing_ratings() {
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "UNRATED",
        Money::new(4_000_000.0, Currency::USD),
        0.07,
        maturity_date(),
        DayCount::Thirty360,
    ));

    let instrument = StructuredCredit::new_clo(
        "TEST_CLO_UNRATED",
        pool,
        tranche_structure(),
        as_of(),
        maturity_date(),
        "USD-OIS",
    );
    let mut context = metric_context(instrument);

    let warf = CloWarfCalculator.calculate(&mut context).unwrap();
    assert!(
        (warf - 3650.0).abs() < 1e-10,
        "Missing ratings should fall back to 3650, got {warf}"
    );
}

#[test]
fn test_rmbs_cpr_and_cdr_use_current_deal_seasoning() {
    let pool = AssetPool::new("POOL", DealType::RMBS, Currency::USD);
    let mut instrument = StructuredCredit::new_rmbs(
        "TEST_RMBS_SPEEDS",
        pool,
        tranche_structure(),
        Date::from_calendar_date(2025, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );
    instrument.behavior_overrides.psa_speed_multiplier = Some(1.0);
    instrument.behavior_overrides.sda_speed_multiplier = Some(1.0);

    let mut context = MetricContext::new(
        Arc::new(instrument),
        Arc::new(MarketContext::new()),
        Date::from_calendar_date(2025, Month::July, 1).unwrap(),
        Money::new(0.0, Currency::USD),
        MetricContext::default_config(),
    );

    let cpr = CprCalculator.calculate(&mut context).unwrap();
    let cdr = CdrCalculator.calculate(&mut context).unwrap();

    assert!(
        (cpr - 0.012).abs() < 1e-12,
        "six-month 100 PSA CPR should be 1.2%, got {cpr}"
    );
    assert!(
        (cdr - 0.0012).abs() < 1e-12,
        "six-month 100 SDA CDR should be 0.12%, got {cdr}"
    );
}
