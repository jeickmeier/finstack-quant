//! Tests for structured credit constructors and behavioral overrides.

use finstack_quant_cashflows::builder::annual_to_monthly_clamped;
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_valuations_structured_credit::structured_credit::{
    AssetPool, CreditModelConfig, DealFees, DealType, PoolAsset, StructuredCredit, Tranche,
    TrancheStructure,
};
use time::Month;

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
        finstack_quant_valuations_core::results::structured_credit::TrancheSeniority::Senior,
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        RateSpec::Fixed { rate: 0.05 },
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
    )
    .unwrap();
    TrancheStructure::new(vec![tranche]).unwrap()
}

#[test]
fn per_type_constructors_set_expected_assumptions() {
    let pool = create_pool_with_balance(1_000_000.0);
    let tranches = create_single_tranche();
    let closing = Date::from_calendar_date(2024, Month::January, 1).unwrap();
    let legal = maturity_date();

    // Expected annual CDRs are the `constructor.default_cdr` values of the
    // embedded registry's deal profiles
    // (`data/assumptions/structured_credit_assumptions.v1.json`).
    let (p, t) = (|| pool.clone(), || tranches.clone());
    let cases = [
        (
            StructuredCredit::new_standard(finstack_quant_valuations_structured_credit::structured_credit::DealType::Abs, finstack_quant_valuations_structured_credit::structured_credit::StandardDealParams { id: "ABS".into(), pool: p(), tranches: t(), closing_date: closing, maturity: legal, discount_curve_id: "USD-OIS".into(), calendar_id: None }),
            DealType::Abs,
            Tenor::monthly(),
            0.02,
        ),
        (
            StructuredCredit::new_standard(finstack_quant_valuations_structured_credit::structured_credit::DealType::Clo, finstack_quant_valuations_structured_credit::structured_credit::StandardDealParams { id: "CLO".into(), pool: p(), tranches: t(), closing_date: closing, maturity: legal, discount_curve_id: "USD-OIS".into(), calendar_id: None }),
            DealType::Clo,
            Tenor::quarterly(),
            0.02,
        ),
        (
            StructuredCredit::new_standard(finstack_quant_valuations_structured_credit::structured_credit::DealType::Cmbs, finstack_quant_valuations_structured_credit::structured_credit::StandardDealParams { id: "CMBS".into(), pool: p(), tranches: t(), closing_date: closing, maturity: legal, discount_curve_id: "USD-OIS".into(), calendar_id: None }),
            DealType::Cmbs,
            Tenor::monthly(),
            0.005,
        ),
        (
            StructuredCredit::new_standard(finstack_quant_valuations_structured_credit::structured_credit::DealType::Rmbs, finstack_quant_valuations_structured_credit::structured_credit::StandardDealParams { id: "RMBS".into(), pool: p(), tranches: t(), closing_date: closing, maturity: legal, discount_curve_id: "USD-OIS".into(), calendar_id: None }),
            DealType::Rmbs,
            Tenor::monthly(),
            0.006,
        ),
    ];

    for (sc, deal_type, expected_frequency, expected_cdr) in cases {
        let sc = sc.expect("valid structured-credit dates");

        assert_eq!(sc.deal_type, deal_type);
        assert_eq!(sc.frequency, expected_frequency);
        assert!(
            (sc.credit_model.default_spec.mdr(1).unwrap()
                - finstack_quant_cashflows::builder::annual_to_monthly(expected_cdr).unwrap())
            .abs()
                < 1e-12
        );
    }
}

/// One CLO profile in the registry: the BSL fee convention (small senior
/// fee, larger subordinated fee), a senior-secured-loan recovery, and the
/// serde credit-model defaults equal to what `new_standard(DealType::Clo, ...)` applies.
#[test]
fn clo_registry_profile_is_the_single_source_of_clo_defaults() {
    let fees = DealFees::clo_standard(Currency::USD);
    assert!(
        fees.senior_mgmt_fee_bp < fees.subordinated_mgmt_fee_bp,
        "senior {} bp must be below subordinated {} bp",
        fees.senior_mgmt_fee_bp,
        fees.subordinated_mgmt_fee_bp
    );
    let clo = StructuredCredit::new_standard(
        finstack_quant_valuations_structured_credit::structured_credit::DealType::Clo,
        finstack_quant_valuations_structured_credit::structured_credit::StandardDealParams {
            id: "CLO".into(),
            pool: create_pool_with_balance(100_000_000.0),
            tranches: create_single_tranche(),
            closing_date: test_date(),
            maturity: maturity_date(),
            discount_curve_id: "USD-OIS".into(),
            calendar_id: None,
        },
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
    let mut sc = StructuredCredit::new_standard(
        finstack_quant_valuations_structured_credit::structured_credit::DealType::Abs,
        finstack_quant_valuations_structured_credit::structured_credit::StandardDealParams {
            id: "TEST-ABS".into(),
            pool,
            tranches,
            closing_date: Date::from_calendar_date(2024, Month::January, 1).unwrap(),
            maturity: maturity_date(),
            discount_curve_id: "USD-OIS".into(),
            calendar_id: None,
        },
    )
    .expect("valid structured-credit dates");

    sc.credit_model.prepayment_spec = PrepaymentModelSpec::abs(0.02);
    let abs_rate = sc.credit_model.prepayment_spec.smm(1).unwrap();
    assert!((abs_rate - 0.02).abs() < 1e-12);

    sc.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.12);
    let cpr_rate = sc.credit_model.prepayment_spec.smm(1).unwrap();
    assert!((cpr_rate - annual_to_monthly_clamped(0.12)).abs() < 1e-12);

    sc.credit_model.prepayment_spec = PrepaymentModelSpec::psa(2.0);
    // PSA standard: CPR ramps linearly to 6% over the first 30 months.
    let seasoning = 3;
    let base_cpr = (seasoning as f64 / 30.0) * 0.06;
    let expected = annual_to_monthly_clamped(base_cpr * 2.0);
    let psa_rate = sc.credit_model.prepayment_spec.smm(seasoning).unwrap();
    assert!((psa_rate - expected).abs() < 1e-12);
}

