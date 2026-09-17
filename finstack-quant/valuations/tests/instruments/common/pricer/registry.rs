//! Pricer registry tests.
//!
//! Tests for:
//! - `InstrumentType` parsing, display, and serialization
//! - `ModelKey` parsing and display
//! - `PricerKey` creation and equality
//! - `PricingError` variants and conversions
//! - `PricerRegistry` lookup and standard registry coverage

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::pricer::*;
use std::collections::BTreeMap;
use std::str::FromStr;
use strum::IntoEnumIterator;
use time::macros::date;

#[test]
fn test_instrument_type_from_str_all_variants() {
    for instrument_type in InstrumentType::iter() {
        let canonical = instrument_type.as_str();
        assert_eq!(InstrumentType::from_str(canonical), Ok(instrument_type));
        assert_eq!(instrument_type.to_string(), canonical);

        let json = serde_json::to_string(&instrument_type).expect("serialize instrument type");
        assert_eq!(json, format!("\"{canonical}\""));
        assert_eq!(
            serde_json::from_str::<InstrumentType>(&json).expect("deserialize instrument type"),
            instrument_type
        );
    }

    for retired in [
        "cdsindex",
        "cdstranche",
        "cdsoption",
        "swap",
        "capfloor",
        "interest_rate_option",
        "basisswap",
        "equityoption",
        "fxoption",
        "fxspot",
        "fxswap",
        "ilb",
        "ir_future",
        "irfuture",
        "varianceswap",
        "clo",
        "abs",
        "rmbs",
        "cmbs",
        "pmf",
        "BOND",
        "Bond",
        "BaSKeT",
        "cds-index",
        "fx-option",
    ] {
        assert!(InstrumentType::from_str(retired).is_err());
    }
}

#[test]
fn test_instrument_type_from_str_errors() {
    assert!(InstrumentType::from_str("unknown").is_err());
    assert!(InstrumentType::from_str("").is_err());
    assert!(InstrumentType::from_str("invalid_type").is_err());

    let err = InstrumentType::from_str("foobar").unwrap_err();
    assert!(err.contains("Unknown instrument type"));
    assert!(err.contains("foobar"));
}

#[test]
fn test_instrument_type_display() {
    assert_eq!(InstrumentType::Bond.to_string(), "bond");
    assert_eq!(InstrumentType::Irs.to_string(), "interest_rate_swap");
    assert_eq!(InstrumentType::CapFloor.to_string(), "cap_floor");
    assert_eq!(InstrumentType::CdsIndex.to_string(), "cds_index");
    assert_eq!(InstrumentType::EquityOption.to_string(), "equity_option");
    assert_eq!(
        InstrumentType::InflationLinkedBond.to_string(),
        "inflation_linked_bond"
    );
    assert_eq!(
        InstrumentType::StructuredCredit.to_string(),
        "structured_credit"
    );
    assert_eq!(
        InstrumentType::PrivateMarketsFund.to_string(),
        "private_markets_fund"
    );
}

#[test]
fn test_model_key_from_str_all_variants() {
    for model in ModelKey::iter() {
        let canonical = model.as_str();
        assert_eq!(ModelKey::from_str(canonical), Ok(model));
        assert_eq!(model.to_string(), canonical);

        let json = serde_json::to_string(&model).expect("serialize model key");
        assert_eq!(json, format!("\"{canonical}\""));
        assert_eq!(
            serde_json::from_str::<ModelKey>(&json).expect("deserialize model key"),
            model
        );
    }

    for retired in [
        "lattice",
        "black",
        "black_76",
        "hullwhite1f",
        "hw1f",
        "hazard",
        "DISCOUNTING",
        "Black76",
        "hull-white-1f",
        "hazard-rate",
    ] {
        assert!(ModelKey::from_str(retired).is_err());
    }
}

#[test]
fn test_model_key_from_str_errors() {
    assert!(ModelKey::from_str("unknown").is_err());
    assert!(ModelKey::from_str("").is_err());
    assert!(ModelKey::from_str("invalid_model").is_err());

    let err = ModelKey::from_str("bad_model").unwrap_err();
    assert!(err.contains("Unknown model key"));
    assert!(err.contains("bad_model"));
}

