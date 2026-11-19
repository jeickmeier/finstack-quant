//! Tests for the surrounding crate component and its documented behavior.
//!
#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::builtins::{
    BalanceSheetArticulation, CashReconciliation, MissingValueCheck, NonFiniteCheck,
    RetainedEarningsReconciliation, SignConventionCheck,
};
use finstack_quant_statements::checks::{
    BuiltinCheckSpec, CheckCategory, CheckSuite, CheckSuiteSpec, FormulaCheckMode,
    FormulaCheckSpec, PeriodScope, Severity,
};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, ForecastMethod, ForecastSpec, NodeId};
use indexmap::indexmap;

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter).expect("valid period fixture")
}

fn s(v: f64) -> AmountOrScalar {
    AmountOrScalar::scalar(v)
}

fn positive_revenue_suite() -> CheckSuite {
    CheckSuite::builder("positive revenue")
        .add_check(FormulaCheckSpec {
            id: "revenue_positive".into(),
            name: "Revenue must be positive".into(),
            category: CheckCategory::InternalConsistency,
            severity: Severity::Error,
            formula: "revenue > 0".into(),
            message_template: "Revenue was non-positive in {period}".into(),
            mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
            scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
            eligibility: None,
            tolerance: None,
        })
        .build()
        .expect("unique check ids")
}

// JSON roundtrip: serialize → deserialize → resolve → check count

#[test]
fn suite_spec_json_roundtrip_and_resolve() {
    let spec = CheckSuiteSpec {
        name: "roundtrip_suite".into(),
        description: Some("Testing JSON roundtrip".into()),
        builtin_checks: vec![
            BuiltinCheckSpec::BalanceSheetArticulation(BalanceSheetArticulation {
                assets_nodes: vec![NodeId::new("total_assets")],
                liabilities_nodes: vec![NodeId::new("total_liabilities")],
                equity_nodes: vec![NodeId::new("total_equity")],
                tolerance: Some(0.5),
            }),
            BuiltinCheckSpec::NonFinite(NonFiniteCheck {
                nodes: finstack_quant_statements::checks::builtins::NodeSelection::named(vec![
                    NodeId::new("revenue"),
                ]),
            }),
            BuiltinCheckSpec::MissingValue(MissingValueCheck {
                required_nodes: vec![NodeId::new("revenue"), NodeId::new("cogs")],
                scope: PeriodScope::AllPeriods,
            }),
        ],
        formula_checks: vec![FormulaCheckSpec {
            id: "revenue_positive".into(),
            name: "Revenue must be positive".into(),
            category: finstack_quant_statements::checks::CheckCategory::InternalConsistency,
            severity: Severity::Error,
            formula: "revenue > 0".into(),
            message_template: "Revenue bad in {period}".into(),
            mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
            scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
            eligibility: None,
            tolerance: None,
        }],
        config: Default::default(),
    };

    let json = serde_json::to_string_pretty(&spec).unwrap();
    let deserialized: CheckSuiteSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.name, "roundtrip_suite");
    assert_eq!(
        deserialized.description.as_deref(),
        Some("Testing JSON roundtrip")
    );
    assert_eq!(deserialized.builtin_checks.len(), 3);
    assert_eq!(deserialized.formula_checks.len(), 1);

    let suite = deserialized.resolve().unwrap();
    assert_eq!(suite.len(), 4);
    assert_eq!(suite.name(), "roundtrip_suite");
    assert_eq!(suite.description(), Some("Testing JSON roundtrip"));
}

// Tagged serde: verify JSON tag format

#[test]
fn builtin_check_spec_tagged_serde() {
    let json = r#"{
        "type": "balance_sheet_articulation",
        "assets_nodes": ["total_assets"],
        "liabilities_nodes": ["total_liabilities"],
        "equity_nodes": ["total_equity"],
        "tolerance": 0.01
    }"#;

    let spec: BuiltinCheckSpec = serde_json::from_str(json).unwrap();
    match &spec {
        BuiltinCheckSpec::BalanceSheetArticulation(BalanceSheetArticulation {
            assets_nodes,
            liabilities_nodes,
            equity_nodes,
            tolerance,
        }) => {
            assert_eq!(assets_nodes, &[NodeId::new("total_assets")]);
            assert_eq!(liabilities_nodes, &[NodeId::new("total_liabilities")]);
            assert_eq!(equity_nodes, &[NodeId::new("total_equity")]);
            assert_eq!(*tolerance, Some(0.01));
        }
        other => panic!("Expected BalanceSheetArticulation, got {other:?}"),
    }

    let roundtrip = serde_json::to_string(&spec).unwrap();
    assert!(roundtrip.contains("\"type\":\"balance_sheet_articulation\""));
}

