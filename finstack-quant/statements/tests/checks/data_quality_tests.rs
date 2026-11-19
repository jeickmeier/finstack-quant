//! Data-quality coverage and validity (STATEMENTS-CONTRACTS-013).
#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::builtins::{
    MissingValueCheck, NodeSelection, NonFiniteCheck, SignConventionCheck,
};
use finstack_quant_statements::checks::{
    Check, CheckContext, CheckOutcome, CheckResult, PeriodScope, Severity,
};
use finstack_quant_statements::evaluator::{
    Evaluator, ResultCell, StatementResult, UnavailableReason,
};
use finstack_quant_statements::types::{FinancialModelSpec, NodeId};

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter).expect("valid period fixture")
}

/// Q1 and Q2 are actual, Q3 and Q4 forecast. `revenue` has every value;
/// `costs` has a typed zero in Q1 and no value in Q2 and Q4.
fn model() -> (FinancialModelSpec, StatementResult) {
    let mut builder = ModelBuilder::new("quality");
    builder.periods("2025Q1..Q4", Some("2025Q2")).unwrap();
    builder.value_scalar(
        "revenue",
        &[(q(1), 100.0), (q(2), 110.0), (q(3), 120.0), (q(4), 130.0)],
    );
    builder.value_scalar("costs", &[(q(1), 0.0), (q(3), -40.0)]);
    let model = builder.build().unwrap();
    let results = Evaluator::new().evaluate(&model).unwrap();
    (model, results)
}

fn nodes(names: &[&str]) -> Vec<NodeId> {
    names.iter().map(|name| NodeId::new(*name)).collect()
}

fn run(check: &dyn Check, model: &FinancialModelSpec, results: &StatementResult) -> CheckResult {
    check.execute(&CheckContext::new(model, results)).unwrap()
}

/// A missing required observation makes the covered scope incomplete, in
/// actual and forecast periods alike; a typed zero is a valid observation.
#[test]
fn missing_required_observations_make_the_scope_incomplete() {
    let (model, results) = model();
    let coverage = |required: &[&str], scope| {
        run(
            &MissingValueCheck {
                required_nodes: nodes(required),
                scope,
            },
            &model,
            &results,
        )
    };
    let complete = coverage(&["revenue"], PeriodScope::AllPeriods);
    assert_eq!(complete.outcome, CheckOutcome::Pass);
    assert_eq!(complete.evaluated, 4);

    let all = coverage(&["revenue", "costs"], PeriodScope::AllPeriods);
    assert_eq!(all.outcome, CheckOutcome::Incomplete);
    assert_eq!(all.evaluated, 8);
    let missing: Vec<_> = all.findings.iter().map(|f| f.period).collect();
    assert_eq!(missing, [Some(q(2)), Some(q(4))]);
    assert!(all.findings.iter().all(|f| f.coverage));
    assert!(all.findings[0].message.contains("'costs' is missing"));

    // The scope selects the periods: Q2 for actuals, Q4 for forecasts.
    let actuals = coverage(&["costs"], PeriodScope::ActualsOnly);
    assert_eq!(actuals.findings.len(), 1);
    assert_eq!(actuals.findings[0].period, Some(q(2)));
    let forecasts = coverage(&["costs"], PeriodScope::ForecastOnly);
    assert_eq!(forecasts.findings[0].period, Some(q(4)));
    assert_eq!(forecasts.evaluated, 2);
}

