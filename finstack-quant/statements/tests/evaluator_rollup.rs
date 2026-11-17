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

/// The plan of these fixtures: fiscal ids use the October 1 opening every
/// fiscal fixture in this file is built with; unprefixed ids are Gregorian.
fn fixture_plan(periods: Vec<Period>) -> finstack_quant_core::dates::PeriodPlan {
    let calendar = if periods.first().is_some_and(|period| period.id.is_fiscal()) {
        finstack_quant_core::dates::PeriodCalendar::Fiscal(
            finstack_quant_core::dates::FiscalConfig::new(10, 1).expect("valid fiscal start"),
        )
    } else {
        finstack_quant_core::dates::PeriodCalendar::Gregorian
    };
    finstack_quant_core::dates::PeriodPlan::try_new(calendar, periods).expect("valid period plan")
}

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
            ("revenue".to_string(), RollupMethod::Sum),
            ("opex".to_string(), RollupMethod::Sum),
            ("ebitda".to_string(), RollupMethod::Sum),
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
        // Tagged like a balance and declared a flow: the declaration decides.
        model_builder.value("tagged_balance", &values([1.0, 1.0, 1.0, 1.0]));
        model_builder
            .compute("ebitda", "revenue * 0.25")
            .expect("valid formula");
        model_builder
            .compute("margin", "ebitda / revenue")
            .expect("valid formula");
        // Tagged like a ratio and declared a flow: the declaration decides.
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
        ("revenue", RollupMethod::Sum),
        ("ebitda", RollupMethod::Sum),
        ("tagged_balance", RollupMethod::Sum),
        ("tagged_ratio", RollupMethod::Sum),
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
        .expect("valid periods");
    let annual = Evaluator::new()
        .evaluate(&model)
        .expect("evaluation")
        .rollup(&model, &annual_spec())
        .expect("annual rollup");
    assert_eq!(
        annual
            .get_periods()
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
    assert_eq!(quarterly.periods.as_ref(), Some(&model.periods));
}

