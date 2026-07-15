//! Tests for instrument-level shock adapters.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_scenarios::{
    ExecutionContext, InstrumentType, OperationSpec, ScenarioEngine, ScenarioSpec,
};
use finstack_quant_statements::FinancialModelSpec;
use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations::instruments::{Attributes, Bond, Instrument};
use indexmap::IndexMap;
use time::Month;

#[test]
fn test_instrument_type_price_shock_matching() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    // Create test instruments
    let mut instruments: Vec<Box<dyn Instrument>> = vec![
        Box::new(
            Bond::builder()
                .id("BOND1".into())
                .notional(finstack_quant_core::money::Money::new(100.0, Currency::USD))
                .issue_date(base_date)
                .maturity(base_date + time::Duration::days(365))
                .cashflow_spec(
                    CashflowSpec::fixed(
                        0.05,
                        finstack_quant_core::dates::Tenor::annual(),
                        finstack_quant_core::dates::DayCount::Thirty360,
                    )
                    .expect("finite test coupon"),
                )
                .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
                .credit_curve_id_opt(None)
                .instrument_pricing_overrides(InstrumentPricingOverrides::default())
                .attributes(Attributes::new())
                .build()
                .unwrap(),
        ),
        Box::new(
            Bond::builder()
                .id("BOND2".into())
                .notional(finstack_quant_core::money::Money::new(100.0, Currency::USD))
                .issue_date(base_date)
                .maturity(base_date + time::Duration::days(730))
                .cashflow_spec(
                    CashflowSpec::fixed(
                        0.04,
                        finstack_quant_core::dates::Tenor::annual(),
                        finstack_quant_core::dates::DayCount::Thirty360,
                    )
                    .expect("finite test coupon"),
                )
                .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
                .credit_curve_id_opt(None)
                .instrument_pricing_overrides(InstrumentPricingOverrides::default())
                .attributes(Attributes::new())
                .build()
                .unwrap(),
        ),
    ];

    let scenario = ScenarioSpec {
        id: "bond_price_shock".into(),
        name: Some("Bond Price Shock".into()),
        description: None,
        operations: vec![OperationSpec::InstrumentPricePctByType {
            instrument_types: vec![InstrumentType::Bond],
            pct: -5.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 2, "Should shock 2 bonds");

    // Verify shock was applied via scenario_overrides (for instruments that support it)
    // or metadata (for instruments that don't)
    for instrument in &instruments {
        // Bond supports get_scenario_pricing_overrides_mut(), so check there
        if let Some(overrides) = instrument.get_scenario_pricing_overrides() {
            assert!(
                overrides.scenario_price_shock_pct.is_some(),
                "scenario_price_shock_pct should be set in pricing_overrides"
            );
            let shock = overrides.scenario_price_shock_pct.unwrap();
            assert!(
                (shock - (-0.05)).abs() < 1e-6,
                "Expected -0.05 decimal, got {}",
                shock
            );
        } else {
            // Fallback for instruments without scenario_overrides
            let meta = &instrument.attributes().meta;
            assert!(meta.contains_key("scenario_price_shock_pct"));
        }
    }
}

#[test]
fn test_instrument_type_spread_shock_matching() {
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let mut instruments: Vec<Box<dyn Instrument>> = vec![Box::new(
        Bond::builder()
            .id("BOND1".into())
            .notional(finstack_quant_core::money::Money::new(100.0, Currency::USD))
            .issue_date(base_date)
            .maturity(base_date + time::Duration::days(365))
            .cashflow_spec(
                CashflowSpec::fixed(
                    0.05,
                    finstack_quant_core::dates::Tenor::annual(),
                    finstack_quant_core::dates::DayCount::Thirty360,
                )
                .expect("finite test coupon"),
            )
            .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .unwrap(),
    )];

    let scenario = ScenarioSpec {
        id: "bond_spread_shock".into(),
        name: Some("Bond Spread Shock".into()),
        description: None,
        operations: vec![OperationSpec::InstrumentSpreadBpByType {
            instrument_types: vec![InstrumentType::Bond],
            bp: 100.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 1);

    // Vanilla bonds consume the spread shock via the typed scenario override
    // (applied as an additional flat Z-spread at valuation); no metadata
    // fallback and no fallback warning.
    let overrides = instruments[0]
        .get_scenario_pricing_overrides()
        .expect("bond exposes scenario overrides");
    let stored = overrides
        .scenario_spread_shock_bp
        .expect("spread shock should be stored in scenario overrides");
    assert!(
        (stored - 100.0).abs() < 1e-6,
        "Expected 100.0 bp in scenario overrides, got {stored}"
    );
    assert!(
        !instruments[0]
            .attributes()
            .meta
            .contains_key("scenario_spread_shock_bp"),
        "first-class path must not also write the metadata fallback"
    );
    assert!(
        report.warnings.is_empty(),
        "no fallback warning expected, got {:?}",
        report.warnings
    );
}

/// The spread shock must move PV: a +100bp flat Z-spread widening lowers a
/// vanilla bond's value, and the magnitude is consistent with discounting at
/// curve + 100bp.
#[test]
fn test_instrument_spread_shock_moves_bond_pv() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;

    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.96), (5.0, 0.82)])
        .build()
        .unwrap();
    let mut market = MarketContext::new().insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let bond = Bond::builder()
        .id("BOND1".into())
        .notional(finstack_quant_core::money::Money::new(
            1_000_000.0,
            Currency::USD,
        ))
        .issue_date(base_date)
        .maturity(base_date + time::Duration::days(1825))
        .cashflow_spec(
            CashflowSpec::fixed(
                0.05,
                finstack_quant_core::dates::Tenor::annual(),
                finstack_quant_core::dates::DayCount::Thirty360,
            )
            .expect("finite test coupon"),
        )
        .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
        .credit_curve_id_opt(None)
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let pv_base = bond.value(&market, base_date).expect("base PV").amount();

    let mut instruments: Vec<Box<dyn Instrument>> = vec![Box::new(bond)];
    let scenario = ScenarioSpec {
        id: "spread_widening".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::InstrumentSpreadBpByType {
            instrument_types: vec![InstrumentType::Bond],
            bp: 100.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };
    engine.apply(&scenario, &mut ctx).unwrap();

    let pv_shocked = instruments[0]
        .value(&market, base_date)
        .expect("shocked PV")
        .amount();

    assert!(
        pv_shocked < pv_base,
        "+100bp spread widening must lower PV: base {pv_base}, shocked {pv_shocked}"
    );
    // ~5y bond: 100bp widening should move PV by roughly 4-5% of notional;
    // assert a loose band so the test pins economics without overfitting.
    let drop = pv_base - pv_shocked;
    assert!(
        drop > 20_000.0 && drop < 60_000.0,
        "PV drop {drop} outside plausible 100bp spread-duration band"
    );
}

