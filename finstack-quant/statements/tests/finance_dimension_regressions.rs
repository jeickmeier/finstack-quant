//! Financial dimension regressions at the canonical model-build boundary.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::FinancialModelSpec;
use finstack_quant_statements::types::NodeValueType;

fn model_with_formula(formula: &str) -> finstack_quant_statements::Result<FinancialModelSpec> {
    let q1 = PeriodId::quarter(2025, 1).expect("valid period");
    let q2 = PeriodId::quarter(2025, 2).expect("valid period");
    {
        let mut model_builder = ModelBuilder::new("dimension-regression");
        model_builder.periods("2025Q1..Q2", None)?;
        model_builder.value_money(
            "usd",
            &[
                (q1, Money::from((100_i64, Currency::USD))),
                (q2, Money::from((120_i64, Currency::USD))),
            ],
        );
        model_builder.value_money(
            "eur",
            &[
                (q1, Money::from((1_i64, Currency::EUR))),
                (q2, Money::from((2_i64, Currency::EUR))),
            ],
        );
        model_builder.value_scalar("rate", &[(q1, 0.1), (q2, 0.2)]);
        model_builder.compute("output", formula)?;
        model_builder.build()
    }
}

fn assert_dimension_error(formula: &str) {
    let error = model_with_formula(formula)
        .expect_err("invalid financial dimensions must fail during model construction");
    assert!(
        error.to_string().contains("Dimensional mismatch"),
        "{formula} failed for an unrelated reason: {error}"
    );
}

#[test]
fn nonlinear_functions_cannot_launder_currency_into_scalars() {
    for formula in [
        "pow(usd, 1) + pow(eur, 1)",
        "pow(usd, 0)",
        "pow(rate, eur)",
        "ln(usd)",
        "exp(usd)",
        "log10(usd)",
        "sqrt(usd)",
        "cumprod(usd)",
        "annualize_rate(usd, 4, 1)",
        "annualize_rate(rate, eur, 1)",
        "annualize_rate(rate, 4, eur)",
    ] {
        assert_dimension_error(formula);
    }
}

#[test]
fn all_series_parameter_families_require_scalar_parameters() {
    for formula in [
        "lag(usd, eur)",
        "shift(usd, eur)",
        "diff(usd, eur)",
        "pct_change(usd, eur)",
        "growth_rate(usd, eur)",
        "rolling_mean(usd, eur)",
        "rolling_mean(usd, 2, eur)",
        "rolling_sum(usd, eur)",
        "rolling_sum(usd, 2, eur)",
        "rolling_min(usd, eur)",
        "rolling_min(usd, 2, eur)",
        "rolling_max(usd, eur)",
        "rolling_max(usd, 2, eur)",
        "rolling_median(usd, eur)",
        "rolling_median(usd, 2, eur)",
        "rolling_std(usd, eur)",
        "rolling_std(usd, 2, eur)",
        "rolling_var(usd, eur)",
        "rolling_var(usd, 2, eur)",
        "rolling_count(usd, eur)",
        "rolling_count(usd, 2, eur)",
        "ewm_mean(usd, eur)",
        "ewm_std(usd, eur)",
        "ewm_std(usd, 0.5, eur)",
        "ewm_var(usd, eur)",
        "ewm_var(usd, 0.5, eur)",
        "quantile(usd, eur)",
        "rank(usd, eur)",
        "fiscal_ytd(usd, eur)",
        "annualize(usd, eur)",
        "round(usd, eur)",
    ] {
        assert_dimension_error(formula);
    }
}

#[test]
fn valid_series_transforms_preserve_currency() {
    for formula in [
        "abs(usd)",
        "lag(usd, 1)",
        "shift(usd, 1)",
        "cumsum(usd)",
        "cummin(usd)",
        "cummax(usd)",
        "diff(usd, 1)",
        "rolling_mean(usd, 2, 1)",
        "rolling_sum(usd, 2, 1)",
        "rolling_min(usd, 2, 1)",
        "rolling_max(usd, 2, 1)",
        "rolling_median(usd, 2, 1)",
        "rolling_std(usd, 2, 1)",
        "ewm_mean(usd, 0.5)",
        "ewm_std(usd, 0.5, 1)",
        "median(usd)",
        "std(usd)",
        "quantile(usd, 0.5)",
        "ttm(usd)",
        "ytd(usd)",
        "qtd(usd)",
        "fiscal_ytd(usd, 4)",
        "annualize(usd, 4)",
        "round(usd, 2)",
        "floor(usd)",
        "ceil(usd)",
    ] {
        let model = model_with_formula(formula)
            .unwrap_or_else(|error| panic!("{formula} should build: {error}"));
        assert_eq!(
            model.get_node("output").expect("output node").value_type,
            Some(NodeValueType::Monetary {
                currency: Currency::USD,
            }),
            "{formula} lost its currency"
        );
        assert_dimension_error(&format!("{formula} + eur"));
    }
}