#[test]
fn retained_earnings_spec_tagged_serde() {
    let json = r#"{
        "type": "retained_earnings_reconciliation",
        "retained_earnings_node": "re",
        "net_income_node": "ni",
        "opening_balance": {"date": "2025-01-01", "value": 100.0},
        "dividends_node": "divs"
    }"#;

    let spec: BuiltinCheckSpec = serde_json::from_str(json).unwrap();
    match &spec {
        BuiltinCheckSpec::RetainedEarningsReconciliation(RetainedEarningsReconciliation {
            retained_earnings_node,
            net_income_node,
            dividends_node,
            ..
        }) => {
            assert_eq!(retained_earnings_node, &NodeId::new("re"));
            assert_eq!(net_income_node, &NodeId::new("ni"));
            assert_eq!(dividends_node, &Some(NodeId::new("divs")));
        }
        other => panic!("Expected RetainedEarningsReconciliation, got {other:?}"),
    }
}

#[test]
fn cash_reconciliation_spec_tagged_serde() {
    let json = r#"{
        "type": "cash_reconciliation",
        "cash_balance_node": "cash",
        "total_cash_flow_node": "total_cf",
        "opening_balance": {"date": "2025-01-01", "value": {"amount": "100", "currency": "USD"}},
        "cfo_node": "cfo",
        "cfi_node": "cfi",
        "cff_node": "cff"
    }"#;

    let spec: BuiltinCheckSpec = serde_json::from_str(json).unwrap();
    match &spec {
        BuiltinCheckSpec::CashReconciliation(CashReconciliation {
            cash_balance_node,
            total_cash_flow_node,
            cfo_node,
            ..
        }) => {
            assert_eq!(cash_balance_node, &NodeId::new("cash"));
            assert_eq!(total_cash_flow_node, &NodeId::new("total_cf"));
            assert_eq!(cfo_node, &Some(NodeId::new("cfo")));
        }
        other => panic!("Expected CashReconciliation, got {other:?}"),
    }
}

#[test]
fn sign_convention_spec_tagged_serde() {
    let json = r#"{
        "type": "sign_convention",
        "positive_nodes": ["revenue"],
        "negative_nodes": ["cogs"]
    }"#;

    let spec: BuiltinCheckSpec = serde_json::from_str(json).unwrap();
    match &spec {
        BuiltinCheckSpec::SignConvention(SignConventionCheck {
            positive_nodes,
            negative_nodes,
        }) => {
            assert_eq!(positive_nodes, &[NodeId::new("revenue")]);
            assert_eq!(negative_nodes, &[NodeId::new("cogs")]);
        }
        other => panic!("Expected SignConvention, got {other:?}"),
    }
}

#[test]
fn formula_check_spec_serde() {
    let json = r#"{
        "id": "margin_check",
        "name": "Margin >= 20%",
        "category": "internal_consistency",
        "severity": "warning",
        "mode": "predicate",
        "scope": "actuals_only",
        "formula": "(revenue - cogs) / revenue >= 0.20",
        "eligibility": "revenue > 0",
        "message_template": "Margin too low in {period}"
    }"#;

    let spec: FormulaCheckSpec = serde_json::from_str(json).unwrap();
    assert_eq!(spec.id, "margin_check");
    assert_eq!(spec.severity, Severity::Warning);
    assert_eq!(spec.tolerance, None);
    assert_eq!(spec.mode, FormulaCheckMode::Predicate);
    assert_eq!(spec.scope, PeriodScope::ActualsOnly);
    assert_eq!(spec.eligibility.as_deref(), Some("revenue > 0"));
}

// Resolved suite runs against a model

