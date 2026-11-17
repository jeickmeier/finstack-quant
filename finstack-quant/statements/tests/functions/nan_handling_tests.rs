//! Checked scalar formulas and unavailable values (STATEMENTS-EVALUATION-002).

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::{
    CellStatus, Evaluator, StatementResult, UnavailableReason,
};
use finstack_quant_statements::types::AmountOrScalar;

fn q1() -> PeriodId {
    PeriodId::quarter(2025, 1).expect("valid period fixture")
}

/// Evaluate named formulas over one quarter. `gone` is unavailable in that
/// quarter (a lag reaching before the first period) and `seed` is 10.
fn evaluate(formulas: &[(&str, &str)]) -> finstack_quant_statements::Result<StatementResult> {
    let mut builder = ModelBuilder::new("test");
    builder.periods("2025Q1..2025Q1", None).unwrap();
    builder.value("seed", &[(q1(), AmountOrScalar::scalar(10.0))]);
    builder.compute("gone", "lag(seed, 1)").unwrap();
    for (node, formula) in formulas {
        builder.compute(*node, *formula)?;
    }
    Evaluator::new().evaluate(&builder.build()?)
}

/// The value of `formula`, or `None` when it is unavailable.
fn value(formula: &str) -> Option<f64> {
    let results = evaluate(&[("result", formula)]).expect("evaluation");
    let cell = results.get_cell("result", &q1()).expect("cell");
    match cell.status {
        CellStatus::Value => cell.numeric(),
        CellStatus::Unavailable => {
            assert_eq!(cell.reason, Some(UnavailableReason::MissingInput));
            None
        }
        other => panic!("unexpected status {other:?}"),
    }
}

/// The error text of a formula that must fail the run.
fn failure(formula: &str) -> String {
    evaluate(&[("result", formula)])
        .expect_err("the formula must fail")
        .to_string()
}

#[test]
fn aggregates_need_every_operand_to_be_available() {
    assert_eq!(value("sum(seed, 20, 30)"), Some(60.0));
    assert_eq!(value("mean(seed, 20, 30)"), Some(20.0));
    assert_eq!(value("min(seed, 20, 30)"), Some(10.0));
    assert_eq!(value("max(seed, 20, 30)"), Some(30.0));
    for function in ["sum", "mean", "min", "max"] {
        assert_eq!(
            value(&format!("{function}(seed, gone, 20)")),
            None,
            "{function}() must not skip an unavailable operand"
        );
        assert_eq!(value(&format!("{function}(gone, gone)")), None);
    }
}

#[test]
fn coalesce_returns_the_first_available_value_including_zero() {
    assert_eq!(value("coalesce(gone, 0, 10)"), Some(0.0));
    assert_eq!(value("coalesce(seed, gone)"), Some(10.0));
    assert_eq!(value("coalesce(gone, gone)"), None);
    // Arguments after the first available one are not evaluated.
    assert_eq!(value("coalesce(seed, 1 / 0)"), Some(10.0));
    // An error in an evaluated argument is not recovered.
    assert!(failure("coalesce(1 / 0, seed)").contains("division by zero"));
}

#[test]
fn is_missing_is_true_only_for_an_unavailable_value() {
    assert_eq!(value("if(is_missing(gone), 1, 2)"), Some(1.0));
    assert_eq!(value("if(is_missing(seed), 1, 2)"), Some(2.0));
    assert_eq!(value("if(is_missing(0), 1, 2)"), Some(2.0));
    assert!(failure("if(is_missing(1 / 0), 1, 2)").contains("division by zero"));
}

