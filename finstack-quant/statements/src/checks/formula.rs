//! Canonical statement-formula check execution.

use std::sync::Arc;

use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;

use super::{
    Check, CheckComparison, CheckContext, CheckFinding, CheckResult, FormulaCheckMode,
    FormulaCheckSpec, PeriodScope, Severity,
};
use crate::Result;
use crate::evaluator::value::Value;
use crate::evaluator::{
    EvaluationContext, PeriodHistory, StatementResult, formula::evaluate_value,
};
use crate::types::NodeId;

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
        let unit_context = self.unit_context(context)?;
        let formula = self.compile(&self.formula, &unit_context, "formula")?;
        let boolean = formula.1;
        match (self.mode, boolean, self.tolerance) {
            (FormulaCheckMode::Predicate, false, _) => {
                return Err(crate::Error::invalid_input(format!(
                    "formula check '{}': a predicate needs a Boolean formula such as a \
                     comparison; use the residual mode for an amount",
                    self.id
                )));
            }
            (FormulaCheckMode::Predicate, true, Some(_)) => {
                return Err(crate::Error::invalid_input(format!(
                    "formula check '{}': a tolerance applies only to the residual mode",
                    self.id
                )));
            }
            (FormulaCheckMode::Residual, true, _) => {
                return Err(crate::Error::invalid_input(format!(
                    "formula check '{}': a residual needs a scalar or monetary formula, not a \
                     Boolean one; use the predicate mode",
                    self.id
                )));
            }
            (FormulaCheckMode::Residual, false, Some(tolerance))
                if !tolerance.is_finite() || tolerance < 0.0 =>
            {
                return Err(crate::Error::invalid_input(
                    "Formula-check tolerance must be finite and nonnegative",
                ));
            }
            _ => {}
        }
        let eligibility = match &self.eligibility {
            Some(text) => {
                let (expression, boolean) = self.compile(text, &unit_context, "eligibility")?;
                if !boolean {
                    return Err(crate::Error::invalid_input(format!(
                        "formula check '{}': the eligibility condition must be Boolean",
                        self.id
                    )));
                }
                Some(expression)
            }
            None => None,
        };

        let history = Arc::new(period_history(context.results, context.periods()));
        let historical_cashflows = Arc::new(period_cashflows(context.results, context.periods()));
        let tolerance = self.tolerance.unwrap_or(0.0);
        let mut findings = Vec::new();
        let mut comparisons = Vec::new();
        let mut evaluated = 0_usize;

        for period in context.periods() {
            let in_scope = match self.scope {
                PeriodScope::AllPeriods => true,
                PeriodScope::ActualsOnly => period.is_actual,
                PeriodScope::ForecastOnly => !period.is_actual,
            };
            if !in_scope {
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
            // The typed cells of the run: a cell without a value stays
            // without one, and is consumed only if evaluation reaches it.
            for (node_id, cells) in &context.results.cells {
                let cell = cells
                    .get(&period.id)
                    .map_or(crate::evaluator::cell::Cell::Missing, |cell| cell.to_cell());
                evaluation.set_cell(node_id, cell)?;
            }

            if let Some(eligibility) = &eligibility {
                match evaluate_value(eligibility, &mut evaluation, Some(&self.id))? {
                    Value::Bool(true) => {}
                    Value::Bool(false) => continue,
                    _ => {
                        evaluated += 1;
                        findings.push(
                            self.incomplete(period.id, "the eligibility condition has no value"),
                        );
                        continue;
                    }
                }
            }
            evaluated += 1;

            // An invalid formula (division by zero, an undefined domain, an
            // overflow) is an execution error. An unavailable result means
            // the formula needed an observation that has no value.
            let value = evaluate_value(&formula.0, &mut evaluation, Some(&self.id))?;
            let breach_comparison = match (self.mode, value) {
                (_, Value::Unavailable(_)) => {
                    findings.push(self.incomplete(
                        period.id,
                        "the formula needed an observation that has no value (a missing or \
                         unavailable cell, or a lag or window reaching before the first period)",
                    ));
                    continue;
                }
                (FormulaCheckMode::Predicate, Value::Bool(true)) => continue,
                (FormulaCheckMode::Predicate, Value::Bool(false)) => None,
                (FormulaCheckMode::Residual, residual) => {
                    let actual = residual.float();
                    let comparison = CheckComparison {
                        identity: self.id.clone(),
                        period: Some(period.id),
                        actual,
                        expected: 0.0,
                        tolerance,
                    };
                    comparisons.push(comparison.clone());
                    if actual.abs() <= tolerance {
                        continue;
                    }
                    Some(comparison)
                }
                (FormulaCheckMode::Predicate, other) => {
                    return Err(crate::Error::eval(format!(
                        "formula check '{}': the predicate evaluated to a non-Boolean value \
                         ({other:?})",
                        self.id
                    )));
                }
            };
            findings.push(CheckFinding {
                check_id: self.id.clone(),
                severity: self.severity,
                message: self
                    .message_template
                    .replace("{period}", &period.id.to_string()),
                period: Some(period.id),
                materiality: None,
                nodes: vec![],
                comparison: breach_comparison,
                coverage: false,
            });
        }

        Ok(CheckResult::new(self, evaluated, findings, comparisons))
    }
}

