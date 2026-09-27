//! Integration tests for JSON serialization and wire format stability.
//!
//! Tests that all structured credit types serialize/deserialize correctly
//! and maintain wire format compatibility.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_models::credit::pool::{
    CorrelationStructure, StochasticDefaultSpec, StochasticPrepaySpec,
};
use finstack_quant_valuations::instruments::fixed_income::loan_terms::RateSpec;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::RepLine;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, CoverageTrigger, DealType, DefaultModelSpec, HedgeSwap, PoolAsset,
    PrepaymentModelSpec, RecoveryModelSpec, ReinvestmentCriteria, ReinvestmentPeriod,
    StructuredCredit, SwapNotional, SwapPriority, Tranche, TrancheSeniority, TrancheStructure,
    TriggerConsequence,
};
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_valuations::instruments::{json_loader::InstrumentEnvelope, PayReceive};
use time::Month;

fn maturity_date() -> Date {
    Date::from_calendar_date(2030, Month::January, 1).unwrap()
}

// Model Spec Serialization Tests

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

// Full Instrument Serialization Tests

#[test]
fn test_clo_json_roundtrip() {
    // Arrange
    let pool = AssetPool::new("TEST_POOL", DealType::Clo, Currency::USD);

    let tranche = Tranche::new(
        "AAA",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        RateSpec::Fixed { rate: 0.05 },
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
fn test_rmbs_credit_model_serialization() {
    // Arrange
    let pool = AssetPool::new("TEST_POOL", DealType::Rmbs, Currency::USD);

    let tranche = Tranche::new(
        "AAA",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        RateSpec::Fixed { rate: 0.05 },
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

    // Set the credit model
    rmbs.credit_model.prepayment_spec = PrepaymentModelSpec::psa(1.5);
    rmbs.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.01);

    // Act
    let json = serde_json::to_string(&rmbs).expect("Serialization failed");
    let deserialized: StructuredCredit =
        serde_json::from_str(&json).expect("Deserialization failed");

    // Assert
    assert_eq!(
        deserialized.credit_model.prepayment_spec,
        PrepaymentModelSpec::psa(1.5)
    );
    assert_eq!(
        deserialized.credit_model.default_spec,
        DefaultModelSpec::constant_cdr(0.01)
    );
}

// JSON Format Stability Tests

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
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, Tenor};
    use finstack_quant_core::types::CreditRating;
    use finstack_quant_core::types::{CurveId, InstrumentId};

    let closing = Date::from_calendar_date(2024, Month::January, 1).unwrap();
    let first_payment = Date::from_calendar_date(2024, Month::April, 1).unwrap();
    let reinvestment_end = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let legal = Date::from_calendar_date(2034, Month::January, 1).unwrap();

    let mut pool = AssetPool::new("POOL-FULL", DealType::Clo, Currency::USD);

    let mut loan = PoolAsset::floating_rate_loan(
        "LOAN1",
        Money::new(12_000_000.0, Currency::USD).expect("valid money fixture"),
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
        Money::new(8_000_000.0, Currency::USD).expect("valid money fixture"),
        0.055,
        Date::from_calendar_date(2029, Month::July, 1).unwrap(),
        DayCount::Act365F,
    )
    .with_rating(CreditRating::A)
    .with_industry("Healthcare")
    .with_obligor("OBLIGOR-2");
    bond.defaulted = true;
    bond.recovery_amount =
        Some(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"));
    bond.purchase_price =
        Some(Money::new(7_800_000.0, Currency::USD).expect("valid money fixture"));

    pool.assets.push(loan);
    pool.assets.push(bond);

    pool.reinvestment_period = Some(ReinvestmentPeriod {
        end: reinvestment_end,
        is_active: true,
        amortizing_tranches: Vec::new(),
        assumptions: None,
        criteria: ReinvestmentCriteria {
            max_price_pct: 102.5,
            min_yield: 0.04,
        },
    });
    pool.collection_account = Money::new(250_000.0, Currency::USD).expect("valid money fixture");
    pool.reserve_account = Money::new(100_000.0, Currency::USD).expect("valid money fixture");
    pool.excess_spread_account = Money::new(75_000.0, Currency::USD).expect("valid money fixture");
    pool.rep_lines = Some(vec![RepLine::new(
        "REP1",
        Money::new(20_000_000.0, Currency::USD).expect("valid money fixture"),
        0.055,
        Date::from_calendar_date(2031, Month::January, 1).unwrap(),
        DayCount::Act360,
        finstack_quant_valuations::instruments::fixed_income::structured_credit::AssetType::FirstLienLoan {},
    )
    .with_cpr(0.08)
    .with_cdr(0.03)
    .with_recovery_rate(0.50)]);

    let mut equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"),
        RateSpec::Fixed { rate: 0.12 },
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
    ));
    equity.rating = Some(CreditRating::BB);
    equity.attributes = Attributes::new()
        .with_tag("equity")
        .with_meta("desk", "alts");

    let floating_coupon = finstack_quant_cashflows::builder::FloatingRateSpec {
        forward_curve_id: CurveId::new("SOFR-3M"),
        spread_bp: rust_decimal::Decimal::try_from(150.0).expect("valid"),
        gearing: rust_decimal::Decimal::try_from(1.0).expect("valid"),
        gearing_includes_spread: true,
        index_floor_bp: Some(rust_decimal::Decimal::try_from(0.0).expect("valid")),
        all_in_floor_bp: Some(rust_decimal::Decimal::try_from(25.0).expect("valid")),
        all_in_cap_bp: Some(rust_decimal::Decimal::try_from(1200.0).expect("valid")),
        index_cap_bp: None,
        overnight_index_constraints: Default::default(),
        reset_frequency: Tenor::quarterly(),
        index_tenor: None,
        reset_lag_days: 2,
        fixing_calendar_id: Some("usny".into()),
        overnight_compounding: None,
        overnight_basis: None,
        fallback: Default::default(),
    };

    let mut senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(45_000_000.0, Currency::USD).expect("valid money fixture"),
        RateSpec::Floating(floating_coupon),
        legal,
    )
    .unwrap();
    senior.rating = Some(CreditRating::AAA);
    senior.attributes = Attributes::new().with_tag("senior");

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let mut deal =
        StructuredCredit::new_clo("FULL-CLO", pool, tranches, closing, legal, "USD-SOFR-DISC");

    deal.first_payment_date = first_payment;
    deal.frequency = Tenor::monthly();
    deal.attributes = Attributes::new()
        .with_tag("full")
        .with_meta("book", "structured_credit");

    deal.credit_model.prepayment_spec = PrepaymentModelSpec::psa(175.0);
    deal.credit_model.default_spec = DefaultModelSpec::sda(125.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.55, 10);

    deal.market_conditions =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::MarketConditions {
            refi_rate: 0.035,
        };

    deal.deal_metadata =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::Metadata {
            manager_id: Some("Manager-X".to_string()),
            servicer_id: Some("Servicer-Y".to_string()),
            master_servicer_id: Some("Master-Z".to_string()),
            special_servicer_id: Some("Special-W".to_string()),
            trustee_id: Some("Trustee-T".to_string()),
        };

    deal.credit_model.stochastic_prepay_spec = Some(StochasticPrepaySpec::factor_correlated(
        PrepaymentModelSpec::psa(1.1),
        0.25,
        0.12,
    ));
    deal.credit_model.stochastic_default_spec =
        Some(StochasticDefaultSpec::gaussian_copula(0.025, 0.35));
    deal.credit_model.correlation_structure =
        Some(CorrelationStructure::sectored(0.28, 0.12, -0.18).expect("valid correlation"));

    let swap = crate::test_support::rates::usd_irs_swap(
        InstrumentId::new("HEDGE-SWAP"),
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        0.015,
        closing,
        Date::from_calendar_date(2028, Month::January, 1).unwrap(),
        PayReceive::Pay,
    )
    .expect("valid swap");
    deal.hedge_swaps
        .push(HedgeSwap::new(swap).on_tranche_par("SENIOR"));

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
        original.pool.reinvestment_period.as_ref().unwrap().end,
        parsed.pool.reinvestment_period.as_ref().unwrap().end
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

    // Market and credit factors
    assert_eq!(
        original.market_conditions.refi_rate,
        parsed.market_conditions.refi_rate
    );

    // Instrument tags
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
    assert_eq!(
        original.hedge_swaps[0].swap.id,
        parsed.hedge_swaps[0].swap.id
    );
    assert_eq!(
        parsed.hedge_swaps[0].notional,
        SwapNotional::TranchePar("SENIOR".into())
    );
    assert_eq!(parsed.hedge_swaps[0].priority, SwapPriority::SeniorFee);
}

