//! Rolling statement results up to coarser periods.
#![allow(clippy::expect_used)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Period, PeriodId, PeriodKind};
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::{Evaluator, RollupSpec};
use finstack_quant_statements::types::{
    AmountOrScalar, FinancialModelSpec, NodeSpec, NodeType, NodeValueType, RollupMethod,
};
use indexmap::IndexMap;
use time::Month;

fn date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).expect("valid date")
}

fn q(year: i32, quarter: u8) -> PeriodId {
    PeriodId::quarter(year, quarter).expect("valid quarter")
}

fn usd(values: &[(PeriodId, f64)]) -> Vec<(PeriodId, AmountOrScalar)> {
    values
        .iter()
        .map(|(period, value)| {
            (
                *period,
                AmountOrScalar::amount(*value, Currency::USD).expect("valid amount"),
            )
        })
        .collect()
}

fn quarters(values: [f64; 8]) -> Vec<(PeriodId, f64)> {
    (0..8)
        .map(|i| (q(2024 + i / 4, (i % 4 + 1) as u8), values[i as usize]))
        .collect()
}

/// Two years of quarterly flows, balances and ratios.
fn quarterly_model() -> FinancialModelSpec {
    {
        let mut model_builder = ModelBuilder::new("rollup");
        model_builder
            .periods("2024Q1..2025Q4", Some("2024Q4"))
            .expect("valid periods");
        model_builder.value(
            "revenue",
            &usd(&quarters([
                100.0, 110.0, 120.0, 130.0, 140.0, 150.0, 160.0, 170.0,
            ])),
        );
        model_builder.value(
            "opex",
            &usd(&quarters([60.0, 60.0, 70.0, 70.0, 80.0, 80.0, 90.0, 90.0])),
        );
        model_builder.value(
            "cash",
            &usd(&quarters([10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0])),
        );
        model_builder.value(
            "debt",
            &usd(&quarters([
                500.0, 500.0, 480.0, 460.0, 440.0, 420.0, 400.0, 380.0,
            ])),
        );
        model_builder
            .compute("ebitda", "revenue - opex")
            .expect("valid formula");
        model_builder
            .compute("ltm_ebitda", "rolling_sum(ebitda, 4)")
            .expect("valid formula");
        model_builder
            .compute("margin", "ebitda / revenue")
            .expect("valid formula");
        model_builder
            .compute("leverage", "debt / ltm_ebitda")
            .expect("valid formula");
        model_builder
            .compute("growth", "revenue / lag(revenue, 1) - 1")
            .expect("valid formula");
        model_builder.build().expect("valid model")
    }
}

fn annual_spec() -> RollupSpec {
    RollupSpec {
        frequency: PeriodKind::Annual,
        methods: IndexMap::from([
            ("cash".to_string(), RollupMethod::PeriodEnd),
            ("debt".to_string(), RollupMethod::PeriodEnd),
            ("ltm_ebitda".to_string(), RollupMethod::PeriodEnd),
            ("margin".to_string(), RollupMethod::Formula),
            ("leverage".to_string(), RollupMethod::Formula),
            ("growth".to_string(), RollupMethod::Formula),
        ]),
    }
}

fn close(actual: Option<f64>, expected: f64) {
    let actual = actual.expect("value present");
    assert!((actual - expected).abs() < 1e-12, "{actual} vs {expected}");
}

