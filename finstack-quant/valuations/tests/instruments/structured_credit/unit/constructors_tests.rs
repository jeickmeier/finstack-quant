//! Tests for structured credit constructors and behavioral overrides.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::loan_terms::RateSpec;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    clamped_cdr_to_mdr, clamped_cpr_to_smm,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, CreditModelConfig, DealFees, DealType, PoolAsset, StructuredCredit, Tranche,
    TrancheStructure,
};
use time::Month;

const DECIMAL_TO_PERCENT: f64 = 100.0;

fn test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn maturity_date() -> Date {
    Date::from_calendar_date(2030, Month::December, 31).unwrap()
}

fn create_pool_with_balance(balance: f64) -> AssetPool {
    let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
    if balance > 0.0 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::new(balance, Currency::USD).expect("valid money fixture"),
            0.06,
            Date::from_calendar_date(2029, Month::January, 1).unwrap(),
            finstack_quant_core::dates::DayCount::Thirty360,
        ));
    }
    pool
}

fn create_single_tranche() -> TrancheStructure {
    let tranche = Tranche::new(
        "SENIOR",
        0.0,
        100.0,
        finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheSeniority::Senior,
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        RateSpec::Fixed { rate: 0.05 },
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

    // Expected annual CDRs are the `constructor.default_cdr` values of the
    // embedded registry's deal profiles
    // (`data/assumptions/structured_credit_assumptions.v1.json`).
    let cases = [
        (DealType::Abs, Tenor::monthly(), 0.02),
        (DealType::Clo, Tenor::quarterly(), 0.02),
        (DealType::Cmbs, Tenor::monthly(), 0.005),
        (DealType::Rmbs, Tenor::monthly(), 0.006),
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
        )
        .expect("valid structured-credit dates");

        assert_eq!(sc.deal_type, deal_type);
        assert_eq!(sc.frequency, expected_frequency);
        assert!(
            (sc.credit_model.default_spec.mdr(1).unwrap()
                - finstack_quant_cashflows::builder::cdr_to_mdr(expected_cdr).unwrap())
            .abs()
                < 1e-12
        );
    }
}

/// One CLO profile in the registry: the BSL fee convention (small senior
/// fee, larger subordinated fee), a senior-secured-loan recovery, and the
/// serde credit-model defaults equal to what `new_clo` applies.
#[test]
fn clo_registry_profile_is_the_single_source_of_clo_defaults() {
    let fees = DealFees::clo_standard(Currency::USD);
    assert!(
        fees.senior_mgmt_fee_bp < fees.subordinated_mgmt_fee_bp,
        "senior {} bp must be below subordinated {} bp",
        fees.senior_mgmt_fee_bp,
        fees.subordinated_mgmt_fee_bp
    );
    let clo = StructuredCredit::new_clo(
        "CLO",
        create_pool_with_balance(100_000_000.0),
        create_single_tranche(),
        test_date(),
        maturity_date(),
        "USD-OIS",
    )
    .expect("valid structured-credit dates");
    assert!(
        clo.credit_model.recovery_spec.rate >= 0.55,
        "senior secured loans recover at least 55%: {}",
        clo.credit_model.recovery_spec.rate
    );
    let defaults = CreditModelConfig::default();
    assert_eq!(
        format!("{:?}", clo.credit_model.prepayment_spec),
        format!("{:?}", defaults.prepayment_spec)
    );
    assert_eq!(
        format!("{:?}", clo.credit_model.default_spec),
        format!("{:?}", defaults.default_spec)
    );
    assert_eq!(
        format!("{:?}", clo.credit_model.recovery_spec),
        format!("{:?}", defaults.recovery_spec)
    );
}

#[test]
fn test_example_has_expected_defaults() {
    let sc = StructuredCredit::example().expect("example");
    let waterfall = sc
        .create_waterfall()
        .expect("valid create_waterfall fixture");

    assert_eq!(sc.tranches.tranches.len(), 1);
    assert_eq!(waterfall.tiers.len(), 3);
    assert_eq!(sc.calendar_id.as_deref(), Some("nyse"));
}