#[test]
fn resolved_suite_runs_against_model() {
    let spec = CheckSuiteSpec {
        name: "bs_check".into(),
        description: None,
        builtin_checks: vec![BuiltinCheckSpec::BalanceSheetArticulation(
            BalanceSheetArticulation {
                assets_nodes: vec![NodeId::new("total_assets")],
                liabilities_nodes: vec![NodeId::new("total_liabilities")],
                equity_nodes: vec![NodeId::new("total_equity")],
                tolerance: None,
            },
        )],
        formula_checks: vec![],
        config: Default::default(),
    };

    let suite = spec.resolve().unwrap();

    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value("total_assets", &[(q(1), s(1000.0)), (q(2), s(1100.0))]);
        model_builder.value("total_liabilities", &[(q(1), s(600.0)), (q(2), s(700.0))]);
        model_builder.value("total_equity", &[(q(1), s(400.0)), (q(2), s(400.0))]);
        model_builder.build().unwrap()
    };

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let report = suite.run(&model, &results).unwrap();
    assert!(!report.has_errors());
    assert_eq!(report.summary.passed, 1);
}

#[test]
fn run_model_evaluates_with_value_forecast_formula_precedence() {
    let model = {
        let mut model_builder = ModelBuilder::new("precedence");
        model_builder.periods("2025Q1..Q4", Some("2025Q2")).unwrap();
        let mut model_builder = model_builder.mixed("revenue");
        model_builder.values(&[(q(1), s(100.0)), (q(2), s(110.0))]);
        model_builder.forecast(
            ForecastSpec::from_params(
                ForecastMethod::GrowthPct,
                indexmap! { "rate".into() => serde_json::json!(0.05) },
            )
            .unwrap(),
        );
        model_builder.formula("0").unwrap();
        let model_builder = model_builder.build().unwrap();
        model_builder.build().unwrap()
    };

    let report = positive_revenue_suite()
        .run_model(
            &model,
            &finstack_quant_statements::evaluator::ObservationCutoff::FullSnapshot,
            None,
        )
        .unwrap();

    assert!(!report.has_errors());
}

#[test]
fn run_model_uses_supplied_results_without_recomputation() {
    let model = {
        let mut model_builder = ModelBuilder::new("supplied");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("revenue", &[(q(1), s(100.0))]);
        model_builder.build().unwrap()
    };
    let mut supplied = Evaluator::new().evaluate(&model).unwrap();
    supplied.set_scalar("revenue", q(1), -1.0).unwrap();

    let report = positive_revenue_suite()
        .run_model(
            &model,
            &finstack_quant_statements::evaluator::ObservationCutoff::FullSnapshot,
            Some(&supplied),
        )
        .unwrap();

    assert!(report.has_errors());
}

/// `total_findings` counts retained findings from `results`, so it matches the
/// summary's severity counts for a run and survives a JSON round trip.
#[test]
fn total_findings_counts_retained_findings() {
    let model = {
        let mut model_builder = ModelBuilder::new("findings");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value("revenue", &[(q(1), s(-1.0)), (q(2), s(-2.0))]);
        model_builder.build().unwrap()
    };
    let report = positive_revenue_suite()
        .run_model(
            &model,
            &finstack_quant_statements::evaluator::ObservationCutoff::FullSnapshot,
            None,
        )
        .unwrap();

    assert_eq!(report.total_findings(), 2);
    assert_eq!(
        report.total_findings(),
        report.summary.errors + report.summary.warnings + report.summary.infos
    );
    let json = serde_json::to_string(&report).unwrap();
    let parsed: finstack_quant_statements::checks::CheckReport =
        serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.total_findings(), 2);
}

// Materiality is a reporting filter, not a verdict knob: an Error finding
// must never be suppressed in a way that flips a failing check to passed.

#[test]
fn materiality_threshold_does_not_flip_error_verdict() {
    use finstack_quant_statements::checks::builtins::BalanceSheetArticulation;
    use finstack_quant_statements::checks::{CheckConfig, CheckSuite};

    // Imbalance of 100 on a 1000 sheet: an unambiguous Error.
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value("total_assets", &[(q(1), s(1000.0))]);
        model_builder.value("total_liabilities", &[(q(1), s(600.0))]);
        model_builder.value("total_equity", &[(q(1), s(300.0))]);
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let suite = CheckSuite::builder("materiality")
        .add_check(BalanceSheetArticulation {
            assets_nodes: vec![NodeId::new("total_assets")],
            liabilities_nodes: vec![NodeId::new("total_liabilities")],
            equity_nodes: vec![NodeId::new("total_equity")],
            tolerance: None,
        })
        .config(CheckConfig {
            materiality_threshold: 1_000_000.0, // far above the 100 imbalance
            ..CheckConfig::default()
        })
        .build()
        .expect("unique check ids");

    let report = suite.run(&model, &results).unwrap();
    let result = &report.results[0];

    assert!(
        !result.passed(),
        "a materiality threshold must not convert a failing identity into a pass"
    );
    assert!(
        result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error),
        "the Error finding must survive materiality filtering, got: {:?}",
        result.findings
    );
}

