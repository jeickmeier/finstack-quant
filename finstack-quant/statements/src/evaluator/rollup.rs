//! Roll evaluated statement results up to a coarser reporting frequency.
//!
//! Quarterly (or monthly, semi-annual) results become annual (or quarterly,
//! semi-annual) results by combining each node's values over the source
//! periods of every target period:
//!
//! - **Flows** (revenue, expenses, cash flows) are summed. This is the default.
//! - **Balances** (cash, debt, equity, last-twelve-month metrics) take the
//!   value of the bucket's last source period (`period_end`).
//! - **Ratios** (margins, leverage, coverage) re-evaluate the node's formula
//!   on the rolled-up inputs (`formula`), since neither a sum nor a period-end
//!   value of a ratio is the ratio of the period.
//!
//! Only `period_end` and `formula` nodes need to be listed in a
//! [`RollupSpec`]; everything else sums.

use crate::error::{Error, Result};
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::dag::evaluate_order;
use crate::evaluator::formula::evaluate_formula;
use crate::evaluator::formula_helpers::is_truthy;
use crate::evaluator::results::{EvalStats, NumericMode, StatementResult};
use crate::evaluator::PeriodHistory;
use crate::types::{FinancialModelSpec, NodeId, NodeValueType};
use finstack_quant_core::dates::{Period, PeriodId, PeriodKind};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// How one node's values combine over the source periods of a target period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RollupMethod {
    /// Sum the source periods: flows such as revenue, expenses and cash flows.
    /// A missing source value leaves the target value missing.
    Sum,
    /// Take the last source period's value: balances such as cash, debt and
    /// equity, and trailing metrics such as last-twelve-month EBITDA.
    PeriodEnd,
    /// Re-evaluate the node's formula (and `where` clause) on the rolled-up
    /// values: ratios such as margins, leverage and coverage. Time-series
    /// functions then run at the target frequency, so `lag(x, 1)` is the
    /// prior target period.
    Formula,
}

/// Specification for [`StatementResult::rollup`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_statements::evaluator::RollupSpec;
///
/// let spec: RollupSpec = serde_json::from_str(
///     r#"{"frequency": "annual", "methods": {"cash": "period_end", "margin": "formula"}}"#,
/// )?;
/// assert_eq!(spec.methods.len(), 2);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RollupSpec {
    /// Target frequency: `quarterly`, `semi_annual` or `annual`. It must be
    /// coarser than the model's periods and hold a whole number of them
    /// (monthly into quarterly, semi-annual or annual; quarterly into
    /// semi-annual or annual; semi-annual into annual).
    pub frequency: PeriodKind,
    /// Rollup method per node id. Nodes not listed are summed, so list
    /// balances as `period_end` and ratios as `formula`.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub methods: IndexMap<String, RollupMethod>,
}

/// One target period and the source periods it combines.
struct Bucket {
    period: Period,
    sources: Vec<PeriodId>,
}