#[test]
fn test_prepayment_spec_shapes_drive_monthly_rates() {
    let pool = create_pool_with_balance(1_000_000.0);
    let tranches = create_single_tranche();
    let mut sc = StructuredCredit::new_abs(
        "TEST-ABS",
        pool,
        tranches,
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .expect("valid structured-credit dates");

    sc.credit_model.prepayment_spec = PrepaymentModelSpec::abs(0.02);
    let abs_rate = sc.calculate_smm(1).unwrap();
    assert!((abs_rate - 0.02).abs() < 1e-12);

    sc.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.12);
    let cpr_rate = sc.calculate_smm(1).unwrap();
    assert!((cpr_rate - clamped_cpr_to_smm(0.12)).abs() < 1e-12);

    sc.credit_model.prepayment_spec = PrepaymentModelSpec::psa(2.0);
    // PSA standard: CPR ramps linearly to 6% over the first 30 months.
    let seasoning = 3;
    let base_cpr = (seasoning as f64 / 30.0) * 0.06;
    let expected = clamped_cpr_to_smm(base_cpr * 2.0);
    let psa_rate = sc.calculate_smm(seasoning).unwrap();
    assert!((psa_rate - expected).abs() < 1e-12);
}

#[test]
fn test_default_spec_shapes_drive_monthly_rates() {
    let pool = create_pool_with_balance(1_000_000.0);
    let tranches = create_single_tranche();
    let mut sc = StructuredCredit::new_abs(
        "TEST-ABS",
        pool,
        tranches,
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .expect("valid structured-credit dates");

    sc.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.12);
    let cdr_rate = sc.calculate_default_rate(1).unwrap();
    assert!((cdr_rate - clamped_cdr_to_mdr(0.12)).abs() < 1e-12);

    sc.credit_model.default_spec = DefaultModelSpec::sda(1.5);

    // Canonical PSA SDA shape (registry `default_models.sda`): CDR peaks at
    // 0.6% in month 30, plateaus through month 60 and declines to 0.03%.
    let sda_peak_month: u32 = 30;
    let sda_peak_cdr: f64 = 0.006;
    let sda_terminal_cdr: f64 = 0.0003;
    let plateau_seasoning = sda_peak_month + 1;
    let plateau_cdr = sda_peak_cdr * 1.5;
    let expected_plateau = 1.0 - (1.0 - plateau_cdr).powf(1.0 / 12.0);
    let plateau_rate = sc.calculate_default_rate(plateau_seasoning).unwrap();
    assert!((plateau_rate - expected_plateau).abs() < 1e-12);

    // Months 61-120 decline linearly from the peak to the terminal CDR;
    // month 90 sits halfway through the decline.
    let decline_seasoning = 90;
    let frac = f64::from(decline_seasoning - 60) / 60.0;
    let decline_cdr = (sda_peak_cdr - frac * (sda_peak_cdr - sda_terminal_cdr)) * 1.5;
    let expected_decline = 1.0 - (1.0 - decline_cdr).powf(1.0 / 12.0);
    let decline_rate = sc.calculate_default_rate(decline_seasoning).unwrap();
    assert!((decline_rate - expected_decline).abs() < 1e-12);
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
    )
    .expect("valid structured-credit dates");
    assert_eq!(sc_zero.current_loss_percentage().unwrap(), 0.0);

    let mut pool = create_pool_with_balance(1_000_000.0);
    pool.cumulative_defaults = Money::new(50_000.0, Currency::USD).expect("valid money fixture");
    pool.cumulative_recoveries = Money::new(10_000.0, Currency::USD).expect("valid money fixture");
    let sc = StructuredCredit::new_abs(
        "TEST-LOSS",
        pool,
        tranches,
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .expect("valid structured-credit dates");
    // Denominator is original balance approximated as:
    // current_balance + cumulative_defaults + cumulative_prepayments
    // = 1,000,000 + 50,000 + 0 = 1,050,000
    let original_balance = 1_000_000.0 + 50_000.0;
    let expected = (50_000.0 - 10_000.0) / original_balance * DECIMAL_TO_PERCENT;
    let actual = sc.current_loss_percentage().unwrap();
    assert!((actual - expected).abs() < 1e-12);
}
