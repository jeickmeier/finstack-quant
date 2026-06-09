//! Pricer registry tests.
//!
//! Tests for:
//! - `InstrumentType` parsing, display, and serialization
//! - `ModelKey` parsing and display
//! - `PricerKey` creation and equality
//! - `PricingError` variants and conversions
//! - `PricerRegistry` lookup, batch pricing, and standard registry coverage

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::bond::Bond;
use finstack_valuations::instruments::rates::deposit::Deposit;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use finstack_valuations::pricer::*;
use finstack_valuations::results::ValuationResult;
use std::str::FromStr;
use time::macros::date;

fn test_market(as_of: Date) -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.99), (5.0, 0.95)])
        .build()
        .expect("DiscountCurve builder should succeed with valid test data");
    MarketContext::new().insert(curve)
}

fn assert_pricing_result_eq(
    left: &std::result::Result<ValuationResult, PricingError>,
    right: &std::result::Result<ValuationResult, PricingError>,
) {
    match (left, right) {
        (Ok(left_val), Ok(right_val)) => {
            assert_eq!(left_val.instrument_id, right_val.instrument_id);
            assert_eq!(left_val.as_of, right_val.as_of);
            assert_eq!(left_val.value, right_val.value);
        }
        (Err(left_err), Err(right_err)) => {
            assert_eq!(left_err.to_string(), right_err.to_string());
        }
        _ => panic!("Expected matching PricingResult variants"),
    }
}

