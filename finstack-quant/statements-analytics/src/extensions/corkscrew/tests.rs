use super::*;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::AmountOrScalar;
use finstack_quant_statements::types::FinancialModelSpec;

#[test]
fn test_corkscrew_extension_with_config() {
    let config = CorkscrewConfig {
        accounts: vec![CorkscrewAccount {
            node_id: "cash".into(),
            account_type: AccountType::Asset,
            changes: vec!["cash_inflows".into(), "cash_outflows".into()],
            decreases: vec![],
            beginning_balance_node: None,
        }],
        tolerance: 0.01,
        fail_on_error: false,
    };

    let extension = CorkscrewExtension::new(config);
    assert_eq!(extension.config().accounts.len(), 1);
}

#[test]
fn test_corkscrew_execute_with_empty_accounts() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder
            .periods("2025Q1..Q1", None)
            .expect("valid periods");
        model_builder.value(
            "cash",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder.build().expect("model should build")
    };
    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate(&model)
        .expect("evaluation should succeed");

    let config = CorkscrewConfig {
        accounts: vec![],
        tolerance: 0.01,
        fail_on_error: false,
    };

    let extension = CorkscrewExtension::new(config);
    let report = extension
        .execute(&model, &results)
        .expect("empty accounts should succeed");
    assert_eq!(report.status, CorkscrewStatus::Success);
}

#[test]
fn test_account_type_serialization() {
    let account_type = AccountType::Asset;
    let json = serde_json::to_string(&account_type).expect("test should succeed");
    assert_eq!(json, r#""asset""#);

    let deserialized: AccountType = serde_json::from_str(&json).expect("test should succeed");
    assert_eq!(deserialized, AccountType::Asset);
}

fn broken_rollforward_model() -> (FinancialModelSpec, StatementResult) {
    // cash: 100 → 250 with only +100 of inflows → 50 break.
    let model = {
        let mut model_builder = ModelBuilder::new("rollforward_break");
        model_builder
            .periods("2025Q1..Q2", None)
            .expect("valid periods");
        model_builder.value(
            "cash",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(250.0),
                ),
            ],
        );
        model_builder.value(
            "inflows",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
            ],
        );
        model_builder.build().expect("model should build")
    };
    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate(&model)
        .expect("evaluation should succeed");
    (model, results)
}

#[test]
fn rollforward_break_fails_report_in_strict_mode() {
    let (model, results) = broken_rollforward_model();

    let config = CorkscrewConfig {
        accounts: vec![CorkscrewAccount {
            node_id: "cash".into(),
            account_type: AccountType::Asset,
            changes: vec!["inflows".into()],
            decreases: vec![],
            beginning_balance_node: None,
        }],
        tolerance: 0.01,
        fail_on_error: true,
    };

    let extension = CorkscrewExtension::new(config);
    let report = extension
        .execute(&model, &results)
        .expect("extension should execute");

    assert_eq!(
        report.status,
        CorkscrewStatus::Failed,
        "a roll-forward identity break must fail the report in strict mode"
    );
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.contains("roll-forward identity failed"))
    );
}

#[test]
fn rollforward_break_warns_in_lenient_mode() {
    let (model, results) = broken_rollforward_model();

    let config = CorkscrewConfig {
        accounts: vec![CorkscrewAccount {
            node_id: "cash".into(),
            account_type: AccountType::Asset,
            changes: vec!["inflows".into()],
            decreases: vec![],
            beginning_balance_node: None,
        }],
        tolerance: 0.01,
        fail_on_error: false,
    };

    let extension = CorkscrewExtension::new(config);
    let report = extension
        .execute(&model, &results)
        .expect("extension should execute");

    assert_eq!(report.status, CorkscrewStatus::Success);
    assert_eq!(report.data["validations"][0]["type"], "asset");
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("roll-forward identity failed")),
        "break must surface as a warning, got {:?}",
        report.warnings
    );
}

