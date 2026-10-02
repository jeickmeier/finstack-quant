//! Regressions for statement DSL units and missing-value/ranking contracts.

use finstack_quant_core::expr::{Expr, Function};
use finstack_quant_statements::dsl::parse_and_compile;
use finstack_quant_statements::evaluator::{formula::evaluate_formula, EvaluationContext};
use finstack_quant_statements::prelude::*;
use finstack_quant_statements::types::NodeValueType;
use indexmap::IndexMap;
use std::sync::Arc;

#[test]
fn monetary_quantile_preserves_currency_and_supports_same_currency_arithmetic() {
    let q1 = PeriodId::quarter(2025, 1).expect("quarter");
    let q2 = PeriodId::quarter(2025, 2).expect("quarter");
    let q3 = PeriodId::quarter(2025, 3).expect("quarter");
    let model = ModelBuilder::new("usd_quantile")
        .periods("2025Q1..Q3", None)
        .expect("periods")
        .value(
            "cash_flow",
            &[
                (
                    q1,
                    AmountOrScalar::amount(100.0, Currency::USD).expect("USD amount"),
                ),
                (
                    q2,
                    AmountOrScalar::amount(300.0, Currency::USD).expect("USD amount"),
                ),
                (
                    q3,
                    AmountOrScalar::amount(200.0, Currency::USD).expect("USD amount"),
                ),
            ],
        )
        .compute("lower_quartile", "quantile(cash_flow, 0.25)")
        .expect("quantile formula")
        .compute("combined", "lower_quartile + cash_flow")
        .expect("USD arithmetic")
        .build()
        .expect("quantile carries USD through dependent formulas");

    let result = Evaluator::new().evaluate(&model).expect("evaluation");
    assert_eq!(result.get("lower_quartile", &q3), Some(150.0));
    assert_eq!(
        result.node_value_types.get("lower_quartile"),
        Some(&NodeValueType::Monetary {
            currency: Currency::USD
        })
    );
    let quantile = result
        .get_money("lower_quartile", &q3)
        .expect("USD quantile");
    assert_eq!(quantile.currency(), Currency::USD);
    assert_eq!(quantile.amount(), 150.0);
    assert_eq!(
        result.get_money("combined", &q3).expect("USD sum").amount(),
        350.0
    );
}

#[test]
fn scalar_quantile_remains_scalar() {
    let q1 = PeriodId::quarter(2025, 1).expect("quarter");
    let q2 = PeriodId::quarter(2025, 2).expect("quarter");
    let model = ModelBuilder::new("scalar_quantile")
        .periods("2025Q1..Q2", None)
        .expect("periods")
        .value("margin", &[(q1, 0.1.into()), (q2, 0.3.into())])
        .compute("median_margin", "quantile(margin, 0.5)")
        .expect("formula")
        .build()
        .expect("scalar model");
    let result = Evaluator::new().evaluate(&model).expect("evaluation");
    assert_eq!(result.get("median_margin", &q2), Some(0.2));
    assert_eq!(
        result.node_value_types.get("median_margin"),
        Some(&NodeValueType::Scalar)
    );
    assert!(result.get_money("median_margin", &q2).is_none());
}

#[test]
fn quantile_rejects_a_monetary_level_and_cross_currency_arithmetic() {
    let q1 = PeriodId::quarter(2025, 1).expect("quarter");
    for (formula, expected_message) in [
        ("quantile(usd_flow, eur_flow)", "quantile level"),
        (
            "quantile(usd_flow, 0.5) + eur_flow",
            "cannot combine USD and EUR",
        ),
    ] {
        let error = ModelBuilder::new("invalid_quantile_units")
            .periods("2025Q1..Q1", None)
            .expect("periods")
            .value(
                "usd_flow",
                &[(
                    q1,
                    AmountOrScalar::amount(100.0, Currency::USD).expect("USD amount"),
                )],
            )
            .value(
                "eur_flow",
                &[(
                    q1,
                    AmountOrScalar::amount(0.5, Currency::EUR).expect("EUR amount"),
                )],
            )
            .compute("result", formula)
            .expect("parseable formula")
            .build()
            .expect_err("invalid quantile units must fail construction");
        assert!(
            error.to_string().contains(expected_message),
            "{formula}: {error}"
        );
    }
}

