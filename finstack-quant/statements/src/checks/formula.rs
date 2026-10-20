//! Canonical statement-formula check execution.

use std::sync::Arc;

use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;

use super::helpers::get_finite_node_value;
use super::{
    Check, CheckComparison, CheckContext, CheckFinding, CheckResult, FormulaCheckSpec, Severity,
};
use crate::evaluator::{
    formula::evaluate_formula, EvalWarning, EvaluationContext, PeriodHistory, StatementResult,
};
use crate::types::NodeId;
use crate::Result;

impl Check for FormulaCheckSpec {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn category(&self) -> super::CheckCategory {
        self.category
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        context.config.validate()?;
        if self.tolerance.is_some_and(|t| !t.is_finite() || t < 0.0) {
            return Err(crate::Error::invalid_input(
                "Formula-check tolerance must be finite and nonnegative",
            ));
        }
        let ast = crate::dsl::parse_formula(&self.formula)?;
        let mut value_types = IndexMap::new();
        for node in context
            .model
            .nodes
            .keys()
            .map(NodeId::as_str)
            .chain(context.results.nodes.keys().map(String::as_str))
        {
            value_types.insert(
                NodeId::new(node),
                super::helpers::validated_input_value_type(self.id(), context, node)?,
            );
        }
        let (reporting_currency, instrument_currencies) =
            super::helpers::validated_capital_structure_currencies(self.id(), context)?;
        let _ = crate::dsl::compiler::infer_value_type(
            &ast,
            &value_types,
            reporting_currency,
            &instrument_currencies,
        )?;
        let expression = crate::dsl::compiler::compile(&ast)?;
        let history = Arc::new(period_history(context.results, context.model));
        let historical_cashflows = Arc::new(period_cashflows(context.results, context.model));
        // Same-period node inputs. Unknown identifiers are left to evaluation,
        // which rejects them as a configuration error.
        let current_inputs: Vec<NodeId> = crate::utils::formula::extract_direct_dependencies(&ast)
            .into_iter()
            .filter(|node| {
                !node.as_str().starts_with("cs.")
                    && (context.results.nodes.contains_key(node.as_str())
                        || context.model.has_node(node.as_str()))
            })
            .collect();
        let mut findings = Vec::new();
        let mut comparisons = Vec::new();

        for period in &context.model.periods {
            let unusable: Vec<NodeId> = current_inputs
                .iter()
                .filter(|node| get_finite_node_value(context.results, node, &period.id).is_none())
                .cloned()
                .collect();
            if !unusable.is_empty() {
                let names: Vec<&str> = unusable.iter().map(NodeId::as_str).collect();
                findings.push(self.insufficient_data(
                    period.id,
                    format!("missing or non-finite inputs [{}]", names.join(", ")),
                    unusable,
                ));
                continue;
            }

            let mut evaluation = EvaluationContext::new_with_history(
                period.id,
                Arc::clone(&history),
                Arc::clone(&historical_cashflows),
            );
            evaluation.capital_structure_cashflows = context
                .results
                .cs_cashflows
                .as_ref()
                .map(|cashflows| cashflows.period_snapshot(period.id));

            for (node_id, values) in &context.results.nodes {
                if let Some(value) = values.get(&period.id) {
                    evaluation.set_value(node_id, *value)?;
                }
            }

            let value = evaluate_formula(&expression, &mut evaluation, Some(&self.id))?;
            // A division by zero over finite inputs is a real defect in the
            // checked figures, so it still fails below. Any other non-finite
            // value reached the formula as data it could not judge: a lag or
            // window reaching before the first period, or a non-finite value
            // from history.
            let divided_by_zero = evaluation
                .warnings
                .iter()
                .any(|warning| matches!(warning, EvalWarning::DivisionByZero { .. }));
            if !divided_by_zero && (!value.is_finite() || evaluation.non_finite_operand) {
                findings.push(
                    self.insufficient_data(
                        period.id,
                        "the formula consumed a non-finite value (for example a lag or rolling \
                     window reaching before the first period)"
                            .to_string(),
                        current_inputs.clone(),
                    ),
                );
                continue;
            }
            let passes = if !value.is_finite() {
                false
            } else if let Some(tolerance) = self.tolerance {
                value.abs() <= tolerance
            } else {
                value != 0.0
            };

            // A tolerance turns the formula into a numeric comparison against
            // zero; without one it is a boolean test with nothing to compare.
            // A non-finite value always fails and is not a comparable number
            // (it would not survive a JSON round trip), so it is not recorded.
            let comparison = self
                .tolerance
                .filter(|_| value.is_finite())
                .map(|tolerance| CheckComparison {
                    identity: self.id.clone(),
                    period: Some(period.id),
                    actual: value,
                    expected: 0.0,
                    tolerance,
                });
            comparisons.extend(comparison.clone());

            if !passes {
                findings.push(CheckFinding {
                    check_id: self.id.clone(),
                    severity: self.severity,
                    message: self
                        .message_template
                        .replace("{period}", &period.id.to_string()),
                    period: Some(period.id),
                    materiality: None,
                    nodes: vec![],
                    comparison,
                });
            }
        }

        let passed = !findings
            .iter()
            .any(|finding| finding.severity >= Severity::Error);

        Ok(CheckResult {
            check_id: self.id.clone(),
            check_name: self.name.clone(),
            category: self.category,
            passed,
            findings,
            comparisons,
        })
    }
}

impl FormulaCheckSpec {
    /// Warning for a period the formula cannot judge. It never fails the
    /// check and carries no comparison, like the skipped periods of the
    /// built-in identity checks.
    fn insufficient_data(
        &self,
        period: PeriodId,
        reason: String,
        nodes: Vec<NodeId>,
    ) -> CheckFinding {
        CheckFinding {
            check_id: self.id.clone(),
            severity: Severity::Warning,
            message: format!("{}: insufficient data for {period}: {reason}", self.name),
            period: Some(period),
            materiality: None,
            nodes,
            comparison: None,
        }
    }
}

fn node_columns(results: &StatementResult) -> IndexMap<NodeId, usize> {
    results
        .nodes
        .keys()
        .enumerate()
        .map(|(index, node_id)| (NodeId::new(node_id), index))
        .collect()
}

fn period_history(
    results: &StatementResult,
    model: &crate::types::FinancialModelSpec,
) -> PeriodHistory {
    let mut history = PeriodHistory::with_periods(Arc::new(node_columns(results)), &model.periods);
    for period in &model.periods {
        let row = results
            .nodes
            .values()
            .map(|series| series.get(&period.id).copied())
            .collect();
        history.push_row(period.id, row);
    }
    history
}

fn period_cashflows(
    results: &StatementResult,
    model: &crate::types::FinancialModelSpec,
) -> IndexMap<PeriodId, crate::capital_structure::CapitalStructureCashflows> {
    let Some(cashflows) = results.cs_cashflows.as_ref() else {
        return IndexMap::new();
    };

    model
        .periods
        .iter()
        .map(|period| (period.id, cashflows.period_snapshot(period.id)))
        .collect()
}