#[test]
fn explicit_beginning_balance_preserves_small_movements() {
    let (model, mut results) = broken_rollforward_model();
    let q1 = PeriodId::quarter(2025, 1).expect("valid period");
    let q2 = PeriodId::quarter(2025, 2).expect("valid period");
    results
        .set_series("cash", indexmap::IndexMap::from([(q1, 1e18), (q2, 1.0)]))
        .expect("finite fixture values");
    results
        .set_series("inflows", indexmap::IndexMap::from([(q2, 1.0)]))
        .expect("finite fixture values");
    results
        .set_series("cash_beg", indexmap::IndexMap::from([(q2, 0.0)]))
        .expect("finite fixture values");
    let extension = CorkscrewExtension::new(CorkscrewConfig {
        accounts: vec![],
        tolerance: 0.01,
        fail_on_error: false,
    });
    let account = CorkscrewAccount {
        node_id: "cash".into(),
        account_type: AccountType::Asset,
        changes: vec!["inflows".into()],
        decreases: vec![],
        beginning_balance_node: Some("cash_beg".into()),
    };
    let validation = extension
        .validate_account(&account, &model, &results, 0.01)
        .expect("complete roll-forward");
    assert!(validation.is_valid);
    assert_eq!(validation.max_error, 0.0);
}

#[test]
fn missing_beginning_balance_period_value_is_hard_error() {
    let (model, mut results) = broken_rollforward_model();
    // Add a beginning-balance node with NO value in Q2.
    results
        .set_series(
            "cash_beg",
            indexmap::IndexMap::from([(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                100.0,
            )]),
        )
        .expect("finite fixture values");

    let config = CorkscrewConfig {
        accounts: vec![CorkscrewAccount {
            node_id: "cash".into(),
            account_type: AccountType::Asset,
            changes: vec!["inflows".into()],
            decreases: vec![],
            beginning_balance_node: Some("cash_beg".into()),
        }],
        tolerance: 0.01,
        fail_on_error: false,
    };

    let extension = CorkscrewExtension::new(config);
    let report = extension
        .execute(&model, &results)
        .expect("extension should execute in lenient mode");

    assert_eq!(report.status, CorkscrewStatus::Failed);
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.contains("cash_beg") && e.contains("no value")),
        "missing beginning-balance period value must be an error, got {:?}",
        report.errors
    );
}

#[test]
fn single_period_model_flags_nothing_to_validate() {
    let model = {
        let mut model_builder = ModelBuilder::new("single_period");
        model_builder
            .periods("2025Q1..Q1", None)
            .expect("valid periods");
        model_builder.value(
            "cash",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder.build().expect("model should build")
    };
    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate(&model)
        .expect("evaluation should succeed");

    let config = CorkscrewConfig {
        accounts: vec![CorkscrewAccount {
            node_id: "cash".into(),
            account_type: AccountType::Asset,
            changes: vec![],
            decreases: vec![],
            beginning_balance_node: None,
        }],
        tolerance: 0.01,
        fail_on_error: false,
    };

    let extension = CorkscrewExtension::new(config);
    let report = extension
        .execute(&model, &results)
        .expect("extension should execute");

    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("no roll-forward transitions")),
        "single-period run must be flagged as vacuous, got {:?}",
        report.warnings
    );
}

#[test]
fn articulation_checked_at_every_period() {
    // Balanced in the last period but imbalanced in the first — the
    // check must catch the earlier imbalance.
    let model = {
        let mut model_builder = ModelBuilder::new("articulation_all_periods");
        model_builder
            .periods("2025Q1..Q2", None)
            .expect("valid periods");
        model_builder.value(
            "assets",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(110.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
            ],
        );
        model_builder.value(
            "liabilities",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
            ],
        );
        model_builder.build().expect("model should build")
    };
    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate(&model)
        .expect("evaluation should succeed");

    let config = CorkscrewConfig {
        accounts: vec![
            CorkscrewAccount {
                node_id: "assets".into(),
                account_type: AccountType::Asset,
                changes: vec![],
                decreases: vec![],
                beginning_balance_node: None,
            },
            CorkscrewAccount {
                node_id: "liabilities".into(),
                account_type: AccountType::Liability,
                changes: vec![],
                decreases: vec![],
                beginning_balance_node: None,
            },
        ],
        tolerance: 0.01,
        fail_on_error: true,
    };

    let extension = CorkscrewExtension::new(config);
    let report = extension
        .execute(&model, &results)
        .expect("extension should execute");

    assert_eq!(report.status, CorkscrewStatus::Failed);
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.contains("Balance sheet not articulated"))
    );
}

