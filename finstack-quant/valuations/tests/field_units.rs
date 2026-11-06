//! Field-unit tables (`finstack_quant_valuations::results::*_field_units`) list
//! every field of the result types they describe, in struct order. The results
//! live in the family crates, so these checks run here rather than in
//! valuations-core.

use finstack_quant_core::currency::Currency;
use finstack_quant_valuations::results::*;
use serde_json::Value;

fn bond_quote_set()
-> finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::BondQuoteSet{
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::compute_quotes;
    use finstack_quant_valuations::instruments::{BondQuoteInput, PricingOptions};

    let bond = Bond::example().expect("bond");
    let as_of = bond.issue_date;
    let curve = DiscountCurve::builder(bond.discount_curve_id.as_str())
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.70)])
        .build()
        .expect("curve");
    compute_quotes(
        &bond,
        &MarketContext::new().insert(curve),
        as_of,
        BondQuoteInput::CleanPricePct(98.5),
        PricingOptions::default(),
    )
    .expect("set")
}

#[test]
fn bond_quote_set_field_units_list_every_field_in_struct_order() {
    // Text, not a Value: a struct writes its fields in the order it declares them.
    let wire = serde_json::to_string(&bond_quote_set()).expect("serialize");
    let keys: Value = serde_json::from_str(&wire).expect("parse");
    let units = bond_quote_set_field_units(Currency::EUR);
    assert_eq!(units.len(), keys.as_object().expect("object").len());
    let positions: Vec<usize> = units
        .iter()
        .map(|unit| {
            wire.find(&format!("\"{}\":", unit.field))
                .unwrap_or_else(|| panic!("{} is on the wire", unit.field))
        })
        .collect();
    assert!(positions.is_sorted(), "struct order: {positions:?}");
    assert_eq!(units[0].label, "EUR");
    assert_eq!(units[1].label, "% of par");
    assert_eq!(units[1].source, "valuations::BondQuoteSet::clean_price_pct");
}

#[test]
fn pool_report_field_units_list_every_numeric_field_in_struct_order() {
    use finstack_quant_valuations_structured_credit::structured_credit::{
        AssetPool, DealType, PoolAsset, pool_report,
    };
    let as_of = time::macros::date!(2025 - 01 - 01);
    let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::EUR);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "A",
        finstack_quant_core::money::Money::new(100.0, Currency::EUR).expect("money"),
        0.05,
        time::macros::date!(2030 - 01 - 01),
        finstack_quant_core::dates::DayCount::Thirty360,
    ));
    let report =
        serde_json::to_value(pool_report(&pool, as_of).expect("report")).expect("serialize");
    let wire_fields = |value: &Value, skip: &[&str]| -> Vec<String> {
        value
            .as_object()
            .expect("object")
            .keys()
            .filter(|key| !skip.contains(&key.as_str()))
            .cloned()
            .collect()
    };
    let names = |rows: &[MetricFieldUnit]| -> Vec<String> {
        rows.iter().map(|row| row.field.clone()).collect()
    };
    let sorted = |mut fields: Vec<String>| {
        fields.sort();
        fields
    };

    let units = pool_report_field_units(Currency::EUR);
    assert_eq!(
        sorted(names(&units.stats)),
        sorted(wire_fields(&report["stats"], &[]))
    );
    assert_eq!(
        sorted(names(&units.concentration)),
        sorted(wire_fields(&report["by_obligor"][0], &["key"]))
    );
    assert_eq!(units.total_balance.label, "EUR");
    assert_eq!(units.warf.source, "valuations::PoolReport::warf");
    assert_eq!(names(&units.stats)[0], "wac");
    assert_eq!(units.stats[0].label, "annual decimal, fixed-rate only");
    assert_eq!(units.stats[1].label, "bp");
    assert_eq!(units.stats[6].label, "% of current pool balance");
    assert_eq!(units.stats[7].label, "EUR");
}

#[test]
fn tranche_field_units_list_every_scalar_field_in_struct_order() {
    use finstack_quant_valuations_structured_credit::structured_credit::TrancheMetrics;
    let fields = [
        "pv",
        "clean_price_pct",
        "dirty_price_pct",
        "accrued",
        "factor",
        "wal",
        "ytm",
        "z_spread_bp",
        "cs01",
        "spread_duration",
        "spread_convexity",
        "effective_duration",
        "effective_convexity",
        "target_price_pct",
        "wal_to_call",
        "z_spread_to_call_bp",
        "dm_to_call_bp",
        "dm_bp",
    ];
    let mut wire: serde_json::Map<String, Value> = fields
        .iter()
        .map(|field| ((*field).to_string(), serde_json::json!(1.0)))
        .collect();
    wire.insert("tranche_id".to_string(), serde_json::json!("A"));
    wire.insert("currency".to_string(), serde_json::json!("USD"));
    // Strict deserialization proves these are the struct's fields; serializing
    // lists them in declaration order, so the table cannot drift unnoticed.
    let metrics: TrancheMetrics =
        serde_json::from_value(Value::Object(wire)).expect("TrancheMetrics fields");
    let mut declared: Vec<String> = serde_json::to_value(&metrics)
        .expect("serialize")
        .as_object()
        .expect("object")
        .keys()
        .filter(|key| !["tranche_id", "currency", "metrics"].contains(&key.as_str()))
        .cloned()
        .collect();
    declared.sort();
    let units = tranche_metric_field_units(Currency::EUR);
    let mut listed: Vec<String> = units.iter().map(|unit| unit.field.clone()).collect();
    assert_eq!(listed, fields);
    listed.sort();
    assert_eq!(listed, declared);
}

