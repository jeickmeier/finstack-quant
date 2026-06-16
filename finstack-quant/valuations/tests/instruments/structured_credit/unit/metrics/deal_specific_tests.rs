//! Unit tests for deal-specific metrics (ABS, CMBS, RMBS).
//!
//! Tests cover:
//! - ABS speed, delinquency, charge-off, excess spread
//! - CMBS LTV, DSCR
//! - RMBS LTV, FICO, WAL adjustments

// Deal-specific metrics are best tested in integration context
// where we can construct full instruments with realistic data

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Percentage, Rate};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AbsChargeOffCalculator, AbsCreditEnhancementCalculator, AbsDelinquencyCalculator,
    AbsExcessSpreadCalculator, AbsSpeedCalculator, AssetPool, CmbsDscrCalculator, DealType,
    PoolAsset, RmbsFicoCalculator, RmbsLtvCalculator, RmbsWalCalculator, StructuredCredit, Tranche,
    TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use finstack_quant_valuations::metrics::{MetricCalculator, MetricContext};
use std::sync::Arc;
use time::Month;

fn rmbs_instrument() -> StructuredCredit {
    let mut pool = AssetPool::new("POOL", DealType::RMBS, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "MORTGAGE-1",
        Money::new(5_000_000.0, Currency::USD),
        0.05,
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        finstack_quant_core::dates::DayCount::Thirty360,
    ));

    let tranche = Tranche::new(
        "A",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(5_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.04 },
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
    )
    .unwrap();
    let tranches = TrancheStructure::new(vec![tranche]).unwrap();

    StructuredCredit::new_rmbs(
        "RMBS-TEST",
        pool,
        tranches,
        Date::from_calendar_date(2025, Month::January, 1).unwrap(),
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse")
}

fn cmbs_instrument() -> StructuredCredit {
    let mut pool = AssetPool::new("POOL", DealType::CMBS, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "MORTGAGE-1",
        Money::new(10_000_000.0, Currency::USD),
        0.05,
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        finstack_quant_core::dates::DayCount::Thirty360,
    ));

    let tranche = Tranche::new(
        "A",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(10_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.04 },
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
    )
    .unwrap();
    let tranches = TrancheStructure::new(vec![tranche]).unwrap();

    StructuredCredit::new_cmbs(
        "CMBS-TEST",
        pool,
        tranches,
        Date::from_calendar_date(2025, Month::January, 1).unwrap(),
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        "USD-OIS",
    )
}

fn abs_instrument() -> StructuredCredit {
    let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "AUTO-1",
        Money::new(80_000_000.0, Currency::USD),
        0.06,
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        finstack_quant_core::dates::DayCount::Thirty360,
    ));
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "AUTO-2",
        Money::new(20_000_000.0, Currency::USD),
        0.06,
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        finstack_quant_core::dates::DayCount::Thirty360,
    ));

    let senior = Tranche::new(
        "A",
        0.0,
        80.0,
        TrancheSeniority::Senior,
        Money::new(80_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.04 },
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
    )
    .unwrap();
    let subordinate = Tranche::new(
        "B",
        80.0,
        100.0,
        TrancheSeniority::Subordinated,
        Money::new(20_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.08 },
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
    )
    .unwrap();
    let tranches = TrancheStructure::new(vec![senior, subordinate]).unwrap();

    StructuredCredit::new_abs(
        "ABS-TEST",
        pool,
        tranches,
        Date::from_calendar_date(2025, Month::January, 1).unwrap(),
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        "USD-OIS",
    )
}

fn metric_context(instrument: StructuredCredit, as_of: Date) -> MetricContext {
    MetricContext::new(
        Arc::new(instrument),
        Arc::new(MarketContext::new()),
        as_of,
        Money::new(0.0, Currency::USD),
        MetricContext::default_config(),
    )
}

#[test]
fn test_abs_speed_calculator_uses_override_or_default() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let calc = AbsSpeedCalculator::new_pct(Percentage::new(1.8));

    let default_speed = calc
        .calculate(&mut metric_context(abs_instrument(), as_of))
        .unwrap();
    assert_eq!(default_speed, 1.8);

    let mut overridden = abs_instrument();
    overridden.behavior_overrides.abs_speed = Some(0.0275);
    let overridden_speed = calc
        .calculate(&mut metric_context(overridden, as_of))
        .unwrap();
    assert_eq!(overridden_speed, 0.0275);
}

#[test]
fn test_abs_speed_calculator_rejects_non_abs_deals() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let err = AbsSpeedCalculator::new(0.02)
        .calculate(&mut metric_context(cmbs_instrument(), as_of))
        .expect_err("non-ABS deals should be rejected");

    assert!(matches!(err, finstack_quant_core::Error::Input(_)));
}

