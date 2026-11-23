use super::runner::*;
use crate::evaluator::cell::UnavailableReason;
use crate::evaluator::results::{CellSource, StatementResult};
use crate::types::FinancialModelSpec;

use crate::builder::ModelBuilder;
use crate::evaluator::CellStatus;
use crate::types::{AmountOrScalar, NodeSpec, NodeType};
use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::market_data::context::MarketContext;
use indexmap::IndexMap;
use time::Month;

/// STATEMENTS-EVALUATION-001 #2: a false mask is a masked cell with no
/// numeric value, and a mask with no value leaves the cell unavailable.
/// Neither is a zero, here or downstream.
#[test]
fn masked_and_unmaskable_cells_have_no_value() {
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let model = {
        let mut builder = ModelBuilder::new("mask-states");
        builder.periods("2025Q1..Q1", None).expect("periods");
        builder.value_scalar("flag", &[]);
        builder.value_scalar("off", &[(q1, 0.0)]);
        builder.compute("unknown_mask", "42").expect("formula");
        builder
            .where_clause("unknown_mask", "flag > 0")
            .expect("predicate");
        builder.compute("masked", "42").expect("formula");
        builder
            .where_clause("masked", "off > 0")
            .expect("predicate");
        builder
            .compute("downstream", "masked + 1")
            .expect("formula");
        builder.compute("kept", "42").expect("formula");
        builder.where_clause("kept", "off == 0").expect("predicate");
        builder.build().expect("valid model")
    };

    let results = Evaluator::new().evaluate(&model).expect("evaluation");
    let cell = |node: &str| *results.get_cell(node, &q1).expect("cell");

    assert_eq!(cell("masked").status, CellStatus::Masked);
    assert_eq!(
        results.get("masked", &q1),
        None,
        "a masked cell is not zero"
    );
    assert_eq!(cell("masked").source, None);
    assert_eq!(
        cell("downstream").status,
        CellStatus::Unavailable,
        "a masked input is not read as zero downstream"
    );
    assert_eq!(cell("downstream").source, Some(CellSource::Formula));
    assert_eq!(
        cell("unknown_mask"),
        crate::evaluator::ResultCell::unavailable(UnavailableReason::MissingMask)
    );
    assert_eq!(results.get("kept", &q1), Some(42.0));
    assert_eq!(cell("kept").source, Some(CellSource::Formula));
    assert_eq!(cell("flag").status, CellStatus::Missing);
    assert_eq!(
        cell("off"),
        crate::evaluator::ResultCell::scalar(0.0)
            .expect("finite")
            .with_source(Some(CellSource::Value)),
        "a known zero is a value"
    );
}

#[test]
fn evaluate_prepared_rejects_a_model_whose_formula_changed() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let build = |factor: &str| {
        let mut model_builder_1 = ModelBuilder::new("prepared-drift");
        model_builder_1
            .periods("2025Q1..Q1", None)
            .expect("valid periods");
        model_builder_1.value("revenue", &[(period, AmountOrScalar::scalar(100.0))]);
        model_builder_1
            .compute("scaled", format!("revenue * {factor}"))
            .expect("valid formula");
        model_builder_1.build().expect("valid model")
    };

    let original = build("2");
    let mut evaluator = Evaluator::new();
    let prepared = evaluator.prepare(&original).expect("prepare");

    // Same nodes, different formula: the node-set guard cannot see this.
    let edited = build("3");
    let err = evaluator
        .evaluate_prepared(&edited, &prepared)
        .expect_err("a changed formula must be rejected, not silently ignored");
    assert!(
        err.to_string().contains("formula"),
        "error should point at the changed formula: {err}"
    );

    // The unedited model still evaluates against its own plan.
    let results = evaluator
        .evaluate_prepared(&original, &prepared)
        .expect("the model the plan was built for still evaluates");
    assert_eq!(results.get("scaled", &period), Some(200.0));
}