/// The resolved units of every node and capital-structure scope a formula
/// check may reference.
struct CheckUnits {
    node_types: IndexMap<NodeId, crate::types::NodeValueType>,
    reporting_currency: Option<finstack_quant_core::currency::Currency>,
    instrument_currencies: IndexMap<String, finstack_quant_core::currency::Currency>,
}

impl FormulaCheckSpec {
    fn unit_context(&self, context: &CheckContext) -> Result<CheckUnits> {
        let mut node_types = IndexMap::new();
        for node in context
            .model
            .nodes
            .keys()
            .map(NodeId::as_str)
            .chain(context.results.node_ids())
        {
            node_types.insert(
                NodeId::new(node),
                super::helpers::validated_input_value_type(self.id(), context, node)?,
            );
        }
        let (reporting_currency, instrument_currencies) =
            super::helpers::validated_capital_structure_currencies(self.id(), context)?;
        Ok(CheckUnits {
            node_types,
            reporting_currency,
            instrument_currencies,
        })
    }

    /// Parse, validate over the whole AST (every branch, identifier and
    /// unit) and compile `text`; also report whether it is Boolean.
    fn compile(
        &self,
        text: &str,
        units: &CheckUnits,
        what: &str,
    ) -> Result<(finstack_quant_core::expr::Expr, bool)> {
        let ast = crate::dsl::parse_formula(text)?;
        // Every identifier, including one read only through `lag` or `shift`.
        let mut references = indexmap::IndexSet::new();
        crate::utils::formula::collect_identifiers_from_ast(&ast, &mut references, false);
        for node in references {
            if !node.starts_with("cs.") && !units.node_types.contains_key(node.as_str()) {
                return Err(crate::Error::invalid_input(format!(
                    "formula check '{}': the {what} references '{node}', which is not a node \
                     of the model",
                    self.id
                )));
            }
        }
        let unit_context = || crate::dsl::compiler::UnitContext {
            node_types: &units.node_types,
            capital_structure_currency: units.reporting_currency,
            instrument_currencies: &units.instrument_currencies,
            provisional: false,
        };
        let _ = crate::dsl::compiler::infer_value_type(&ast, unit_context())?;
        let boolean = crate::dsl::compiler::is_boolean(&ast, unit_context())?;
        Ok((crate::dsl::compiler::compile(&ast)?, boolean))
    }

    /// Coverage finding for a period the formula could not judge.
    fn incomplete(&self, period: PeriodId, reason: &str) -> CheckFinding {
        CheckFinding {
            check_id: self.id.clone(),
            severity: Severity::Warning,
            message: format!("{}: incomplete for {period}: {reason}", self.name),
            period: Some(period),
            materiality: None,
            nodes: vec![],
            comparison: None,
            coverage: true,
        }
    }
}

fn node_columns(results: &StatementResult) -> IndexMap<NodeId, usize> {
    results
        .node_ids()
        .enumerate()
        .map(|(index, node_id)| (NodeId::new(node_id), index))
        .collect()
}

fn period_history(
    results: &StatementResult,
    periods: &finstack_quant_core::dates::PeriodPlan,
) -> PeriodHistory {
    let mut history = PeriodHistory::with_periods(Arc::new(node_columns(results)), periods);
    for period in periods {
        let cells: Vec<Option<crate::evaluator::cell::Cell>> = results
            .cells
            .values()
            .map(|cells| cells.get(&period.id).map(|cell| cell.to_cell()))
            .collect();
        let row = cells
            .iter()
            .map(|cell| cell.map(|cell| cell.numeric().unwrap_or(f64::NAN)))
            .collect();
        history.push_typed_row(period.id, row, cells);
    }
    history
}

fn period_cashflows(
    results: &StatementResult,
    periods: &finstack_quant_core::dates::PeriodPlan,
) -> IndexMap<PeriodId, crate::capital_structure::CapitalStructureCashflows> {
    let Some(cashflows) = results.cs_cashflows.as_ref() else {
        return IndexMap::new();
    };

    periods
        .iter()
        .map(|period| (period.id, cashflows.period_snapshot(period.id)))
        .collect()
}