#[test]
fn test_instrument_attr_price_shock_matching() {
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let mut instruments: Vec<Box<dyn Instrument>> = vec![
        Box::new(
            Bond::builder()
                .id("ENERGY_BBB".into())
                .notional(finstack_quant_core::money::Money::new(100.0, Currency::USD))
                .issue_date(base_date)
                .maturity(base_date + time::Duration::days(365))
                .cashflow_spec(
                    CashflowSpec::fixed(
                        0.05,
                        finstack_quant_core::dates::Tenor::annual(),
                        finstack_quant_core::dates::DayCount::Thirty360,
                    )
                    .expect("finite test coupon"),
                )
                .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
                .credit_curve_id_opt(None)
                .instrument_pricing_overrides(InstrumentPricingOverrides::default())
                .attributes(
                    Attributes::new()
                        .with_meta("sector", "Energy")
                        .with_meta("rating", "BBB"),
                )
                .build()
                .unwrap(),
        ),
        Box::new(
            Bond::builder()
                .id("TECH_AA".into())
                .notional(finstack_quant_core::money::Money::new(100.0, Currency::USD))
                .issue_date(base_date)
                .maturity(base_date + time::Duration::days(365))
                .cashflow_spec(
                    CashflowSpec::fixed(
                        0.05,
                        finstack_quant_core::dates::Tenor::annual(),
                        finstack_quant_core::dates::DayCount::Thirty360,
                    )
                    .expect("finite test coupon"),
                )
                .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
                .credit_curve_id_opt(None)
                .instrument_pricing_overrides(InstrumentPricingOverrides::default())
                .attributes(
                    Attributes::new()
                        .with_meta("sector", "Technology")
                        .with_meta("rating", "AA"),
                )
                .build()
                .unwrap(),
        ),
    ];

    let mut attrs = IndexMap::new();
    attrs.insert("SECTOR".into(), "energy".into()); // case-insensitive match
    attrs.insert("rating".into(), "bbb".into());

    let scenario = ScenarioSpec {
        id: "attr_price_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::InstrumentPricePctByAttr { attrs, pct: -4.0 }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 1);

    let first_overrides = instruments[0]
        .get_scenario_pricing_overrides()
        .and_then(|o| o.scenario_price_shock_pct);
    assert_eq!(first_overrides, Some(-0.04));

    let second_overrides = instruments[1]
        .get_scenario_pricing_overrides()
        .and_then(|o| o.scenario_price_shock_pct);
    assert_eq!(second_overrides, None);
}

#[test]
fn test_instrument_attr_price_shock_no_matches() {
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let mut instruments: Vec<Box<dyn Instrument>> = vec![Box::new(
        Bond::builder()
            .id("ENERGY_BBB".into())
            .notional(finstack_quant_core::money::Money::new(100.0, Currency::USD))
            .issue_date(base_date)
            .maturity(base_date + time::Duration::days(365))
            .cashflow_spec(
                CashflowSpec::fixed(
                    0.05,
                    finstack_quant_core::dates::Tenor::annual(),
                    finstack_quant_core::dates::DayCount::Thirty360,
                )
                .expect("finite test coupon"),
            )
            .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .attributes(Attributes::new().with_meta("sector", "Energy"))
            .build()
            .unwrap(),
    )];

    let mut attrs = IndexMap::new();
    attrs.insert("sector".into(), "Utilities".into());

    let scenario = ScenarioSpec {
        id: "attr_price_shock_none".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::InstrumentPricePctByAttr { attrs, pct: -4.0 }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 0);
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0]
        .to_string()
        .contains("No instruments matched attribute filter"));
}

