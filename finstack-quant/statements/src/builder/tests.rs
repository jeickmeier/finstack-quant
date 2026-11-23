use super::*;
use crate::types::{AmountOrScalar, NodeSpec, NodeType};
use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;

/// STATEMENTS-CONTRACTS-006 #4: an import shares an unchanged template,
/// rejects a conflicting definition without importing anything, and an
/// intentional override is an explicit edit after the import.
#[test]
fn metric_imports_share_templates_and_reject_conflicts() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let registry = crate::registry::Registry::with_builtins().expect("catalogue");
    let base = || {
        let mut builder = ModelBuilder::new("imports");
        builder.periods("2025Q1..Q1", None).expect("valid periods");
        builder
    };

    // A node of the same name with another definition is a conflict, and
    // the import is atomic: its dependency is not added either.
    let mut conflicting = base();
    conflicting.value("fin.gross_margin", &[(period, AmountOrScalar::scalar(0.4))]);
    let error = conflicting
        .add_metric_from_registry("fin.gross_margin", &registry)
        .map(|_| ())
        .expect_err("conflict");
    assert!(
        error.to_string().contains("already defined differently"),
        "{error}"
    );
    assert!(!conflicting.nodes().contains_key("fin.gross_profit"));
    assert!(conflicting.with_builtin_metrics().map(|_| ()).is_err());
    assert_eq!(conflicting.nodes().len(), 1);

    // Two imports share the unchanged template of a common dependency.
    let mut shared = base();
    shared
        .add_metric_from_registry("fin.gross_margin", &registry)
        .expect("first import");
    let template = shared.nodes()["fin.gross_profit"].clone();
    shared
        .add_metric_from_registry("fin.gross_profit", &registry)
        .expect("second import shares the node");
    shared
        .with_builtin_metrics()
        .expect("catalogue shares both");
    assert_eq!(shared.nodes()["fin.gross_profit"], template);

    // An intentional override is a named edit after the import.
    let mut overridden = base();
    overridden
        .add_metric_from_registry("fin.gross_profit", &registry)
        .expect("import");
    overridden
        .replace_node(
            NodeSpec::new("fin.gross_profit", NodeType::Calculated)
                .with_formula("revenue - cogs - freight"),
        )
        .expect("explicit override");
    assert_eq!(
        overridden.nodes()["fin.gross_profit"]
            .formula_text
            .as_deref(),
        Some("revenue - cogs - freight")
    );
}

/// STATEMENTS-CONTRACTS-002 #1: a repeated node id, a repeated period
/// and a repeated availability date are duplicates, not replacements.
#[test]
fn duplicate_definitions_and_observations_are_rejected() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let base = || {
        let mut builder = ModelBuilder::new("duplicates");
        builder.periods("2025Q1..Q1", None).expect("valid periods");
        builder.value("revenue", &[(period, AmountOrScalar::scalar(100.0))]);
        builder
    };
    let rejected = |builder: ModelBuilder, expected: &str| {
        let error = builder.build().expect_err(expected);
        assert!(error.to_string().contains(expected), "{error}");
    };

    let mut builder = base();
    builder.value_scalar("revenue", &[]);
    rejected(builder, "defined more than once");
    let mut builder = base();
    builder.value_scalar("revenue", &[]);
    rejected(builder, "defined more than once");
    let mut builder = base();
    builder.value_money("revenue", &[]);
    rejected(builder, "defined more than once");
    let mut builder = base();
    builder.insert_node(NodeSpec::new("revenue", NodeType::Value));
    rejected(builder, "defined more than once");

    let mut builder = base();
    let error = builder
        .compute("revenue", "1")
        .expect_err("compute rejects a defined id");
    assert!(
        error.to_string().contains("defined more than once"),
        "{error}"
    );
    let error = base()
        .mixed("revenue")
        .build()
        .expect_err("a mixed node rejects a defined id");
    assert!(
        error.to_string().contains("defined more than once"),
        "{error}"
    );

    let mut builder = base();
    builder.value(
        "cost",
        &[
            (period, AmountOrScalar::scalar(1.0)),
            (period, AmountOrScalar::scalar(2.0)),
        ],
    );
    rejected(builder, "more than one observation for period 2025Q1");

    let date = finstack_quant_core::dates::Date::from_calendar_date(2025, time::Month::April, 25)
        .expect("date");
    let mut builder = base();
    let error = builder
        .availability_dates("revenue", &[(period, date), (period, date)])
        .expect_err("a repeated availability date is rejected");
    assert!(
        error
            .to_string()
            .contains("more than one availability date"),
        "{error}"
    );
    assert!(builder.nodes()["revenue"].availability_dates.is_empty());

    // The named edits change an existing node.
    let mut builder = base();
    builder
        .replace_node(
            NodeSpec::new("revenue", NodeType::Value)
                .with_values(IndexMap::from([(period, AmountOrScalar::scalar(110.0))])),
        )
        .expect("replace an existing node");
    builder
        .replace_node(NodeSpec::new("absent", NodeType::Value))
        .expect_err("replace needs an existing node");
    builder
        .remove_node("absent")
        .expect_err("remove needs an existing node");
    let model = builder.build().expect("model builds");
    assert_eq!(
        model.nodes["revenue"].values.as_ref().expect("values")[&period].value(),
        110.0
    );
}

