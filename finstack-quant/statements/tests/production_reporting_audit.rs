//! Production regressions for forecast anchoring and financial checks.
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::checks::CheckSuiteSpec;
use finstack_quant_statements::{
    builder::ModelBuilder,
    evaluator::Evaluator,
    types::{AmountOrScalar, ForecastSpec, SeasonalMode},
};
use serde_json::json;

#[test]
fn leading_explicit_forecasts_preserve_run_anchor_and_seasonal_phase() {
    for forecast in [
        ForecastSpec::growth(0.1),
        ForecastSpec::seasonal(
            vec![100., 60., 120., 80., 100., 60., 120., 80.],
            4,
            SeasonalMode::Additive,
        ),
    ] {
        let q1 = PeriodId::quarter(2025, 1).unwrap();
        let q2 = PeriodId::quarter(2025, 2).unwrap();
        let baseline = ModelBuilder::new("forecast")
            .periods("2025Q1..Q4", Some("2025Q1"))
            .unwrap()
            .value("revenue", &[(q1, AmountOrScalar::scalar(80.0))])
            .forecast("revenue", forecast)
            .build()
            .unwrap();
        let mut overridden = baseline.clone();
        overridden
            .nodes
            .get_mut("revenue")
            .unwrap()
            .values
            .as_mut()
            .unwrap()
            .insert(q2, AmountOrScalar::scalar(999.));
        let expected = Evaluator::new().evaluate(&baseline).unwrap();
        let actual = Evaluator::new().evaluate(&overridden).unwrap();
        assert_eq!(actual.get("revenue", &q2), Some(999.));
        for q in [3, 4] {
            let period = PeriodId::quarter(2025, q).unwrap();
            assert_eq!(
                actual.get("revenue", &period),
                expected.get("revenue", &period)
            );
        }
    }
}

#[test]
fn formula_tolerance_is_residual_acceptance_and_no_tolerance_is_predicate() {
    let periods: Vec<_> = (1..=4)
        .map(|q| PeriodId::quarter(2025, q).unwrap())
        .collect();
    let values: Vec<_> = periods
        .iter()
        .copied()
        .zip([0., 0.1, -0.1, 0.2].map(AmountOrScalar::scalar))
        .collect();
    let model = ModelBuilder::new("residual")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value("residual", &values)
        .build()
        .unwrap();
    let results = Evaluator::new().evaluate(&model).unwrap();
    for (formula, tolerance) in [("residual", Some(0.1)), ("residual <= 0.1", None)] {
        let suite:CheckSuiteSpec=serde_json::from_value(json!({"name":"residual", "formula_checks":[{"id":"r","name":"residual","category":"accounting_identity","severity":"error","formula":formula,"message_template":"bad {period}","tolerance":tolerance}]})).unwrap();
        let report = suite.resolve().unwrap().run(&model, &results).unwrap();
        assert_eq!(report.results[0].findings.len(), 1);
        assert_eq!(report.results[0].findings[0].period, Some(periods[3]));
    }
}

#[test]
fn retained_earnings_uses_signed_dividend_change() {
    let ps: Vec<_> = (1..=4)
        .map(|q| PeriodId::quarter(2025, q).unwrap())
        .collect();
    let mut builder = ModelBuilder::new("signed-div")
        .periods("2025Q1..Q4", None)
        .unwrap();
    for (node, values) in [
        ("income", [100.; 4]),
        ("re", [1000., 1080., 1160., 1240.]),
        ("div", [-20.; 4]),
    ] {
        let values: Vec<_> = ps
            .iter()
            .copied()
            .zip(values.map(AmountOrScalar::scalar))
            .collect();
        builder = builder.value(node, &values);
    }
    let model = builder.build().unwrap();
    let suite:CheckSuiteSpec=serde_json::from_value(json!({"name":"signed", "builtin_checks":[{"type":"retained_earnings_reconciliation","retained_earnings_node":"re","net_income_node":"income","dividends_node":"div","dividends_sign_convention":"inflow_positive"}]})).unwrap();
    let report = suite
        .resolve()
        .unwrap()
        .run(&model, &Evaluator::new().evaluate(&model).unwrap())
        .unwrap();
    assert!(report.results.iter().all(|r| r.findings.is_empty()));
}