#[test]
fn test_instrument_shock_empty_list() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let mut instruments: Vec<Box<dyn Instrument>> = vec![];

    let scenario = ScenarioSpec {
        id: "empty_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::InstrumentPricePctByType {
            instrument_types: vec![InstrumentType::Bond],
            pct: -5.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 0, "No instruments to shock");
}

#[test]
fn test_instrument_shock_no_matching_types() {
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let mut instruments: Vec<Box<dyn Instrument>> = vec![Box::new(
        Bond::builder()
            .id("BOND1".into())
            .notional(finstack_quant_core::money::Money::new(100.0, Currency::USD))
            .issue_date(base_date)
            .maturity(base_date + time::Duration::days(365))
            .cashflow_spec(
                CashflowSpec::fixed(
                    0.05,
                    finstack_quant_core::dates::Tenor::annual(),
                    finstack_quant_core::dates::DayCount::Thirty360,
                )
                .expect("finite test coupon"),
            )
            .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .unwrap(),
    )];

    let scenario = ScenarioSpec {
        id: "no_match_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::InstrumentPricePctByType {
            instrument_types: vec![InstrumentType::CDS], // Looking for CDS, have Bond
            pct: -5.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 0, "No CDS instruments to shock");
}

#[test]
fn test_instrument_shock_without_instruments_provided() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "no_instruments".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::InstrumentPricePctByType {
            instrument_types: vec![InstrumentType::Bond],
            pct: -5.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None, // No instruments provided
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 0);
    assert!(!report.warnings.is_empty(), "Should have warning");
    assert!(report.warnings[0]
        .to_string()
        .contains("no instruments provided"));
}