/// A year of quarters declaring every method, with tags that must not matter.
fn declared_model() -> FinancialModelSpec {
    let mut model = {
        let mut model_builder = ModelBuilder::new("declared");
        model_builder
            .periods("2024Q1..Q4", Some("2024Q4"))
            .expect("valid periods");
        let values = |values: [f64; 4]| {
            usd(&(0..4)
                .map(|i| (q(2024, i as u8 + 1), values[i]))
                .collect::<Vec<_>>())
        };
        model_builder.value("revenue", &values([100.0, 110.0, 120.0, 130.0]));
        model_builder.value("cash", &values([10.0, 20.0, 30.0, 40.0]));
        model_builder.value("debt_draws", &values([1.0, 2.0, 3.0, 4.0]));
        // Tagged like a balance, declaring nothing: it must still sum.
        model_builder.value("tagged_balance", &values([1.0, 1.0, 1.0, 1.0]));
        model_builder
            .compute("ebitda", "revenue * 0.25")
            .expect("valid formula");
        model_builder
            .compute("margin", "ebitda / revenue")
            .expect("valid formula");
        // Tagged like a ratio, declaring nothing: it must still sum.
        model_builder
            .compute("tagged_ratio", "cash / revenue")
            .expect("valid formula");
        model_builder
            .compute("total_debt", "cumsum(debt_draws)")
            .expect("valid formula");
        model_builder.build().expect("valid model")
    };
    for (node, method) in [
        ("cash", RollupMethod::PeriodEnd),
        ("total_debt", RollupMethod::PeriodEnd),
        ("margin", RollupMethod::Formula),
        ("debt_draws", RollupMethod::Sum),
    ] {
        model.get_node_mut(node).expect("node").rollup = Some(method);
    }
    model.get_node_mut("tagged_balance").expect("node").tags = vec!["balance_sheet".to_string()];
    model.get_node_mut("tagged_ratio").expect("node").tags =
        vec!["ratio".to_string(), "metric".to_string()];
    model
}

fn empty_annual_spec() -> RollupSpec {
    RollupSpec {
        frequency: PeriodKind::Annual,
        methods: IndexMap::new(),
    }
}

/// A target period is actual only when every source is: the split can fall
/// inside a year.
#[test]
fn a_target_period_with_any_forecast_source_is_not_actual() {
    let mut model = quarterly_model();
    model.periods = finstack_quant_core::dates::build_periods("2024Q1..2025Q4", Some("2025Q1"))
        .expect("valid periods")
        .periods;
    let annual = Evaluator::new()
        .evaluate(&model)
        .expect("evaluation")
        .rollup(&model, &annual_spec())
        .expect("annual rollup");
    assert_eq!(
        annual
            .periods
            .iter()
            .map(|period| period.is_actual)
            .collect::<Vec<_>>(),
        [true, false]
    );
}

#[test]
fn an_evaluated_result_carries_the_model_periods() {
    let model = quarterly_model();
    let quarterly = Evaluator::new().evaluate(&model).expect("evaluation");
    assert_eq!(quarterly.periods, model.periods);
}

#[test]
fn a_declared_method_applies_with_an_empty_spec() {
    let model = declared_model();
    let quarterly = Evaluator::new().evaluate(&model).expect("evaluation");
    let annual = quarterly
        .rollup(&model, &empty_annual_spec())
        .expect("annual rollup");
    let year = PeriodId::annual(2024);

    close(annual.get("cash", &year), 40.0);
    close(annual.get("total_debt", &year), 10.0);
    close(annual.get("margin", &year), 0.25);
    close(annual.get("debt_draws", &year), 10.0);
}

/// Every line built through the public builder can declare its method: mixed
/// lines on the mixed builder, the others on `ModelBuilder::rollup`.
#[test]
fn lines_built_through_the_builder_roll_up_with_their_declared_method() {
    let quarter_values: Vec<(PeriodId, f64)> =
        (1..=4).map(|i| (q(2024, i), f64::from(i) * 10.0)).collect();
    let scalars: Vec<(PeriodId, AmountOrScalar)> = quarter_values
        .iter()
        .map(|(period, value)| (*period, AmountOrScalar::scalar(*value)))
        .collect();

    let mut builder = ModelBuilder::new("builder-declared");
    builder.periods("2024Q1..Q4", None).expect("valid periods");
    builder.value("flow", &scalars);
    builder.value("balance", &scalars);
    builder
        .rollup("balance", RollupMethod::PeriodEnd)
        .expect("known node");
    builder.compute("ratio", "balance / flow").expect("formula");
    builder
        .rollup("ratio", RollupMethod::Formula)
        .expect("known node");
    let mut mixed = builder.mixed("mixed_balance");
    mixed.values(&scalars);
    mixed.rollup(RollupMethod::PeriodEnd);
    let builder = mixed.build().expect("mixed node");
    let mut mixed = builder.mixed("mixed_flow");
    mixed.values(&scalars);
    let model = mixed
        .build()
        .expect("mixed node")
        .build()
        .expect("model builds");

    let annual = Evaluator::new()
        .evaluate(&model)
        .expect("evaluation")
        .rollup(&model, &empty_annual_spec())
        .expect("annual rollup");
    let year = PeriodId::annual(2024);

    close(annual.get("flow", &year), 100.0);
    close(annual.get("balance", &year), 40.0);
    close(annual.get("ratio", &year), 0.4);
    close(annual.get("mixed_balance", &year), 40.0);
    close(annual.get("mixed_flow", &year), 100.0);
}