#[test]
fn test_instrument_type_from_str_all_variants() {
    // Basic variants
    assert_eq!(
        InstrumentType::from_str("bond").unwrap(),
        InstrumentType::Bond
    );
    assert_eq!(
        InstrumentType::from_str("loan").unwrap(),
        InstrumentType::Loan
    );
    assert_eq!(
        InstrumentType::from_str("cds").unwrap(),
        InstrumentType::CDS
    );
    assert_eq!(
        InstrumentType::from_str("deposit").unwrap(),
        InstrumentType::Deposit
    );
    assert_eq!(
        InstrumentType::from_str("equity").unwrap(),
        InstrumentType::Equity
    );
    assert_eq!(
        InstrumentType::from_str("repo").unwrap(),
        InstrumentType::Repo
    );
    assert_eq!(
        InstrumentType::from_str("fra").unwrap(),
        InstrumentType::FRA
    );
    assert_eq!(
        InstrumentType::from_str("basket").unwrap(),
        InstrumentType::Basket
    );

    // CDS variants with multiple aliases
    assert_eq!(
        InstrumentType::from_str("cds_index").unwrap(),
        InstrumentType::CDSIndex
    );
    assert_eq!(
        InstrumentType::from_str("cdsindex").unwrap(),
        InstrumentType::CDSIndex
    );
    assert_eq!(
        InstrumentType::from_str("cds_tranche").unwrap(),
        InstrumentType::CDSTranche
    );
    assert_eq!(
        InstrumentType::from_str("cdstranche").unwrap(),
        InstrumentType::CDSTranche
    );
    assert_eq!(
        InstrumentType::from_str("cds_option").unwrap(),
        InstrumentType::CDSOption
    );
    assert_eq!(
        InstrumentType::from_str("cdsoption").unwrap(),
        InstrumentType::CDSOption
    );

    // IRS with multiple aliases
    assert_eq!(
        InstrumentType::from_str("irs").unwrap(),
        InstrumentType::IRS
    );
    assert_eq!(
        InstrumentType::from_str("swap").unwrap(),
        InstrumentType::IRS
    );
    assert_eq!(
        InstrumentType::from_str("interest_rate_swap").unwrap(),
        InstrumentType::IRS
    );

    // CapFloor with aliases
    assert_eq!(
        InstrumentType::from_str("cap_floor").unwrap(),
        InstrumentType::CapFloor
    );
    assert_eq!(
        InstrumentType::from_str("capfloor").unwrap(),
        InstrumentType::CapFloor
    );
    assert_eq!(
        InstrumentType::from_str("interest_rate_option").unwrap(),
        InstrumentType::CapFloor
    );

    // Swaption
    assert_eq!(
        InstrumentType::from_str("swaption").unwrap(),
        InstrumentType::Swaption
    );

    // Basis Swap
    assert_eq!(
        InstrumentType::from_str("basis_swap").unwrap(),
        InstrumentType::BasisSwap
    );
    assert_eq!(
        InstrumentType::from_str("basisswap").unwrap(),
        InstrumentType::BasisSwap
    );

    // Convertible with alias
    assert_eq!(
        InstrumentType::from_str("convertible").unwrap(),
        InstrumentType::Convertible
    );
    assert_eq!(
        InstrumentType::from_str("convertible_bond").unwrap(),
        InstrumentType::Convertible
    );

    // Options
    assert_eq!(
        InstrumentType::from_str("equity_option").unwrap(),
        InstrumentType::EquityOption
    );
    assert_eq!(
        InstrumentType::from_str("equityoption").unwrap(),
        InstrumentType::EquityOption
    );
    assert_eq!(
        InstrumentType::from_str("fx_option").unwrap(),
        InstrumentType::FxOption
    );
    assert_eq!(
        InstrumentType::from_str("fxoption").unwrap(),
        InstrumentType::FxOption
    );

    // FX instruments
    assert_eq!(
        InstrumentType::from_str("fx_spot").unwrap(),
        InstrumentType::FxSpot
    );
    assert_eq!(
        InstrumentType::from_str("fxspot").unwrap(),
        InstrumentType::FxSpot
    );
    assert_eq!(
        InstrumentType::from_str("fx_swap").unwrap(),
        InstrumentType::FxSwap
    );
    assert_eq!(
        InstrumentType::from_str("fxswap").unwrap(),
        InstrumentType::FxSwap
    );

    // Inflation
    assert_eq!(
        InstrumentType::from_str("inflation_linked_bond").unwrap(),
        InstrumentType::InflationLinkedBond
    );
    assert_eq!(
        InstrumentType::from_str("ilb").unwrap(),
        InstrumentType::InflationLinkedBond
    );
    assert_eq!(
        InstrumentType::from_str("inflation_swap").unwrap(),
        InstrumentType::InflationSwap
    );

    // IR Future with aliases
    assert_eq!(
        InstrumentType::from_str("interest_rate_future").unwrap(),
        InstrumentType::InterestRateFuture
    );
    assert_eq!(
        InstrumentType::from_str("ir_future").unwrap(),
        InstrumentType::InterestRateFuture
    );
    assert_eq!(
        InstrumentType::from_str("irfuture").unwrap(),
        InstrumentType::InterestRateFuture
    );

    // Variance Swap
    assert_eq!(
        InstrumentType::from_str("variance_swap").unwrap(),
        InstrumentType::VarianceSwap
    );
    assert_eq!(
        InstrumentType::from_str("varianceswap").unwrap(),
        InstrumentType::VarianceSwap
    );

    assert_eq!(
        InstrumentType::from_str("structured_credit").unwrap(),
        InstrumentType::StructuredCredit
    );
    assert_eq!(
        InstrumentType::from_str("clo").unwrap(),
        InstrumentType::StructuredCredit
    );
    assert_eq!(
        InstrumentType::from_str("abs").unwrap(),
        InstrumentType::StructuredCredit
    );
    assert_eq!(
        InstrumentType::from_str("rmbs").unwrap(),
        InstrumentType::StructuredCredit
    );
    assert_eq!(
        InstrumentType::from_str("cmbs").unwrap(),
        InstrumentType::StructuredCredit
    );

    // Private Markets Fund
    assert_eq!(
        InstrumentType::from_str("private_markets_fund").unwrap(),
        InstrumentType::PrivateMarketsFund
    );
    assert_eq!(
        InstrumentType::from_str("pmf").unwrap(),
        InstrumentType::PrivateMarketsFund
    );

    // Case insensitivity
    assert_eq!(
        InstrumentType::from_str("BOND").unwrap(),
        InstrumentType::Bond
    );
    assert_eq!(
        InstrumentType::from_str("Bond").unwrap(),
        InstrumentType::Bond
    );
    assert_eq!(
        InstrumentType::from_str("BaSKeT").unwrap(),
        InstrumentType::Basket
    );

    // Dash handling
    assert_eq!(
        InstrumentType::from_str("cds-index").unwrap(),
        InstrumentType::CDSIndex
    );
    assert_eq!(
        InstrumentType::from_str("fx-option").unwrap(),
        InstrumentType::FxOption
    );
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
    assert_eq!(InstrumentType::IRS.to_string(), "irs");
    assert_eq!(InstrumentType::CapFloor.to_string(), "cap_floor");
    assert_eq!(InstrumentType::CDSIndex.to_string(), "cds_index");
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
fn test_instrument_type_as_str() {
    assert_eq!(InstrumentType::Bond.as_str(), "Bond");
    assert_eq!(InstrumentType::IRS.as_str(), "InterestRateSwap");
    assert_eq!(InstrumentType::CapFloor.as_str(), "CapFloor");
    assert_eq!(InstrumentType::CDSOption.as_str(), "CDSOption");
    assert_eq!(InstrumentType::Convertible.as_str(), "ConvertibleBond");
    assert_eq!(
        InstrumentType::StructuredCredit.as_str(),
        "StructuredCredit"
    );
}

#[test]
fn test_model_key_from_str_all_variants() {
    // Basic variants
    assert_eq!(
        ModelKey::from_str("discounting").unwrap(),
        ModelKey::Discounting
    );
    assert_eq!(ModelKey::from_str("tree").unwrap(), ModelKey::Tree);
    assert_eq!(ModelKey::from_str("lattice").unwrap(), ModelKey::Tree);

    // Black76 with aliases
    assert_eq!(ModelKey::from_str("black76").unwrap(), ModelKey::Black76);
    assert_eq!(ModelKey::from_str("black").unwrap(), ModelKey::Black76);
    assert_eq!(ModelKey::from_str("black_76").unwrap(), ModelKey::Black76);

    // Hull-White with aliases
    assert_eq!(
        ModelKey::from_str("hull_white_1f").unwrap(),
        ModelKey::HullWhite1F
    );
    assert_eq!(
        ModelKey::from_str("hullwhite1f").unwrap(),
        ModelKey::HullWhite1F
    );
    assert_eq!(ModelKey::from_str("hw1f").unwrap(), ModelKey::HullWhite1F);

    // Hazard Rate
    assert_eq!(
        ModelKey::from_str("hazard_rate").unwrap(),
        ModelKey::HazardRate
    );
    assert_eq!(ModelKey::from_str("hazard").unwrap(), ModelKey::HazardRate);

    // Case insensitivity
    assert_eq!(
        ModelKey::from_str("DISCOUNTING").unwrap(),
        ModelKey::Discounting
    );
    assert_eq!(ModelKey::from_str("Black76").unwrap(), ModelKey::Black76);

    // Dash handling
    assert_eq!(
        ModelKey::from_str("hull-white-1f").unwrap(),
        ModelKey::HullWhite1F
    );
    assert_eq!(
        ModelKey::from_str("hazard-rate").unwrap(),
        ModelKey::HazardRate
    );
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
    let key4 = PricerKey::new(InstrumentType::IRS, ModelKey::Discounting);

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
        got: InstrumentType::IRS,
    };
    let msg2 = err2.to_string();
    assert!(msg2.contains("Type mismatch"));
    assert!(msg2.contains("bond"));
    assert!(msg2.contains("irs"));

    let err3 = PricingError::model_failure_with_context("test error", PricingErrorContext::default());
    assert!(err3.to_string().contains("Model failure: test error"));
}