#[test]
fn test_model_key_display() {
    assert_eq!(ModelKey::Discounting.to_string(), "discounting");
    assert_eq!(ModelKey::Tree.to_string(), "tree");
    assert_eq!(ModelKey::Black76.to_string(), "black76");
    assert_eq!(ModelKey::HullWhite1F.to_string(), "hull_white_1f");
    assert_eq!(ModelKey::HazardRate.to_string(), "hazard_rate");
    assert_eq!(ModelKey::RatesCredit.to_string(), "rates_credit");
}

#[test]
fn test_pricer_key_creation() {
    let key = PricerKey::new(InstrumentType::Bond, ModelKey::Discounting);
    assert_eq!(key.instrument, InstrumentType::Bond);
    assert_eq!(key.model, ModelKey::Discounting);

    let key2 = PricerKey::new(InstrumentType::Swaption, ModelKey::Black76);
    assert_eq!(key2.instrument, InstrumentType::Swaption);
    assert_eq!(key2.model, ModelKey::Black76);
}

#[test]
fn test_pricer_key_equality() {
    let key1 = PricerKey::new(InstrumentType::Bond, ModelKey::Discounting);
    let key2 = PricerKey::new(InstrumentType::Bond, ModelKey::Discounting);
    let key3 = PricerKey::new(InstrumentType::Bond, ModelKey::Tree);
    let key4 = PricerKey::new(InstrumentType::Irs, ModelKey::Discounting);

    assert_eq!(key1, key2);
    assert_ne!(key1, key3);
    assert_ne!(key1, key4);
}

#[test]
fn test_pricing_error_display() {
    let key = PricerKey::new(InstrumentType::Bond, ModelKey::HazardRate);
    let err = PricingError::UnknownPricer {
        key,
        available_models: Vec::new(),
    };
    let msg = err.to_string();
    assert!(msg.contains("No pricer found"));
    assert!(msg.contains("bond"));
    assert!(msg.contains("hazard_rate"));

    // With available models, the message should list them so an analyst can
    // pick a working alternative without consulting the source.
    let err_with_models = PricingError::UnknownPricer {
        key,
        available_models: vec![ModelKey::Discounting, ModelKey::Tree],
    };
    let msg_with_models = err_with_models.to_string();
    assert!(msg_with_models.contains("Available models"));
    assert!(msg_with_models.contains("discounting"));
    assert!(msg_with_models.contains("tree"));

    let err2 = PricingError::TypeMismatch {
        expected: InstrumentType::Bond,
        got: InstrumentType::Irs,
    };
    let msg2 = err2.to_string();
    assert!(msg2.contains("Type mismatch"));
    assert!(msg2.contains("bond"));
    assert!(msg2.contains("interest_rate_swap"));

    let err3 =
        PricingError::model_failure_with_context("test error", PricingErrorContext::default());
    assert!(err3.to_string().contains("Model failure: test error"));
}

#[test]
fn test_pricing_error_from_core_error() {
    // `from_core` replaces the former blanket `From<finstack_quant_core::Error>` impl;
    // a core `Validation` error maps to `InvalidInput`.
    let core_err = finstack_quant_core::Error::Validation("test error".to_string());
    let pricing_err = PricingError::from_core(core_err, PricingErrorContext::default());

    match pricing_err {
        PricingError::InvalidInput { message, .. } => {
            assert!(message.contains("test"));
        }
        other => panic!("Expected InvalidInput, got {other:?}"),
    }
}

#[test]
fn test_registry_get_unknown_pricer() {
    let registry = PricerRegistry::new();
    let key = PricerKey::new(InstrumentType::Bond, ModelKey::HazardRate);

    // Should return None for unregistered pricer
    assert!(registry.get_pricer(key).is_none());
}

#[test]
fn test_registry_price_with_unknown_pricer() {
    let registry = PricerRegistry::new();
    let market = MarketContext::new();

    // Create a simple bond
    let bond = Bond::fixed(
        "TEST_BOND",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-TREASURY",
    )
    .unwrap();

    // Try to price with an unregistered model
    let as_of = finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
        .unwrap();
    let result = registry.price_with_metrics(
        &bond,
        ModelKey::HazardRate,
        &market,
        as_of,
        &[],
        Default::default(),
    );

    assert!(result.is_err());
    match result.unwrap_err() {
        PricingError::UnknownPricer {
            key,
            available_models,
        } => {
            assert_eq!(key.instrument, InstrumentType::Bond);
            assert_eq!(key.model, ModelKey::HazardRate);
            // Empty registry has no models for any instrument.
            assert!(available_models.is_empty());
        }
        _ => panic!("Expected UnknownPricer error"),
    }
}

