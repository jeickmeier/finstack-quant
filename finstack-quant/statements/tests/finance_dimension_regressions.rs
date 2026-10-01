//! Financial dimension regressions at the canonical model-build boundary.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_statements::prelude::*;
use finstack_quant_statements::types::NodeValueType;

fn model_with_formula(
    formula: &str,
) -> finstack_quant_statements::error::Result<FinancialModelSpec> {
    let q1 = PeriodId::quarter(2025, 1).expect("valid period");
    let q2 = PeriodId::quarter(2025, 2).expect("valid period");
    ModelBuilder::new("dimension-regression")
        .periods("2025Q1..Q2", None)?
        .value_money(
            "usd",
            &[
                (q1, Money::from((100_i64, Currency::USD))),
                (q2, Money::from((120_i64, Currency::USD))),
            ],
        )
        .value_money(
            "eur",
            &[
                (q1, Money::from((1_i64, Currency::EUR))),
                (q2, Money::from((2_i64, Currency::EUR))),
            ],
        )
        .value_scalar("rate", &[(q1, 0.1), (q2, 0.2)])
        .compute("output", formula)?
        .build()
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
        "ltm(usd)",
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
fn unknown_units_do_not_certify_scalar_math_or_hide_monetary_operands() {
    for formula in ["var(usd)", "pow(var(usd), 1)", "ln(var(usd))"] {
        let model = model_with_formula(formula).expect("squared units remain unrepresented");
        assert_eq!(
            model.get_node("output").expect("output node").value_type,
            None,
            "{formula} must not claim unknown squared monetary units are scalar"
        );
    }
    for formula in [
        "pow(var(usd), eur)",
        "var(usd) % eur",
        "var(usd) and eur",
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