#[test]
fn test_default_spec_shapes_drive_monthly_rates() {
    let pool = create_pool_with_balance(1_000_000.0);
    let tranches = create_single_tranche();
    let mut sc = StructuredCredit::new_standard(
        finstack_quant_valuations_structured_credit::structured_credit::DealType::Abs,
        finstack_quant_valuations_structured_credit::structured_credit::StandardDealParams {
            id: "TEST-ABS".into(),
            pool,
            tranches,
            closing_date: Date::from_calendar_date(2024, Month::January, 1).unwrap(),
            maturity: maturity_date(),
            discount_curve_id: "USD-OIS".into(),
            calendar_id: None,
        },
    )
    .expect("valid structured-credit dates");

    sc.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.12);
    let cdr_rate = sc.credit_model.default_spec.mdr(1).unwrap();
    assert!((cdr_rate - annual_to_monthly_clamped(0.12)).abs() < 1e-12);

    sc.credit_model.default_spec = DefaultModelSpec::sda(1.5);

    // Canonical PSA SDA shape (`finstack_quant_cashflows::builder::SDA_PEAK_CDR`): CDR peaks at
    // 0.6% in month 30, plateaus through month 60 and declines to 0.03%.
    let sda_peak_month: u32 = 30;
    let sda_peak_cdr: f64 = 0.006;
    let sda_terminal_cdr: f64 = 0.0003;
    let plateau_seasoning = sda_peak_month + 1;
    let plateau_cdr = sda_peak_cdr * 1.5;
    let expected_plateau = 1.0 - (1.0 - plateau_cdr).powf(1.0 / 12.0);
    let plateau_rate = sc.credit_model.default_spec.mdr(plateau_seasoning).unwrap();
    assert!((plateau_rate - expected_plateau).abs() < 1e-12);

    // Months 61-120 decline linearly from the peak to the terminal CDR;
    // month 90 sits halfway through the decline.
    let decline_seasoning = 90;
    let frac = f64::from(decline_seasoning - 60) / 60.0;
    let decline_cdr = (sda_peak_cdr - frac * (sda_peak_cdr - sda_terminal_cdr)) * 1.5;
    let expected_decline = 1.0 - (1.0 - decline_cdr).powf(1.0 / 12.0);
    let decline_rate = sc.credit_model.default_spec.mdr(decline_seasoning).unwrap();
    assert!((decline_rate - expected_decline).abs() < 1e-12);
}

#[test]
fn standard_constructor_applies_calendar_and_rejects_unsupported_profiles() {
    use finstack_quant_valuations_structured_credit::structured_credit::StandardDealParams;
    let params = StandardDealParams {
        id: "STANDARD".into(),
        pool: create_pool_with_balance(1_000_000.0),
        tranches: create_single_tranche(),
        closing_date: test_date(),
        maturity: maturity_date(),
        discount_curve_id: "USD-OIS".into(),
        calendar_id: Some("nyse".into()),
    };
    let deal = StructuredCredit::new_standard(DealType::Abs, params.clone()).unwrap();
    assert_eq!(deal.calendar_id.as_deref(), Some("nyse"));
    let mut no_calendar = params.clone();
    no_calendar.calendar_id = None;
    assert!(StructuredCredit::new_standard(DealType::Abs, no_calendar)
        .unwrap()
        .calendar_id
        .is_none());
    for unsupported in [DealType::Cbo, DealType::Auto, DealType::Card] {
        assert!(StructuredCredit::new_standard(unsupported, params.clone()).is_err());
    }
    let mut invalid = params;
    invalid.maturity = invalid.closing_date;
    assert!(StructuredCredit::new_standard(DealType::Abs, invalid).is_err());
}

#[test]
fn standard_constructor_rejects_invalid_nested_collateral_before_pricing() {
    use finstack_quant_valuations_structured_credit::structured_credit::{
        AssetType, BalloonSpec, StandardDealParams,
    };

    let mut pool = create_pool_with_balance(1_000_000.0);
    pool.assets[0].asset_type = AssetType::CommercialMortgage { ltv: None };
    pool.assets[0].balloon = Some(BalloonSpec {
        extension_prob: 1.5,
        extension_months: 24,
        extension_rate: None,
        loss_prob: 0.0,
        severity_pct: 0.0,
        workout_months: 0,
    });
    let error = StructuredCredit::new_standard(
        DealType::Cmbs,
        StandardDealParams {
            id: "INVALID-COLLATERAL".into(),
            pool,
            tranches: create_single_tranche(),
            closing_date: test_date(),
            maturity: maturity_date(),
            discount_curve_id: "USD-OIS".into(),
            calendar_id: None,
        },
    )
    .expect_err("constructor validates caller-supplied collateral terms");
    assert!(error.to_string().contains("extension_prob"), "{error}");
}