#[test]
fn a_rollup_declared_on_an_unknown_node_is_an_error_naming_it() {
    let mut builder = ModelBuilder::new("unknown");
    let error = builder
        .rollup("ghost", RollupMethod::Sum)
        .expect_err("unknown node rejected")
        .to_string();
    assert!(error.contains("ghost"), "{error}");
}

#[test]
fn a_line_declaring_nothing_sums_whatever_its_tags_say() {
    let model = declared_model();
    let quarterly = Evaluator::new().evaluate(&model).expect("evaluation");
    let annual = quarterly
        .rollup(&model, &empty_annual_spec())
        .expect("annual rollup");
    let year = PeriodId::annual(2024);

    close(annual.get("revenue", &year), 460.0);
    close(annual.get("ebitda", &year), 115.0);
    close(annual.get("tagged_balance", &year), 4.0);
    let summed_ratio = 10.0 / 100.0 + 20.0 / 110.0 + 30.0 / 120.0 + 40.0 / 130.0;
    close(annual.get("tagged_ratio", &year), summed_ratio);
}

#[test]
fn a_spec_entry_overrides_the_declared_method() {
    let model = declared_model();
    let quarterly = Evaluator::new().evaluate(&model).expect("evaluation");
    let annual = quarterly
        .rollup(
            &model,
            &RollupSpec {
                frequency: PeriodKind::Annual,
                methods: IndexMap::from([
                    ("cash".to_string(), RollupMethod::Sum),
                    ("tagged_balance".to_string(), RollupMethod::PeriodEnd),
                ]),
            },
        )
        .expect("annual rollup");
    let year = PeriodId::annual(2024);

    close(annual.get("cash", &year), 100.0);
    close(annual.get("tagged_balance", &year), 1.0);
    // Lines the spec does not name keep their declarations.
    close(annual.get("total_debt", &year), 10.0);
}

#[test]
fn declared_methods_lists_only_declared_lines_and_ignores_tags() {
    let spec = RollupSpec::declared_methods(&declared_model(), PeriodKind::Annual);
    assert_eq!(spec.frequency, PeriodKind::Annual);
    assert_eq!(
        spec.methods,
        IndexMap::from([
            ("cash".to_string(), RollupMethod::PeriodEnd),
            ("debt_draws".to_string(), RollupMethod::Sum),
            ("margin".to_string(), RollupMethod::Formula),
            ("total_debt".to_string(), RollupMethod::PeriodEnd),
        ])
    );
}

#[test]
fn declared_methods_gives_the_same_rollup_as_an_empty_spec() {
    let model = declared_model();
    let quarterly = Evaluator::new().evaluate(&model).expect("evaluation");
    let listed = quarterly
        .rollup(
            &model,
            &RollupSpec::declared_methods(&model, PeriodKind::Annual),
        )
        .expect("listed rollup");
    let empty = quarterly
        .rollup(&model, &empty_annual_spec())
        .expect("empty rollup");
    assert_eq!(listed.nodes, empty.nodes);
}

#[test]
fn declared_methods_keeps_the_requested_frequency_and_lists_nothing_for_an_undeclared_model() {
    let spec = RollupSpec::declared_methods(&quarterly_model(), PeriodKind::SemiAnnual);
    assert_eq!(spec.frequency, PeriodKind::SemiAnnual);
    assert!(spec.methods.is_empty());
}

