//! Attribution JSON serialization roundtrip tests.
//!
//! Ensures attribution envelopes, configuration types, and model parameters
//! can be serialized to JSON and deserialized back without loss.

use finstack_attribution::{
    AttributionConfig, AttributionEnvelope, AttributionFactor, AttributionMethod, AttributionSpec,
    ExecutionPolicy,
};
use finstack_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_core::currency::Currency;
use finstack_core::dates::create_date;
use finstack_core::market_data::context::MarketContextState;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::convertible::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, DividendAdjustment,
};
use finstack_valuations::instruments::json_loader::InstrumentJson;
use finstack_valuations::instruments::model_params::ModelParamsSnapshot;
use finstack_valuations::instruments::Bond;
use time::Month;

#[test]
fn test_attribution_envelope_json_roundtrip() {
    let bond = Bond::fixed(
        "TEST-BOND",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        create_date(2024, Month::January, 1).unwrap(),
        create_date(2034, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .unwrap();

    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0: MarketContextState {
            version: finstack_core::market_data::context::MARKET_CONTEXT_STATE_VERSION,
            curves: vec![],
            fx: None,
            surfaces: vec![],
            prices: std::collections::BTreeMap::new(),
            series: vec![],
            inflation_indices: vec![],
            dividends: vec![],
            credit_indices: vec![],
            collateral: std::collections::BTreeMap::new(),
            fx_delta_vol_surfaces: vec![],
            hierarchy: None,
            vol_cubes: vec![],
        },
        market_t1: MarketContextState {
            version: finstack_core::market_data::context::MARKET_CONTEXT_STATE_VERSION,
            curves: vec![],
            fx: None,
            surfaces: vec![],
            prices: std::collections::BTreeMap::new(),
            series: vec![],
            inflation_indices: vec![],
            dividends: vec![],
            credit_indices: vec![],
            collateral: std::collections::BTreeMap::new(),
            fx_delta_vol_surfaces: vec![],
            hierarchy: None,
            vol_cubes: vec![],
        },
        as_of_t0: create_date(2025, Month::January, 1).unwrap(),
        as_of_t1: create_date(2025, Month::January, 2).unwrap(),
        method: AttributionMethod::Parallel,
        config: None,
        model_params_t0: None,
        credit_factor_model: None,
        credit_factor_detail_options: Default::default(),
        full_cross_attribution: false,
    };

    let envelope = AttributionEnvelope::new(spec);

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&envelope).unwrap();

    // Deserialize back
    let parsed: AttributionEnvelope = serde_json::from_str(&json).unwrap();

    // Verify schema version
    assert_eq!(parsed.schema, "finstack.attribution/1");

    // Verify dates
    assert_eq!(parsed.attribution.as_of_t0, envelope.attribution.as_of_t0);
    assert_eq!(parsed.attribution.as_of_t1, envelope.attribution.as_of_t1);

    // Verify method
    assert!(matches!(
        parsed.attribution.method,
        AttributionMethod::Parallel
    ));
}