#[test]
fn removed_option_discounting_aliases_return_unknown_pricer() {
    use finstack_quant_valuations::instruments::commodity::CommodityOption;
    use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
    use finstack_quant_valuations::instruments::rates::swaption::Swaption;

    let equity_option = EquityOption::example().expect("equity option example");
    let swaption = Swaption::example();
    let commodity_option = CommodityOption::example();
    let registry = standard_pricer_registry();
    let market = MarketContext::new();
    let as_of = date!(2025 - 01 - 01);

    for (instrument, expected_type) in [
        (
            &equity_option as &dyn Instrument,
            InstrumentType::EquityOption,
        ),
        (&swaption as &dyn Instrument, InstrumentType::Swaption),
        (
            &commodity_option as &dyn Instrument,
            InstrumentType::CommodityOption,
        ),
    ] {
        let error = registry
            .price_with_metrics(
                instrument,
                ModelKey::Discounting,
                &market,
                as_of,
                &[],
                Default::default(),
            )
            .expect_err("removed Discounting alias must not route to a Black-76 kernel");

        match error {
            PricingError::UnknownPricer {
                key,
                available_models,
            } => {
                assert_eq!(key, PricerKey::new(expected_type, ModelKey::Discounting));
                assert!(available_models.contains(&ModelKey::Black76));
                assert!(!available_models.contains(&ModelKey::Discounting));
            }
            other => panic!("Expected UnknownPricer, got {other:?}"),
        }
    }
}