/// One node resolved by a different layer in each period, plus a masked
/// node: the result records which layer produced every cell.
#[test]
fn node_sources_record_value_formula_forecast_and_where_mask() {
    use crate::types::ForecastSpec;

    let q = |quarter| PeriodId::quarter(2025, quarter).expect("valid period fixture");
    // Q1 and Q2 are actuals, Q3 is a forecast period.
    let periods = finstack_quant_core::dates::build_periods("2025Q1..Q3", Some("2025Q2"))
        .expect("periods")
        .into_periods();
    let mut model = FinancialModelSpec::new(
        "cell-sources",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            periods,
        )
        .expect("valid period plan"),
    );
    model.add_node(
        NodeSpec::new("revenue", NodeType::Mixed)
            .with_values(IndexMap::from([(q(1), AmountOrScalar::scalar(100.0))]))
            .with_forecast(ForecastSpec::growth(0.10))
            .with_formula("123"),
    );
    let mut masked = NodeSpec::new("masked", NodeType::Calculated).with_formula("42");
    masked.where_text = Some("1 == 0".to_string());
    model.add_node(masked);

    let results = Evaluator::new().evaluate(&model).expect("evaluation");

    // Explicit value wins in Q1; Q2 is an actual with no value, so the
    // formula fires (forecasts never run in actuals); Q3 is forecast.
    assert_eq!(results.get("revenue", &q(1)), Some(100.0));
    assert_eq!(
        results.get_source("revenue", &q(1)),
        Some(CellSource::Value)
    );
    assert_eq!(results.get("revenue", &q(2)), Some(123.0));
    assert_eq!(
        results.get_source("revenue", &q(2)),
        Some(CellSource::Formula)
    );
    assert_eq!(
        results.get_source("revenue", &q(3)),
        Some(CellSource::Forecast)
    );
    for quarter in 1..=3 {
        let cell = results.get_cell("masked", &q(quarter)).expect("cell");
        assert_eq!(cell.status, CellStatus::Masked);
        assert_eq!(cell.source, None);
    }

    // Every node has a cell in every period, and each value cell records
    // the source that produced it.
    for cells in results.cells.values() {
        assert_eq!(cells.len(), 3);
        for cell in cells.values() {
            assert_eq!(cell.status == CellStatus::Value, cell.source.is_some());
        }
    }

    // Status and source survive the wire format and reach the long table.
    let json = serde_json::to_value(&results).expect("serialize");
    assert_eq!(json["cells"]["revenue"]["2025Q3"]["source"], "forecast");
    assert_eq!(json["cells"]["revenue"]["2025Q1"]["value"], 100.0);
    assert_eq!(
        json["cells"]["masked"]["2025Q1"],
        serde_json::json!({ "status": "masked" })
    );
    let back: StatementResult = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back.cells, results.cells);

    let table = results.to_table_long().expect("long table");
    let labels = |name: &str| match &table.column(name).expect("column").data {
        finstack_quant_core::table::TableColumnData::NullableString(labels) => labels.clone(),
        finstack_quant_core::table::TableColumnData::String(labels) => {
            labels.iter().cloned().map(Some).collect()
        }
        other => panic!("{name} must be a string column: {other:?}"),
    };
    let expected_sources: Vec<Option<String>> = results
        .cells
        .values()
        .flat_map(|cells| cells.values())
        .map(|cell| cell.source.map(|source| source.as_str().to_string()))
        .collect();
    assert_eq!(labels("source"), expected_sources);
    assert!(labels("source").contains(&Some("forecast".to_string())));
    assert!(labels("status").contains(&Some("masked".to_string())));
}

/// The wire form of a cell is closed: a status must agree with its
/// value and reason.
#[test]
fn result_cells_reject_inconsistent_wire_forms() {
    for invalid in [
        serde_json::json!({ "status": "value" }),
        serde_json::json!({ "status": "masked", "value": 1.0 }),
        serde_json::json!({ "status": "unavailable" }),
        serde_json::json!({ "status": "missing", "reason": "missing_mask" }),
        serde_json::json!({ "status": "value", "value": 1.0, "extra": true }),
    ] {
        serde_json::from_value::<crate::evaluator::ResultCell>(invalid.clone())
            .expect_err(&invalid.to_string());
    }
    let money = serde_json::json!({
        "status": "value",
        "value": { "amount": "125.01", "currency": "USD" },
        "source": "formula"
    });
    let cell: crate::evaluator::ResultCell =
        serde_json::from_value(money.clone()).expect("money cell");
    assert_eq!(serde_json::to_value(cell).expect("serialize"), money);
}