#[test]
fn coalesce_skips_nan_and_both_infinities_and_retains_finite_zero() {
    let period = PeriodId::quarter(2025, 1).expect("quarter");
    let model = ModelBuilder::new("nonfinite_fallbacks")
        .periods("2025Q1..Q1", None)
        .expect("periods")
        .compute("positive_infinity", "coalesce(exp(1000), 7)")
        .expect("formula")
        .compute("negative_infinity", "coalesce(-exp(1000), 8)")
        .expect("formula")
        .compute("mixed", "coalesce(0 / 0, exp(1000), -exp(1000), 9)")
        .expect("formula")
        .compute("all_missing", "coalesce(exp(1000), -exp(1000), 0 / 0)")
        .expect("formula")
        .compute("zero", "coalesce(0, 10)")
        .expect("formula")
        .compute("short_circuit", "coalesce(3, 1 / 0)")
        .expect("formula")
        .build()
        .expect("model");
    let result = Evaluator::new().evaluate(&model).expect("evaluation");
    for (node, expected) in [
        ("positive_infinity", 7.0),
        ("negative_infinity", 8.0),
        ("mixed", 9.0),
        ("zero", 0.0),
        ("short_circuit", 3.0),
    ] {
        assert_eq!(result.get(node, &period), Some(expected), "{node}");
    }
    assert!(result
        .get("all_missing", &period)
        .expect("NaN result")
        .is_nan());
    assert!(!result.meta.warnings.iter().any(|warning| matches!(
        warning,
        finstack_quant_statements::evaluator::EvalWarning::DivisionByZero { node_id, .. }
            if node_id == "short_circuit"
    )));
}

#[test]
fn ranks_honor_direction_and_minimum_ties_while_skipping_missing_history() {
    let periods: Vec<_> = (1..=6)
        .map(|month| PeriodId::month(2025, month).expect("month"))
        .collect();
    let observations: Vec<_> = periods
        .iter()
        .copied()
        .zip([20.0, 30.0, f64::NAN, 30.0, 20.0, 10.0].map(AmountOrScalar::scalar))
        .collect();
    let model = ModelBuilder::new("ranking")
        .periods("2025M01..M06", None)
        .expect("periods")
        .value("observations", &observations)
        .compute("descending_flag", "0")
        .expect("formula")
        .compute("default_rank", "rank(observations)")
        .expect("formula")
        .compute("ascending_rank", "rank(observations, 1)")
        .expect("formula")
        .compute("descending_rank", "rank(observations, descending_flag)")
        .expect("formula")
        .build()
        .expect("model");
    let result = Evaluator::new().evaluate(&model).expect("evaluation");
    for (index, ascending, descending) in [(3, 2.0, 1.0), (4, 1.0, 3.0), (5, 1.0, 5.0)] {
        assert_eq!(result.get("default_rank", &periods[index]), Some(ascending));
        assert_eq!(
            result.get("ascending_rank", &periods[index]),
            Some(ascending)
        );
        assert_eq!(
            result.get("descending_rank", &periods[index]),
            Some(descending)
        );
    }
    for node in ["default_rank", "ascending_rank", "descending_rank"] {
        assert!(result
            .get(node, &periods[2])
            .expect("missing rank")
            .is_nan());
    }
}

#[test]
fn rank_rejects_invalid_arity_at_compilation_and_standalone_evaluation() {
    for formula in ["rank()", "rank(x, 1, 2)"] {
        let error = parse_and_compile(formula).expect_err("invalid arity");
        assert!(error
            .to_string()
            .contains("rank() requires 1 or 2 arguments"));
    }
    for args in [Vec::new(), vec![Expr::literal(1.0); 3]] {
        let mut context = EvaluationContext::new(
            PeriodId::quarter(2025, 1).expect("quarter"),
            Arc::new(IndexMap::new()),
            Arc::new(IndexMap::new()),
        );
        let error = evaluate_formula(
            &Expr::call(Function::Rank, args),
            &mut context,
            Some("rank"),
        )
        .expect_err("direct expressions must also validate arity");
        assert!(error
            .to_string()
            .contains("rank() requires 1 or 2 arguments"));
    }
}

#[test]
fn rank_rejects_nonfinite_and_monetary_direction_flags() {
    let period = PeriodId::quarter(2025, 1).expect("quarter");
    for direction in ["exp(1000)", "0 / 0"] {
        let model = ModelBuilder::new("invalid_rank_direction")
            .periods("2025Q1..Q1", None)
            .expect("periods")
            .value("observations", &[(period, 10.0.into())])
            .compute("ranked", format!("rank(observations, {direction})"))
            .expect("formula")
            .build()
            .expect("dimensionless flag");
        let error = Evaluator::new()
            .evaluate(&model)
            .expect_err("nonfinite direction");
        assert!(error
            .to_string()
            .contains("rank() ascending flag must be finite"));
    }
    let error = ModelBuilder::new("monetary_rank_direction")
        .periods("2025Q1..Q1", None)
        .expect("periods")
        .value("observations", &[(period, 10.0.into())])
        .value(
            "direction",
            &[(
                period,
                AmountOrScalar::amount(1.0, Currency::USD).expect("USD amount"),
            )],
        )
        .compute("ranked", "rank(observations, direction)")
        .expect("formula")
        .build()
        .expect_err("direction flags must be scalar");
    assert!(error.to_string().contains("rank ascending flag"));
}