/// STATEMENTS-CONTRACTS-002 #1: a `where` clause needs a valid named
/// target and a predicate that compiles.
#[test]
fn where_clause_needs_a_named_target() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let mut builder = ModelBuilder::new("where-target");
    builder.periods("2025Q1..Q1", None).expect("valid periods");
    builder.value("revenue", &[(period, AmountOrScalar::scalar(100.0))]);
    builder.compute("bonus", "revenue * 0.1").expect("formula");
    builder.compute("tax", "revenue * 0.2").expect("formula");
    builder
        .where_clause("bonus", "revenue > 1000")
        .expect("named target");
    builder
        .where_clause("absent", "revenue > 1000")
        .expect_err("an unknown target is rejected");
    builder
        .where_clause("tax", "revenue >")
        .expect_err("an invalid predicate is rejected");
    let model = builder.build().expect("model builds");

    let where_text = |id: &str| model.get_node(id).expect("node exists").where_text.clone();
    assert_eq!(where_text("bonus").as_deref(), Some("revenue > 1000"));
    assert_eq!(where_text("tax"), None);
}

#[test]
fn test_formula_references_require_exact_node_ids() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let model = {
        let mut model_builder_1 = ModelBuilder::new("alias-test");
        model_builder_1
            .periods("2025Q1..Q1", None)
            .expect("valid periods");
        model_builder_1.value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))]);
        model_builder_1.value("cogs", &[(period, AmountOrScalar::scalar(40_000.0))]);
        model_builder_1
            .compute("gross_profit", "rev - cogs")
            .expect("valid formula");
        model_builder_1
    };

    let error = model
        .build()
        .expect_err("the alias must not be rewritten to revenue");
    let message = error.to_string();
    assert!(message.contains("Unknown identifier 'rev'"));
    assert!(message.contains("Did you mean one of: revenue"));
}

#[test]
fn test_build_rejects_cross_currency_addition() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let result = {
        let mut model_builder_1 = ModelBuilder::new("dimension-test");
        model_builder_1
            .periods("2025Q1..Q1", None)
            .expect("valid periods");
        model_builder_1.value_money(
            "usd_revenue",
            &[(
                period,
                finstack_quant_core::money::Money::from((
                    100_i64,
                    finstack_quant_core::currency::Currency::USD,
                )),
            )],
        );
        model_builder_1.value_money(
            "eur_cost",
            &[(
                period,
                finstack_quant_core::money::Money::from((
                    40_i64,
                    finstack_quant_core::currency::Currency::EUR,
                )),
            )],
        );
        model_builder_1
            .compute("bad_total", "usd_revenue + eur_cost")
            .expect("formula syntax is valid");
        model_builder_1.build()
    };

    assert!(result.is_err());
    assert!(
        result
            .expect_err("cross-currency addition should fail")
            .to_string()
            .contains("Dimensional mismatch")
    );
}
