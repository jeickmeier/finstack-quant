//! Rolling statement results up to coarser periods.
#![allow(clippy::expect_used)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{PeriodId, PeriodKind};
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::{Evaluator, RollupMethod, RollupSpec};
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec, NodeValueType};
use indexmap::IndexMap;

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
    use finstack_quant_core::dates::{build_fiscal_periods, FiscalConfig};

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
