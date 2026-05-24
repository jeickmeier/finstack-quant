//! Tests for structured credit constructors and behavioral overrides.

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, Tenor};
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::structured_credit::{
    abs_auto_standard_cdr, clo_standard_cdr, cmbs_standard_cdr, psa_ramp_months, psa_terminal_cpr,
    rmbs_standard_cdr, sda_peak_cdr, sda_peak_month, sda_terminal_cdr, AssetPool, DealType,
    PoolAsset, StructuredCredit, Tranche, TrancheCoupon, TrancheStructure,
};
use finstack_valuations::instruments::fixed_income::structured_credit::{cdr_to_mdr, cpr_to_smm};
use time::Month;

const DECIMAL_TO_PERCENT: f64 = 100.0;

fn test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn maturity_date() -> Date {
    Date::from_calendar_date(2030, Month::December, 31).unwrap()
}

fn create_pool_with_balance(balance: f64) -> AssetPool {
    let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
    if balance > 0.0 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::new(balance, Currency::USD),
            0.06,
            Date::from_calendar_date(2029, Month::January, 1).unwrap(),
            finstack_core::dates::DayCount::Thirty360,
        ));
    }
    pool
}

fn create_single_tranche() -> TrancheStructure {
    let tranche = Tranche::new(
        "SENIOR",
        0.0,
        100.0,
        finstack_valuations::instruments::fixed_income::structured_credit::TrancheSeniority::Senior,
        Money::new(1_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
    )
    .unwrap();
    TrancheStructure::new(vec![tranche]).unwrap()
}

#[test]
fn test_apply_deal_defaults_sets_expected_assumptions() {
    let pool = create_pool_with_balance(1_000_000.0);
    let tranches = create_single_tranche();
    let closing = Date::from_calendar_date(2024, Month::January, 1).unwrap();
    let legal = maturity_date();

    let cases = [
        (DealType::ABS, Tenor::monthly(), abs_auto_standard_cdr()),
        (DealType::CLO, Tenor::quarterly(), clo_standard_cdr()),
        (DealType::CMBS, Tenor::monthly(), cmbs_standard_cdr()),
        (DealType::RMBS, Tenor::monthly(), rmbs_standard_cdr()),
    ];

    for (deal_type, expected_frequency, expected_cdr) in cases {
        let sc = StructuredCredit::apply_deal_defaults(
            format!("TEST-{deal_type:?}"),
            deal_type,
            pool.clone(),
            tranches.clone(),
            closing,
            legal,
            "USD-OIS",
        );

        assert_eq!(sc.deal_type, deal_type);
        assert_eq!(sc.frequency, expected_frequency);
        assert!((sc.default_assumptions.base_cdr_annual - expected_cdr).abs() < 1e-12);
    }
}

#[test]
fn test_example_has_expected_defaults() {
    let sc = StructuredCredit::example();
    let waterfall = sc.create_waterfall();

    assert_eq!(sc.tranches.tranches.len(), 1);
    assert_eq!(waterfall.tiers.len(), 3);
    assert_eq!(sc.payment_calendar_id.as_deref(), Some("nyse"));
}

#[test]
fn test_prepayment_overrides_use_expected_priority() {
    let pool = create_pool_with_balance(1_000_000.0);
    let tranches = create_single_tranche();
    let mut sc = StructuredCredit::new_abs(
        "TEST-ABS",
        pool,
        tranches,
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    sc.behavior_overrides.abs_speed = Some(0.02);
    let abs_rate = sc.calculate_prepayment_rate(test_date(), 1).unwrap();
    assert!((abs_rate - 0.02).abs() < 1e-12);

    sc.behavior_overrides.abs_speed = None;
    sc.behavior_overrides.cpr_annual = Some(0.12);
    let cpr_rate = sc.calculate_prepayment_rate(test_date(), 1).unwrap();
    assert!((cpr_rate - cpr_to_smm(0.12)).abs() < 1e-12);

    sc.behavior_overrides.cpr_annual = None;
    sc.behavior_overrides.psa_speed_multiplier = Some(2.0);
    let seasoning = 3;
    let base_cpr = (seasoning as f64 / psa_ramp_months() as f64) * psa_terminal_cpr();
    let expected = cpr_to_smm(base_cpr * 2.0);
    let psa_rate = sc
        .calculate_prepayment_rate(test_date(), seasoning)
        .unwrap();
    assert!((psa_rate - expected).abs() < 1e-12);
}

#[test]
fn test_default_overrides_use_expected_priority() {
    let pool = create_pool_with_balance(1_000_000.0);
    let tranches = create_single_tranche();
    let mut sc = StructuredCredit::new_abs(
        "TEST-ABS",
        pool,
        tranches,
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    sc.behavior_overrides.cdr_annual = Some(0.12);
    let cdr_rate = sc.calculate_default_rate(test_date(), 1).unwrap();
    assert!((cdr_rate - cdr_to_mdr(0.12)).abs() < 1e-12);

    sc.behavior_overrides.cdr_annual = None;
    sc.behavior_overrides.sda_speed_multiplier = Some(1.5);
    let seasoning = sda_peak_month() + 1;
    let decline_period = (sda_peak_month() * 2 - sda_peak_month()) as f64;
    let months_past_peak = (seasoning - sda_peak_month()) as f64;
    let cdr = (sda_peak_cdr()
        - (months_past_peak / decline_period) * (sda_peak_cdr() - sda_terminal_cdr()))
        * 1.5;
    let expected = 1.0 - (1.0 - cdr).powf(1.0 / 12.0);
    let sda_rate = sc.calculate_default_rate(test_date(), seasoning).unwrap();
    assert!((sda_rate - expected).abs() < 1e-12);
}

#[test]
fn test_current_loss_percentage_handles_zero_balance_and_offsets() {
    let empty_pool = create_pool_with_balance(0.0);
    let tranches = create_single_tranche();
    let sc_zero = StructuredCredit::new_abs(
        "TEST-ZERO",
        empty_pool,
        tranches.clone(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );
    assert_eq!(sc_zero.current_loss_percentage().unwrap(), 0.0);

    let mut pool = create_pool_with_balance(1_000_000.0);
    pool.cumulative_defaults = Money::new(50_000.0, Currency::USD);
    pool.cumulative_recoveries = Money::new(10_000.0, Currency::USD);
    let sc = StructuredCredit::new_abs(
        "TEST-LOSS",
        pool,
        tranches,
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );
    // Denominator is original balance approximated as:
    // current_balance + cumulative_defaults + cumulative_prepayments
    // = 1,000,000 + 50,000 + 0 = 1,050,000
    let original_balance = 1_000_000.0 + 50_000.0;
    let expected = (50_000.0 - 10_000.0) / original_balance * DECIMAL_TO_PERCENT;
    let actual = sc.current_loss_percentage().unwrap();
    assert!((actual - expected).abs() < 1e-12);
}
