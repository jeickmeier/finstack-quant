//! Counterexamples from the statements process and binding audit.
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_statements::adjustments::engine::NormalizationEngine;
use finstack_quant_statements::adjustments::types::{Adjustment, NormalizationConfig};
use finstack_quant_statements::evaluator::{EvalWarning, StatementResult};
use finstack_quant_statements::prelude::*;
use finstack_quant_statements::types::NodeValueType;
use indexmap::indexmap;
use serde_json::json;
use time::macros::date;

fn q(year: i32, quarter: u8) -> PeriodId {
    PeriodId::quarter(year, quarter).unwrap()
}

#[test]
fn declared_units_must_match_observations() {
    let model = ModelBuilder::new("units")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value(
            "x",
            &[(q(2025, 1), Money::from((100_i64, Currency::EUR)).into())],
        )
        .build()
        .unwrap();
    for value_type in [
        NodeValueType::Scalar,
        NodeValueType::Monetary {
            currency: Currency::USD,
        },
    ] {
        let mut invalid = model.clone();
        invalid.nodes.get_mut("x").unwrap().value_type = Some(value_type);
        assert!(invalid.validate_semantics().is_err());
    }
}

#[test]
fn sparse_periods_do_not_shorten_lags_or_fill_ttm() {
    let mut model = ModelBuilder::new("sparse")
        .periods("2024Q1..2025Q1", None)
        .unwrap()
        .value(
            "x",
            &[
                (q(2024, 1), 100.0.into()),
                (q(2024, 2), 200.0.into()),
                (q(2024, 3), 100.0.into()),
                (q(2024, 4), 100.0.into()),
                (q(2025, 1), 100.0.into()),
            ],
        )
        .compute("far", "lag(x,100)")
        .unwrap()
        .compute("ttm", "ttm(x)")
        .unwrap()
        .compute("previous_year", "lag(x,4)")
        .unwrap()
        .build()
        .unwrap();
    model.periods.retain(|period| period.id != q(2024, 2));
    model
        .nodes
        .get_mut("x")
        .unwrap()
        .values
        .as_mut()
        .unwrap()
        .shift_remove(&q(2024, 2));
    let result = Evaluator::new().evaluate(&model).unwrap();
    assert!(result.get("far", &q(2025, 1)).unwrap().is_nan());
    assert!(result.get("ttm", &q(2025, 1)).unwrap().is_nan());
    assert_eq!(result.get("previous_year", &q(2025, 1)), Some(100.0));
}

#[test]
fn forecasts_rebase_at_each_visible_actual_without_looking_ahead() {
    let mut model = ModelBuilder::new("availability")
        .periods("2025Q1..Q4", Some("2025Q3"))
        .unwrap()
        .value(
            "x",
            &[
                (q(2025, 1), 100.0.into()),
                (q(2025, 2), 200.0.into()),
                (q(2025, 3), 300.0.into()),
            ],
        )
        .build()
        .unwrap();
    let node = model.nodes.get_mut("x").unwrap();
    node.forecast =
        Some(serde_json::from_value(json!({"method":"growth_pct","params":{"rate":0.1}})).unwrap());
    node.node_type = finstack_quant_statements::types::NodeType::Mixed;
    node.availability_dates = indexmap! {q(2025,1) => date!(2025-04-10), q(2025,2) => date!(2025-12-01), q(2025,3) => date!(2025-10-10)};
    let result = Evaluator::new()
        .evaluate_with_market(&model, &MarketContext::new(), date!(2025 - 11 - 01))
        .unwrap();
    assert!((result.get("x", &q(2025, 2)).unwrap() - 110.0).abs() < 1e-10);
    assert!((result.get("x", &q(2025, 4)).unwrap() - 330.0).abs() < 1e-10);
}

#[test]
fn ewm_normalizes_after_each_observation() {
    let model = ModelBuilder::new("ewm")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "x",
            &[
                (q(2025, 1), 1.0.into()),
                (q(2025, 2), f64::NAN.into()),
                (q(2025, 3), 3.0.into()),
                (q(2025, 4), 4.0.into()),
            ],
        )
        .compute("mean", "ewm_mean(x,0.5)")
        .unwrap()
        .compute("var", "ewm_var(x,0.5)")
        .unwrap()
        .build()
        .unwrap();
    let result = Evaluator::new().evaluate(&model).unwrap();
    assert!((result.get("mean", &q(2025, 4)).unwrap() - 19.0 / 6.0).abs() < 1e-12);
    assert!((result.get("var", &q(2025, 4)).unwrap() - 41.0 / 22.0).abs() < 1e-12);
}

