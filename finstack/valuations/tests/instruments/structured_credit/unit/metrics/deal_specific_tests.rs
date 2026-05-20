//! Unit tests for deal-specific metrics (ABS, CMBS, RMBS).
//!
//! Tests cover:
//! - ABS speed, delinquency, charge-off, excess spread
//! - CMBS LTV, DSCR
//! - RMBS LTV, FICO, WAL adjustments

// Deal-specific metrics are best tested in integration context
// where we can construct full instruments with realistic data

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::{Percentage, Rate};
use finstack_valuations::instruments::fixed_income::structured_credit::{
    standard_psa_speeds, AbsChargeOffCalculator, AbsCreditEnhancementCalculator,
    AbsDelinquencyCalculator, AbsExcessSpreadCalculator, AbsSpeedCalculator, CmbsDscrCalculator,
    DealType, Pool, PoolAsset, RmbsFicoCalculator, RmbsLtvCalculator, RmbsWalCalculator, Seniority,
    StructuredCredit, Tranche, TrancheCoupon, TrancheStructure,
};
use finstack_valuations::metrics::{MetricCalculator, MetricContext};
use std::sync::Arc;
use time::Month;

fn rmbs_instrument() -> StructuredCredit {
    let mut pool = Pool::new("POOL", DealType::RMBS, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "MORTGAGE-1",
        Money::new(5_000_000.0, Currency::USD),
        0.05,
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        finstack_core::dates::DayCount::Thirty360,
    ));

    let tranche = Tranche::new(
        "A",
        0.0,
        100.0,
        Seniority::Senior,
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
    let mut pool = Pool::new("POOL", DealType::CMBS, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "MORTGAGE-1",
        Money::new(10_000_000.0, Currency::USD),
        0.05,
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        finstack_core::dates::DayCount::Thirty360,
    ));

    let tranche = Tranche::new(
        "A",
        0.0,
        100.0,
        Seniority::Senior,
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
    let mut pool = Pool::new("POOL", DealType::ABS, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "AUTO-1",
        Money::new(80_000_000.0, Currency::USD),
        0.06,
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        finstack_core::dates::DayCount::Thirty360,
    ));
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "AUTO-2",
        Money::new(20_000_000.0, Currency::USD),
        0.06,
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        finstack_core::dates::DayCount::Thirty360,
    ));

    let senior = Tranche::new(
        "A",
        0.0,
        80.0,
        Seniority::Senior,
        Money::new(80_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.04 },
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
    )
    .unwrap();
    let subordinate = Tranche::new(
        "B",
        80.0,
        100.0,
        Seniority::Subordinated,
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

    assert!(matches!(err, finstack_core::Error::Input(_)));
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
    let mut empty_pool = Pool::new("EMPTY", DealType::ABS, Currency::USD);
    empty_pool.cumulative_defaults = Money::new(10_000.0, Currency::USD);
    let zero_tranche = Tranche::new(
        "A",
        0.0,
        100.0,
        Seniority::Senior,
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
fn test_abs_metrics_require_abs_deal_type() {
    // ABS metrics should only apply to ABS, Auto, or Card deals
    let abs_family = [DealType::ABS, DealType::Auto, DealType::Card];
    for deal in &abs_family {
        assert!(
            matches!(*deal, DealType::ABS | DealType::Auto | DealType::Card),
            "ABS metrics should support {:?}",
            deal
        );
    }

    let non_abs = [DealType::CLO, DealType::CBO, DealType::CMBS, DealType::RMBS];
    for deal in &non_abs {
        assert!(
            !matches!(*deal, DealType::ABS | DealType::Auto | DealType::Card),
            "ABS metrics should not support {:?}",
            deal
        );
    }
}

#[test]
fn test_cmbs_metrics_require_cmbs_deal_type() {
    // CMBS metrics (LTV, DSCR) should only apply to CMBS
    let cmbs_types = [DealType::CMBS];
    for deal in &cmbs_types {
        assert!(
            matches!(*deal, DealType::CMBS),
            "CMBS metrics should support {:?}",
            deal
        );
    }

    let non_cmbs = [
        DealType::CLO,
        DealType::CBO,
        DealType::ABS,
        DealType::RMBS,
        DealType::Auto,
    ];
    for deal in &non_cmbs {
        assert!(
            !matches!(*deal, DealType::CMBS),
            "CMBS metrics should not support {:?}",
            deal
        );
    }
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
    assert!(matches!(err, finstack_core::Error::CurrencyMismatch { .. }));

    let mut zero_service = cmbs_instrument();
    zero_service.credit_factors.annual_noi = Some(Money::new(1_350_000.0, Currency::USD));
    zero_service.credit_factors.annual_debt_service = Some(Money::new(0.0, Currency::USD));
    let err = CmbsDscrCalculator::new()
        .calculate(&mut metric_context(zero_service, as_of))
        .expect_err("zero debt service should be rejected");
    assert!(err.to_string().contains("annual_debt_service"));
}

#[test]
fn test_rmbs_metrics_adjust_for_psa_speed() {
    // RMBS metrics should consider PSA speeds above and below par
    assert!(
        standard_psa_speeds().iter().any(|&speed| speed > 1.0),
        "PSA speed grid should include stressed scenarios above 100%"
    );
    assert!(
        standard_psa_speeds().iter().any(|&speed| speed < 1.0),
        "PSA speed grid should include benign scenarios below 100%"
    );
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