#[test]
fn test_pricing_error_from_core_error() {
    // Create a core error through a validation error
    let core_err = finstack_core::Error::Validation("test error".to_string());
    let pricing_err: PricingError = core_err.into();

    match pricing_err {
        PricingError::ModelFailure { message, .. } => {
            assert!(message.contains("test"));
        }
        _ => panic!("Expected ModelFailure"),
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
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        "USD-TREASURY",
    )
    .unwrap();

    // Try to price with an unregistered model
    let as_of =
        finstack_core::dates::Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
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
fn test_standard_registry_has_all_bond_pricers() {
    let registry = standard_registry();

    assert!(registry
        .get_pricer(PricerKey::new(InstrumentType::Bond, ModelKey::Discounting))
        .is_some());
    assert!(registry
        .get_pricer(PricerKey::new(InstrumentType::Bond, ModelKey::Tree))
        .is_some());
}

#[test]
fn test_standard_registry_has_all_rates_pricers() {
    let registry = standard_registry();

    // IRS
    assert!(registry
        .get_pricer(PricerKey::new(InstrumentType::IRS, ModelKey::Discounting))
        .is_some());

    // FRA
    assert!(registry
        .get_pricer(PricerKey::new(InstrumentType::FRA, ModelKey::Discounting))
        .is_some());

    // Basis Swap
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::BasisSwap,
            ModelKey::Discounting
        ))
        .is_some());

    // Deposit
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::Deposit,
            ModelKey::Discounting
        ))
        .is_some());

    // IR Future
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::InterestRateFuture,
            ModelKey::Discounting
        ))
        .is_some());
}