#[test]
fn normalization_preserves_units_and_rejects_foreign_sources_and_caps() {
    let model = ModelBuilder::new("normalization")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value(
            "usd",
            &[(q(2025, 1), Money::from((100_i64, Currency::USD)).into())],
        )
        .value(
            "eur",
            &[(q(2025, 1), Money::from((100_i64, Currency::EUR)).into())],
        )
        .build()
        .unwrap();
    let mut result = Evaluator::new().evaluate(&model).unwrap();
    let invalid: NormalizationConfig = serde_json::from_value(json!({"target_node":"usd","adjustments":[{"id":"a","name":"a","value":{"type":"percentage_of_node","node_id":"eur","percentage":0.1}}]})).unwrap();
    assert!(NormalizationEngine::normalize(&result, &invalid).is_err());
    let invalid_cap: NormalizationConfig = serde_json::from_value(json!({"target_node":"usd","adjustments":[{"id":"a","name":"a","value":{"type":"fixed","amounts":{"2025Q1":10}},"cap":{"base_node":"eur","value":0.1}}]})).unwrap();
    assert!(NormalizationEngine::normalize(&result, &invalid_cap).is_err());
    let config = NormalizationConfig::new("usd")
        .add_adjustment(Adjustment::fixed("a", "a", indexmap! {q(2025,1)=>10.0}))
        .unwrap();
    let normalized = NormalizationEngine::normalize(&result, &config).unwrap();
    for target in ["adjusted", "usd", "eur"] {
        NormalizationEngine::merge_into_results(&mut result, &normalized, target).unwrap();
        assert_eq!(result.get(target, &q(2025, 1)), Some(110.0));
        assert_eq!(
            result.get_money(target, &q(2025, 1)),
            Some(Money::from((110_i64, Currency::USD)))
        );
    }
}

#[test]
fn every_cash_period_checks_its_components_even_without_a_prior_balance() {
    use finstack_quant_statements::checks::CheckSuiteSpec;
    let model = ModelBuilder::new("cash")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("cash", &[(q(2025, 1), 100.0.into())])
        .value("total", &[(q(2025, 1), 0.0.into())])
        .value("cfo", &[(q(2025, 1), 100.0.into())])
        .value("cfi", &[(q(2025, 1), 0.0.into())])
        .value("cff", &[(q(2025, 1), 0.0.into())])
        .build()
        .unwrap();
    let spec: CheckSuiteSpec = serde_json::from_value(json!({"name":"cash","builtin_checks":[{"type":"cash_reconciliation","cash_balance_node":"cash","total_cash_flow_node":"total","cfo_node":"cfo","cfi_node":"cfi","cff_node":"cff"}]})).unwrap();
    let result = Evaluator::new()
        .with_checks(spec.resolve().unwrap())
        .evaluate(&model)
        .unwrap();
    assert_eq!(result.check_report.unwrap().summary.failed, 1);
}

#[test]
fn non_finite_result_cells_and_warnings_round_trip_and_validate_schema() {
    let mut result = StatementResult::new();
    for (id, value) in [
        ("nan", f64::NAN),
        ("positive", f64::INFINITY),
        ("negative", f64::NEG_INFINITY),
    ] {
        result.nodes.insert(id.into(), indexmap! {q(2025,1)=>value});
        result.meta.warnings.push(EvalWarning::NonFiniteValue {
            node_id: id.into(),
            period: q(2025, 1),
            value,
        });
    }
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["nodes"]["nan"]["2025Q1"], "nan");
    let restored: StatementResult = serde_json::from_value(json.clone()).unwrap();
    assert!(restored.get("nan", &q(2025, 1)).unwrap().is_nan());
    assert_eq!(restored.get("positive", &q(2025, 1)), Some(f64::INFINITY));
    assert_eq!(
        restored.get("negative", &q(2025, 1)),
        Some(f64::NEG_INFINITY)
    );
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../schemas/statements/1/statement_result.schema.json"
    ))
    .unwrap();
    assert!(jsonschema::validator_for(&schema).unwrap().is_valid(&json));
}

