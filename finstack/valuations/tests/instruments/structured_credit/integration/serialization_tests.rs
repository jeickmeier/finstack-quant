//! Integration tests for JSON serialization and wire format stability.
//!
//! Tests that all structured credit types serialize/deserialize correctly
//! and maintain wire format compatibility.

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::structured_credit::RepLine;
use finstack_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, CorrelationStructure, CoverageTrigger, DealType, DefaultAssumptions,
    DefaultModelSpec, Overrides, PoolAsset, PrepaymentModelSpec, RecoveryModelSpec,
    ReinvestmentCriteria, ReinvestmentPeriod, StochasticDefaultSpec, StochasticPrepaySpec,
    StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    TriggerConsequence,
};
use finstack_valuations::instruments::json_loader::InstrumentJson;
use finstack_valuations::instruments::Attributes;
use finstack_valuations::instruments::{json_loader::InstrumentEnvelope, PayReceive};
use time::Month;

fn maturity_date() -> Date {
    Date::from_calendar_date(2030, Month::January, 1).unwrap()
}

// ============================================================================
// Model Spec Serialization Tests
// ============================================================================

#[test]
fn test_prepayment_spec_all_variants_serialize() {
    // Arrange
    let specs = vec![
        PrepaymentModelSpec::psa(100.0),
        PrepaymentModelSpec::constant_cpr(0.15),
    ];

    for spec in specs {
        // Act
        let json = serde_json::to_string(&spec).expect("Serialization failed");
        let deserialized: PrepaymentModelSpec =
            serde_json::from_str(&json).expect("Deserialization failed");

        // Assert
        assert_eq!(spec, deserialized, "Roundtrip failed for {:?}", spec);
    }
}

#[test]
fn test_default_spec_all_variants_serialize() {
    // Arrange
    let specs = vec![
        DefaultModelSpec::constant_cdr(0.02),
        DefaultModelSpec::sda(100.0),
    ];

    for spec in specs {
        // Act
        let json = serde_json::to_string(&spec).expect("Serialization failed");
        let deserialized: DefaultModelSpec =
            serde_json::from_str(&json).expect("Deserialization failed");

        // Assert
        assert_eq!(spec, deserialized, "Roundtrip failed for {:?}", spec);
    }
}

#[test]
fn test_recovery_spec_all_variants_serialize() {
    // Arrange
    let specs = vec![
        RecoveryModelSpec::with_lag(0.70, 12),
        RecoveryModelSpec::with_lag(0.40, 18),
    ];

    for spec in specs {
        // Act
        let json = serde_json::to_string(&spec).expect("Serialization failed");
        let deserialized: RecoveryModelSpec =
            serde_json::from_str(&json).expect("Deserialization failed");

        // Assert
        assert_eq!(spec, deserialized, "Roundtrip failed for {:?}", spec);
    }
}

// ============================================================================
// Full Instrument Serialization Tests
// ============================================================================

