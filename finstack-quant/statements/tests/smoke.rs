//! Smoke test for statements crate basic functionality.

use finstack_quant_statements::prelude::*;

#[test]
fn test_basic_model_creation() {
    // Test that we can create a basic model
    let result = ModelBuilder::new("smoke_test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .build();

    assert!(result.is_ok());
    let model = result.unwrap();
    assert_eq!(model.id, "smoke_test");
    assert_eq!(model.periods.len(), 2);
}