#[test]
fn priced_run_provenance_includes_the_same_day_market() {
    let q1 = PeriodId::quarter(2025, 1).expect("first quarter");
    let day = Date::from_calendar_date(2024, Month::December, 31).expect("run date");
    let mut builder = ModelBuilder::new("same-day");
    builder
        .periods("2025Q1..Q2", None)
        .expect("forecast periods");
    let q2 = PeriodId::quarter(2025, 2).expect("second quarter");
    builder.value_scalar("balance", &[(q1, 200.0), (q2, 201.0)]);
    let model = builder.build().expect("source model");

    let mut market = MarketContext::new();
    market.insert_dividend_yield("DIV", 0.02).expect("quote");
    let evaluate = |context: &MarketContext| {
        Evaluator::new()
            .evaluate_with_market(&model, context, day, ObservationCutoff::FullSnapshot)
            .expect("market-aware evaluation")
            .provenance
            .expect("run provenance")
    };
    let first = evaluate(&market);
    assert_eq!(first, evaluate(&market.clone()));
    let mut replacement = market.clone();
    replacement
        .insert_dividend_yield("DIV", 0.03)
        .expect("new same-day quote");
    let second = evaluate(&replacement);
    assert_ne!(first.market_content_hash, second.market_content_hash);
    assert_ne!(first.run_id, second.run_id);
    assert_eq!(first.valuation_date, second.valuation_date);
}

#[test]
fn as_of_hides_a_dated_observation_outside_the_actuals_range() {
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let august = Date::from_calendar_date(2025, Month::August, 1).expect("date");
    // No period is flagged actual.
    let periods = finstack_quant_core::dates::build_periods("2025Q1..Q2", None)
        .expect("periods")
        .into_periods();
    let evaluate = |q2_available_on: Option<Date>| {
        let mut model = FinancialModelSpec::new(
            "as-of-policy",
            finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                periods.clone(),
            )
            .expect("valid period plan"),
        );
        let mut availability = IndexMap::from([(q1, as_of)]);
        if let Some(date) = q2_available_on {
            availability.insert(q2, date);
        }
        model.add_node(
            NodeSpec::new("revenue", NodeType::Mixed)
                .with_values(IndexMap::from([
                    (q1, AmountOrScalar::scalar(100.0)),
                    (q2, AmountOrScalar::scalar(999.0)),
                ]))
                .with_availability_dates(availability)
                .with_formula("123"),
        );
        model.source_snapshot = Some("original".to_string());
        Evaluator::new()
            .evaluate_with_market(
                &model,
                &MarketContext::new(),
                as_of,
                crate::evaluator::ObservationCutoff::Through(as_of),
            )
            .expect("evaluation")
            .get("revenue", &q2)
    };
    // Dated after the cutoff: hidden, so the formula answers.
    assert_eq!(evaluate(Some(august)), Some(123.0));
    // Undated value in a forecast period is an assumption and stays visible.
    assert_eq!(evaluate(None), Some(999.0));
}

#[test]
fn as_of_hides_future_actual_values() {
    let periods = finstack_quant_core::dates::build_periods("2025Q1..Q2", Some("2025Q2"))
        .expect("periods")
        .into_periods();
    let mut model = FinancialModelSpec::new(
        "as-of-policy",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            periods,
        )
        .expect("valid period plan"),
    );
    let mut values = IndexMap::new();
    values.insert(
        PeriodId::quarter(2025, 1).expect("valid period fixture"),
        AmountOrScalar::scalar(100.0),
    );
    values.insert(
        PeriodId::quarter(2025, 2).expect("valid period fixture"),
        AmountOrScalar::scalar(999.0),
    );
    model.add_node(
        NodeSpec::new("revenue", NodeType::Mixed)
            .with_values(values)
            .with_availability_dates(IndexMap::from([
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    Date::from_calendar_date(2025, Month::January, 1).expect("date"),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    Date::from_calendar_date(2025, Month::July, 1).expect("date"),
                ),
            ]))
            .with_formula("123"),
    );
    model.source_snapshot = Some("original".to_string());

    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate_with_market(
            &model,
            &MarketContext::new(),
            as_of,
            crate::evaluator::ObservationCutoff::Through(as_of),
        )
        .expect("evaluation");

    assert_eq!(
        results.get(
            "revenue",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(100.0)
    );
    assert_eq!(
        results.get(
            "revenue",
            &PeriodId::quarter(2025, 2).expect("valid period fixture")
        ),
        Some(123.0)
    );
    // The Q2 value exists in the spec but is hidden by the as-of cutoff,
    // so the recorded source is the formula that actually fired.
    assert_eq!(
        results.get_source(
            "revenue",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(CellSource::Value)
    );
    assert_eq!(
        results.get_source(
            "revenue",
            &PeriodId::quarter(2025, 2).expect("valid period fixture")
        ),
        Some(CellSource::Formula)
    );
}