#[test]
fn test_clo_json_roundtrip() {
    // Arrange
    let pool = AssetPool::new("TEST_POOL", DealType::CLO, Currency::USD);

    let tranche = Tranche::new(
        "AAA",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(10_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![tranche]).unwrap();
    let original = StructuredCredit::new_clo(
        "TEST_CLO",
        pool,
        tranches,
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    // Act
    let json = serde_json::to_string(&original).expect("Serialization failed");
    let deserialized: StructuredCredit =
        serde_json::from_str(&json).expect("Deserialization failed");

    // Assert
    assert_eq!(original.id.as_str(), deserialized.id.as_str());
    assert_eq!(original.deal_type, deserialized.deal_type);
    assert_eq!(
        original.credit_model.prepayment_spec,
        deserialized.credit_model.prepayment_spec
    );
    assert_eq!(
        original.credit_model.default_spec,
        deserialized.credit_model.default_spec
    );
}

#[test]
fn test_rmbs_with_overrides_serialization() {
    // Arrange
    let pool = AssetPool::new("TEST_POOL", DealType::RMBS, Currency::USD);

    let tranche = Tranche::new(
        "AAA",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(10_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![tranche]).unwrap();
    let mut rmbs = StructuredCredit::new_rmbs(
        "TEST_RMBS",
        pool,
        tranches,
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    // Set behavior overrides
    rmbs.behavior_overrides.psa_speed_multiplier = Some(1.5);
    rmbs.behavior_overrides.cdr_annual = Some(0.01);

    // Act
    let json = serde_json::to_string(&rmbs).expect("Serialization failed");
    let deserialized: StructuredCredit =
        serde_json::from_str(&json).expect("Deserialization failed");

    // Assert
    assert_eq!(
        deserialized.behavior_overrides.psa_speed_multiplier,
        Some(1.5)
    );
    assert_eq!(deserialized.behavior_overrides.cdr_annual, Some(0.01));
}

// ============================================================================
// JSON Format Stability Tests
// ============================================================================

#[test]
fn test_prepayment_spec_json_format() {
    // Arrange
    let spec = PrepaymentModelSpec::psa(150.0);

    // Act
    let json = serde_json::to_string(&spec).unwrap();

    // Assert: Check JSON structure (wire format stability)
    assert!(json.contains("\"cpr\""));
    assert!(json.contains("\"curve\""));
    assert!(json.contains("\"psa\""));
    assert!(json.contains("\"speed_multiplier\""));
    assert!(json.contains("150"));
}

#[test]
fn test_default_spec_json_format() {
    // Arrange
    let spec = DefaultModelSpec::constant_cdr(0.02);

    // Act
    let json = serde_json::to_string(&spec).unwrap();

    // Assert: Check JSON structure
    assert!(json.contains("\"cdr\""));
    assert!(json.contains("0.02"));
}

#[test]
fn test_recovery_spec_json_format() {
    // Arrange
    let spec = RecoveryModelSpec::with_lag(0.70, 12);

    // Act
    let json = serde_json::to_string(&spec).unwrap();

    // Assert: Check JSON structure
    assert!(json.contains("\"rate\""));
    assert!(json.contains("\"recovery_lag\""));
    assert!(json.contains("0.7"));
    assert!(json.contains("12"));
}

fn build_full_feature_structured_credit() -> StructuredCredit {
    use finstack_core::currency::Currency;
    use finstack_core::dates::{BusinessDayConvention, Date, DayCount, Tenor};
    use finstack_core::types::CreditRating;
    use finstack_core::types::{CurveId, InstrumentId};

    let closing = Date::from_calendar_date(2024, Month::January, 1).unwrap();
    let first_payment = Date::from_calendar_date(2024, Month::April, 1).unwrap();
    let reinvestment_end = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let legal = Date::from_calendar_date(2034, Month::January, 1).unwrap();

    let mut pool = AssetPool::new("POOL-FULL", DealType::CLO, Currency::USD);

    let mut loan = PoolAsset::floating_rate_loan(
        "LOAN1",
        Money::new(12_000_000.0, Currency::USD),
        "SOFR-3M",
        350.0,
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        DayCount::Act360,
    )
    .with_rating(CreditRating::BB)
    .with_industry("Technology")
    .with_obligor("OBLIGOR-1");
    loan.smm_override = Some(0.0123);
    loan.mdr_override = Some(0.0042);

    let mut bond = PoolAsset::fixed_rate_bond(
        "BOND1",
        Money::new(8_000_000.0, Currency::USD),
        0.055,
        Date::from_calendar_date(2029, Month::July, 1).unwrap(),
        DayCount::Act365F,
    )
    .with_rating(CreditRating::A)
    .with_industry("Healthcare")
    .with_obligor("OBLIGOR-2");
    bond.is_defaulted = true;
    bond.recovery_amount = Some(Money::new(1_000_000.0, Currency::USD));
    bond.purchase_price = Some(Money::new(7_800_000.0, Currency::USD));

    pool.assets.push(loan);
    pool.assets.push(bond);

    pool.reinvestment_period = Some(ReinvestmentPeriod {
        end_date: reinvestment_end,
        is_active: true,
        criteria: ReinvestmentCriteria {
            max_price: 102.5,
            min_yield: 0.04,
            maintain_credit_quality: true,
            maintain_wal: false,
            apply_eligibility_criteria: true,
        },
    });
    pool.collection_account = Money::new(250_000.0, Currency::USD);
    pool.reserve_account = Money::new(100_000.0, Currency::USD);
    pool.excess_spread_account = Money::new(75_000.0, Currency::USD);
    pool.rep_lines = Some(vec![RepLine::new(
        "REP1",
        Money::new(20_000_000.0, Currency::USD),
        0.055,
        Some(180.0),
        Some("SOFR-1M".to_string()),
        Date::from_calendar_date(2031, Month::January, 1).unwrap(),
        12,
        DayCount::Act360,
    )
    .with_cpr(0.08)
    .with_cdr(0.03)
    .with_recovery_rate(0.50)]);

    let mut equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(5_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.12 },
        legal,
    )
    .unwrap()
    .with_oc_trigger(CoverageTrigger::new(
        1.15,
        TriggerConsequence::DivertCashFlow,
    ))
    .with_ic_trigger(CoverageTrigger::new(
        1.05,
        TriggerConsequence::DivertCashFlow,
    ))
    .revolving();
    equity.expected_maturity = Some(Date::from_calendar_date(2030, Month::January, 1).unwrap());
    equity.rating = Some(CreditRating::BB);
    equity.attributes = Attributes::new()
        .with_tag("equity")
        .with_meta("desk", "alts");

    let floating_coupon = finstack_cashflows::builder::FloatingRateSpec {
        index_id: CurveId::new("SOFR-3M"),
        spread_bp: rust_decimal::Decimal::try_from(150.0).expect("valid"),
        gearing: rust_decimal::Decimal::try_from(1.0).expect("valid"),
        gearing_includes_spread: true,
        index_floor_bp: Some(rust_decimal::Decimal::try_from(0.0).expect("valid")),
        all_in_floor_bp: Some(rust_decimal::Decimal::try_from(25.0).expect("valid")),
        all_in_cap_bp: Some(rust_decimal::Decimal::try_from(1200.0).expect("valid")),
        index_cap_bp: None,
        reset_freq: Tenor::quarterly(),
        index_tenor: None,
        reset_lag_days: 2,
        dc: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: "NYC".to_string(),
        fixing_calendar_id: Some("NYC".to_string()),
        end_of_month: false,
        overnight_compounding: None,
        overnight_basis: None,
        fallback: Default::default(),
        payment_lag_days: 0,
    };

    let mut senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(45_000_000.0, Currency::USD),
        TrancheCoupon::Floating(floating_coupon),
        legal,
    )
    .unwrap();
    senior.target_balance = Some(Money::new(30_000_000.0, Currency::USD));
    senior.rating = Some(CreditRating::AAA);
    senior.attributes = Attributes::new().with_tag("senior");

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let mut deal =
        StructuredCredit::new_clo("FULL-CLO", pool, tranches, closing, legal, "USD-SOFR-DISC");

    deal.first_payment_date = first_payment;
    deal.reinvestment_end_date = Some(reinvestment_end);
    deal.frequency = Tenor::monthly();
    deal.attributes = Attributes::new()
        .with_tag("full")
        .with_meta("book", "structured_credit");

    deal.credit_model.prepayment_spec = PrepaymentModelSpec::psa(175.0);
    deal.credit_model.default_spec = DefaultModelSpec::sda(125.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.55, 10);

    deal.market_conditions =
        finstack_valuations::instruments::fixed_income::structured_credit::MarketConditions {
            refi_rate: 0.035,
            original_rate: Some(0.05),
            hpa: Some(0.02),
            unemployment: Some(0.05),
            seasonal_factor: Some(0.98),
            custom_factors: vec![("stress".to_string(), 1.2)].into_iter().collect(),
        };

    deal.credit_factors =
        finstack_valuations::instruments::fixed_income::structured_credit::CreditFactors {
            credit_score: Some(720),
            dti: Some(0.32),
            ltv: Some(0.85),
            delinquency_days: 15,
            unemployment_rate: Some(0.04),
            annual_noi: None,
            annual_debt_service: None,
            custom_factors: vec![("fico_band".to_string(), 700.0)].into_iter().collect(),
        };

    deal.deal_metadata =
        finstack_valuations::instruments::fixed_income::structured_credit::Metadata {
            manager_id: Some("Manager-X".to_string()),
            servicer_id: Some("Servicer-Y".to_string()),
            master_servicer_id: Some("Master-Z".to_string()),
            special_servicer_id: Some("Special-W".to_string()),
            trustee_id: Some("Trustee-T".to_string()),
        };

    deal.behavior_overrides = Overrides {
        cpr_annual: Some(0.18),
        abs_speed: Some(0.012),
        psa_speed_multiplier: Some(1.3),
        cdr_annual: Some(0.025),
        sda_speed_multiplier: Some(1.1),
        recovery_rate: Some(0.42),
        recovery_lag_months: Some(9),
        reinvestment_price: Some(101.0),
    };

    deal.default_assumptions = DefaultAssumptions {
        base_cdr_annual: 0.03,
        base_recovery_rate: 0.45,
        base_cpr_annual: 0.09,
        psa_speed: Some(110.0),
        sda_speed: Some(1.2),
        abs_speed_monthly: Some(0.011),
        cpr_by_asset_type: vec![("abs_auto".to_string(), 0.20)].into_iter().collect(),
        cdr_by_asset_type: vec![("abs_auto".to_string(), 0.03)].into_iter().collect(),
        recovery_by_asset_type: vec![("abs_auto".to_string(), 0.50)].into_iter().collect(),
    };

    deal.credit_model.stochastic_prepay_spec = Some(StochasticPrepaySpec::factor_correlated(
        PrepaymentModelSpec::psa(1.1),
        0.25,
        0.12,
    ));
    deal.credit_model.stochastic_default_spec =
        Some(StochasticDefaultSpec::gaussian_copula(0.025, 0.35));
    deal.credit_model.correlation_structure =
        Some(CorrelationStructure::sectored(0.28, 0.12, -0.18));

    let swap = crate::finstack_test_utils::usd_irs_swap(
        InstrumentId::new("HEDGE-SWAP"),
        Money::new(10_000_000.0, Currency::USD),
        0.015,
        closing,
        Date::from_calendar_date(2028, Month::January, 1).unwrap(),
        PayReceive::Pay,
    )
    .expect("valid swap");
    deal.hedge_swaps.push(swap);

    deal
}

#[test]
fn test_structured_credit_full_feature_json_roundtrip() {
    let original = build_full_feature_structured_credit();
    let json = serde_json::to_string_pretty(&original).expect("serialize");
    let parsed: StructuredCredit = serde_json::from_str(&json).expect("deserialize");

    // Core identifiers and schedule
    assert_eq!(original.id, parsed.id);
    assert_eq!(original.deal_type, parsed.deal_type);
    assert_eq!(original.frequency, parsed.frequency);
    assert_eq!(original.first_payment_date, parsed.first_payment_date);
    assert_eq!(original.reinvestment_end_date, parsed.reinvestment_end_date);

    // AssetPool with overrides, reinvestment, rep lines, and accounts
    assert_eq!(original.pool.assets.len(), parsed.pool.assets.len());
    assert_eq!(
        original.pool.assets[0].smm_override,
        parsed.pool.assets[0].smm_override
    );
    assert_eq!(
        original.pool.assets[0].mdr_override,
        parsed.pool.assets[0].mdr_override
    );
    assert_eq!(
        original.pool.reinvestment_period.as_ref().unwrap().end_date,
        parsed.pool.reinvestment_period.as_ref().unwrap().end_date
    );
    assert_eq!(
        original.pool.collection_account,
        parsed.pool.collection_account
    );
    assert_eq!(original.pool.reserve_account, parsed.pool.reserve_account);
    assert_eq!(
        original.pool.excess_spread_account,
        parsed.pool.excess_spread_account
    );
    assert_eq!(
        original.pool.rep_lines.as_ref().unwrap()[0].cpr,
        parsed.pool.rep_lines.as_ref().unwrap()[0].cpr
    );

    // Tranche structure (ratings, triggers, targets, attributes)
    assert_eq!(
        original.tranches.tranches.len(),
        parsed.tranches.tranches.len()
    );
    let orig_equity = &original.tranches.tranches[0];
    let parsed_equity = &parsed.tranches.tranches[0];
    assert_eq!(orig_equity.rating, parsed_equity.rating);
    assert_eq!(
        orig_equity.oc_trigger.as_ref().unwrap().trigger_level,
        parsed_equity.oc_trigger.as_ref().unwrap().trigger_level
    );
    assert_eq!(orig_equity.attributes.tags, parsed_equity.attributes.tags);

    let orig_senior = &original.tranches.tranches[1];
    let parsed_senior = &parsed.tranches.tranches[1];
    assert_eq!(orig_senior.target_balance, parsed_senior.target_balance);
    assert_eq!(orig_senior.rating, parsed_senior.rating);

    // Behavioral specs and overrides
    assert_eq!(
        original.credit_model.prepayment_spec,
        parsed.credit_model.prepayment_spec
    );
    assert_eq!(
        original.credit_model.default_spec,
        parsed.credit_model.default_spec
    );
    assert_eq!(
        original.credit_model.recovery_spec,
        parsed.credit_model.recovery_spec
    );
    assert_eq!(
        original.behavior_overrides.cpr_annual,
        parsed.behavior_overrides.cpr_annual
    );
    assert_eq!(
        original.behavior_overrides.reinvestment_price,
        parsed.behavior_overrides.reinvestment_price
    );

    // Market and credit factors
    assert_eq!(
        original.market_conditions.refi_rate,
        parsed.market_conditions.refi_rate
    );
    assert_eq!(
        original.market_conditions.original_rate,
        parsed.market_conditions.original_rate
    );
    assert_eq!(
        original.credit_factors.credit_score,
        parsed.credit_factors.credit_score
    );
    assert_eq!(original.credit_factors.dti, parsed.credit_factors.dti);

    // Default assumptions and tags
    assert_eq!(
        original.default_assumptions.abs_speed_monthly,
        parsed.default_assumptions.abs_speed_monthly
    );
    assert_eq!(
        original
            .default_assumptions
            .cpr_by_asset_type
            .get("abs_auto"),
        parsed.default_assumptions.cpr_by_asset_type.get("abs_auto")
    );
    assert_eq!(original.attributes.tags, parsed.attributes.tags);
    assert_eq!(original.attributes.meta, parsed.attributes.meta);

    // Stochastic specs and correlation
    assert_eq!(
        original.credit_model.stochastic_prepay_spec,
        parsed.credit_model.stochastic_prepay_spec
    );
    assert_eq!(
        original.credit_model.stochastic_default_spec,
        parsed.credit_model.stochastic_default_spec
    );
    assert_eq!(
        original.credit_model.correlation_structure,
        parsed.credit_model.correlation_structure
    );

    // Hedge swap coverage
    assert_eq!(original.hedge_swaps.len(), parsed.hedge_swaps.len());
    assert_eq!(original.hedge_swaps[0].id, parsed.hedge_swaps[0].id);
}

#[test]
fn test_structured_credit_instrument_envelope_roundtrip() {
    let instrument = build_full_feature_structured_credit();
    let envelope = InstrumentEnvelope {
        schema: "finstack.instrument/1".to_string(),
        instrument: InstrumentJson::StructuredCredit(Box::new(instrument.clone())),
    };

    let json = serde_json::to_string_pretty(&envelope).expect("serialize");
    let parsed: InstrumentEnvelope = serde_json::from_str(&json).expect("deserialize");

    match parsed.instrument {
        InstrumentJson::StructuredCredit(sc) => {
            assert_eq!(sc.id, instrument.id);
            assert_eq!(sc.pool.assets.len(), instrument.pool.assets.len());
            assert_eq!(
                sc.tranches.tranches.len(),
                instrument.tranches.tranches.len()
            );
            assert_eq!(
                sc.credit_model.stochastic_default_spec,
                instrument.credit_model.stochastic_default_spec,
                "Stochastic default spec should survive envelope roundtrip"
            );
        }
        other => panic!("Unexpected instrument variant: {:?}", other),
    }
}

#[test]
fn test_structured_credit_full_example_json_file_roundtrip() {
    let json = include_str!("../../json_examples/structured_credit_full.json");
    let envelope: InstrumentEnvelope = serde_json::from_str(json).expect("deserialize example");

    match &envelope.instrument {
        InstrumentJson::StructuredCredit(sc) => {
            assert_eq!(sc.id.as_str(), "FULL-CLO");
            assert_eq!(sc.tranches.tranches.len(), 2);
            assert_eq!(sc.pool.assets.len(), 2);
            assert!(sc.pool.reinvestment_period.is_some());
            assert!(sc.pool.rep_lines.is_some());
            assert!(sc.behavior_overrides.cpr_annual.is_some());
            assert!(sc.default_assumptions.abs_speed_monthly.is_some());
            assert!(sc.credit_model.stochastic_prepay_spec.is_some());
            assert!(sc.credit_model.stochastic_default_spec.is_some());
            assert!(sc.credit_model.correlation_structure.is_some());
            assert_eq!(sc.hedge_swaps.len(), 1);
        }
        other => panic!("Unexpected instrument variant: {:?}", other),
    }

    // Ensure re-serialization maintains the shape
    let serialized = serde_json::to_string(&envelope).expect("serialize");
    let parsed: InstrumentEnvelope = serde_json::from_str(&serialized).expect("second deserialize");

    match parsed.instrument {
        InstrumentJson::StructuredCredit(sc) => assert_eq!(sc.id.as_str(), "FULL-CLO"),
        other => panic!("Unexpected instrument variant: {:?}", other),
    }
}