#[test]
fn test_instrument_shock_multiple_types() {
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let mut instruments: Vec<Box<dyn Instrument>> = vec![
        Box::new(
            Bond::builder()
                .id("BOND1".into())
                .notional(finstack_quant_core::money::Money::new(100.0, Currency::USD))
                .issue_date(base_date)
                .maturity(base_date + time::Duration::days(365))
                .cashflow_spec(
                    CashflowSpec::fixed(
                        0.05,
                        finstack_quant_core::dates::Tenor::annual(),
                        finstack_quant_core::dates::DayCount::Thirty360,
                    )
                    .expect("finite test coupon"),
                )
                .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
                .credit_curve_id_opt(None)
                .instrument_pricing_overrides(InstrumentPricingOverrides::default())
                .attributes(Attributes::new())
                .build()
                .unwrap(),
        ),
        Box::new(
            Bond::builder()
                .id("BOND2".into())
                .notional(finstack_quant_core::money::Money::new(100.0, Currency::USD))
                .issue_date(base_date)
                .maturity(base_date + time::Duration::days(730))
                .cashflow_spec(
                    CashflowSpec::fixed(
                        0.04,
                        finstack_quant_core::dates::Tenor::annual(),
                        finstack_quant_core::dates::DayCount::Thirty360,
                    )
                    .expect("finite test coupon"),
                )
                .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
                .credit_curve_id_opt(None)
                .instrument_pricing_overrides(InstrumentPricingOverrides::default())
                .attributes(Attributes::new())
                .build()
                .unwrap(),
        ),
    ];

    let scenario = ScenarioSpec {
        id: "multi_type_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::InstrumentPricePctByType {
            instrument_types: vec![InstrumentType::Bond, InstrumentType::Loan],
            pct: -10.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 2, "Both bonds should be shocked");
}

#[test]
fn test_empty_attr_filter_is_rejected() {
    // Empty `attrs` would silently match every instrument, hiding user intent.
    // Validation now rejects it at engine entry; callers should use
    // `InstrumentPricePctByType` when they mean "all instruments of type T".
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);
    let mut instruments: Vec<Box<dyn Instrument>> = vec![];

    let scenario = ScenarioSpec {
        id: "wildcard_attrs".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::InstrumentPricePctByAttr {
            attrs: IndexMap::new(),
            pct: -2.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let err = engine
        .apply(&scenario, &mut ctx)
        .expect_err("empty attrs must be rejected by validation");
    assert!(err.to_string().contains("attrs must not be empty"));
}

#[test]
fn test_attr_filter_ignores_tags_uses_meta_only() {
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let mut instruments: Vec<Box<dyn Instrument>> = vec![Box::new(
        Bond::builder()
            .id("TAGONLY".into())
            .notional(finstack_quant_core::money::Money::new(100.0, Currency::USD))
            .issue_date(base_date)
            .maturity(base_date + time::Duration::days(365))
            .cashflow_spec(
                CashflowSpec::fixed(
                    0.05,
                    finstack_quant_core::dates::Tenor::annual(),
                    finstack_quant_core::dates::DayCount::Thirty360,
                )
                .expect("finite test coupon"),
            )
            .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .attributes(Attributes::new().with_tag("Energy"))
            .build()
            .unwrap(),
    )];

    let mut attrs = IndexMap::new();
    attrs.insert("sector".into(), "Energy".into());

    let scenario = ScenarioSpec {
        id: "meta_only".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::InstrumentPricePctByAttr { attrs, pct: -3.0 }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 0);
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.to_string().contains("No instruments matched")),
        "expected no match when only tags overlap: {:?}",
        report.warnings
    );
}