// --- E1: numerical comparison and check-suite policy (STATEMENTS-CONTRACTS-014) ---

use finstack_quant_statements::checks::{CheckConfig, CheckOutcome};
use finstack_quant_statements::evaluator::ObservationCutoff;
use finstack_quant_statements::types::FinancialModelSpec;

/// Assets of 100 against liabilities plus equity of 100 - `gap`.
fn balance_model(gap: f64) -> FinancialModelSpec {
    let mut builder = ModelBuilder::new("policy");
    builder.periods("2025Q1..Q1", None).unwrap();
    builder.value("assets", &[(q(1), s(100.0))]);
    builder.value("liabilities", &[(q(1), s(60.0))]);
    builder.value("equity", &[(q(1), s(40.0 - gap))]);
    builder.build().unwrap()
}

fn balance_check() -> BalanceSheetArticulation {
    BalanceSheetArticulation {
        assets_nodes: vec![NodeId::new("assets")],
        liabilities_nodes: vec![NodeId::new("liabilities")],
        equity_nodes: vec![NodeId::new("equity")],
        tolerance: None,
    }
}

fn formula(id: &str, severity: Severity, formula: &str) -> FormulaCheckSpec {
    FormulaCheckSpec {
        id: id.into(),
        name: id.into(),
        category: CheckCategory::InternalConsistency,
        severity,
        formula: formula.into(),
        message_template: format!("{id} in {{period}}"),
        mode: finstack_quant_statements::checks::FormulaCheckMode::Predicate,
        scope: finstack_quant_statements::checks::PeriodScope::AllPeriods,
        eligibility: None,
        tolerance: None,
    }
}

fn run(
    model: &FinancialModelSpec,
    suite: &CheckSuite,
) -> finstack_quant_statements::checks::CheckReport {
    suite
        .run_model(model, &ObservationCutoff::FullSnapshot, None)
        .unwrap()
}

/// The default identity tolerance is zero. A stated tolerance allows
/// max(absolute, relative x reference), and a difference exactly at the
/// allowance is within it.
#[test]
fn identity_tolerance_is_zero_unless_stated_and_inclusive_at_the_allowance() {
    let suite = |config: CheckConfig| {
        CheckSuite::builder("tolerance")
            .add_check(balance_check())
            .config(config)
            .build()
            .unwrap()
    };
    // A gap of 0.5: exact comparison fails.
    let model = balance_model(0.5);
    assert_eq!(
        run(&model, &suite(CheckConfig::default())).outcome,
        CheckOutcome::Fail
    );
    // An absolute tolerance of exactly 0.5 allows it; just below does not.
    let absolute = |tolerance: f64| CheckConfig {
        default_tolerance: tolerance,
        ..CheckConfig::default()
    };
    let at = run(&model, &suite(absolute(0.5)));
    assert_eq!(at.outcome, CheckOutcome::Pass);
    let comparison = &at.results[0].comparisons[0];
    assert_eq!(
        (
            comparison.actual.value(),
            comparison.expected.value(),
            comparison.tolerance.value()
        ),
        (100.0, 99.5, 0.5)
    );
    assert_eq!(comparison.difference.value(), 0.5);
    assert_eq!(
        run(&model, &suite(absolute(0.499))).outcome,
        CheckOutcome::Fail
    );
    // The relative term uses the identity's reference, total assets of 100:
    // 0.005 x 100 = 0.5 is exactly the allowance; the larger term wins.
    let relative = |fraction: f64| CheckConfig {
        default_tolerance: 0.1,
        default_relative_tolerance: fraction,
        ..CheckConfig::default()
    };
    assert_eq!(
        run(&model, &suite(relative(0.005))).outcome,
        CheckOutcome::Pass
    );
    assert_eq!(
        run(&model, &suite(relative(0.004))).outcome,
        CheckOutcome::Fail
    );
    // A per-check absolute override replaces both terms.
    let overridden = CheckSuite::builder("override")
        .add_check(BalanceSheetArticulation {
            tolerance: Some(0.2),
            ..balance_check()
        })
        .config(relative(0.5))
        .build()
        .unwrap();
    assert_eq!(run(&model, &overridden).outcome, CheckOutcome::Fail);
    // An exactly balanced sheet passes with no tolerance at all.
    assert_eq!(
        run(&balance_model(0.0), &suite(CheckConfig::default())).outcome,
        CheckOutcome::Pass
    );
}