/// An observation that is unavailable for a stated reason is reported with
/// its reason; a scope with nothing in it is unevaluated; unknown and
/// duplicate required nodes are configuration errors.
#[test]
fn unavailable_observations_and_empty_scopes_are_reported_as_such() {
    let (model, mut results) = model();
    results.cells.get_mut("revenue").unwrap().insert(
        q(3),
        ResultCell::unavailable(UnavailableReason::MissingInput),
    );
    let check = MissingValueCheck {
        required_nodes: nodes(&["revenue"]),
        scope: PeriodScope::AllPeriods,
    };
    let result = run(&check, &model, &results);
    assert_eq!(result.outcome, CheckOutcome::Incomplete);
    assert!(
        result.findings[0]
            .message
            .contains("unavailable (missing_input)"),
        "{}",
        result.findings[0].message
    );

    let empty = run(
        &MissingValueCheck {
            required_nodes: vec![],
            scope: PeriodScope::AllPeriods,
        },
        &model,
        &results,
    );
    assert_eq!(empty.outcome, CheckOutcome::Unevaluated);
    assert_eq!(empty.evaluated, 0);

    let context = CheckContext::new(&model, &results);
    for (required, expected) in [
        (&["revenue", "revenu"][..], "not a node of the model"),
        (&["revenue", "revenue"][..], "appears twice"),
    ] {
        let error = MissingValueCheck {
            required_nodes: nodes(required),
            scope: PeriodScope::AllPeriods,
        }
        .execute(&context)
        .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
}

/// Sign diagnostics are advisory, apply to finite values, accept zero in
/// either group and reject overlapping groups.
#[test]
fn sign_diagnostics_are_advisory_and_groups_do_not_overlap() {
    let (model, results) = model();
    let check = SignConventionCheck {
        // Revenue is positive; `costs` holds a zero and a negative value.
        positive_nodes: nodes(&["revenue"]),
        negative_nodes: nodes(&["costs"]),
    };
    let clean = run(&check, &model, &results);
    assert_eq!(clean.outcome, CheckOutcome::Pass);
    assert!(clean.findings.is_empty());
    // Six cells have a value: four of revenue, two of costs.
    assert_eq!(clean.evaluated, 6);

    // Swapped expectations: every non-zero value is flagged as a warning,
    // the zero is valid in either group, and the check still does not fail.
    let swapped = SignConventionCheck {
        positive_nodes: nodes(&["costs"]),
        negative_nodes: nodes(&["revenue"]),
    };
    let flagged = run(&swapped, &model, &results);
    assert_eq!(flagged.outcome, CheckOutcome::Pass);
    assert_eq!(flagged.findings.len(), 5);
    assert!(
        flagged
            .findings
            .iter()
            .all(|f| f.severity == Severity::Warning)
    );
    // Per period, positive-group findings come before negative-group ones.
    assert!(flagged.findings[0].message.contains("'revenue'"));
    assert!(
        flagged
            .findings
            .iter()
            .any(|f| f.message.contains("'costs'"))
    );

    let overlapping = SignConventionCheck {
        positive_nodes: nodes(&["revenue"]),
        negative_nodes: nodes(&["revenue"]),
    };
    let error = overlapping
        .execute(&CheckContext::new(&model, &results))
        .unwrap_err();
    assert!(error.to_string().contains("appears in both"), "{error}");
}

/// A raw NaN or infinity is an error wherever it is, including on a node no
/// other check requires; the selection of all nodes is explicit.
#[test]
fn invalid_numbers_fail_validity_even_on_optional_cells() {
    let (model, mut results) = model();
    let all = NonFiniteCheck {
        nodes: NodeSelection::all(),
    };
    let clean = run(&all, &model, &results);
    assert_eq!(clean.outcome, CheckOutcome::Pass);
    // Six cells hold a value; the two cells without one are not examined.
    assert_eq!(clean.evaluated, 6);

    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut cell = ResultCell::scalar(1.0).unwrap();
        cell.value = Some(finstack_quant_statements::types::AmountOrScalar::scalar(
            invalid,
        ));
        results.cells.get_mut("costs").unwrap().insert(q(3), cell);
        let result = run(&all, &model, &results);
        assert_eq!(result.outcome, CheckOutcome::Fail);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].severity, Severity::Error);
        assert_eq!(result.findings[0].nodes, nodes(&["costs"]));
        // A named selection that leaves the node out does not see it.
        let named = NonFiniteCheck {
            nodes: NodeSelection::named(nodes(&["revenue"])),
        };
        assert_eq!(run(&named, &model, &results).outcome, CheckOutcome::Pass);
    }
}

/// Named selections reject unknown and duplicate identifiers, and a cell
/// outside the model's period plan is an error.
#[test]
fn validity_selection_and_plan_are_checked() {
    let (model, mut results) = model();
    let context = CheckContext::new(&model, &results);
    for (selected, expected) in [
        (&["revenue", "revenu"][..], "not a node of the model"),
        (&["revenue", "revenue"][..], "appears twice"),
    ] {
        let error = NonFiniteCheck {
            nodes: NodeSelection::named(nodes(selected)),
        }
        .execute(&context)
        .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
    results.cells.get_mut("revenue").unwrap().insert(
        PeriodId::quarter(2026, 1).unwrap(),
        ResultCell::scalar(1.0).unwrap(),
    );
    let error = NonFiniteCheck {
        nodes: NodeSelection::all(),
    }
    .execute(&CheckContext::new(&model, &results))
    .unwrap_err();
    assert!(
        error.to_string().contains("not a period of the model"),
        "{error}"
    );

    // The JSON form is "all" or a list; an omitted selection is rejected.
    let all: NonFiniteCheck = serde_json::from_str(r#"{"nodes": "all"}"#).unwrap();
    assert_eq!(all.nodes, NodeSelection::all());
    let named: NonFiniteCheck = serde_json::from_str(r#"{"nodes": ["revenue"]}"#).unwrap();
    assert_eq!(named.nodes, NodeSelection::named(nodes(&["revenue"])));
    assert!(serde_json::from_str::<NonFiniteCheck>("{}").is_err());
}