#[test]
fn test_attribution_envelope_execute_rejects_unknown_schema() {
    let bond = Bond::fixed(
        "TEST-BOND",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        create_date(2024, Month::January, 1).unwrap(),
        create_date(2034, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .unwrap();

    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0: MarketContextState {
            version: finstack_core::market_data::context::MARKET_CONTEXT_STATE_VERSION,
            curves: vec![],
            fx: None,
            surfaces: vec![],
            prices: std::collections::BTreeMap::new(),
            series: vec![],
            inflation_indices: vec![],
            dividends: vec![],
            credit_indices: vec![],
            collateral: std::collections::BTreeMap::new(),
            fx_delta_vol_surfaces: vec![],
            hierarchy: None,
            vol_cubes: vec![],
        },
        market_t1: MarketContextState {
            version: finstack_core::market_data::context::MARKET_CONTEXT_STATE_VERSION,
            curves: vec![],
            fx: None,
            surfaces: vec![],
            prices: std::collections::BTreeMap::new(),
            series: vec![],
            inflation_indices: vec![],
            dividends: vec![],
            credit_indices: vec![],
            collateral: std::collections::BTreeMap::new(),
            fx_delta_vol_surfaces: vec![],
            hierarchy: None,
            vol_cubes: vec![],
        },
        as_of_t0: create_date(2025, Month::January, 1).unwrap(),
        as_of_t1: create_date(2025, Month::January, 2).unwrap(),
        method: AttributionMethod::Parallel,
        config: None,
        model_params_t0: None,
        credit_factor_model: None,
        credit_factor_detail_options: Default::default(),
        full_cross_attribution: false,
    };

    let mut envelope = AttributionEnvelope::new(spec);
    envelope.schema = "finstack.attribution/2".to_string();

    let error = envelope.execute().expect_err("unknown schema must fail");
    assert!(
        error.to_string().contains("Unsupported attribution schema"),
        "schema error should be explicit, got: {error}"
    );
}

#[test]
fn test_attribution_envelope_waterfall_roundtrip() {
    use finstack_attribution::AttributionFactor;

    let bond = Bond::fixed(
        "TEST-BOND",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        create_date(2024, Month::January, 1).unwrap(),
        create_date(2034, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .unwrap();

    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0: MarketContextState {
            version: finstack_core::market_data::context::MARKET_CONTEXT_STATE_VERSION,
            curves: vec![],
            fx: None,
            surfaces: vec![],
            prices: std::collections::BTreeMap::new(),
            series: vec![],
            inflation_indices: vec![],
            dividends: vec![],
            credit_indices: vec![],
            collateral: std::collections::BTreeMap::new(),
            fx_delta_vol_surfaces: vec![],
            hierarchy: None,
            vol_cubes: vec![],
        },
        market_t1: MarketContextState {
            version: finstack_core::market_data::context::MARKET_CONTEXT_STATE_VERSION,
            curves: vec![],
            fx: None,
            surfaces: vec![],
            prices: std::collections::BTreeMap::new(),
            series: vec![],
            inflation_indices: vec![],
            dividends: vec![],
            credit_indices: vec![],
            collateral: std::collections::BTreeMap::new(),
            fx_delta_vol_surfaces: vec![],
            hierarchy: None,
            vol_cubes: vec![],
        },
        as_of_t0: create_date(2025, Month::January, 1).unwrap(),
        as_of_t1: create_date(2025, Month::January, 2).unwrap(),
        method: AttributionMethod::Waterfall(vec![
            AttributionFactor::Carry,
            AttributionFactor::RatesCurves,
            AttributionFactor::CreditCurves,
        ]),
        config: None,
        model_params_t0: None,
        credit_factor_model: None,
        credit_factor_detail_options: Default::default(),
        full_cross_attribution: false,
    };

    let envelope = AttributionEnvelope::new(spec);
    let json = serde_json::to_string_pretty(&envelope).unwrap();
    let parsed: AttributionEnvelope = serde_json::from_str(&json).unwrap();

    // Verify waterfall method with correct order
    if let AttributionMethod::Waterfall(factors) = parsed.attribution.method {
        assert_eq!(factors.len(), 3);
        assert_eq!(factors[0], AttributionFactor::Carry);
        assert_eq!(factors[1], AttributionFactor::RatesCurves);
        assert_eq!(factors[2], AttributionFactor::CreditCurves);
    } else {
        panic!("Expected Waterfall method");
    }
}

#[test]
fn test_attribution_config_roundtrip() {
    let config = AttributionConfig {
        tolerance_abs: Some(0.01),
        tolerance_pct: Some(0.001),
        metrics: Some(vec!["theta".to_string(), "dv01".to_string()]),
        strict_validation: Some(false),
        rounding_scale: None,
        rate_bump_bp: None,
        target_ccy: None,
        execution_policy: Some(ExecutionPolicy::Serial),
    };

    let json = serde_json::to_string(&config).unwrap();
    let parsed: AttributionConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.tolerance_abs, Some(0.01));
    assert_eq!(parsed.tolerance_pct, Some(0.001));
    assert_eq!(parsed.metrics.as_ref().unwrap().len(), 2);
    assert_eq!(parsed.execution_policy, Some(ExecutionPolicy::Serial));
}

#[test]
fn test_attribution_envelope_from_example_json() {
    // Load the example JSON file
    let json = include_str!("json_examples/bond_attribution_parallel.example.json");

    // Parse it
    let envelope: AttributionEnvelope = serde_json::from_str(json).unwrap();

    // Verify structure
    assert_eq!(envelope.schema, "finstack.attribution/1");
    assert!(matches!(
        envelope.attribution.method,
        AttributionMethod::Parallel
    ));

    // Verify instrument
    if let InstrumentJson::Bond(bond) = &envelope.attribution.instrument {
        assert_eq!(bond.id.as_str(), "CORP-BOND-001");
        assert_eq!(bond.notional.currency(), Currency::USD);
    } else {
        panic!("Expected Bond instrument");
    }
}

#[test]
fn test_attribution_envelope_to_from_json_helpers() {
    let bond = Bond::fixed(
        "TEST-BOND",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        create_date(2024, Month::January, 1).unwrap(),
        create_date(2034, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .unwrap();

    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0: MarketContextState {
            version: finstack_core::market_data::context::MARKET_CONTEXT_STATE_VERSION,
            curves: vec![],
            fx: None,
            surfaces: vec![],
            prices: std::collections::BTreeMap::new(),
            series: vec![],
            inflation_indices: vec![],
            dividends: vec![],
            credit_indices: vec![],
            collateral: std::collections::BTreeMap::new(),
            fx_delta_vol_surfaces: vec![],
            hierarchy: None,
            vol_cubes: vec![],
        },
        market_t1: MarketContextState {
            version: finstack_core::market_data::context::MARKET_CONTEXT_STATE_VERSION,
            curves: vec![],
            fx: None,
            surfaces: vec![],
            prices: std::collections::BTreeMap::new(),
            series: vec![],
            inflation_indices: vec![],
            dividends: vec![],
            credit_indices: vec![],
            collateral: std::collections::BTreeMap::new(),
            fx_delta_vol_surfaces: vec![],
            hierarchy: None,
            vol_cubes: vec![],
        },
        as_of_t0: create_date(2025, Month::January, 1).unwrap(),
        as_of_t1: create_date(2025, Month::January, 2).unwrap(),
        method: AttributionMethod::MetricsBased,
        config: None,
        model_params_t0: None,
        credit_factor_model: None,
        credit_factor_detail_options: Default::default(),
        full_cross_attribution: false,
    };

    let envelope = AttributionEnvelope::new(spec);

    let json_str = serde_json::to_string_pretty(&envelope).unwrap();
    let parsed = serde_json::from_str::<AttributionEnvelope>(&json_str).unwrap();

    assert_eq!(parsed.schema, envelope.schema);
    assert!(matches!(
        parsed.attribution.method,
        AttributionMethod::MetricsBased
    ));
}

#[test]
fn test_attribution_result_envelope_roundtrip() {
    use finstack_attribution::{AttributionResult, AttributionResultEnvelope, PnlAttribution};
    use finstack_core::config::results_meta;

    let total = Money::new(1000.0, Currency::USD);
    let pnl_attr = PnlAttribution::new(
        total,
        "TEST-BOND",
        create_date(2025, Month::January, 1).unwrap(),
        create_date(2025, Month::January, 2).unwrap(),
        AttributionMethod::Parallel,
    );

    let result = AttributionResult {
        attribution: pnl_attr,
        results_meta: results_meta(&finstack_core::config::FinstackConfig::default()),
    };

    let envelope = AttributionResultEnvelope::new(result);
    let json_str = serde_json::to_string_pretty(&envelope).unwrap();
    let parsed = serde_json::from_str::<AttributionResultEnvelope>(&json_str).unwrap();

    assert_eq!(parsed.schema, "finstack.attribution/1");
    assert_eq!(parsed.result.attribution.total_pnl, total);
}

// =============================================================================
// Attribution Type Serialization Tests
// =============================================================================

#[test]
fn test_attribution_method_parallel_roundtrip() {
    let method = AttributionMethod::Parallel;
    let json = serde_json::to_string(&method).unwrap();
    let deserialized: AttributionMethod = serde_json::from_str(&json).unwrap();

    assert!(matches!(deserialized, AttributionMethod::Parallel));
}

#[test]
fn test_attribution_method_waterfall_roundtrip() {
    let method = AttributionMethod::Waterfall(vec![
        AttributionFactor::Carry,
        AttributionFactor::RatesCurves,
        AttributionFactor::CreditCurves,
    ]);

    let json = serde_json::to_string(&method).unwrap();
    let deserialized: AttributionMethod = serde_json::from_str(&json).unwrap();

    if let AttributionMethod::Waterfall(factors) = deserialized {
        assert_eq!(factors.len(), 3);
        assert_eq!(factors[0], AttributionFactor::Carry);
        assert_eq!(factors[1], AttributionFactor::RatesCurves);
        assert_eq!(factors[2], AttributionFactor::CreditCurves);
    } else {
        panic!("Expected Waterfall variant");
    }
}

#[test]
fn test_attribution_method_metrics_based_roundtrip() {
    let method = AttributionMethod::MetricsBased;
    let json = serde_json::to_string(&method).unwrap();
    let deserialized: AttributionMethod = serde_json::from_str(&json).unwrap();

    assert!(matches!(deserialized, AttributionMethod::MetricsBased));
}

#[test]
fn test_execution_policy_roundtrip() {
    let policy = ExecutionPolicy::Serial;
    let json = serde_json::to_string(&policy).unwrap();
    assert_eq!(json, "\"serial\"");

    let deserialized: ExecutionPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, ExecutionPolicy::Serial);
    assert_eq!(ExecutionPolicy::default(), ExecutionPolicy::Parallel);
}

#[test]
fn test_attribution_factor_roundtrip() {
    let factors = vec![
        AttributionFactor::Carry,
        AttributionFactor::RatesCurves,
        AttributionFactor::CreditCurves,
        AttributionFactor::InflationCurves,
        AttributionFactor::Correlations,
        AttributionFactor::Fx,
        AttributionFactor::Volatility,
        AttributionFactor::ModelParameters,
        AttributionFactor::MarketScalars,
    ];

    for factor in factors {
        let json = serde_json::to_string(&factor).unwrap();
        let deserialized: AttributionFactor = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, factor);
    }
}