#[test]
fn standard_pricer_registry_has_exact_expected_coverage() {
    use InstrumentType as I;
    use ModelKey as M;

    let expected = BTreeMap::from([
        (
            I::Bond,
            vec![
                M::Discounting,
                M::Tree,
                M::HazardRate,
                M::RatesCredit,
                M::MertonMc,
            ],
        ),
        (I::Cds, vec![M::HazardRate]),
        (I::CdsIndex, vec![M::HazardRate]),
        (I::CdsTranche, vec![M::HazardRate]),
        (I::CdsOption, vec![M::BloombergCdso]),
        (I::Irs, vec![M::Discounting]),
        (I::CapFloor, vec![M::Black76, M::HullWhite1F]),
        (I::Swaption, vec![M::Black76, M::HullWhite1F, M::Normal]),
        (
            I::BermudanSwaption,
            vec![M::HullWhite1F, M::MonteCarloHullWhite1F, M::LmmMonteCarlo],
        ),
        (I::BasisSwap, vec![M::Discounting]),
        (I::Basket, vec![M::Discounting]),
        (I::Convertible, vec![M::Tree]),
        (I::Deposit, vec![M::Discounting]),
        (
            I::EquityOption,
            vec![
                M::Black76,
                M::MonteCarloHeston,
                M::HestonFourier,
                M::MonteCarloRoughBergomi,
                M::MonteCarloRoughHeston,
                M::RoughHestonFourier,
                M::PdeCrankNicolson1D,
                M::PdeAdi2D,
            ],
        ),
        (I::FxOption, vec![M::Black76]),
        (I::FxSpot, vec![M::Discounting]),
        (I::FxSwap, vec![M::Discounting]),
        (I::XccySwap, vec![M::Discounting]),
        (I::InflationLinkedBond, vec![M::Discounting]),
        (I::InflationSwap, vec![M::Discounting]),
        (I::YoYInflationSwap, vec![M::Discounting]),
        (I::InflationCapFloor, vec![M::Black76, M::Normal]),
        (I::InterestRateFuture, vec![M::Discounting]),
        (I::VarianceSwap, vec![M::Discounting]),
        (I::FxVarianceSwap, vec![M::Discounting]),
        (I::Equity, vec![M::Discounting]),
        (I::Repo, vec![M::Discounting]),
        (I::Fra, vec![M::Discounting]),
        (
            I::StructuredCredit,
            vec![M::Discounting, M::StructuredCreditStochastic],
        ),
        (I::PrivateMarketsFund, vec![M::Discounting]),
        (I::RevolvingCredit, vec![M::Discounting, M::MonteCarloGBM]),
        (
            I::AsianOption,
            vec![
                M::MonteCarloGBM,
                M::MonteCarloHeston,
                M::AsianGeometricBS,
                M::AsianTurnbullWakeman,
            ],
        ),
        (
            I::BarrierOption,
            vec![
                M::MonteCarloGBM,
                M::MonteCarloHeston,
                M::BarrierBSContinuous,
                M::PdeCrankNicolson1D,
            ],
        ),
        (
            I::LookbackOption,
            vec![M::MonteCarloGBM, M::LookbackBSContinuous],
        ),
        (I::QuantoOption, vec![M::QuantoBS]),
        (I::Autocallable, vec![M::MonteCarloGBM]),
        (I::CmsOption, vec![M::Black76, M::StaticReplication]),
        (I::CmsSwap, vec![M::Black76, M::StaticReplication]),
        (I::CliquetOption, vec![M::MonteCarloGBM]),
        (
            I::RangeAccrual,
            vec![M::MonteCarloGBM, M::StaticReplication],
        ),
        (
            I::FxBarrierOption,
            vec![M::MonteCarloGBM, M::FxBarrierBSContinuous],
        ),
        (I::TermLoan, vec![M::Discounting, M::Tree]),
        (I::Dcf, vec![M::Discounting]),
        (I::RealEstateAsset, vec![M::Discounting]),
        (I::LeveredRealEstateEquity, vec![M::Discounting]),
        (I::EquityTotalReturnSwap, vec![M::Discounting]),
        (I::FiIndexTotalReturnSwap, vec![M::Discounting]),
        (I::BondFuture, vec![M::BondFutureCleanPriceProxy]),
        (I::CommodityForward, vec![M::Discounting]),
        (I::CommoditySwap, vec![M::Discounting]),
        (
            I::CommodityOption,
            vec![M::Black76, M::MonteCarloSchwartzSmith],
        ),
        (I::CommodityAsianOption, vec![M::AsianTurnbullWakeman]),
        (I::CommoditySwaption, vec![M::Black76]),
        (I::CommoditySpreadOption, vec![M::Black76]),
        (I::VolatilityIndexFuture, vec![M::Discounting]),
        (I::FxForward, vec![M::Discounting]),
        (I::Ndf, vec![M::Discounting]),
        (I::AgencyMbsPassthrough, vec![M::Discounting]),
        (I::AgencyTba, vec![M::Discounting]),
        (I::DollarRoll, vec![M::Discounting]),
        (I::AgencyCmo, vec![M::Discounting]),
        (I::FxDigitalOption, vec![M::Black76]),
        (I::FxTouchOption, vec![M::Black76]),
        (I::Tarn, vec![M::MonteCarloHullWhite1F]),
        (I::CmsSpreadOption, vec![M::StaticReplication]),
        (I::CallableRangeAccrual, vec![M::MonteCarloHullWhite1F]),
        (I::Snowball, vec![M::Discounting, M::MonteCarloHullWhite1F]),
        (I::Composite, vec![M::Discounting]),
        (I::CommodityFuture, vec![M::Discounting]),
        (I::FxFuture, vec![M::Discounting]),
        (I::EquityFuture, vec![M::Discounting]),
        (I::EquityTotalReturnFuture, vec![M::Discounting]),
        (I::InterestRateFutureOption, vec![M::Discounting]),
        (I::EquityFutureOption, vec![M::Discounting]),
        (I::FxFutureOption, vec![M::Discounting]),
        (I::CommodityFutureOption, vec![M::Discounting]),
        (I::VolatilityIndexFutureOption, vec![M::Discounting]),
        (I::AssetBackedFacility, vec![M::Discounting]),
    ]);

    assert_eq!(standard_pricer_registry().all_models_grouped(), expected);
}

#[test]
fn test_expect_inst_type_mismatch() {
    // Create a bond but try to expect it as IRS
    let bond = Bond::fixed(
        "TEST_BOND",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-TREASURY",
    )
    .unwrap();

    let instrument: &dyn Instrument = &bond;

    // This should fail with TypeMismatch
    let result = expect_inst::<Bond>(instrument, InstrumentType::Irs);

    assert!(result.is_err());
    match result.unwrap_err() {
        PricingError::TypeMismatch { expected, got } => {
            assert_eq!(expected, InstrumentType::Irs);
            assert_eq!(got, InstrumentType::Bond);
        }
        _ => panic!("Expected TypeMismatch error"),
    }
}