/// Truth table of each control over available, unavailable and error
/// operands. `T` and `F` are true and false, `U` is unavailable.
#[test]
fn controls_are_lazy_and_propagate_an_unavailable_condition() {
    let flag = |formula: &str| value(&format!("if({formula}, 1, 0)"));
    let (t, f, u) = ("seed > 0", "seed < 0", "gone > 0");
    let error = "1 / 0 > 0";

    // and: a false operand decides, and a false left skips the right.
    assert_eq!(flag(&format!("({t}) and ({t})")), Some(1.0));
    assert_eq!(flag(&format!("({t}) and ({f})")), Some(0.0));
    assert_eq!(flag(&format!("({f}) and ({u})")), Some(0.0));
    assert_eq!(flag(&format!("({f}) and ({error})")), Some(0.0));
    assert_eq!(flag(&format!("({t}) and ({u})")), None);
    assert_eq!(flag(&format!("({u}) and ({f})")), Some(0.0));
    assert_eq!(flag(&format!("({u}) and ({t})")), None);
    assert_eq!(flag(&format!("({u}) and ({u})")), None);
    assert!(failure(&format!("if(({u}) and ({error}), 1, 0)")).contains("division by zero"));
    assert!(failure(&format!("if(({t}) and ({error}), 1, 0)")).contains("division by zero"));

    // or: a true operand decides, and a true left skips the right.
    assert_eq!(flag(&format!("({f}) or ({f})")), Some(0.0));
    assert_eq!(flag(&format!("({f}) or ({t})")), Some(1.0));
    assert_eq!(flag(&format!("({t}) or ({u})")), Some(1.0));
    assert_eq!(flag(&format!("({t}) or ({error})")), Some(1.0));
    assert_eq!(flag(&format!("({f}) or ({u})")), None);
    assert_eq!(flag(&format!("({u}) or ({t})")), Some(1.0));
    assert_eq!(flag(&format!("({u}) or ({f})")), None);
    assert_eq!(flag(&format!("({u}) or ({u})")), None);
    assert!(failure(&format!("if(({f}) or ({error}), 1, 0)")).contains("division by zero"));

    // not
    assert_eq!(flag(&format!("!({f})")), Some(1.0));
    assert_eq!(flag(&format!("!({u})")), None);

    // if: only the selected branch is evaluated.
    assert_eq!(value(&format!("if({t}, seed, 1 / 0)")), Some(10.0));
    assert_eq!(value(&format!("if({f}, 1 / 0, seed)")), Some(10.0));
    assert_eq!(value(&format!("if({t}, gone, seed)")), None);
    assert_eq!(value(&format!("if({u}, 1, 2)")), None);
    assert!(failure(&format!("if({error}, 1, 2)")).contains("division by zero"));
    assert!(failure(&format!("if({t}, 1 / 0, 2)")).contains("division by zero"));
}

#[test]
fn controls_must_be_boolean() {
    for formula in [
        "if(seed, 1, 2)",
        "if(1, 1, 2)",
        "seed and seed > 0",
        "!seed",
    ] {
        let error = failure(formula);
        assert!(error.contains("must be Boolean"), "{formula}: {error}");
    }
    // A Boolean is not a number, and a branch cannot mix the two.
    for formula in [
        "(seed > 0) + 1",
        "if(seed > 0, 1, seed > 5)",
        "sum(seed > 0, 1)",
    ] {
        let error = failure(formula);
        assert!(error.contains("Boolean"), "{formula}: {error}");
    }
    // A Boolean node is published as 1 or 0 and is a number when read back.
    let results = evaluate(&[("flag", "seed > 0"), ("uses", "if(flag == 1, 5, 6)")]).unwrap();
    assert_eq!(results.get("flag", &q1()), Some(1.0));
    assert_eq!(results.get("uses", &q1()), Some(5.0));
}

#[test]
fn invalid_arithmetic_is_an_error_naming_the_node_and_period() {
    for (formula, cause) in [
        ("1 / 0", "division by zero"),
        ("seed % 0", "remainder by zero"),
        ("ln(0 - 5)", "ln() is undefined"),
        ("log10(0)", "log10() is undefined"),
        ("sqrt(0 - 1)", "sqrt() is undefined"),
        ("exp(1000)", "exp() is undefined or overflows"),
        ("pow(10, 400)", "pow() is undefined or overflows"),
        ("pow(0 - 8, 0.5)", "pow() is undefined or overflows"),
        ("1e308 * 10", "multiplication is undefined or overflows"),
        ("clamp(seed, 5, 1)", "lower bound is above"),
    ] {
        let error = failure(formula);
        assert!(error.contains(cause), "{formula}: {error}");
        assert!(
            error.contains("result") && error.contains("2025Q1"),
            "{error}"
        );
    }
    // An unavailable operand is not an error: nothing is computed.
    assert_eq!(value("gone / 0"), None);
    assert_eq!(value("ln(gone)"), None);
}

#[test]
fn comparisons_are_exact() {
    assert_eq!(value("if(0.1 + 0.2 == 0.3, 1, 0)"), Some(0.0));
    assert_eq!(value("if(seed == 10, 1, 0)"), Some(1.0));
    assert_eq!(value("if(seed + 0.00000000001 != seed, 1, 0)"), Some(1.0));
}