impl StatementResult {
    /// Roll these results up to a coarser frequency, such as quarterly to
    /// annual.
    ///
    /// Each target period combines its source periods node by node: summed by
    /// default, the last source value for `period_end` nodes, and the node's
    /// formula re-evaluated on the rolled-up values for `formula` nodes (see
    /// [`RollupMethod`]). Every target period must be complete: a model that
    /// starts or ends mid-year cannot roll up to annual periods.
    ///
    /// The rolled-up result holds `nodes`, `monetary_nodes` and
    /// `node_value_types` for the target periods, and any warnings raised by
    /// re-evaluated formulas. It records no `node_sources`, capital-structure
    /// cashflows or check report: those describe the source evaluation.
    ///
    /// # Arguments
    ///
    /// * `model` - The model these results were evaluated from. It supplies
    ///   the periods, with their dates and actual/forecast split, and the
    ///   formulas re-evaluated for `formula` nodes.
    /// * `spec` - Target frequency and per-node rollup methods.
    ///
    /// # Returns
    ///
    /// A [`StatementResult`] keyed by the target periods (`2025`, `2025H1`,
    /// `2025Q1`, with the `FY` prefix kept for fiscal periods).
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error if the target frequency is not a whole
    /// multiple of the model's frequency, the model mixes frequencies, a
    /// target period is incomplete, `spec.methods` names an unknown node, or
    /// a `formula` node has no formula or reads capital-structure (`cs.*`)
    /// values, which cannot be re-evaluated without rolled-up cashflows.
    /// Formula evaluation errors, such as a `formula` node reading a value
    /// missing from these results, are propagated.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{PeriodId, PeriodKind};
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_statements::evaluator::{Evaluator, RollupMethod, RollupSpec};
    /// use finstack_quant_statements::types::AmountOrScalar;
    /// use indexmap::IndexMap;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let quarterly = |values: [f64; 4]| -> Result<Vec<(PeriodId, AmountOrScalar)>, Box<dyn std::error::Error>> {
    ///     (1..=4)
    ///         .zip(values)
    ///         .map(|(q, v)| Ok((PeriodId::quarter(2025, q)?, AmountOrScalar::scalar(v))))
    ///         .collect()
    /// };
    /// let model = ModelBuilder::new("rollup")
    ///     .periods("2025Q1..Q4", None)?
    ///     .value("revenue", &quarterly([100.0; 4])?)
    ///     .value("cash", &quarterly([10.0, 20.0, 30.0, 40.0])?)
    ///     .compute("ebitda", "revenue * 0.25")?
    ///     .compute("margin", "ebitda / revenue")?
    ///     .build()?;
    /// let quarterly = Evaluator::new().evaluate(&model)?;
    ///
    /// let spec = RollupSpec {
    ///     frequency: PeriodKind::Annual,
    ///     methods: IndexMap::from([
    ///         ("cash".to_string(), RollupMethod::PeriodEnd),
    ///         ("margin".to_string(), RollupMethod::Formula),
    ///     ]),
    /// };
    /// let annual = quarterly.rollup(&model, &spec)?;
    /// let year = PeriodId::annual(2025);
    /// assert_eq!(annual.get("revenue", &year), Some(400.0));
    /// assert_eq!(annual.get("cash", &year), Some(40.0));
    /// assert_eq!(annual.get("margin", &year), Some(0.25));
    /// # Ok(())
    /// # }
    /// ```
    pub fn rollup(&self, model: &FinancialModelSpec, spec: &RollupSpec) -> Result<StatementResult> {
        let buckets = bucket_periods(&model.periods, spec.frequency)?;
        validate_methods(model, spec)?;

        let mut validated = model.clone();
        let analysis = validated.analyze_semantics()?;
        let eval_order = evaluate_order(&analysis.graph?)?;
        let node_to_column: Arc<IndexMap<NodeId, usize>> = Arc::new(
            eval_order
                .iter()
                .enumerate()
                .map(|(column, node_id)| (node_id.clone(), column))
                .collect(),
        );
        let target_periods: Vec<Period> = buckets.iter().map(|b| b.period.clone()).collect();
        let mut history = Arc::new(PeriodHistory::with_periods(
            Arc::clone(&node_to_column),
            &target_periods,
        ));

        let mut rolled = StatementResult::new();
        let mut warnings = Vec::new();
        for bucket in &buckets {
            let target = bucket.period.id;
            let mut context =
                EvaluationContext::new_with_history(target, Arc::clone(&history), Arc::default());
            for node_id in &eval_order {
                let node = node_id.as_str();
                let value = match spec.methods.get(node).copied().unwrap_or(RollupMethod::Sum) {
                    RollupMethod::Sum => bucket
                        .sources
                        .iter()
                        .map(|period| self.get(node, period))
                        .sum::<Option<f64>>(),
                    RollupMethod::PeriodEnd => bucket
                        .sources
                        .last()
                        .and_then(|period| self.get(node, period)),
                    RollupMethod::Formula => {
                        let where_key = NodeId::new(format!("__where__{node}"));
                        let masked = match analysis.compiled.get(&where_key) {
                            Some(condition) => {
                                !is_truthy(evaluate_formula(condition, &mut context, Some(node))?)
                            }
                            None => false,
                        };
                        if masked {
                            Some(0.0)
                        } else {
                            let formula = analysis.compiled.get(node_id).ok_or_else(|| {
                                Error::eval(format!("Formula for node '{node}' was not compiled"))
                            })?;
                            Some(evaluate_formula(formula, &mut context, Some(node))?)
                        }
                    }
                };
                if let Some(value) = value {
                    context.set_value(node, value)?;
                    rolled
                        .nodes
                        .entry(node.to_string())
                        .or_default()
                        .insert(target, value);
                }
            }
            warnings.append(&mut context.warnings);
            Arc::make_mut(&mut history).push_row(target, context.current_values);
        }

        rolled.meta = EvalStats {
            eval_time_ms: None,
            num_nodes: eval_order.len(),
            num_periods: buckets.len(),
            numeric_mode: NumericMode::Float64,
            parallel: false,
            warnings,
        };
        let value_types: IndexMap<NodeId, NodeValueType> = eval_order
            .iter()
            .filter_map(|node_id| {
                self.node_value_types
                    .get(node_id.as_str())
                    .copied()
                    .or_else(|| validated.get_node(node_id.as_str())?.value_type)
                    .map(|value_type| (node_id.clone(), value_type))
            })
            .collect();
        rolled.populate_value_types(&value_types);
        Ok(rolled)
    }
}