#[test]
fn a_node_declaring_formula_without_a_formula_is_an_error_naming_it() {
    let mut model = declared_model();
    model.get_node_mut("revenue").expect("node").rollup = Some(RollupMethod::Formula);
    let quarterly = Evaluator::new().evaluate(&model).expect("evaluation");

    let error = quarterly
        .rollup(&model, &empty_annual_spec())
        .expect_err("no formula to re-evaluate")
        .to_string();
    assert!(error.contains("'revenue'"), "{error}");
    assert!(error.contains("no formula"), "{error}");

    // The spec decides first, so a host can roll that line up another way.
    let summed = quarterly
        .rollup(
            &model,
            &RollupSpec {
                frequency: PeriodKind::Annual,
                methods: IndexMap::from([("revenue".to_string(), RollupMethod::Sum)]),
            },
        )
        .expect("spec overrides the declaration");
    close(summed.get("revenue", &PeriodId::annual(2024)), 460.0);
}

#[test]
fn a_node_declaring_formula_that_reads_capital_structure_is_rejected() {
    let mut model = declared_model();
    model.add_node(
        NodeSpec::new("cs_ratio", NodeType::Calculated)
            .with_formula("cs.interest_expense.total / revenue")
            .with_rollup(RollupMethod::Formula),
    );
    let quarterly = Evaluator::new()
        .evaluate(&declared_model())
        .expect("evaluation");

    let error = quarterly
        .rollup(&model, &empty_annual_spec())
        .expect_err("cs values are not rolled up")
        .to_string();
    assert!(error.contains("cs_ratio"), "{error}");
    assert!(error.contains("capital-structure"), "{error}");
}

#[test]
fn a_declared_method_round_trips_through_the_model_wire_format() {
    let node = NodeSpec::new("cash", NodeType::Value).with_rollup(RollupMethod::PeriodEnd);
    let wire = serde_json::to_value(&node).expect("serialize");
    assert_eq!(wire["rollup"], "period_end");
    assert_eq!(
        serde_json::from_value::<NodeSpec>(wire).expect("deserialize"),
        node
    );

    let undeclared = serde_json::to_value(NodeSpec::new("cash", NodeType::Value)).expect("json");
    assert!(undeclared.get("rollup").is_none());
}

#[test]
fn quarterly_results_roll_up_to_annual_flows_balances_and_ratios() {
    let model = quarterly_model();
    let quarterly = Evaluator::new().evaluate(&model).expect("evaluation");
    let annual = quarterly
        .rollup(&model, &annual_spec())
        .expect("annual rollup");
    let (y2024, y2025) = (PeriodId::annual(2024), PeriodId::annual(2025));

    // Flows sum, including formula flows left at the default.
    close(annual.get("revenue", &y2024), 460.0);
    close(annual.get("revenue", &y2025), 620.0);
    close(annual.get("ebitda", &y2024), 200.0);
    close(annual.get("ebitda", &y2025), 280.0);

    // Balances and trailing metrics take the fourth quarter.
    close(annual.get("cash", &y2024), 40.0);
    close(annual.get("debt", &y2025), 380.0);
    close(annual.get("ltm_ebitda", &y2025), 280.0);

    // Ratios are re-evaluated on the annual figures.
    close(annual.get("margin", &y2024), 200.0 / 460.0);
    close(annual.get("margin", &y2025), 280.0 / 620.0);
    close(annual.get("leverage", &y2025), 380.0 / 280.0);
    // `lag` steps back one year at the annual frequency.
    assert!(annual.get("growth", &y2024).expect("value").is_nan());
    close(annual.get("growth", &y2025), 620.0 / 460.0 - 1.0);

    assert_eq!(annual.meta.num_periods, 2);
    assert_eq!(
        annual.periods,
        [
            Period {
                id: y2024,
                start: date(2024, Month::January, 1),
                end: date(2025, Month::January, 1),
                // Every 2024 quarter is an actual.
                is_actual: true,
            },
            Period {
                id: y2025,
                start: date(2025, Month::January, 1),
                end: date(2026, Month::January, 1),
                is_actual: false,
            },
        ]
    );
    assert_eq!(
        annual.nodes["revenue"].keys().copied().collect::<Vec<_>>(),
        [y2024, y2025]
    );
    assert_eq!(
        annual.node_value_types["revenue"],
        NodeValueType::Monetary {
            currency: Currency::USD
        }
    );
    assert_eq!(annual.node_value_types["margin"], NodeValueType::Scalar);
    assert_eq!(
        annual.monetary_nodes["cash"][&y2024].amount(),
        40.0,
        "monetary values are rebuilt for the target periods"
    );
    assert!(annual.node_sources.is_empty());
    assert!(annual.cs_cashflows.is_none());
}