#[test]
fn rounding_defaults_to_zero_places_and_rounds_ties_away_from_zero() {
    assert_eq!(value("round(2.5)"), Some(3.0));
    assert_eq!(value("round(0 - 2.5)"), Some(-3.0));
    assert_eq!(value("round(seed / 4, 1)"), Some(2.5));
    assert_eq!(value("round(1234.5, 0 - 2)"), Some(1200.0));
    assert!(failure("round(seed, 0.5)").contains("integer"));
    assert!(failure("round(seed, 1.0000000001)").contains("integer"));
    assert_eq!(value("clamp(seed, 0, 10)"), Some(10.0));
    assert_eq!(value("clamp(seed, 0, 5)"), Some(5.0));
    assert_eq!(value("clamp(gone, 0, 5)"), None);
}

#[test]
fn annualization_needs_an_explicit_positive_finite_factor() {
    assert_eq!(value("annualize(seed, 4)"), Some(40.0));
    assert_eq!(value("annualize(gone, 4)"), None);
    for formula in ["annualize(seed, 0)", "annualize(seed, 0 - 4)"] {
        assert!(failure(formula).contains("must be positive and finite"));
    }
    // Simple: rate × factor. Compound: (1 + rate)^factor − 1.
    assert_eq!(value("annualize_rate(0.02, 4, 0)"), Some(0.08));
    let compound = value("annualize_rate(0.02, 4, 1)").expect("value");
    assert!((compound - (1.02_f64.powi(4) - 1.0)).abs() < 1e-15);
    // The compound domain includes −100% and rejects rates below it.
    assert_eq!(value("annualize_rate(0 - 1, 4, 1)"), Some(-1.0));
    assert!(failure("annualize_rate(0 - 1.5, 4, 1)").contains("below -100%"));
    assert_eq!(value("annualize_rate(0 - 1.5, 4, 0)"), Some(-6.0));
    assert!(failure("annualize_rate(0.02, 4, 2)").contains("0 (simple) or 1 (compound)"));
    assert!(failure("annualize_rate(0.02, 0, 1)").contains("must be positive and finite"));
    assert_eq!(value("annualize_rate(gone, 4, 1)"), None);
}

#[test]
fn test_ttm_with_nan() {
    // Revenue is missing its 2024Q2 observation; a missing cell is absent,
    // never a stored NaN.
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2024Q1..2024Q4", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2024, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2024, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(150.0),
                ),
                (
                    PeriodId::quarter(2024, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(200.0),
                ),
            ],
        );
        model_builder
            .compute("revenue_ttm", "ttm(revenue)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // TTM propagates NaN strictly — any non-finite value
    // in the trailing window makes the TTM NaN, rather than silently
    // treating a gap as zero. This is the correct behavior for a
    // trailing-twelve-months metric: a missing quarter is a material
    // signal that the metric is not well-defined and should not be
    // consumed downstream.
    let ttm = results.get(
        "revenue_ttm",
        &PeriodId::quarter(2024, 4).expect("valid period fixture"),
    );
    assert!(
        ttm.is_none(),
        "ttm() should propagate NaN strictly under C18 guard, got {ttm:?}"
    );
}

#[test]
fn an_unavailable_operand_propagates_through_nested_functions() {
    assert_eq!(value("mean(sum(seed, gone), coalesce(gone, 20))"), None);
    assert_eq!(value("mean(sum(seed, 10), coalesce(gone, 20))"), Some(20.0));
}

#[test]
fn test_sum_empty_args() {
    // Test that sum() with no arguments produces an error at compile time
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q1", None).unwrap();
        match model_builder.compute("total", "sum()") {
            Ok(_) => Ok(model_builder),
            Err(error) => Err(error),
        }
    };

    assert!(
        result.is_err(),
        "sum() with no arguments should produce an error"
    );
}

#[test]
fn test_mean_empty_args() {
    // Test that mean() with no arguments produces an error at compile time
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q1", None).unwrap();
        match model_builder.compute("average", "mean()") {
            Ok(_) => Ok(model_builder),
            Err(error) => Err(error),
        }
    };

    assert!(
        result.is_err(),
        "mean() with no arguments should produce an error"
    );
}

#[test]
fn test_coalesce_single_arg() {
    // Test that coalesce() with one argument produces an error
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q1", None).unwrap();
        model_builder.compute("value", "10.0").unwrap();
        match model_builder.compute("result", "coalesce(value)") {
            Ok(_) => Ok(model_builder),
            Err(error) => Err(error),
        }
    };

    assert!(
        result.is_err(),
        "coalesce() with one argument should produce an error"
    );
}