fn amortizing_loan_model(periods: &str, currency: Currency) -> FinancialModelSpec {
    use finstack_quant_statements::types::FinancialStatementInstrument;
    use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
    let mut spec = serde_json::to_value(TermLoan::example().unwrap()).unwrap();
    for (key,value) in json!({"id":"LOAN","currency":currency.to_string(),"notional_limit":{"amount":"1000000","currency":currency.to_string()},"issue_date":"2025-01-15","maturity":"2026-01-15","frequency":{"count":1,"unit":"months"},"rate":{"fixed":{"rate_bp":1200}},"business_day_convention":"unadjusted","amortization":{"percent_per_period":{"bp":1000}}}).as_object().unwrap() {
        spec[key]=value.clone();
    }
    let loan = serde_json::from_value(spec).unwrap();
    ModelBuilder::new("loan")
        .periods(periods, None)
        .unwrap()
        .add_debt("LOAN", FinancialStatementInstrument::TermLoan(loan))
        .compute("interest", "cs.interest_expense_cash.total")
        .unwrap()
        .compute("debt", "cs.debt_balance.total")
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn reporting_grid_does_not_reprice_contractual_amortizing_interest() {
    let market = MarketContext::new();
    let quarterly = Evaluator::new()
        .evaluate_with_market(
            &amortizing_loan_model("2025Q1..Q3", Currency::USD),
            &market,
            date!(2025 - 01 - 15),
        )
        .unwrap();
    let monthly = Evaluator::new()
        .evaluate_with_market(
            &amortizing_loan_model("2025M1..M9", Currency::USD),
            &market,
            date!(2025 - 01 - 15),
        )
        .unwrap();
    for quarter in 1..=3 {
        let total: f64 = ((quarter - 1) * 3 + 1..=quarter * 3)
            .map(|m| {
                monthly
                    .get("interest", &PeriodId::month(2025, m).unwrap())
                    .unwrap()
            })
            .sum();
        assert!((quarterly.get("interest", &q(2025, quarter)).unwrap() - total).abs() < 1e-8);
    }
    assert!((quarterly.get("interest", &q(2025, 2)).unwrap() - 22_439.7).abs() < 1e-8);
}

#[test]
fn reporting_currency_requires_fx_even_with_one_native_currency() {
    let mut model = amortizing_loan_model("2025Q1..Q1", Currency::EUR);
    model.capital_structure.as_mut().unwrap().reporting_currency = Some(Currency::USD);
    assert!(Evaluator::new()
        .evaluate_with_market(&model, &MarketContext::new(), date!(2025 - 01 - 15))
        .is_err());
}

#[test]
fn single_currency_totals_are_converted_at_the_reporting_snapshot() {
    use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
    struct SnapshotFx;
    impl FxProvider for SnapshotFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            on: time::Date,
            policy: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            assert_eq!(
                (from, to, on, policy),
                (
                    Currency::EUR,
                    Currency::USD,
                    date!(2025 - 03 - 31),
                    FxConversionPolicy::PeriodEnd
                )
            );
            Ok(1.2)
        }
    }
    let mut model = amortizing_loan_model("2025Q1..Q1", Currency::EUR);
    model.capital_structure.as_mut().unwrap().reporting_currency = Some(Currency::USD);
    model.validate_semantics().unwrap();
    let market = MarketContext::new().insert_fx(FxMatrix::new(std::sync::Arc::new(SnapshotFx)));
    let result = Evaluator::new()
        .evaluate_with_market(&model, &market, date!(2025 - 01 - 15))
        .unwrap();
    let flows = result.cs_cashflows.as_ref().unwrap();
    assert_eq!(flows.reporting_currency, Some(Currency::USD));
    assert_eq!(
        flows.by_instrument["LOAN"][&q(2025, 1)]
            .debt_balance
            .currency(),
        Currency::EUR
    );
    assert!((result.get_money("debt", &q(2025, 1)).unwrap().amount() - 972_000.0).abs() < 1e-8);
    assert!((result.get("interest", &q(2025, 1)).unwrap() - 18_733.33333333333 * 1.2).abs() < 1e-8);
}

#[test]
fn small_ewm_weights_do_not_disappear_through_subtraction() {
    let model = ModelBuilder::new("small-alpha")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("x", &[(q(2025, 1), 0.0.into()), (q(2025, 2), 1e20.into())])
        .compute("mean", "ewm_mean(x, 0.00000000000000000001)")
        .unwrap()
        .build()
        .unwrap();
    let result = Evaluator::new().evaluate(&model).unwrap();
    assert!((result.get("mean", &q(2025, 2)).unwrap() - 1.0).abs() < 1e-12);
}