#[test]
fn test_price_batch_preserves_order() {
    let registry = standard_registry();
    let as_of = date!(2024 - 01 - 01);
    let market = test_market(as_of);

    let bond_one = Bond::fixed(
        "BOND-ORDER-1",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2026 - 01 - 01),
        "USD-OIS",
    )
    .expect("Bond::fixed should succeed with valid parameters");
    let deposit = Deposit::example().unwrap();
    let bond_two = Bond::fixed(
        "BOND-ORDER-2",
        Money::new(500_000.0, Currency::USD),
        0.03,
        as_of,
        date!(2027 - 01 - 01),
        "USD-OIS",
    )
    .expect("Bond::fixed should succeed with valid parameters");

    let instruments: Vec<&dyn Instrument> = vec![&bond_one, &deposit, &bond_two];
    let results = registry.price_batch(
        &instruments,
        ModelKey::Discounting,
        &market,
        as_of,
        &[],
        Default::default(),
    );

    assert_eq!(results.len(), instruments.len());
    let ids: Vec<&str> = results
        .iter()
        .map(|result| {
            result
                .as_ref()
                .expect("Pricing should succeed")
                .instrument_id
                .as_str()
        })
        .collect();

    assert_eq!(ids, vec![bond_one.id(), deposit.id(), bond_two.id()]);
}

#[test]
fn test_price_batch_matches_serial_results() {
    let registry = standard_registry();
    let as_of = date!(2024 - 01 - 01);
    let market = test_market(as_of);

    let bond_one = Bond::fixed(
        "BOND-PAR-1",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2026 - 01 - 01),
        "USD-OIS",
    )
    .expect("Bond::fixed should succeed with valid parameters");
    let deposit = Deposit::example().unwrap();
    let bond_two = Bond::fixed(
        "BOND-PAR-2",
        Money::new(500_000.0, Currency::USD),
        0.03,
        as_of,
        date!(2027 - 01 - 01),
        "USD-OIS",
    )
    .expect("Bond::fixed should succeed with valid parameters");

    let instruments: Vec<&dyn Instrument> = vec![&bond_one, &deposit, &bond_two];
    let serial_results: Vec<_> = instruments
        .iter()
        .map(|&instrument| {
            registry.price_with_metrics(
                instrument,
                ModelKey::Discounting,
                &market,
                as_of,
                &[],
                Default::default(),
            )
        })
        .collect();
    let batch_results = registry.price_batch(
        &instruments,
        ModelKey::Discounting,
        &market,
        as_of,
        &[],
        Default::default(),
    );

    assert_eq!(batch_results.len(), serial_results.len());
    for (serial, batch) in serial_results.iter().zip(batch_results.iter()) {
        assert_pricing_result_eq(serial, batch);
    }
}

#[test]
fn test_standard_price_batch_with_metrics_matches_serial_results() {
    let registry = standard_registry();
    let as_of = date!(2024 - 01 - 01);
    let market = test_market(as_of);

    let bond_one = Bond::fixed(
        "BOND-METRIC-PAR-1",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2026 - 01 - 01),
        "USD-OIS",
    )
    .expect("Bond::fixed should succeed with valid parameters");
    let bond_two = Bond::fixed(
        "BOND-METRIC-PAR-2",
        Money::new(500_000.0, Currency::USD),
        0.03,
        as_of,
        date!(2027 - 01 - 01),
        "USD-OIS",
    )
    .expect("Bond::fixed should succeed with valid parameters");

    let metrics = [MetricId::DirtyPrice];
    let instruments: Vec<&dyn Instrument> = vec![&bond_one, &bond_two];
    let serial_results: Vec<_> = instruments
        .iter()
        .map(|&instrument| {
            registry.price_with_metrics(
                instrument,
                ModelKey::Discounting,
                &market,
                as_of,
                &metrics,
                Default::default(),
            )
        })
        .collect();
    let batch_results = registry.price_batch(
        &instruments,
        ModelKey::Discounting,
        &market,
        as_of,
        &metrics,
        Default::default(),
    );

    assert_eq!(batch_results.len(), serial_results.len());
    for (serial, batch) in serial_results.iter().zip(batch_results.iter()) {
        assert_pricing_result_eq(serial, batch);
        assert_eq!(
            serial.as_ref().expect("serial pricing should succeed").measures,
            batch.as_ref().expect("batch pricing should succeed").measures
        );
    }
}