#[test]
fn a_declared_method_applies_with_an_empty_spec() {
    let model = declared_model();
    let quarterly = Evaluator::new().evaluate(&model).expect("evaluation");
    let annual = quarterly
        .rollup(&model, &empty_annual_spec())
        .expect("annual rollup");
    let year = PeriodId::annual(2024).expect("valid annual period");

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
    builder
        .rollup("flow", RollupMethod::Sum)
        .expect("known node");
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
    mixed.rollup(RollupMethod::Sum);
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
    let year = PeriodId::annual(2024).expect("valid annual period");

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
fn a_declared_sum_sums_whatever_the_tags_say() {
    let model = declared_model();
    let quarterly = Evaluator::new().evaluate(&model).expect("evaluation");
    let annual = quarterly
        .rollup(&model, &empty_annual_spec())
        .expect("annual rollup");
    let year = PeriodId::annual(2024).expect("valid annual period");

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
    let year = PeriodId::annual(2024).expect("valid annual period");

    close(annual.get("cash", &year), 100.0);
    close(annual.get("tagged_balance", &year), 1.0);
    // Lines the spec does not name keep their declarations.
    close(annual.get("total_debt", &year), 10.0);
}

#[test]
fn declared_methods_lists_the_declared_lines() {
    let spec = RollupSpec::declared_methods(&declared_model(), PeriodKind::Annual);
    assert_eq!(spec.frequency, PeriodKind::Annual);
    assert_eq!(
        spec.methods,
        IndexMap::from([
            ("cash".to_string(), RollupMethod::PeriodEnd),
            ("debt_draws".to_string(), RollupMethod::Sum),
            ("margin".to_string(), RollupMethod::Formula),
            ("total_debt".to_string(), RollupMethod::PeriodEnd),
            ("revenue".to_string(), RollupMethod::Sum),
            ("ebitda".to_string(), RollupMethod::Sum),
            ("tagged_balance".to_string(), RollupMethod::Sum),
            ("tagged_ratio".to_string(), RollupMethod::Sum),
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
    assert_eq!(listed.cells, empty.cells);
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
    close(
        summed.get(
            "revenue",
            &PeriodId::annual(2024).expect("valid annual period"),
        ),
        460.0,
    );
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
    let (y2024, y2025) = (
        PeriodId::annual(2024).expect("valid annual period"),
        PeriodId::annual(2025).expect("valid annual period"),
    );

    // Flows sum, including formula flows.
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
    assert!(annual.get("growth", &y2024).is_none());
    close(annual.get("growth", &y2025), 620.0 / 460.0 - 1.0);

    assert_eq!(annual.meta.num_periods, 2);
    assert_eq!(
        annual.get_periods(),
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
        annual.cells["revenue"].keys().copied().collect::<Vec<_>>(),
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
        annual.get_money("cash", &y2024).unwrap().amount(),
        40.0,
        "monetary values are rebuilt for the target periods"
    );
    assert!(
        annual
            .cells
            .values()
            .flat_map(|cells| cells.values())
            .all(|cell| cell.source.is_none()),
        "a rolled-up cell has no single value source"
    );
    assert!(annual.cs_cashflows.is_none());

    // The rolled result is linked to its source run, methods and periods.
    assert!(annual.provenance.is_none(), "a rollup is not a run");
    let provenance = annual
        .rollup_provenance
        .as_ref()
        .expect("rollup provenance");
    assert_eq!(Some(&provenance.source_run), quarterly.provenance.as_ref());
    assert_eq!(provenance.frequency, PeriodKind::Annual);
    assert_eq!(provenance.methods["revenue"], RollupMethod::Sum);
    assert_eq!(provenance.methods["cash"], RollupMethod::PeriodEnd);
    assert_eq!(provenance.methods.len(), model.nodes.len());
    assert_eq!(
        provenance.source_periods[&y2025],
        (1..=4).map(|quarter| q(2025, quarter)).collect::<Vec<_>>()
    );
    assert!(quarterly.rollup_provenance.is_none());
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
                methods: IndexMap::from([("sales".to_string(), RollupMethod::Sum)]),
            },
        )
        .expect("quarterly rollup");

    assert_eq!(quarterly.get("sales", &q(2025, 1)), Some(6.0));
    assert_eq!(quarterly.get("sales", &q(2025, 2)), Some(15.0));
}

#[test]
fn fiscal_quarters_roll_up_to_fiscal_years_and_halves() {
    use finstack_quant_core::dates::{FiscalConfig, build_fiscal_periods};

    let periods = build_fiscal_periods(
        "FY2025Q1..FY2025Q4",
        FiscalConfig::new(10, 1).expect("valid fiscal start"),
        None,
    )
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
            .periods_explicit(fixture_plan(periods))
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
                methods: IndexMap::from([("x".to_string(), RollupMethod::Sum)]),
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
                methods: IndexMap::from([("x".to_string(), RollupMethod::Sum)]),
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
    trimmed.periods.periods.remove(0);
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
        // D&A included in the reported costs above, added back in EBITDA.
        builder.value("depreciation", &series([4.0; 4]));
        builder.value("amortization", &series([1.0; 4]));
        builder.value("total_debt", &series([400.0, 410.0, 420.0, 460.0]));
        builder.value("total_equity", &series([200.0, 205.0, 210.0, 230.0]));
        for flow in ["revenue", "cogs", "opex", "depreciation", "amortization"] {
            builder
                .rollup(flow, RollupMethod::Sum)
                .expect("declared flow");
        }
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
    let year = PeriodId::annual(2025).expect("valid annual period");
    // Flows still sum.
    close(annual.get("fin.gross_profit", &year), 184.0);
    close(annual.get("fin.ebitda", &year), 112.0);
    // Ratios are recomputed on the annual inputs.
    close(annual.get("fin.gross_margin", &year), 184.0 / 460.0);
    close(annual.get("fin.debt_to_equity", &year), 2.0);
    close(annual.get("fin.debt_to_ebitda", &year), 460.0 / 112.0);
}

/// Every bundled metric declares how it rolls up: currency flows sum and
/// every other metric is recomputed.
#[test]
fn every_bundled_metric_declares_a_rollup_method() {
    use finstack_quant_statements::registry::{Registry, UnitType};
    let registry = Registry::with_builtins().expect("built-in metrics");
    let mut ratios = 0;
    for (id, metric) in registry.all_metrics() {
        let definition = &metric.definition;
        if definition.unit_type == Some(UnitType::Currency) {
            assert_eq!(
                definition.rollup,
                Some(RollupMethod::Sum),
                "{id} is a flow and declares sum"
            );
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

// Explicit methods, typed accumulation and provenance (STATEMENTS-EVALUATION-014)

use finstack_quant_core::money::Money;
use finstack_quant_statements::evaluator::{CellStatus, StatementResult, UnavailableReason};

fn h1() -> PeriodId {
    "2025H1".parse().expect("half-year id")
}

fn half_year_spec() -> RollupSpec {
    RollupSpec {
        frequency: PeriodKind::SemiAnnual,
        methods: IndexMap::new(),
    }
}

/// The record's example: two quarters of revenue, profit and closing cash in
/// USD and a margin, each declaring its method. `revenue` holds the quarters
/// given.
fn half_year(revenue: &[(u8, f64)]) -> (FinancialModelSpec, StatementResult) {
    let quarter = |values: &[(u8, f64)]| {
        usd(&values
            .iter()
            .map(|(quarter, value)| (q(2025, *quarter), *value))
            .collect::<Vec<_>>())
    };
    let mut builder = ModelBuilder::new("half-year");
    builder.periods("2025Q1..Q2", None).expect("valid periods");
    builder.value("revenue", &quarter(revenue));
    builder.value("profit", &quarter(&[(1, 20.0), (2, 30.0)]));
    builder.value("cash", &quarter(&[(1, 40.0), (2, 50.0)]));
    builder
        .compute("margin", "profit / revenue")
        .expect("valid formula");
    for (node, method) in [
        ("revenue", RollupMethod::Sum),
        ("profit", RollupMethod::Sum),
        ("cash", RollupMethod::PeriodEnd),
        ("margin", RollupMethod::Formula),
    ] {
        builder.rollup(node, method).expect("known node");
    }
    let model = builder.build().expect("valid model");
    let results = Evaluator::new().evaluate(&model).expect("evaluation");
    (model, results)
}

fn usd_amount(text: &str) -> Money {
    Money::from_decimal_str(text, Currency::USD).expect("amount")
}

#[test]
fn the_half_year_example_sums_flows_copies_the_close_and_recomputes_the_margin() {
    let (model, quarterly) = half_year(&[(1, 100.0), (2, 300.0)]);
    let half = quarterly
        .rollup(&model, &half_year_spec())
        .expect("half-year rollup");

    assert_eq!(half.get_money("revenue", &h1()), Some(usd_amount("400")));
    assert_eq!(half.get_money("profit", &h1()), Some(usd_amount("50")));
    assert_eq!(half.get_money("cash", &h1()), Some(usd_amount("50")));
    assert_eq!(half.get("margin", &h1()), Some(0.125));
    assert_eq!(
        half.rollup_provenance
            .as_ref()
            .expect("provenance")
            .source_periods[&h1()],
        [q(2025, 1), q(2025, 2)]
    );
}

#[test]
fn an_absent_flow_leaves_the_sum_and_the_ratio_unavailable_but_not_the_close() {
    let (model, quarterly) = half_year(&[(1, 100.0)]);
    let half = quarterly
        .rollup(&model, &half_year_spec())
        .expect("half-year rollup");

    for node in ["revenue", "margin"] {
        let cell = half.get_cell(node, &h1()).expect("cell");
        assert_eq!(cell.status, CellStatus::Unavailable, "{node}");
        assert_eq!(cell.reason, Some(UnavailableReason::MissingInput), "{node}");
        assert_eq!(half.get(node, &h1()), None);
    }
    assert_eq!(half.get_money("profit", &h1()), Some(usd_amount("50")));
    assert_eq!(half.get_money("cash", &h1()), Some(usd_amount("50")));
}

#[test]
fn a_node_with_no_method_fails_the_rollup_naming_it() {
    let (mut model, _) = half_year(&[(1, 100.0), (2, 300.0)]);
    for node in ["revenue", "cash"] {
        model.get_node_mut(node).expect("node").rollup = None;
    }
    let quarterly = Evaluator::new().evaluate(&model).expect("evaluation");

    let error = quarterly
        .rollup(&model, &half_year_spec())
        .expect_err("nothing is summed by default")
        .to_string();
    assert!(error.contains("revenue, cash"), "{error}");
    assert!(error.contains("no rollup method"), "{error}");

    // The request can supply the missing methods.
    let mut spec = half_year_spec();
    spec.methods.insert("revenue".into(), RollupMethod::Sum);
    spec.methods.insert("cash".into(), RollupMethod::PeriodEnd);
    let half = quarterly
        .rollup(&model, &spec)
        .expect("resolved by request");
    assert_eq!(half.get_money("revenue", &h1()), Some(usd_amount("400")));
}

#[test]
fn results_of_another_model_or_without_a_run_are_rejected() {
    let (model, quarterly) = half_year(&[(1, 100.0), (2, 300.0)]);
    let (other, _) = half_year(&[(1, 100.0), (2, 301.0)]);
    let error = quarterly
        .rollup(&other, &half_year_spec())
        .expect_err("another model's results")
        .to_string();
    assert!(error.contains("not a verified run"), "{error}");

    let mut hand_built = quarterly.clone();
    hand_built.provenance = None;
    assert!(hand_built.rollup(&model, &half_year_spec()).is_err());

    // A rolled result is not a run and cannot be rolled again.
    let half = quarterly.rollup(&model, &half_year_spec()).expect("rollup");
    let annual = RollupSpec {
        frequency: PeriodKind::Annual,
        methods: IndexMap::new(),
    };
    assert!(half.rollup(&model, &annual).is_err());
}

fn scalar_year(
    values: [f64; 4],
    configure: impl FnOnce(&mut ModelBuilder),
) -> (FinancialModelSpec, StatementResult) {
    let series: Vec<(PeriodId, f64)> = (1..=4_u8)
        .zip(values)
        .map(|(i, v)| (q(2025, i), v))
        .collect();
    let mut builder = ModelBuilder::new("scalar-year");
    builder.periods("2025Q1..Q4", None).expect("valid periods");
    builder.value_scalar("x", &series);
    builder.rollup("x", RollupMethod::Sum).expect("known node");
    configure(&mut builder);
    let model = builder.build().expect("valid model");
    let results = Evaluator::new().evaluate(&model).expect("evaluation");
    (model, results)
}

#[test]
fn a_scalar_sum_is_compensated_money_is_exact_and_an_overflow_is_an_error() {
    let year = PeriodId::annual(2025).expect("valid annual period");
    // Ordinary summation loses both units: 1e16 + 1 + 1 - 1e16 gives 0.
    // The result does not depend on where the large terms fall.
    for quarters in [[1e16, 1.0, 1.0, -1e16], [1e16, 1.0, -1e16, 1.0]] {
        let (model, results) = scalar_year(quarters, |_| {});
        let annual = results
            .rollup(&model, &empty_annual_spec())
            .expect("rollup");
        assert_eq!(annual.get("x", &year), Some(2.0), "{quarters:?}");
    }

    let (model, results) = scalar_year([f64::MAX, f64::MAX, 0.0, 0.0], |_| {});
    let error = results
        .rollup(&model, &empty_annual_spec())
        .expect_err("the sum cannot be represented")
        .to_string();
    assert!(error.contains("'x'") || error.contains("x"), "{error}");

    // Money: ten cents and twenty cents are thirty cents, exactly.
    let (model, quarterly) = half_year(&[(1, 0.1), (2, 0.2)]);
    let half = quarterly
        .rollup(&model, &half_year_spec())
        .expect("half-year rollup");
    assert_eq!(half.get_money("revenue", &h1()), Some(usd_amount("0.30")));
}

#[test]
fn period_end_copies_the_status_of_the_final_cell() {
    let year = PeriodId::annual(2025).expect("valid annual period");
    let (model, results) = scalar_year([1.0, 2.0, 3.0, 4.0], |builder| {
        // Masked in the fourth quarter only.
        builder.compute("masked", "x").expect("formula");
        builder.where_clause("masked", "x < 4").expect("mask");
        builder
            .rollup("masked", RollupMethod::PeriodEnd)
            .expect("node");
        // No change is defined before the first quarter, only after it.
        builder.compute("early", "lag(x, 3)").expect("formula");
        builder
            .rollup("early", RollupMethod::PeriodEnd)
            .expect("node");
        builder.compute("late", "lag(x, 4)").expect("formula");
        builder
            .rollup("late", RollupMethod::PeriodEnd)
            .expect("node");
        // A mask re-evaluated on the rolled inputs: the year's x is 10.
        builder.compute("ratio", "x / 2").expect("formula");
        builder.where_clause("ratio", "x < 10").expect("mask");
        builder
            .rollup("ratio", RollupMethod::Formula)
            .expect("node");
    });
    let annual = results
        .rollup(&model, &empty_annual_spec())
        .expect("rollup");
    let status = |node: &str| annual.get_cell(node, &year).expect("cell").status;

    assert_eq!(status("masked"), CellStatus::Masked);
    assert_eq!(annual.get("early", &year), Some(1.0));
    assert_eq!(status("late"), CellStatus::Unavailable);
    // A false mask is a masked cell, never a zero.
    assert_eq!(status("ratio"), CellStatus::Masked);
    assert_eq!(annual.get("ratio", &year), None);
}

#[test]
fn a_formula_invalid_at_the_target_cadence_is_an_error() {
    let months: Vec<(PeriodId, f64)> = (1..=6)
        .map(|m| (PeriodId::month(2025, m).expect("valid month"), f64::from(m)))
        .collect();
    let mut builder = ModelBuilder::new("cadence");
    builder.periods("2025M1..M6", None).expect("valid periods");
    builder.value_scalar("sales", &months);
    builder.rollup("sales", RollupMethod::Sum).expect("node");
    builder.compute("sales_qtd", "qtd(sales)").expect("formula");
    builder
        .rollup("sales_qtd", RollupMethod::Formula)
        .expect("node");
    let model = builder.build().expect("valid model");
    let monthly = Evaluator::new().evaluate(&model).expect("evaluation");

    let spec = RollupSpec {
        frequency: PeriodKind::Quarterly,
        methods: IndexMap::new(),
    };
    let error = monthly
        .rollup(&model, &spec)
        .expect_err("qtd is a monthly function")
        .to_string();
    assert!(error.contains("monthly"), "{error}");

    // Copying the quarter-end value is the method for a to-date measure.
    let mut spec = spec;
    spec.methods
        .insert("sales_qtd".into(), RollupMethod::PeriodEnd);
    let quarterly = monthly.rollup(&model, &spec).expect("rollup");
    assert_eq!(quarterly.get("sales_qtd", &q(2025, 1)), Some(6.0));
}