#[test]
fn test_corkscrew_uses_absolute_tolerance_for_articulation() {
    let model = {
        let mut model_builder = ModelBuilder::new("articulation_tolerance");
        model_builder
            .periods("2025Q1..Q1", None)
            .expect("valid periods");
        model_builder.value(
            "assets",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(1_000_000.00),
            )],
        );
        model_builder.value(
            "liabilities",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(999_999.98),
            )],
        );
        model_builder.value(
            "equity",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(0.0),
            )],
        );
        model_builder.build().expect("model should build")
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate(&model)
        .expect("evaluation should succeed");

    let config = CorkscrewConfig {
        accounts: vec![
            CorkscrewAccount {
                node_id: "assets".into(),
                account_type: AccountType::Asset,
                changes: vec![],
                decreases: vec![],
                beginning_balance_node: None,
            },
            CorkscrewAccount {
                node_id: "liabilities".into(),
                account_type: AccountType::Liability,
                changes: vec![],
                decreases: vec![],
                beginning_balance_node: None,
            },
            CorkscrewAccount {
                node_id: "equity".into(),
                account_type: AccountType::Equity,
                changes: vec![],
                decreases: vec![],
                beginning_balance_node: None,
            },
        ],
        tolerance: 0.01,
        fail_on_error: true,
    };

    let extension = CorkscrewExtension::new(config);
    let report = extension
        .execute(&model, &results)
        .expect("extension should execute");

    assert_eq!(report.status, CorkscrewStatus::Failed);
    assert!(
        report
            .errors
            .iter()
            .any(|msg| msg.contains("Balance sheet not articulated")),
        "expected articulation failure, got {:?}",
        report.errors
    );
}

#[test]
fn decreases_are_subtracted_from_expected_balance() {
    let model = {
        let mut model_builder = ModelBuilder::new("decreases_identity");
        model_builder
            .periods("2025Q1..Q2", None)
            .expect("valid periods");
        model_builder.value(
            "inventory",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(130.0),
                ),
            ],
        );
        model_builder.value(
            "additions",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(50.0),
                ),
            ],
        );
        model_builder.value(
            "disposals",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(20.0),
                ),
            ],
        );
        model_builder.build().expect("model should build")
    };
    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate(&model)
        .expect("evaluation should succeed");

    let config = CorkscrewConfig {
        accounts: vec![CorkscrewAccount {
            node_id: "inventory".into(),
            account_type: AccountType::Asset,
            changes: vec!["additions".into()],
            decreases: vec!["disposals".into()],
            beginning_balance_node: None,
        }],
        tolerance: 0.01,
        fail_on_error: false,
    };

    let extension = CorkscrewExtension::new(config);
    let report = extension
        .execute(&model, &results)
        .expect("extension should execute");
    assert_eq!(
        report.data["validations"][0]["is_valid"].as_bool(),
        Some(true)
    );
    assert!(
        report
            .warnings
            .iter()
            .all(|w| !w.contains("roll-forward identity failed")),
        "decreases must keep the identity intact, got {:?}",
        report.warnings
    );
}

#[test]
fn omitted_decreases_field_deserializes_as_empty() {
    let json = r#"{
        "node_id": "cash",
        "account_type": "asset",
        "changes": ["inflows"]
    }"#;
    let account: CorkscrewAccount =
        serde_json::from_str(json).expect("legacy corkscrew JSON should parse");
    assert!(account.decreases.is_empty());
    assert_eq!(account.changes, vec!["inflows".to_string()]);
}