/// Group the model's periods into complete target periods.
fn bucket_periods(periods: &[Period], target: PeriodKind) -> Result<Vec<Bucket>> {
    let first = periods
        .first()
        .ok_or_else(|| Error::invalid_input("Cannot roll up a model without periods"))?;
    let source = first.id.kind();
    if let Some(other) = periods.iter().find(|period| period.id.kind() != source) {
        return Err(Error::invalid_input(format!(
            "Cannot roll up a model that mixes {source} and {} periods",
            other.id.kind()
        )));
    }
    // Months, quarters, halves and years nest exactly; days and weeks do not.
    let nested = [
        PeriodKind::Monthly,
        PeriodKind::Quarterly,
        PeriodKind::SemiAnnual,
        PeriodKind::Annual,
    ];
    if !nested.contains(&source)
        || !nested.contains(&target)
        || target.periods_per_year() >= source.periods_per_year()
    {
        return Err(Error::invalid_input(format!(
            "Cannot roll {source} periods up to {target} periods: the target must be coarser \
             and hold a whole number of source periods (monthly into quarterly, semi-annual or \
             annual; quarterly into semi-annual or annual; semi-annual into annual)"
        )));
    }
    let per_bucket = source.periods_per_year() / target.periods_per_year();

    let mut groups: IndexMap<PeriodId, Vec<&Period>> = IndexMap::new();
    for period in periods {
        groups
            .entry(target_period_id(period.id, target, per_bucket)?)
            .or_default()
            .push(period);
    }

    groups
        .into_iter()
        .map(|(id, members)| {
            if members.len() != usize::from(per_bucket) {
                let present: Vec<String> = members.iter().map(|p| p.id.to_string()).collect();
                return Err(Error::invalid_input(format!(
                    "Target period {id} is incomplete: the model has {} of its {per_bucket} \
                     {source} periods ({}); only whole {target} periods can be rolled up",
                    members.len(),
                    present.join(", ")
                )));
            }
            let (Some(first), Some(last)) = (members.first(), members.last()) else {
                return Err(Error::invalid_input(format!("Target period {id} is empty")));
            };
            Ok(Bucket {
                period: Period {
                    id,
                    start: first.start,
                    end: last.end,
                    is_actual: members.iter().all(|period| period.is_actual),
                },
                sources: members.iter().map(|period| period.id).collect(),
            })
        })
        .collect()
}

/// Identifier of the `target` period containing `source`, keeping its year
/// and fiscal labelling.
fn target_period_id(source: PeriodId, target: PeriodKind, per_bucket: u16) -> Result<PeriodId> {
    let prefix = if source.is_fiscal() { "FY" } else { "" };
    let year = source.year();
    let index = (source.index() - 1) / per_bucket + 1;
    let label = match target {
        PeriodKind::Quarterly => format!("{prefix}{year}Q{index}"),
        PeriodKind::SemiAnnual => format!("{prefix}{year}H{index}"),
        _ => format!("{prefix}{year}"),
    };
    Ok(label.parse()?)
}

/// Reject methods for unknown nodes and `formula` methods that cannot be
/// re-evaluated.
fn validate_methods(model: &FinancialModelSpec, spec: &RollupSpec) -> Result<()> {
    let mut unrecomputable = Vec::new();
    for (node_id, method) in &spec.methods {
        let node = model.get_node(node_id).ok_or_else(|| {
            Error::invalid_input(format!("Rollup method names unknown node '{node_id}'"))
        })?;
        if *method != RollupMethod::Formula {
            continue;
        }
        let formula = node.formula_text.as_deref().ok_or_else(|| {
            Error::invalid_input(format!(
                "Rollup method for node '{node_id}' is formula, but the node has no formula"
            ))
        })?;
        let reads_capital_structure =
            crate::utils::formula::contains_capital_structure_reference(formula)?
                || node
                    .where_text
                    .as_deref()
                    .map(crate::utils::formula::contains_capital_structure_reference)
                    .transpose()?
                    .unwrap_or(false);
        if reads_capital_structure {
            unrecomputable.push(node_id.as_str());
        }
    }
    if !unrecomputable.is_empty() {
        return Err(Error::invalid_input(format!(
            "Nodes [{}] read capital-structure (cs.*) values, which are not rolled up, so their \
             formulas cannot be re-evaluated; roll them up with sum or period_end",
            unrecomputable.join(", ")
        )));
    }
    Ok(())
}