/// An empty suite, and a suite whose checks had nothing to examine, are
/// unevaluated: a pass requires evaluated work.
#[test]
fn a_suite_with_no_evaluated_work_is_unevaluated() {
    let model = balance_model(0.0);
    let empty = CheckSuite::builder("empty").build().unwrap();
    let report = run(&model, &empty);
    assert_eq!(report.outcome, CheckOutcome::Unevaluated);
    assert_eq!(report.summary.total_checks, 0);

    // A missing-value check with no required node examines nothing.
    let idle = CheckSuite::builder("idle")
        .add_check(MissingValueCheck {
            required_nodes: vec![],
            scope: PeriodScope::AllPeriods,
        })
        .build()
        .unwrap();
    let report = run(&model, &idle);
    assert_eq!(report.outcome, CheckOutcome::Unevaluated);
    assert_eq!(report.results[0].outcome, CheckOutcome::Unevaluated);
    assert_eq!((report.summary.passed, report.summary.unevaluated), (0, 1));
}

/// The reporting filters hide an advisory finding and nothing else: the
/// failure, its comparison and a coverage finding stay, and every verdict
/// is the one an unfiltered run gives.
#[test]
fn filters_hide_advisories_but_not_failures_or_coverage() {
    let model = balance_model(0.5);
    let suite = |config: CheckConfig| {
        CheckSuite::builder("filters")
            .add_check(balance_check())
            .add_check(formula("advisory", Severity::Warning, "assets < 50"))
            // `lag` reaches before the first period: no value to judge.
            .add_check(formula("coverage", Severity::Warning, "lag(assets, 1) > 0"))
            .config(config)
            .build()
            .unwrap()
    };
    let unfiltered = run(&model, &suite(CheckConfig::default()));
    let filtered = run(
        &model,
        &suite(CheckConfig {
            min_severity: Severity::Error,
            materiality_threshold: 1e9,
            ..CheckConfig::default()
        }),
    );
    let outcomes = |report: &finstack_quant_statements::checks::CheckReport| {
        report.results.iter().map(|r| r.outcome).collect::<Vec<_>>()
    };
    assert_eq!(
        outcomes(&unfiltered),
        [
            CheckOutcome::Fail,
            CheckOutcome::Pass,
            CheckOutcome::Incomplete
        ]
    );
    assert_eq!(outcomes(&filtered), outcomes(&unfiltered));
    assert_eq!(filtered.outcome, CheckOutcome::Fail);
    assert_eq!(filtered.summary, {
        let mut summary = unfiltered.summary.clone();
        summary.warnings -= 1;
        summary
    });
    // The advisory warning is hidden; the error, the coverage finding and
    // the recorded comparison are not.
    assert_eq!(unfiltered.results[1].findings.len(), 1);
    assert!(filtered.results[1].findings.is_empty());
    assert_eq!(filtered.results[0].findings.len(), 1);
    assert_eq!(filtered.results[0].comparisons.len(), 1);
    assert_eq!(filtered.results[2].findings.len(), 1);
    assert!(filtered.results[2].findings[0].coverage);
    // Without the failing check the report is incomplete, not passed.
    let incomplete = CheckSuite::builder("incomplete")
        .add_check(formula("coverage", Severity::Warning, "lag(assets, 1) > 0"))
        .add_check(formula("fine", Severity::Error, "assets > 0"))
        .build()
        .unwrap();
    let report = run(&model, &incomplete);
    assert_eq!(report.outcome, CheckOutcome::Incomplete);
    assert_eq!((report.summary.passed, report.summary.incomplete), (1, 1));
}

