//! Canonical statement-formula check execution.

use std::sync::Arc;

use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;

use super::{Check, CheckContext, CheckFinding, CheckResult, FormulaCheckSpec, Severity};
use crate::evaluator::{formula::evaluate_formula, EvaluationContext, StatementResult};
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
        if self.tolerance.is_some_and(|t| !t.is_finite() || t < 0.0) {
            return Err(crate::Error::invalid_input(
                "Formula-check tolerance must be finite and nonnegative",
            ));
        }
        let expression = crate::dsl::parse_and_compile(&self.formula)?;
        let node_to_column = Arc::new(node_columns(context.results));
        let historical_results = Arc::new(period_results(context.results, context.model));
        let historical_cashflows = Arc::new(period_cashflows(context.results, context.model));
        let node_value_types = Arc::new(context.results.node_value_types.clone());
        let mut findings = Vec::new();

        for period in &context.model.periods {
            let mut evaluation = EvaluationContext::new(
                period.id,
                Arc::clone(&node_to_column),
                Arc::clone(&historical_results),
            );
            evaluation.historical_capital_structure_cashflows = Arc::clone(&historical_cashflows);
            evaluation.node_value_types = Arc::clone(&node_value_types);
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
            let passes = if !value.is_finite() {
                false
            } else if let Some(tolerance) = self.tolerance {
                value.abs() <= tolerance
            } else {
                value != 0.0
            };

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
        })
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

fn period_results(
    results: &StatementResult,
    model: &crate::types::FinancialModelSpec,
) -> IndexMap<PeriodId, IndexMap<String, f64>> {
    model
        .periods
        .iter()
        .map(|period| {
            let values = results
                .nodes
                .iter()
                .filter_map(|(node_id, series)| {
                    series
                        .get(&period.id)
                        .map(|value| (node_id.clone(), *value))
                })
                .collect();
            (period.id, values)
        })
        .collect()
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
