#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::{Check, CheckContext, Severity};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};
use finstack_quant_statements_analytics::analysis::checks::{
    CoverageFloorCheck, FcfSignCheck, LeverageRangeCheck, LiquidityRunwayCheck, TrendCheck,
    TrendDirection,
};

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter)
}

fn s(v: f64) -> AmountOrScalar {
    AmountOrScalar::scalar(v)
}

// ============================================================================
// LeverageRangeCheck
// ============================================================================

#[test]
fn leverage_within_range_passes() {
    // Debt/EBITDA = 500/200 = 2.5x
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("debt", &[(q(1), s(500.0))])
        .value("ebitda", &[(q(1), s(200.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = LeverageRangeCheck {
        debt_node: NodeId::new("debt"),
        ebitda_node: NodeId::new("ebitda"),
        warn_range: (0.0, 6.0),
        error_range: (0.0, 10.0),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

#[test]
fn leverage_above_warn_flags_warning() {
    // Debt/EBITDA = 1400/200 = 7.0x → above warn 6.0, below error 10.0
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("debt", &[(q(1), s(1400.0))])
        .value("ebitda", &[(q(1), s(200.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = LeverageRangeCheck {
        debt_node: NodeId::new("debt"),
        ebitda_node: NodeId::new("ebitda"),
        warn_range: (0.0, 6.0),
        error_range: (0.0, 10.0),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed); // warnings don't cause failure
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, Severity::Warning);
}

#[test]
fn leverage_above_error_flags_error() {
    // Debt/EBITDA = 2200/200 = 11.0x → above error 10.0
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("debt", &[(q(1), s(2200.0))])
        .value("ebitda", &[(q(1), s(200.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = LeverageRangeCheck {
        debt_node: NodeId::new("debt"),
        ebitda_node: NodeId::new("ebitda"),
        warn_range: (0.0, 6.0),
        error_range: (0.0, 10.0),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);
    assert_eq!(result.findings[0].severity, Severity::Error);
}

#[test]
fn leverage_flags_non_positive_ebitda_as_error() {
    // Debt/EBITDA is undefined when EBITDA <= 0; the check must surface this
    // as a high-severity finding rather than silently passing.
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("debt", &[(q(1), s(500.0))])
        .value("ebitda", &[(q(1), s(-100.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = LeverageRangeCheck {
        debt_node: NodeId::new("debt"),
        ebitda_node: NodeId::new("ebitda"),
        warn_range: (0.0, 6.0),
        error_range: (0.0, 10.0),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, Severity::Error);
    assert!(result.findings[0].message.contains("undefined"));
}

// ============================================================================
// CoverageFloorCheck
// ============================================================================

#[test]
fn coverage_above_floor_passes() {
    // Ratio = 300/100 = 3.0x, above both floors
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("ebitda", &[(q(1), s(300.0))])
        .value("debt_service", &[(q(1), s(100.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = CoverageFloorCheck {
        numerator_node: NodeId::new("ebitda"),
        denominator_node: NodeId::new("debt_service"),
        min_warning: 1.5,
        min_error: 1.0,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

#[test]
fn coverage_below_warning_flags() {
    // Ratio = 120/100 = 1.2x, below 1.5 warning but above 1.0 error
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("ebitda", &[(q(1), s(120.0))])
        .value("debt_service", &[(q(1), s(100.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = CoverageFloorCheck {
        numerator_node: NodeId::new("ebitda"),
        denominator_node: NodeId::new("debt_service"),
        min_warning: 1.5,
        min_error: 1.0,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, Severity::Warning);
}

#[test]
fn coverage_below_error_flags() {
    // Ratio = 80/100 = 0.8x, below 1.0 error
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("ebitda", &[(q(1), s(80.0))])
        .value("debt_service", &[(q(1), s(100.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = CoverageFloorCheck {
        numerator_node: NodeId::new("ebitda"),
        denominator_node: NodeId::new("debt_service"),
        min_warning: 1.5,
        min_error: 1.0,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);
    assert_eq!(result.findings[0].severity, Severity::Error);
}

#[test]
fn coverage_negative_denominator_is_error() {
    // Negative debt service makes the ratio undefined — must surface as an
    // Error finding (consistent with LeverageRangeCheck), not a silent skip.
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("ebitda", &[(q(1), s(300.0))])
        .value("debt_service", &[(q(1), s(-100.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = CoverageFloorCheck {
        numerator_node: NodeId::new("ebitda"),
        denominator_node: NodeId::new("debt_service"),
        min_warning: 1.5,
        min_error: 1.0,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, Severity::Error);
    assert!(result.findings[0].message.contains("undefined"));
}

#[test]
fn coverage_zero_denominator_is_skipped() {
    // Zero debt service: nothing to cover — documented skip.
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("ebitda", &[(q(1), s(300.0))])
        .value("debt_service", &[(q(1), s(0.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = CoverageFloorCheck {
        numerator_node: NodeId::new("ebitda"),
        denominator_node: NodeId::new("debt_service"),
        min_warning: 1.5,
        min_error: 1.0,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

// ============================================================================
// FcfSignCheck
// ============================================================================

#[test]
fn fcf_positive_passes() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "fcf",
            &[
                (q(1), s(10.0)),
                (q(2), s(20.0)),
                (q(3), s(15.0)),
                (q(4), s(25.0)),
            ],
        )
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = FcfSignCheck {
        fcf_node: NodeId::new("fcf"),
        consecutive_negative_warning: 2,
        consecutive_negative_error: 4,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

#[test]
fn fcf_consecutive_negative_warning() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "fcf",
            &[
                (q(1), s(10.0)),
                (q(2), s(-5.0)),
                (q(3), s(-10.0)),
                (q(4), s(5.0)),
            ],
        )
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = FcfSignCheck {
        fcf_node: NodeId::new("fcf"),
        consecutive_negative_warning: 2,
        consecutive_negative_error: 4,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    let warnings: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].period, Some(q(3)));
}

#[test]
fn fcf_consecutive_negative_error() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "fcf",
            &[
                (q(1), s(-1.0)),
                (q(2), s(-2.0)),
                (q(3), s(-3.0)),
                (q(4), s(-4.0)),
            ],
        )
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = FcfSignCheck {
        fcf_node: NodeId::new("fcf"),
        consecutive_negative_warning: 2,
        consecutive_negative_error: 4,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);
    let errors: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .collect();
    assert!(!errors.is_empty());
}

// ============================================================================
// TrendCheck
// ============================================================================

#[test]
fn trend_improving_passes() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "coverage",
            &[
                (q(1), s(1.5)),
                (q(2), s(1.6)),
                (q(3), s(1.7)),
                (q(4), s(1.8)),
            ],
        )
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = TrendCheck {
        node: NodeId::new("coverage"),
        direction: TrendDirection::IncreasingIsGood,
        lookback_periods: 3,
        severity: Severity::Warning,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.findings.is_empty());
}

#[test]
fn trend_deteriorating_flags() {
    // 3 consecutive decreases when IncreasingIsGood
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "coverage",
            &[
                (q(1), s(2.0)),
                (q(2), s(1.8)),
                (q(3), s(1.5)),
                (q(4), s(1.2)),
            ],
        )
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = TrendCheck {
        node: NodeId::new("coverage"),
        direction: TrendDirection::IncreasingIsGood,
        lookback_periods: 3,
        severity: Severity::Warning,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.findings.is_empty());
    assert_eq!(result.findings[0].period, Some(q(4)));
}

#[test]
fn trend_decreasing_is_good_deterioration() {
    // Leverage increasing = bad when DecreasingIsGood
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "leverage",
            &[
                (q(1), s(3.0)),
                (q(2), s(3.5)),
                (q(3), s(4.0)),
                (q(4), s(4.5)),
            ],
        )
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = TrendCheck {
        node: NodeId::new("leverage"),
        direction: TrendDirection::DecreasingIsGood,
        lookback_periods: 2,
        severity: Severity::Warning,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.findings.is_empty());
}

// ============================================================================
// LiquidityRunwayCheck
// ============================================================================

#[test]
fn liquidity_runway_adequate_passes() {
    // Quarterly model: runway = (1200 / 100) quarters * 3 months = 36 months,
    // above both thresholds.
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("cash", &[(q(1), s(1200.0))])
        .value("burn", &[(q(1), s(100.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = LiquidityRunwayCheck {
        cash_node: NodeId::new("cash"),
        cash_burn_node: NodeId::new("burn"),
        min_months_warning: 6.0,
        min_months_error: 3.0,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

#[test]
fn liquidity_runway_below_warning() {
    // Quarterly model: runway = (150 / 100) quarters * 3 months = 4.5
    // months, below 6 warning but above 3 error.
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("cash", &[(q(1), s(150.0))])
        .value("burn", &[(q(1), s(100.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = LiquidityRunwayCheck {
        cash_node: NodeId::new("cash"),
        cash_burn_node: NodeId::new("burn"),
        min_months_warning: 6.0,
        min_months_error: 3.0,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, Severity::Warning);
}

#[test]
fn liquidity_runway_below_error() {
    // Quarterly model: runway = (50 / 100) quarters * 3 months = 1.5
    // months, below 3 error.
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("cash", &[(q(1), s(50.0))])
        .value("burn", &[(q(1), s(100.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = LiquidityRunwayCheck {
        cash_node: NodeId::new("cash"),
        cash_burn_node: NodeId::new("burn"),
        min_months_warning: 6.0,
        min_months_error: 3.0,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);
    assert_eq!(result.findings[0].severity, Severity::Error);
}

#[test]
fn liquidity_runway_converts_periods_to_months() {
    // Regression for periods-as-months bug: a quarterly model with exactly
    // 2 periods of runway has 6 months — at the 6-month warning floor it
    // must pass (not be treated as "2 months" and error).
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("cash", &[(q(1), s(200.0))])
        .value("burn", &[(q(1), s(100.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = LiquidityRunwayCheck {
        cash_node: NodeId::new("cash"),
        cash_burn_node: NodeId::new("burn"),
        min_months_warning: 6.0,
        min_months_error: 3.0,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(
        result.passed,
        "6 months runway must not breach a 3-month error floor"
    );
    assert!(
        result.findings.is_empty(),
        "6 months runway is not below the 6-month warning floor: {:?}",
        result.findings
    );
}

#[test]
fn liquidity_runway_skips_no_burn() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("cash", &[(q(1), s(200.0))])
        .value("burn", &[(q(1), s(0.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = LiquidityRunwayCheck {
        cash_node: NodeId::new("cash"),
        cash_burn_node: NodeId::new("burn"),
        min_months_warning: 6.0,
        min_months_error: 3.0,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.findings.is_empty());
}