#[test]
fn test_structured_credit_instrument_envelope_roundtrip() {
    let instrument = build_full_feature_structured_credit();
    let envelope = InstrumentEnvelope {
        schema: finstack_quant_valuations::instruments::json_loader::InstrumentSchema::CURRENT,
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

/// Binding-parity: a deal configured with the new `waterfall_rules` round-trips
/// through the canonical instrument envelope the Python/WASM bindings use, and prices
/// through the same `price_instrument_from_json` entry point. Proves the structural
/// features are reachable from the JSON bindings without any binding code change
/// (validation and deserialization are serde-based on the Rust type).
#[test]
fn waterfall_rules_round_trip_and_price_through_json() {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        AfcSpec, WaterfallRules,
    };
    use finstack_quant_valuations::pricer::{parse_boxed_instrument_from_json, price_instrument};

    let closing = Date::from_calendar_date(2024, Month::January, 1).unwrap();
    let mat = Date::from_calendar_date(2027, Month::January, 1).unwrap();
    let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "A1",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        0.05,
        mat,
        DayCount::Thirty360,
    ));
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "SR",
            0.0,
            80.0,
            TrancheSeniority::Senior,
            Money::new(800_000.0, Currency::USD).expect("valid money fixture"),
            RateSpec::Fixed { rate: 0.06 },
            mat,
        )
        .unwrap(),
        Tranche::new(
            "EQ",
            80.0,
            100.0,
            TrancheSeniority::Equity,
            Money::new(200_000.0, Currency::USD).expect("valid money fixture"),
            RateSpec::Fixed { rate: 0.0 },
            mat,
        )
        .unwrap(),
    ])
    .unwrap();
    let mut sc = StructuredCredit::new_abs("ABS-WF", pool, tranches, closing, mat, "USD-OIS")
        .with_calendar("nyse");
    sc.waterfall_rules = Some(WaterfallRules {
        afc: Some(AfcSpec {
            capped_tranches: vec!["SR".to_string()],
            net_wac_fee_bp: None,
            carryover: false,
        }),
        excess_spread: None,
        step_down: None,
        shifting_interest: None,
        early_amortization: None,
        controlled_accumulation: None,
        reserve: None,
        target_oc: None,
    });

    // Round-trip through the canonical envelope the bindings serialize.
    let json = serde_json::to_string(&InstrumentEnvelope::new(InstrumentJson::StructuredCredit(
        Box::new(sc),
    )))
    .expect("serialize");
    assert!(
        json.contains("waterfall_rules") && json.contains("capped_tranches"),
        "the AFC rule must appear in the wire format"
    );
    let parsed: InstrumentEnvelope = serde_json::from_str(&json).expect("deserialize");
    let json2 = serde_json::to_string(&parsed).expect("re-serialize");
    assert_eq!(json, json2, "waterfall_rules wire format must be stable");

    // Price through the entry point the Python/WASM bindings wrap.
    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(closing)
            .knots(vec![(0.0, 1.0), (5.0, 0.90)])
            .build()
            .unwrap(),
    );
    let instrument = parse_boxed_instrument_from_json(&json, None).expect("parse envelope");
    price_instrument(
        &instrument,
        &market,
        "2024-01-01",
        "default",
        &[],
        None,
        finstack_quant_valuations::instruments::PricingOptions::default(),
    )
    .expect("a waterfall_rules deal must price through the JSON binding path");
}