/// Checks run in declared order with unique identifiers, and suites merge
/// only when their policies are equal and their identifiers distinct.
#[test]
fn check_identifiers_are_unique_and_merges_are_compatible() {
    let pair = |first: &str, second: &str| {
        CheckSuite::builder("ids")
            .add_check(formula(first, Severity::Error, "assets > 0"))
            .add_check(formula(second, Severity::Error, "assets > 0"))
            .build()
    };
    let error = pair("same", "same").err().unwrap();
    assert!(error.to_string().contains("identifier 'same'"), "{error}");
    let duplicate_spec = CheckSuiteSpec {
        name: "spec".into(),
        description: None,
        builtin_checks: vec![],
        formula_checks: vec![
            formula("same", Severity::Error, "assets > 0"),
            formula("same", Severity::Error, "assets > 1"),
        ],
        config: CheckConfig::default(),
    };
    assert!(duplicate_spec.resolve().is_err());

    let left = pair("b", "a").unwrap();
    let right = CheckSuite::builder("right")
        .add_check(formula("c", Severity::Error, "assets > 0"))
        .build()
        .unwrap();
    let merged = left.merge(right).unwrap();
    assert_eq!(merged.check_ids(), ["b", "a", "c"]);
    let report = run(&balance_model(0.0), &merged);
    let order: Vec<&str> = report.results.iter().map(|r| r.check_id.as_str()).collect();
    assert_eq!(order, ["b", "a", "c"]);

    // A repeated identifier, or a different policy, is not a compatible merge.
    let clash = pair("b", "a").unwrap().merge(pair("a", "z").unwrap());
    assert!(clash.err().unwrap().to_string().contains("identifier 'a'"));
    let other_policy = CheckSuite::builder("loose")
        .add_check(formula("c", Severity::Error, "assets > 0"))
        .config(CheckConfig {
            default_tolerance: 1.0,
            ..CheckConfig::default()
        })
        .build()
        .unwrap();
    let error = pair("b", "a").unwrap().merge(other_policy).err().unwrap();
    assert!(
        error.to_string().contains("different configurations"),
        "{error}"
    );
}

/// Supplied results are accepted only as a verified run of this model
/// under the requested cutoff.
#[test]
fn supplied_results_need_verified_provenance() {
    let model = balance_model(0.0);
    let suite = CheckSuite::builder("provenance")
        .add_check(balance_check())
        .build()
        .unwrap();
    let results = Evaluator::new().evaluate(&model).unwrap();
    let full = ObservationCutoff::FullSnapshot;
    assert_eq!(
        suite
            .run_model(&model, &full, Some(&results))
            .unwrap()
            .outcome,
        CheckOutcome::Pass
    );
    let rejected =
        |model: &FinancialModelSpec,
         cutoff: &ObservationCutoff,
         results: &finstack_quant_statements::evaluator::StatementResult| {
            let error = suite.run_model(model, cutoff, Some(results)).err().unwrap();
            assert!(error.to_string().contains("not a verified run"), "{error}");
            error.to_string()
        };
    // No provenance: hand-built or stripped results.
    let mut stripped = results.clone();
    stripped.provenance = None;
    assert!(rejected(&model, &full, &stripped).contains("no provenance"));
    assert!(suite.run(&model, &stripped).is_err());
    // Another model's content.
    assert!(rejected(&balance_model(0.5), &full, &results).contains("model content"));
    // A tampered run identity.
    let mut tampered = results.clone();
    tampered.provenance.as_mut().unwrap().run_id = "sha256:00".into();
    rejected(&model, &full, &tampered);
    // A different period plan.
    let mut replanned = results.clone();
    replanned.periods = None;
    assert!(rejected(&model, &full, &replanned).contains("period plan"));
    // A run under another cutoff than the one requested.
    let cutoff = ObservationCutoff::Through(time::macros::date!(2025 - 06 - 30));
    assert!(rejected(&model, &cutoff, &results).contains("cutoff"));
}

/// The suite attached to an evaluator runs at finalization under the same
/// policy: exact comparison by default, on the run's own verified results.
#[test]
fn a_configured_suite_runs_at_single_run_finalization() {
    let suite = || {
        CheckSuite::builder("finalization")
            .add_check(balance_check())
            .build()
            .unwrap()
    };
    let balanced = Evaluator::new()
        .with_checks(suite())
        .evaluate(&balance_model(0.0))
        .unwrap();
    let report = balanced.check_report.as_ref().unwrap();
    assert_eq!(report.outcome, CheckOutcome::Pass);
    assert_eq!(report.results[0].comparisons[0].tolerance.value(), 0.0);
    let off = Evaluator::new()
        .with_checks(suite())
        .evaluate(&balance_model(0.005))
        .unwrap();
    assert_eq!(off.check_report.unwrap().outcome, CheckOutcome::Fail);
}