#[test]
fn monthly_results_roll_up_to_quarters() {
    let months: Vec<(PeriodId, AmountOrScalar)> = (1..=6)
        .map(|m| {
            (
                PeriodId::month(2025, m).expect("valid month"),
                AmountOrScalar::scalar(f64::from(m)),
            )
        })
        .collect();
    let model = {
        let mut model_builder = ModelBuilder::new("monthly");
        model_builder
            .periods("2025M1..M6", None)
            .expect("valid periods");
        model_builder.value("sales", &months);
        model_builder.build().expect("valid model")
    };
    let monthly = Evaluator::new().evaluate(&model).expect("evaluation");

    let quarterly = monthly
        .rollup(
            &model,
            &RollupSpec {
                frequency: PeriodKind::Quarterly,
                methods: IndexMap::new(),
            },
        )
        .expect("quarterly rollup");

    assert_eq!(quarterly.get("sales", &q(2025, 1)), Some(6.0));
    assert_eq!(quarterly.get("sales", &q(2025, 2)), Some(15.0));
}

#[test]
fn fiscal_quarters_roll_up_to_fiscal_years_and_halves() {
    use finstack_quant_core::dates::{FiscalConfig, build_fiscal_periods};

    let periods = build_fiscal_periods("FY2025Q1..FY2025Q4", FiscalConfig::us_federal(), None)
        .expect("fiscal quarters")
        .periods;
    let values: Vec<_> = periods
        .iter()
        .zip([1.0, 2.0, 4.0, 8.0])
        .map(|(period, value)| (period.id, AmountOrScalar::scalar(value)))
        .collect();
    let model = {
        let mut model_builder = ModelBuilder::new("fiscal-rollup");
        model_builder
            .periods_explicit(periods)
            .expect("valid periods");
        model_builder.value("x", &values);
        model_builder.build().expect("valid model")
    };
    let results = Evaluator::new().evaluate(&model).expect("evaluation");

    let annual = results
        .rollup(
            &model,
            &RollupSpec {
                frequency: PeriodKind::Annual,
                methods: IndexMap::new(),
            },
        )
        .expect("fiscal annual rollup");
    let fy2025: PeriodId = "FY2025".parse().expect("fiscal year id");
    assert!(fy2025.is_fiscal());
    assert_eq!(annual.get("x", &fy2025), Some(15.0));

    let halves = results
        .rollup(
            &model,
            &RollupSpec {
                frequency: PeriodKind::SemiAnnual,
                methods: IndexMap::new(),
            },
        )
        .expect("fiscal half-year rollup");
    let h2: PeriodId = "FY2025H2".parse().expect("fiscal half id");
    assert_eq!(halves.get("x", &h2), Some(12.0));
}

#[test]
fn rollup_rejects_incomplete_years_and_invalid_targets() {
    let model = quarterly_model();
    let results = Evaluator::new().evaluate(&model).expect("evaluation");

    let mut trimmed = model.clone();
    trimmed.periods.remove(0);
    let err = results
        .rollup(&trimmed, &annual_spec())
        .expect_err("2024 has three quarters");
    let message = err.to_string();
    assert!(message.contains("2024 is incomplete"), "{message}");

    for frequency in [PeriodKind::Quarterly, PeriodKind::Monthly] {
        let spec = RollupSpec {
            frequency,
            methods: IndexMap::new(),
        };
        let err = results
            .rollup(&model, &spec)
            .expect_err("target must be coarser");
        assert!(err.to_string().contains("Cannot roll"), "{err}");
    }

    let mut unknown = annual_spec();
    unknown
        .methods
        .insert("nope".to_string(), RollupMethod::PeriodEnd);
    let err = results.rollup(&model, &unknown).expect_err("unknown node");
    assert!(err.to_string().contains("'nope'"));

    let mut no_formula = annual_spec();
    no_formula
        .methods
        .insert("revenue".to_string(), RollupMethod::Formula);
    let err = results
        .rollup(&model, &no_formula)
        .expect_err("revenue has no formula");
    assert!(err.to_string().contains("has no formula"));
}