// Nested serde strictness: `deny_unknown_fields` does not propagate to nested
// types. Inject a typo at each nesting depth and assert rejection.

/// Serialize the canonical Rust provider as mutable JSON.
fn full_example_value() -> serde_json::Value {
    serde_json::to_value(InstrumentEnvelope {
        schema: finstack_quant_valuations::instruments::json_loader::InstrumentSchema::CURRENT,
        instrument: InstrumentJson::StructuredCredit(Box::new(
            build_full_feature_structured_credit(),
        )),
    })
    .expect("provider must serialize")
}

/// Assert that a mutated fixture is REJECTED by the deserializer, and that the
/// error names the offending field.
fn assert_rejected(value: &serde_json::Value, bad_field: &str, context: &str) {
    let text = serde_json::to_string(value).expect("re-serialize mutated fixture");
    let result: Result<InstrumentEnvelope, _> = serde_json::from_str(&text);
    let err = match result {
        Ok(_) => panic!(
            "unknown field `{bad_field}` in {context} was accepted; \
             enclosing type needs #[serde(deny_unknown_fields)]"
        ),
        Err(e) => e.to_string(),
    };
    assert!(
        err.contains(bad_field),
        "rejection of `{bad_field}` in {context} must name the offending \
         field so the user can find the typo; got: {err}"
    );
}