#[test]
fn genuine_monetary_ratios_counts_and_predicates_remain_scalar() {
    for formula in [
        "usd / usd",
        "sign(usd)",
        "pct_change(usd, 1)",
        "growth_rate(usd, 1)",
        "rolling_count(usd, 2, 1)",
        "rank(usd, 1)",
        "is_missing(usd)",
        "pow(rate, 2)",
        "ln(rate)",
        "exp(rate)",
        "log10(rate)",
        "sqrt(rate)",
        "cumprod(rate)",
        "annualize_rate(rate, 4, 1)",
        "var(rate)",
        "rolling_var(rate, 2, 1)",
        "ewm_var(rate, 0.5, 1)",
    ] {
        let model = model_with_formula(formula)
            .unwrap_or_else(|error| panic!("{formula} should build: {error}"));
        assert_eq!(
            model.get_node("output").expect("output node").value_type,
            Some(NodeValueType::Scalar),
            "{formula} should be dimensionless"
        );
    }
}

#[test]
fn monetary_variance_is_rejected_and_monetary_operands_are_not_hidden() {
    // STATEMENTS-CONTRACTS-005 #1: squared currency is not a supported unit.
    for formula in [
        "var(usd)",
        "rolling_var(usd, 2)",
        "ewm_var(usd, 0.5)",
        "pow(var(usd), 1)",
        "ln(var(usd))",
        "sqrt(var(usd))",
    ] {
        let Err(error) = model_with_formula(formula) else {
            panic!("{formula} has squared-currency units and must be rejected")
        };
        assert!(
            error.to_string().contains("squared-currency units"),
            "{formula}: {error}"
        );
    }
    for formula in [
        "pow(usd, eur)",
        "usd % eur",
        "usd and eur",
        "not usd",
        "if(usd, rate, rate)",
    ] {
        assert_dimension_error(formula);
    }
}

#[test]
fn currency_preserving_series_and_scalar_growth_evaluate_together() {
    let model = model_with_formula("lag(usd, 1) * (1 + pct_change(usd, 1))")
        .expect("amount times scalar growth");
    let results = Evaluator::new().evaluate(&model).expect("evaluation");
    let q2 = PeriodId::quarter(2025, 2).expect("valid period");
    let amount = results.get_money("output", &q2).expect("monetary output");
    assert_eq!(amount.currency(), Currency::USD);
    assert!((amount.amount() - 120.0).abs() < 1e-9);
}

