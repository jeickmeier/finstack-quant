//! Goal-seek acceptance must certify the target before mutating the model.
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::{builder::ModelBuilder, types::AmountOrScalar};
use finstack_quant_statements_analytics::analysis::goal_seek;

#[test]
fn discontinuous_objective_is_rejected_without_mutation() {
    let q = PeriodId::quarter(2025, 1).unwrap();
    for bounds in [None, Some((-1., 1.))] {
        let mut model = ModelBuilder::new("discontinuous")
            .periods("2025Q1..Q1", None)
            .unwrap()
            .value("driver", &[(q, AmountOrScalar::scalar(1.))])
            .compute("target", "if(driver < 0, 0, 1)")
            .unwrap()
            .build()
            .unwrap();
        let before = serde_json::to_value(&model).unwrap();
        let error = goal_seek(&mut model, "target", q, 0.5, "driver", q, true, bounds)
            .expect_err("no attainable target");
        assert!(error.to_string().contains("residual"), "{error}");
        assert_eq!(serde_json::to_value(&model).unwrap(), before);
    }
}