/// A misspelled per-asset prepayment override must be rejected, not silently
/// dropped back to the pool-level assumption.
#[test]
fn typo_in_pool_asset_field_is_rejected() {
    let mut v = full_example_value();
    let asset = &mut v["instrument"]["spec"]["pool"]["assets"][0];
    // One missing 'r' — the classic failure this guards.
    asset["smm_overide"] = serde_json::json!(0.02);
    assert_rejected(&v, "smm_overide", "PoolAsset");
}

/// A field that simply does not exist on `PoolAsset` must be rejected rather
/// than looking as though it configured something.
#[test]
fn unknown_pool_asset_field_is_rejected() {
    let mut v = full_example_value();
    v["instrument"]["spec"]["pool"]["assets"][0]["undefined_macro_factor"] = serde_json::json!(0.4);
    assert_rejected(&v, "undefined_macro_factor", "PoolAsset");
}

/// Removed runtime defaults must fail instead of shadowing canonical models.
#[test]
fn removed_runtime_default_assumptions_are_rejected() {
    let mut v = full_example_value();
    v["instrument"]["spec"]["default_assumptions"] = serde_json::json!({});
    assert_rejected(&v, "default_assumptions", "StructuredCredit");
}

/// Strictness must reach nested collections, not just the top two levels.
#[test]
fn typo_in_nested_pool_configuration_is_rejected() {
    let mut v = full_example_value();
    v["instrument"]["spec"]["pool"]["reinvestment_period"]["end_dat"] =
        serde_json::json!("2027-01-01");
    assert_rejected(&v, "end_dat", "ReinvestmentPeriod");
}

/// The top-level deny must still work (guards against a regression that
/// removes it while adding the nested ones).
#[test]
fn typo_in_top_level_field_is_rejected() {
    let mut v = full_example_value();
    v["instrument"]["spec"]["cleanup_call_pc"] = serde_json::json!(0.1);
    assert_rejected(&v, "cleanup_call_pc", "StructuredCredit");
}

/// `credit_model` is the only channel for deal behaviour and loan-level
/// `noi` the only NOI input, so the retired top-level containers are rejected.
#[test]
// schema-rejection-test: behavior_overrides, credit_factors (retired StructuredCredit keys)
fn retired_behavior_and_credit_factor_channels_are_rejected() {
    for (key, payload) in [
        (
            "behavior_overrides",
            serde_json::json!({"cpr_annual": 0.1, "reinvestment_price": 99.0}),
        ),
        (
            "credit_factors",
            serde_json::json!({"annual_noi": {"amount": "1", "currency": "USD"}}),
        ),
    ] {
        let mut v = full_example_value();
        v["instrument"]["spec"][key] = payload;
        assert_rejected(&v, key, "StructuredCredit");
    }
}

#[test]
// schema-rejection-test: fees.trustee_fee_annual (now fees.trustee_fee)
fn retired_trustee_fee_annual_key_is_rejected() {
    let deal = build_full_feature_structured_credit().with_standard_fees();
    let mut v = serde_json::to_value(InstrumentEnvelope {
        schema: finstack_quant_valuations::instruments::json_loader::InstrumentSchema::CURRENT,
        instrument: InstrumentJson::StructuredCredit(Box::new(deal)),
    })
    .expect("provider must serialize");
    let fees = v["instrument"]["spec"]["fees"]
        .as_object_mut()
        .expect("standard fees");
    let fee = fees.remove("trustee_fee").expect("trustee_fee key");
    fees.insert("trustee_fee_annual".into(), fee);
    assert_rejected(&v, "trustee_fee_annual", "DealFees");
}

/// Industry lives on `PoolAsset.industry` only; the loan and bond asset types
/// carry no payload, so a stray `industry` inside `asset_type` is rejected
/// instead of being silently dropped.
#[test]
// schema-rejection-test: pool.assets[].asset_type.industry (now pool.assets[].industry)
fn retired_asset_type_industry_payload_is_rejected() {
    let mut v = full_example_value();
    v["instrument"]["spec"]["pool"]["assets"][0]["asset_type"] =
        serde_json::json!({"type": "first_lien_loan", "industry": "Technology"});
    assert_rejected(&v, "industry", "AssetType");
}