/// STATEMENTS-CONTRACTS-005 #2: one case per row of the unit rules.
#[test]
fn unit_rules_resolve_every_supported_operation() {
    let usd = Some(NodeValueType::Monetary {
        currency: Currency::USD,
    });
    let scalar = Some(NodeValueType::Scalar);
    for (formula, expected) in [
        // Addition, subtraction and selection keep matching units.
        ("usd + usd", usd),
        ("usd - usd", usd),
        ("rate + rate", scalar),
        ("if(rate > 0, usd, usd)", usd),
        ("coalesce(usd, usd)", usd),
        ("min(usd, usd)", usd),
        ("clamp(usd, usd, usd)", usd),
        // Comparisons return Scalar.
        ("usd > usd", scalar),
        ("rate == rate", scalar),
        // Multiplication: scalars, or scalar scaling of money.
        ("rate * rate", scalar),
        ("usd * rate", usd),
        ("2 * usd", usd),
        // Division: scalar/scalar, money/scalar, same-currency money ratio.
        ("rate / rate", scalar),
        ("usd / 2", usd),
        ("usd / usd", scalar),
        // Aggregates and level transforms keep the input unit, including a
        // monetary standard deviation.
        ("sum(usd, usd)", usd),
        ("mean(usd, usd)", usd),
        ("std(usd)", usd),
        ("rolling_std(usd, 2)", usd),
        ("ewm_std(usd, 0.5)", usd),
        ("median(usd)", usd),
        ("cumsum(usd)", usd),
        ("lag(usd, 1)", usd),
        ("diff(usd)", usd),
        ("ttm(usd)", usd),
        ("round(usd, 2)", usd),
        ("abs(-usd)", usd),
        ("quantile(usd, 0.5)", usd),
        // Counts, ranks, signs, missingness, changes and growth are Scalar.
        ("rolling_count(usd, 2)", scalar),
        ("rank(usd)", scalar),
        ("sign(usd)", scalar),
        ("is_missing(usd)", scalar),
        ("pct_change(usd)", scalar),
        ("growth_rate(usd, 1)", scalar),
        // Scalar-only operations on scalars, including variance.
        ("var(rate)", scalar),
        ("rate % 2", scalar),
        ("rate > 0 and not (rate > 1)", scalar),
        ("cumprod(1 + rate)", scalar),
        ("annualize_rate(rate, 4, 1)", scalar),
        ("sqrt(pow(rate, 2))", scalar),
    ] {
        let model = model_with_formula(formula)
            .unwrap_or_else(|error| panic!("{formula} should build: {error}"));
        assert_eq!(model.nodes["output"].value_type, expected, "{formula}");
    }

    for formula in [
        "usd + eur",
        "usd + rate",
        "usd + 0",
        "usd > 0",
        "min(usd, eur)",
        "if(rate > 0, usd, eur)",
        "if(usd, rate, rate)",
        "usd * usd",
        "usd * eur",
        "rate / usd",
        "usd / eur",
        "usd % 2",
        "usd and rate",
        "cumprod(usd)",
        "annualize_rate(usd, 4, 1)",
        "sqrt(usd)",
        "lag(usd, eur)",
    ] {
        assert_dimension_error(formula);
    }
}

/// Clause 1: a declared unit must agree with inference, every unit resolves
/// before execution, and a corkscrew resolves from its flow.
#[test]
fn declared_units_agree_and_every_unit_resolves() {
    use finstack_quant_statements::types::{NodeSpec, NodeType};

    let q1 = PeriodId::quarter(2025, 1).expect("valid period");
    let builder = || {
        let mut builder = ModelBuilder::new("units");
        builder.periods("2025Q1..Q2", None).unwrap();
        builder.value_money("flow", &[(q1, Money::from((10_i64, Currency::USD)))]);
        builder
    };
    let usd = NodeValueType::Monetary {
        currency: Currency::USD,
    };

    let mut declared = NodeSpec::new("total", NodeType::Calculated).with_formula("flow * 2");
    declared.value_type = Some(NodeValueType::Scalar);
    let mut disagreeing = builder();
    disagreeing.insert_node(declared.clone());
    let error = disagreeing
        .build()
        .expect_err("a declared unit must agree with inference");
    assert!(error.to_string().contains("declares"), "{error}");
    declared.value_type = Some(usd);
    let mut agreeing = builder();
    agreeing.insert_node(declared);
    agreeing
        .build()
        .expect("an agreeing declaration is accepted");

    // A balance that reads only its own lag resolves from the flow it adds.
    let mut corkscrew = builder();
    corkscrew
        .compute("balance", "lag(balance, 1) + flow")
        .unwrap();
    corkscrew.compute("ratio", "flow / balance").unwrap();
    let model = corkscrew
        .build()
        .expect("a corkscrew resolves from its flow");
    assert_eq!(model.nodes["balance"].value_type, Some(usd));
    assert_eq!(model.nodes["ratio"].value_type, Some(NodeValueType::Scalar));

    // Nothing anchors these units, so the model is not runnable.
    let mut unanchored = builder();
    unanchored.compute("drift", "lag(drift, 1) * 2").unwrap();
    let error = unanchored
        .build()
        .expect_err("an unanchored unit is unresolved");
    assert!(
        error
            .to_string()
            .contains("Cannot resolve the units of formula node 'drift'"),
        "{error}"
    );
    let mut declared_drift =
        NodeSpec::new("drift", NodeType::Calculated).with_formula("lag(drift, 1) * 2");
    declared_drift.value_type = Some(NodeValueType::Scalar);
    let mut anchored = builder();
    anchored.insert_node(declared_drift);
    anchored.build().expect("a declaration resolves the unit");

    // A provisional resolution is still checked: money cannot equal a scalar.
    let mut inconsistent = builder();
    inconsistent
        .compute("balance", "lag(balance, 1) + flow")
        .unwrap();
    inconsistent.compute("bad", "balance + 1").unwrap();
    inconsistent
        .build()
        .expect_err("the final pass checks every formula against resolved units");
}