#[test]
fn oas_field_units_list_every_oas_result_field_in_struct_order() {
    let result =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::OasResult {
            oas: 0.01,
            model_price: 99.0,
            market_price: 99.5,
            num_paths: 100,
            price_std_error: 0.1,
        };
    let wire = serde_json::to_value(&result).expect("serialize");
    let mut declared: Vec<&str> = wire
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    declared.sort_unstable();
    let units = tranche_solve_field_units();
    let mut listed: Vec<&str> = units.oas.iter().map(|unit| unit.field.as_str()).collect();
    assert_eq!(listed[0], "oas");
    listed.sort_unstable();
    assert_eq!(listed, declared);
    assert_eq!(units.oas[0].source, "valuations::OasResult::oas");
    assert_eq!(units.breakeven_cdr.label, "decimal");
    assert_eq!(
        units.discount_margin.source,
        "valuations::tranche_discount_margin"
    );
}

#[test]
fn scenario_table_units_list_every_cell_field_in_struct_order() {
    use finstack_quant_valuations_structured_credit::structured_credit::ScenarioCell;
    let cell = ScenarioCell {
        cpr: 0.0,
        cdr: 0.0,
        severity: 0.0,
        price: 0.0,
        wal: 0.0,
        writedown: 0.0,
    };
    let value = serde_json::to_value(&cell).expect("serialize");
    let mut declared: Vec<&str> = value
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    declared.sort_unstable();
    let units = scenario_table_field_units(Currency::EUR);
    let order: Vec<&str> = units
        .axes
        .iter()
        .chain(&units.measures)
        .map(|unit| unit.field.as_str())
        .collect();
    assert_eq!(
        order,
        ["cpr", "cdr", "severity", "price", "wal", "writedown"]
    );
    let mut listed = order;
    listed.sort_unstable();
    assert_eq!(listed, declared);
    assert_eq!(units.measures[0].label, "clean, % of current balance");
    assert_eq!(units.measures[2].label, "EUR");
    assert_eq!(units.measures[1].source, "valuations::ScenarioCell::wal");
}

#[test]
fn equity_units_name_every_numeric_field_of_the_result() {
    use finstack_quant_valuations_structured_credit::structured_credit::EquityMetrics;
    let metrics = EquityMetrics {
        tranche_id: "E".to_string(),
        currency: Currency::EUR,
        invested: 0.0,
        irr: Some(0.0),
        moic: Some(0.0),
        nav_pct: 0.0,
        cash_on_cash: Vec::new(),
    };
    let mut declared: Vec<String> = serde_json::to_value(&metrics)
        .expect("serialize")
        .as_object()
        .expect("object")
        .iter()
        .filter(|(_, field)| field.is_number() || field.is_array())
        .map(|(name, _)| name.clone())
        .collect();
    declared.sort_unstable();
    let units =
        serde_json::to_value(equity_metrics_field_units(Currency::EUR)).expect("serialize units");
    let mut listed: Vec<String> = units
        .as_object()
        .expect("object")
        .iter()
        // The argument the result depends on, not a field of it.
        .filter(|(name, _)| name.as_str() != "purchase_price")
        .map(|(name, unit)| {
            assert_eq!(unit["field"], name.as_str());
            assert_eq!(unit["source"], format!("valuations::EquityMetrics::{name}"));
            name.clone()
        })
        .collect();
    listed.sort_unstable();
    assert_eq!(listed, declared);
    assert_eq!(units["invested"]["label"], "EUR");
    assert_eq!(
        units["purchase_price"]["label"],
        "% of current balance; omitted means par"
    );
}

#[test]
fn simulation_units_list_every_coverage_field_in_struct_order() {
    use finstack_quant_valuations_structured_credit::structured_credit::CoverageTestDiagnostic;
    let diagnostic = CoverageTestDiagnostic {
        test_id: "OC_A".to_string(),
        ratio: Some(0.0),
        trigger_level: 0.0,
        cushion: Some(0.0),
        passing: true,
    };
    let value = serde_json::to_value(&diagnostic).expect("serialize");
    let mut declared: Vec<&str> = value
        .as_object()
        .expect("object")
        .iter()
        .filter(|(_, field)| field.is_number())
        .map(|(name, _)| name.as_str())
        .collect();
    declared.sort_unstable();
    let units = simulation_field_units(Currency::EUR);
    let order: Vec<&str> = units
        .coverage_tests
        .iter()
        .map(|unit| unit.field.as_str())
        .collect();
    assert_eq!(order, ["ratio", "trigger_level", "cushion"]);
    let mut listed = order;
    listed.sort_unstable();
    assert_eq!(listed, declared);
    assert_eq!(units.diverted_cash.label, "EUR");
    assert_eq!(
        units.excess_spread.source,
        "valuations::PeriodDiagnostics::excess_spread"
    );
}

#[test]
fn tranche_field_units_state_the_result_currency_and_source() {
    let units = tranche_metric_field_units(Currency::EUR);
    let unit = |field: &str| {
        units
            .iter()
            .find(|unit| unit.field == field)
            .expect("listed")
    };
    assert_eq!(unit("pv").label, "EUR");
    assert_eq!(unit("cs01").label, "EUR per bp");
    assert_eq!(unit("wal").label, "years");
    assert_eq!(unit("factor").source, "valuations::TrancheMetrics::factor");
}