#[test]
fn test_model_params_snapshot_structured_credit_roundtrip() {
    let snapshot = ModelParamsSnapshot::StructuredCredit {
        prepayment_spec: PrepaymentModelSpec::psa(1.5),
        default_spec: DefaultModelSpec::constant_cdr(0.02),
        recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
    };

    let json = serde_json::to_string(&snapshot).unwrap();
    let deserialized: ModelParamsSnapshot = serde_json::from_str(&json).unwrap();

    if let ModelParamsSnapshot::StructuredCredit {
        prepayment_spec,
        default_spec,
        recovery_spec,
    } = deserialized
    {
        // Verify prepayment
        assert_eq!(prepayment_spec.cpr, PrepaymentModelSpec::psa(1.5).cpr);

        // Verify default
        assert_eq!(default_spec.cdr, 0.02);

        // Verify recovery
        assert_eq!(recovery_spec.rate, 0.60);
        assert_eq!(recovery_spec.recovery_lag, 12);
    } else {
        panic!("Expected StructuredCredit variant");
    }
}

#[test]
fn test_model_params_snapshot_convertible_roundtrip() {
    let conversion_spec = ConversionSpec {
        ratio: Some(25.0),
        price: None,
        policy: ConversionPolicy::Voluntary,
        anti_dilution: AntiDilutionPolicy::None,
        dividend_adjustment: DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let snapshot = ModelParamsSnapshot::Convertible { conversion_spec };

    let json = serde_json::to_string(&snapshot).unwrap();
    let deserialized: ModelParamsSnapshot = serde_json::from_str(&json).unwrap();

    if let ModelParamsSnapshot::Convertible {
        conversion_spec: cs,
    } = deserialized
    {
        assert_eq!(cs.ratio, Some(25.0));
        assert_eq!(cs.price, None);
    } else {
        panic!("Expected Convertible variant");
    }
}

#[test]
fn test_model_params_snapshot_none_roundtrip() {
    let snapshot = ModelParamsSnapshot::None;

    let json = serde_json::to_string(&snapshot).unwrap();
    let deserialized: ModelParamsSnapshot = serde_json::from_str(&json).unwrap();

    assert!(matches!(deserialized, ModelParamsSnapshot::None));
}

#[test]
fn test_attribution_method_json_structure() {
    // Verify the JSON structure for waterfall matches expected shape
    let method = AttributionMethod::Waterfall(vec![
        AttributionFactor::Carry,
        AttributionFactor::RatesCurves,
    ]);

    let json = serde_json::to_value(&method).unwrap();

    // Should be a tagged enum with "Waterfall" key
    assert!(json.is_object());
    assert!(json.get("Waterfall").is_some());

    // The waterfall value should be an array of factors
    let factors = json.get("Waterfall").unwrap();
    assert!(factors.is_array());
    assert_eq!(factors.as_array().unwrap().len(), 2);
}

#[test]
fn test_model_params_snapshot_json_structure() {
    let snapshot = ModelParamsSnapshot::StructuredCredit {
        prepayment_spec: PrepaymentModelSpec::psa(1.0),
        default_spec: DefaultModelSpec::constant_cdr(0.02),
        recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
    };

    let json = serde_json::to_value(&snapshot).unwrap();

    // Should be a tagged enum
    assert!(json.is_object());
    assert!(json.get("StructuredCredit").is_some());

    // Should contain the three spec fields
    let structured = json.get("StructuredCredit").unwrap();
    assert!(structured.get("prepayment_spec").is_some());
    assert!(structured.get("default_spec").is_some());
    assert!(structured.get("recovery_spec").is_some());
}

/// Quant review M10/tests-5: serde roundtrip with EVERY optional detail field
/// POPULATED, including the formerly tuple-keyed maps (`by_tenor`, `by_pair`)
/// that plain derived `Serialize` could not represent in JSON at all
/// ("key must be a string" at runtime). Pins the string-keyed wire format:
/// `"{curve_id}|{tenor}"` and `"{FROM}/{TO}"`.
#[test]
fn test_populated_detail_fields_roundtrip_through_json() {
    use finstack_attribution::{
        CorrelationsAttribution, CreditCurvesAttribution, CrossFactorDetail, FxAttribution,
        InflationCurvesAttribution, RatesCurvesAttribution, VolAttribution,
    };
    use finstack_core::types::CurveId;
    use indexmap::IndexMap;

    let usd = |v: f64| Money::new(v, Currency::USD);

    let mut attr = finstack_attribution::PnlAttribution::new(
        usd(1000.0),
        "DETAIL-BOND",
        create_date(2025, Month::January, 1).unwrap(),
        create_date(2025, Month::January, 2).unwrap(),
        AttributionMethod::Parallel,
    );

    let mut rates_by_curve = IndexMap::new();
    rates_by_curve.insert(CurveId::new("USD-OIS"), usd(50.0));
    let mut rates_by_tenor = IndexMap::new();
    rates_by_tenor.insert((CurveId::new("USD-OIS"), "5Y".to_string()), usd(30.0));
    rates_by_tenor.insert((CurveId::new("USD-OIS"), "10Y".to_string()), usd(20.0));
    attr.rates_detail = Some(RatesCurvesAttribution {
        by_curve: rates_by_curve,
        by_tenor: rates_by_tenor,
        discount_total: usd(50.0),
        forward_total: usd(0.0),
    });

    let mut credit_by_curve = IndexMap::new();
    credit_by_curve.insert(CurveId::new("ACME-HAZ"), usd(-12.0));
    let mut credit_by_tenor = IndexMap::new();
    credit_by_tenor.insert((CurveId::new("ACME-HAZ"), "5Y".to_string()), usd(-12.0));
    attr.credit_detail = Some(CreditCurvesAttribution {
        by_curve: credit_by_curve,
        by_tenor: credit_by_tenor,
    });

    let mut infl_by_curve = IndexMap::new();
    infl_by_curve.insert(CurveId::new("US-CPI"), usd(3.0));
    let mut infl_by_tenor = IndexMap::new();
    infl_by_tenor.insert((CurveId::new("US-CPI"), "5Y".to_string()), usd(3.0));
    attr.inflation_detail = Some(InflationCurvesAttribution {
        by_curve: infl_by_curve,
        by_tenor: Some(infl_by_tenor),
    });

    let mut corr_by_curve = IndexMap::new();
    corr_by_curve.insert(CurveId::new("CDX-BASE-CORR"), usd(1.5));
    attr.correlations_detail = Some(CorrelationsAttribution {
        by_curve: corr_by_curve,
    });

    let mut fx_by_pair = IndexMap::new();
    fx_by_pair.insert((Currency::EUR, Currency::USD), usd(7.0));
    attr.fx_detail = Some(FxAttribution {
        by_pair: fx_by_pair,
    });

    let mut vol_by_surface = IndexMap::new();
    vol_by_surface.insert(CurveId::new("SPX-VOL"), usd(-4.0));
    attr.vol_detail = Some(VolAttribution {
        by_surface: vol_by_surface,
    });

    let mut cross_by_pair = IndexMap::new();
    cross_by_pair.insert("Rates×Credit".to_string(), usd(-2.0));
    attr.cross_factor_detail = Some(CrossFactorDetail {
        total: usd(-2.0),
        by_pair: cross_by_pair,
    });

    // Serialization must SUCCEED (the old tuple-keyed derive failed here) and
    // the string keys must follow the documented format.
    let json = serde_json::to_string(&attr).expect("populated detail must serialize to JSON");
    assert!(json.contains("USD-OIS|5Y"), "tenor keys use 'curve|tenor'");
    assert!(json.contains("EUR/USD"), "fx keys use 'FROM/TO'");

    let parsed: finstack_attribution::PnlAttribution =
        serde_json::from_str(&json).expect("roundtrip parse");
    let rates = parsed.rates_detail.expect("rates detail");
    assert_eq!(
        rates.by_tenor[&(CurveId::new("USD-OIS"), "5Y".to_string())],
        usd(30.0)
    );
    assert_eq!(
        parsed.credit_detail.expect("credit detail").by_tenor
            [&(CurveId::new("ACME-HAZ"), "5Y".to_string())],
        usd(-12.0)
    );
    assert_eq!(
        parsed
            .inflation_detail
            .expect("inflation detail")
            .by_tenor
            .expect("inflation by_tenor")[&(CurveId::new("US-CPI"), "5Y".to_string())],
        usd(3.0)
    );
    assert_eq!(
        parsed.fx_detail.expect("fx detail").by_pair[&(Currency::EUR, Currency::USD)],
        usd(7.0)
    );
    assert_eq!(
        parsed.vol_detail.expect("vol detail").by_surface[&CurveId::new("SPX-VOL")],
        usd(-4.0)
    );
}
