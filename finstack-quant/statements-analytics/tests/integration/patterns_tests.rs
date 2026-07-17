//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::prelude::*;
use finstack_quant_statements_analytics::templates::{TemplatesExtension, VintageExtension};

#[test]
fn test_roll_forward_pattern_arr() -> Result<()> {
    let model = ModelBuilder::new("SaaS Model")
        .periods("2025Q1..2025Q4", None)?
        // Inputs
        .value_scalar(
            "new_arr",
            &[
                (PeriodId::quarter(2025, 1), 100.0),
                (PeriodId::quarter(2025, 2), 120.0),
                (PeriodId::quarter(2025, 3), 140.0),
                (PeriodId::quarter(2025, 4), 160.0),
            ],
        )
        .value_scalar(
            "churn_arr",
            &[
                (PeriodId::quarter(2025, 1), 10.0),
                (PeriodId::quarter(2025, 2), 12.0),
                (PeriodId::quarter(2025, 3), 14.0),
                (PeriodId::quarter(2025, 4), 16.0),
            ],
        )
        // Apply Pattern
        .add_roll_forward("arr", &["new_arr"], &["churn_arr"])?
        .build()?;

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model)?;

    // Q1: Beg = 0, End = 0 + 100 - 10 = 90
    assert_eq!(
        results.get("arr_beg", &PeriodId::quarter(2025, 1)),
        Some(0.0)
    );
    assert_eq!(
        results.get("arr_end", &PeriodId::quarter(2025, 1)),
        Some(90.0)
    );

    // Q2: Beg = 90, End = 90 + 120 - 12 = 198
    assert_eq!(
        results.get("arr_beg", &PeriodId::quarter(2025, 2)),
        Some(90.0)
    );
    assert_eq!(
        results.get("arr_end", &PeriodId::quarter(2025, 2)),
        Some(198.0)
    );

    // Q3: Beg = 198, End = 198 + 140 - 14 = 324
    assert_eq!(
        results.get("arr_beg", &PeriodId::quarter(2025, 3)),
        Some(198.0)
    );
    assert_eq!(
        results.get("arr_end", &PeriodId::quarter(2025, 3)),
        Some(324.0)
    );

    Ok(())
}

#[test]
fn test_vintage_buildup_pattern_revenue() -> Result<()> {
    // Decay curve: 100% in period 0, 80% in period 1, 50% in period 2, 0% thereafter
    let decay_curve = vec![1.0, 0.8, 0.5, 0.0];

    let model = ModelBuilder::new("Cohort Model")
        .periods("2025Q1..2025Q4", None)?
        .value_scalar(
            "new_sales",
            &[
                (PeriodId::quarter(2025, 1), 100.0),
                (PeriodId::quarter(2025, 2), 200.0),
                (PeriodId::quarter(2025, 3), 300.0),
                (PeriodId::quarter(2025, 4), 400.0),
            ],
        )
        .add_vintage_buildup("revenue", "new_sales", &decay_curve)?
        .build()?;

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model)?;

    // Q1: New=100.
    // Rev = 100*1.0 = 100
    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 1)),
        Some(100.0)
    );

    // Q2: New=200. Q1 cohort is now 1 period old.
    // Rev = (200*1.0) + (100*0.8) = 200 + 80 = 280
    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 2)),
        Some(280.0)
    );

    // Q3: New=300. Q2 is 1 old. Q1 is 2 old.
    // Rev = (300*1.0) + (200*0.8) + (100*0.5) = 300 + 160 + 50 = 510
    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 3)),
        Some(510.0)
    );

    // Q4: New=400. Q3 is 1 old. Q2 is 2 old. Q1 is 3 old (0.0).
    // Rev = (400*1.0) + (300*0.8) + (200*0.5) + (100*0.0) = 400 + 240 + 100 + 0 = 740
    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 4)),
        Some(740.0)
    );

    Ok(())
}