#[test]
fn test_standard_registry_has_all_options_pricers() {
    let registry = standard_registry();

    // CapFloor
    assert!(registry
        .get_pricer(PricerKey::new(InstrumentType::CapFloor, ModelKey::Black76))
        .is_some());
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::CapFloor,
            ModelKey::Discounting
        ))
        .is_some());

    // Swaption
    assert!(registry
        .get_pricer(PricerKey::new(InstrumentType::Swaption, ModelKey::Black76))
        .is_some());
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::Swaption,
            ModelKey::Discounting
        ))
        .is_some());

    // Equity Option
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::EquityOption,
            ModelKey::Black76
        ))
        .is_some());
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::EquityOption,
            ModelKey::Discounting
        ))
        .is_some());

    // FX Option
    assert!(registry
        .get_pricer(PricerKey::new(InstrumentType::FxOption, ModelKey::Black76))
        .is_some());
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::FxOption,
            ModelKey::Discounting
        ))
        .is_some());

    // CDS Option
    assert!(registry
        .get_pricer(PricerKey::new(InstrumentType::CDSOption, ModelKey::Black76))
        .is_some());
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::CDSOption,
            ModelKey::Discounting
        ))
        .is_some());
}

#[test]
fn test_standard_registry_has_all_credit_pricers() {
    let registry = standard_registry();

    // CDS
    assert!(registry
        .get_pricer(PricerKey::new(InstrumentType::CDS, ModelKey::HazardRate))
        .is_some());
    assert!(registry
        .get_pricer(PricerKey::new(InstrumentType::CDS, ModelKey::Discounting))
        .is_some());

    // CDS Index
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::CDSIndex,
            ModelKey::HazardRate
        ))
        .is_some());
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::CDSIndex,
            ModelKey::Discounting
        ))
        .is_some());

    // CDS Tranche
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::CDSTranche,
            ModelKey::HazardRate
        ))
        .is_some());
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::CDSTranche,
            ModelKey::Discounting
        ))
        .is_some());
}

#[test]
fn test_standard_registry_has_all_fx_pricers() {
    let registry = standard_registry();

    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::FxSpot,
            ModelKey::Discounting
        ))
        .is_some());
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::FxSwap,
            ModelKey::Discounting
        ))
        .is_some());
}

#[test]
fn test_standard_registry_has_other_pricers() {
    let registry = standard_registry();

    // Equity
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::Equity,
            ModelKey::Discounting
        ))
        .is_some());

    // TRS
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::EquityTotalReturnSwap,
            ModelKey::Discounting
        ))
        .is_some());
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::FIIndexTotalReturnSwap,
            ModelKey::Discounting
        ))
        .is_some());

    // Convertible
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::Convertible,
            ModelKey::Discounting
        ))
        .is_some());

    // Inflation
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::InflationSwap,
            ModelKey::Discounting
        ))
        .is_some());
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::InflationLinkedBond,
            ModelKey::Discounting
        ))
        .is_some());

    // Variance Swap
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::VarianceSwap,
            ModelKey::Discounting
        ))
        .is_some());

    // Repo
    assert!(registry
        .get_pricer(PricerKey::new(InstrumentType::Repo, ModelKey::Discounting))
        .is_some());

    // Basket
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::Basket,
            ModelKey::Discounting
        ))
        .is_some());

    // Structured Credit
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::StructuredCredit,
            ModelKey::Discounting
        ))
        .is_some());

    // Private Markets Fund
    assert!(registry
        .get_pricer(PricerKey::new(
            InstrumentType::PrivateMarketsFund,
            ModelKey::Discounting
        ))
        .is_some());
}

#[test]
fn test_expect_inst_type_mismatch() {
    // Create a bond but try to expect it as IRS
    let bond = Bond::fixed(
        "TEST_BOND",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        "USD-TREASURY",
    )
    .unwrap();

    let instrument: &dyn Instrument = &bond;

    // This should fail with TypeMismatch
    let result = expect_inst::<Bond>(instrument, InstrumentType::IRS);

    assert!(result.is_err());
    match result.unwrap_err() {
        PricingError::TypeMismatch { expected, got } => {
            assert_eq!(expected, InstrumentType::IRS);
            assert_eq!(got, InstrumentType::Bond);
        }
        _ => panic!("Expected TypeMismatch error"),
    }
}

#[test]
fn test_expect_inst_success() {
    let bond = Bond::fixed(
        "TEST_BOND",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
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
    assert_eq!(InstrumentType::Loan as u16, 2);
    assert_eq!(InstrumentType::CDS as u16, 3);
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
}