#[test]
fn test_expect_inst_success() {
    let bond = Bond::fixed(
        "TEST_BOND",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-TREASURY",
    )
    .unwrap();

    let instrument: &dyn Instrument = &bond;

    // This should succeed
    let result = expect_inst::<Bond>(instrument, InstrumentType::Bond);
    assert!(result.is_ok());

    let bond_ref = result.unwrap();
    assert_eq!(bond_ref.notional.amount(), bond.notional.amount());
}

#[test]
fn test_instrument_type_serde_roundtrip() {
    let original = InstrumentType::StructuredCredit;
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: InstrumentType = serde_json::from_str(&json).unwrap();
    assert_eq!(original, deserialized);
}

#[test]
fn test_instrument_type_repr_values() {
    // Verify repr values for ABI stability
    assert_eq!(InstrumentType::Bond as u16, 1);
    assert_eq!(InstrumentType::TermLoan as u16, 41);
    assert_eq!(InstrumentType::Cds as u16, 3);
    assert_eq!(InstrumentType::StructuredCredit as u16, 26);
    assert_eq!(InstrumentType::PrivateMarketsFund as u16, 30);
}

#[test]
fn test_model_key_repr_values() {
    // Verify repr values for ABI stability
    assert_eq!(ModelKey::Discounting as u16, 1);
    assert_eq!(ModelKey::Tree as u16, 2);
    assert_eq!(ModelKey::Black76 as u16, 3);
    assert_eq!(ModelKey::HullWhite1F as u16, 4);
    assert_eq!(ModelKey::HazardRate as u16, 5);
    assert_eq!(ModelKey::RatesCredit as u16, 7);
}

#[test]
fn test_all_models_is_sorted_deduplicated_and_registry_derived() {
    let registry = standard_pricer_registry();
    let models = registry.all_models();

    assert!(!models.is_empty());
    let mut sorted = models.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(models, sorted, "all_models must be sorted and deduplicated");

    // Registry-derived: every reported model must actually price something.
    for model in &models {
        assert!(
            registry
                .all_models_grouped()
                .values()
                .any(|instrument_models| instrument_models.contains(model)),
            "model {model} has no registered pricer"
        );
    }
}

#[test]
fn test_all_models_grouped_agrees_with_available_models_for_instrument() {
    let registry = standard_pricer_registry();
    let grouped = registry.all_models_grouped();
    assert!(!grouped.is_empty());

    for (instrument, models) in &grouped {
        let mut available = registry.available_models_for_instrument(*instrument);
        available.sort_unstable();
        available.dedup();
        assert_eq!(&available, models);
        assert!(!models.is_empty());
    }

    let mut flattened: Vec<ModelKey> = grouped.values().flatten().copied().collect();
    flattened.sort_unstable();
    flattened.dedup();
    assert_eq!(flattened, registry.all_models());
}

#[test]
fn test_list_models_mirrors_the_standard_pricer_registry_and_is_deterministic() {
    let expected: Vec<String> = standard_pricer_registry()
        .all_models()
        .into_iter()
        .map(|model| model.to_string())
        .collect();

    assert_eq!(list_models(), expected);
    assert_eq!(list_models(), list_models());
    assert!(list_models().contains(&"discounting".to_string()));
    assert!(list_models().contains(&"rates_credit".to_string()));

    // Every advertised name must round-trip through the canonical parser.
    for name in list_models() {
        let parsed = parse_model_key(&name).expect("listed model must parse");
        assert_eq!(parsed.to_string(), name);
    }
}

#[test]
fn test_list_models_grouped_covers_the_same_models_as_list_models() {
    let grouped = list_models_grouped();
    assert!(!grouped.is_empty());

    let mut flattened: Vec<String> = grouped.values().flatten().cloned().collect();
    flattened.sort();
    flattened.dedup();
    let mut flat = list_models();
    flat.sort();
    assert_eq!(flattened, flat);

    // Keys are canonical instrument-type names in ascending order.
    let keys: Vec<&String> = grouped.keys().collect();
    let mut sorted_keys = keys.clone();
    sorted_keys.sort();
    assert_eq!(keys, sorted_keys);
    assert!(grouped.contains_key(&InstrumentType::Bond.to_string()));
    assert_eq!(
        grouped
            .get(&InstrumentType::Bond.to_string())
            .expect("bond models"),
        &standard_pricer_registry()
            .available_models_for_instrument(InstrumentType::Bond)
            .into_iter()
            .map(|model| model.to_string())
            .collect::<Vec<_>>()
    );
}