#[test]
fn test_abs_deal_specific_calculators_return_expected_values() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut abs = abs_instrument();
    abs.pool.cumulative_defaults = Money::new(5_000_000.0, Currency::USD);

    let delinquency = AbsDelinquencyCalculator::new_pct(Percentage::new(3.5))
        .calculate(&mut metric_context(abs.clone(), as_of))
        .unwrap();
    let charge_off = AbsChargeOffCalculator
        .calculate(&mut metric_context(abs.clone(), as_of))
        .unwrap();
    let excess_spread = AbsExcessSpreadCalculator::new_rate(Rate::from_decimal(0.005))
        .calculate(&mut metric_context(abs.clone(), as_of))
        .unwrap();
    let credit_enhancement = AbsCreditEnhancementCalculator
        .calculate(&mut metric_context(abs, as_of))
        .unwrap();

    assert!((delinquency - 3.5).abs() < 1e-12);
    assert_eq!(charge_off, 5.0);
    assert!((excess_spread - 0.7).abs() < 1e-12);
    assert!((credit_enhancement - 20.0).abs() < 1e-12);
}

#[test]
fn test_abs_charge_off_and_credit_enhancement_handle_zero_balances() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut empty_pool = AssetPool::new("EMPTY", DealType::ABS, Currency::USD);
    empty_pool.cumulative_defaults = Money::new(10_000.0, Currency::USD);
    let zero_tranche = Tranche::new(
        "A",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(0.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.04 },
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
    )
    .unwrap();
    let empty_abs = StructuredCredit::new_abs(
        "ABS-EMPTY",
        empty_pool,
        TrancheStructure::new(vec![zero_tranche]).unwrap(),
        as_of,
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        "USD-OIS",
    );

    let charge_off = AbsChargeOffCalculator
        .calculate(&mut metric_context(empty_abs.clone(), as_of))
        .unwrap();
    let credit_enhancement = AbsCreditEnhancementCalculator
        .calculate(&mut metric_context(empty_abs, as_of))
        .unwrap();

    assert_eq!(charge_off, 0.0);
    assert_eq!(credit_enhancement, 0.0);
}

#[test]
fn test_cmbs_dscr_calculator_uses_typed_noi_and_debt_service() {
    let mut cmbs = cmbs_instrument();
    cmbs.credit_factors.annual_noi = Some(Money::new(1_350_000.0, Currency::USD));
    cmbs.credit_factors.annual_debt_service = Some(Money::new(1_000_000.0, Currency::USD));

    let dscr = CmbsDscrCalculator::new()
        .calculate(&mut metric_context(
            cmbs,
            Date::from_calendar_date(2025, Month::January, 1).unwrap(),
        ))
        .unwrap();

    assert!((dscr - 1.35).abs() < 1e-12);
}

#[test]
fn test_cmbs_dscr_requires_typed_inputs_and_matching_currency() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let missing = CmbsDscrCalculator::new()
        .calculate(&mut metric_context(cmbs_instrument(), as_of))
        .expect_err("missing typed inputs should be rejected");
    assert!(missing.to_string().contains("annual_noi"));

    let mut mismatch = cmbs_instrument();
    mismatch.credit_factors.annual_noi = Some(Money::new(1_350_000.0, Currency::USD));
    mismatch.credit_factors.annual_debt_service = Some(Money::new(1_000_000.0, Currency::EUR));
    let err = CmbsDscrCalculator::new()
        .calculate(&mut metric_context(mismatch, as_of))
        .expect_err("currency mismatch should be rejected");
    assert!(matches!(
        err,
        finstack_quant_core::Error::CurrencyMismatch { .. }
    ));

    let mut zero_service = cmbs_instrument();
    zero_service.credit_factors.annual_noi = Some(Money::new(1_350_000.0, Currency::USD));
    zero_service.credit_factors.annual_debt_service = Some(Money::new(0.0, Currency::USD));
    let err = CmbsDscrCalculator::new()
        .calculate(&mut metric_context(zero_service, as_of))
        .expect_err("zero debt service should be rejected");
    assert!(err.to_string().contains("annual_debt_service"));
}

#[test]
fn test_rmbs_wal_adjusts_with_psa_speed() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let mut base = rmbs_instrument();
    base.behavior_overrides.psa_speed_multiplier = Some(1.0);
    let wal_base = RmbsWalCalculator
        .calculate(&mut metric_context(base, as_of))
        .unwrap();

    let mut fast = rmbs_instrument();
    fast.behavior_overrides.psa_speed_multiplier = Some(2.0);
    let wal_fast = RmbsWalCalculator
        .calculate(&mut metric_context(fast, as_of))
        .unwrap();

    assert!(wal_fast < wal_base, "Higher PSA speeds should shorten WAL");
}

#[test]
fn test_rmbs_ltv_uses_credit_factors_when_present() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let rmbs = rmbs_instrument();
    let ltv = RmbsLtvCalculator::new(75.0)
        .calculate(&mut metric_context(rmbs, as_of))
        .unwrap();

    assert_eq!(ltv, 80.0, "RMBS default LTV should be 80% from deal config");
}

#[test]
fn test_rmbs_fico_defaults_when_missing() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let rmbs = rmbs_instrument();
    let fico = RmbsFicoCalculator::new(700.0)
        .calculate(&mut metric_context(rmbs, as_of))
        .unwrap();

    assert_eq!(fico, 700.0, "RMBS FICO should use default when missing");
}