/// The unmutated fixture must still parse — strictness must not have broken
/// any legitimate field.
#[test]
fn canonical_fixture_still_parses_under_strict_nested_serde() {
    let v = full_example_value();
    let text = serde_json::to_string(&v).expect("serialize");
    let envelope: InstrumentEnvelope = serde_json::from_str(&text)
        .expect("the canonical fixture must still parse with nested deny_unknown_fields");
    match envelope.instrument {
        InstrumentJson::StructuredCredit(sc) => {
            assert_eq!(sc.id.as_str(), "FULL-CLO");
            assert_eq!(sc.pool.assets.len(), 2);
        }
        other => panic!("Unexpected instrument variant: {other:?}"),
    }
}

// Structured-credit deal-term wire names (naming slice D7)

fn pool_asset_json() -> serde_json::Value {
    let asset = PoolAsset::fixed_rate_bond(
        "B1",
        Money::new(1_000_000.0, Currency::USD).unwrap(),
        0.06,
        maturity_date(),
        finstack_quant_core::dates::DayCount::Thirty360,
    );
    serde_json::to_value(&asset).unwrap()
}

#[test]
fn pool_asset_uses_rating_and_defaulted_keys() {
    let json = pool_asset_json();
    assert!(json.get("rating").is_some());
    assert_eq!(json["defaulted"], serde_json::json!(false));

    // `defaulted` is optional on the wire, like `CDSIndexConstituent.defaulted`.
    let mut without_flag = json;
    without_flag.as_object_mut().unwrap().remove("defaulted");
    let asset: PoolAsset = serde_json::from_value(without_flag).unwrap();
    assert!(!asset.defaulted);
}

#[test]
// schema-rejection-test
fn pool_asset_rejects_retired_is_defaulted_key() {
    let mut json = pool_asset_json();
    let map = json.as_object_mut().unwrap();
    map.remove("defaulted");
    map.insert("is_defaulted".into(), serde_json::json!(false));
    assert!(serde_json::from_value::<PoolAsset>(json).is_err());
}

#[test]
// schema-rejection-test
fn pool_asset_rejects_retired_credit_quality_key() {
    let mut json = pool_asset_json();
    let map = json.as_object_mut().unwrap();
    map.remove("rating");
    map.insert("credit_quality".into(), serde_json::json!("BB"));
    assert!(serde_json::from_value::<PoolAsset>(json).is_err());
}

#[test]
// schema-rejection-test
fn asset_type_rejects_retired_hotel_mortgage_tag() {
    let mut json = pool_asset_json();
    json["asset_type"] = serde_json::json!({"type": "hospitality_mortgage", "ltv": 0.6});
    serde_json::from_value::<PoolAsset>(json.clone()).expect("hospitality_mortgage loads");
    json["asset_type"] = serde_json::json!({"type": "hotel_mortgage", "ltv": 0.6});
    assert!(serde_json::from_value::<PoolAsset>(json).is_err());
}

#[test]
// schema-rejection-test
fn other_mortgage_rejects_retired_property_type_key() {
    let mut json = pool_asset_json();
    json["asset_type"] =
        serde_json::json!({"type": "other_mortgage", "description": "self storage", "ltv": null});
    serde_json::from_value::<PoolAsset>(json.clone()).expect("description loads");
    json["asset_type"] =
        serde_json::json!({"type": "other_mortgage", "property_type": "self storage", "ltv": null});
    assert!(serde_json::from_value::<PoolAsset>(json).is_err());
}

#[test]
// schema-rejection-test
fn asset_pool_rejects_retired_base_currency_key() {
    let pool = AssetPool::new("P", DealType::Abs, Currency::USD);
    let mut json = serde_json::to_value(&pool).unwrap();
    assert_eq!(json["currency"], serde_json::json!("USD"));
    let map = json.as_object_mut().unwrap();
    map.remove("currency");
    map.insert("base_currency".into(), serde_json::json!("USD"));
    assert!(serde_json::from_value::<AssetPool>(json).is_err());
}

#[test]
// schema-rejection-test
fn waterfall_rejects_retired_base_currency_key() {
    let waterfall = build_full_feature_structured_credit()
        .create_waterfall()
        .unwrap();
    let mut json = serde_json::to_value(&waterfall).unwrap();
    assert_eq!(json["currency"], serde_json::json!("USD"));
    let map = json.as_object_mut().unwrap();
    map.remove("currency");
    map.insert("base_currency".into(), serde_json::json!("USD"));
    assert!(serde_json::from_value::<
        finstack_quant_valuations::instruments::fixed_income::structured_credit::Waterfall,
    >(json)
    .is_err());
}