#[test]
fn rollup_spec_wire_format_is_strict_snake_case() {
    let spec: RollupSpec = serde_json::from_str(
        r#"{"frequency": "annual", "methods": {"cash": "period_end", "margin": "formula"}}"#,
    )
    .expect("valid spec");
    assert_eq!(spec.methods["cash"], RollupMethod::PeriodEnd);
    assert_eq!(
        serde_json::from_str::<RollupSpec>(r#"{"frequency": "annual"}"#)
            .expect("methods default to empty")
            .methods
            .len(),
        0
    );
    assert!(serde_json::from_str::<RollupSpec>(r#"{"frequency": "annual", "x": 1}"#).is_err());
}

/// The ratio metrics the crate ships declare `formula`, so a rolled-up margin
/// or multiple is recomputed from rolled-up inputs instead of summed
/// (a 40% quarterly margin used to roll up to 1.6).
#[test]
fn bundled_ratio_metrics_roll_up_as_ratios() {
    use finstack_quant_statements::registry::Registry;
    let registry = Registry::with_builtins().expect("built-in metrics");
    let series = |values: [f64; 4]| -> Vec<(PeriodId, AmountOrScalar)> {
        (1..=4_u8)
            .zip(values)
            .map(|(quarter, value)| (q(2025, quarter), AmountOrScalar::scalar(value)))
            .collect()
    };
    let model = {
        let mut builder = ModelBuilder::new("bundled-ratios");
        builder.periods("2025Q1..Q4", None).expect("valid periods");
        builder.value("revenue", &series([100.0, 110.0, 120.0, 130.0]));
        builder.value("cogs", &series([60.0, 66.0, 72.0, 78.0]));
        builder.value("opex", &series([20.0, 22.0, 24.0, 26.0]));
        builder.value("total_debt", &series([400.0, 410.0, 420.0, 460.0]));
        builder.value("total_equity", &series([200.0, 205.0, 210.0, 230.0]));
        for balance in ["total_debt", "total_equity"] {
            builder
                .rollup(balance, RollupMethod::PeriodEnd)
                .expect("declared balance");
        }
        for metric in [
            "fin.gross_margin",
            "fin.debt_to_equity",
            "fin.debt_to_ebitda",
        ] {
            builder
                .add_metric_from_registry(metric, &registry)
                .expect("bundled metric");
        }
        builder.build().expect("model")
    };
    let quarterly = Evaluator::new().evaluate(&model).expect("evaluation");
    let annual = quarterly
        .rollup(&model, &empty_annual_spec())
        .expect("annual rollup");
    let year = PeriodId::annual(2025);
    // Flows still sum.
    close(annual.get("fin.gross_profit", &year), 184.0);
    close(annual.get("fin.ebitda", &year), 92.0);
    // Ratios are recomputed on the annual inputs.
    close(annual.get("fin.gross_margin", &year), 184.0 / 460.0);
    close(annual.get("fin.debt_to_equity", &year), 2.0);
    close(annual.get("fin.debt_to_ebitda", &year), 460.0 / 92.0);
}

/// Every bundled metric that is not a currency flow declares how it rolls up.
#[test]
fn every_bundled_ratio_metric_declares_a_rollup_method() {
    use finstack_quant_statements::registry::{Registry, UnitType};
    let registry = Registry::with_builtins().expect("built-in metrics");
    let mut ratios = 0;
    for (id, metric) in registry.all_metrics() {
        let definition = &metric.definition;
        if definition.unit_type == Some(UnitType::Currency) {
            assert_eq!(definition.rollup, None, "{id} is a flow and sums");
        } else {
            ratios += 1;
            assert_eq!(
                definition.rollup,
                Some(RollupMethod::Formula),
                "{id} is not a currency flow and must declare its rollup"
            );
        }
    }
    assert_eq!(ratios, 16);
}
