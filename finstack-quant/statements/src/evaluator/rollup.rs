//! Roll evaluated statement results up to a coarser reporting frequency.
//!
//! Quarterly (or monthly, semi-annual) results become annual (or quarterly,
//! semi-annual) results by combining each node's values over the source
//! periods of every target period:
//!
//! - **Flows** (revenue, expenses, cash flows) are summed (`sum`): exact
//!   money, or a compensated scalar sum. A source cell without a value leaves
//!   the target unavailable.
//! - **Balances** (cash, debt, equity, last-twelve-month metrics) take the
//!   final source cell of the bucket with its status (`period_end`).
//! - **Ratios** (margins, leverage, coverage) re-evaluate the node's formula
//!   and mask on the rolled-up inputs at the target cadence (`formula`),
//!   since neither a sum nor a period-end value of a ratio is the ratio of
//!   the period.
//!
//! Every node needs an explicit method: its entry in [`RollupSpec::methods`],
//! or else its declaration ([`NodeSpec::rollup`]). Nothing is summed by
//! default and tags never choose a method; a node with no method fails the
//! rollup. [`RollupSpec::declared_methods`] lists the declarations for a host
//! that shows or edits them. The rolled result records the source run, the
//! method used for each node and the source periods of each target period
//! ([`RollupProvenance`]).
//!
//! [`NodeSpec::rollup`]: crate::types::NodeSpec::rollup

use crate::error::{Error, Result};
use crate::evaluator::Evaluator;
use crate::evaluator::PeriodHistory;
use crate::evaluator::cell::{Cell, UnavailableReason};
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::formula::evaluate_value;
use crate::evaluator::results::{
    EvalStats, NumericMode, ResultCell, RunProvenance, StatementResult,
};
use crate::evaluator::value::{self, Aggregate, At, Value};
use crate::types::{FinancialModelSpec, NodeId, NodeValueType, RollupMethod};
use finstack_quant_core::dates::{Period, PeriodId, PeriodKind, PeriodPlan};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    /// Rollup method per node id, overriding the node's own declaration. A
    /// node not listed uses its declared [`NodeSpec::rollup`]; a node with
    /// neither fails the rollup.
    ///
    /// [`NodeSpec::rollup`]: crate::types::NodeSpec::rollup
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub methods: IndexMap<String, RollupMethod>,
}

impl RollupSpec {
    /// List the rollup methods a model's lines declare, so a host can seed
    /// the method choices it shows.
    ///
    /// Only [`NodeSpec::rollup`] is read. A line that declares nothing is not
    /// listed, whatever its tags say, and needs a method before the model
    /// can be rolled up. Passing the result to
    /// [`StatementResult::rollup`] gives the same numbers as passing a spec
    /// with no methods, because a line resolves to its declaration anyway.
    ///
    /// # Arguments
    ///
    /// * `model` - Model whose node declarations are listed. Only its nodes
    ///   are read; nothing is evaluated.
    /// * `frequency` - Target frequency stored on the returned spec. It is not
    ///   checked against the model's periods here; [`StatementResult::rollup`]
    ///   does that.
    ///
    /// # Returns
    ///
    /// A [`RollupSpec`] for `frequency` listing every declared node with its
    /// method, in model order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::PeriodKind;
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_statements::evaluator::RollupSpec;
    /// use finstack_quant_statements::types::RollupMethod;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut model = {
    ///     let mut model_builder = ModelBuilder::new("declared");
    ///     model_builder.periods("2025Q1..Q4", None)?;
    ///     model_builder.value_scalar("revenue", &[]);
    ///     model_builder.value_scalar("cash", &[]);
    ///     model_builder.compute("margin", "revenue / revenue")?;
    ///     model_builder.build()?
    /// };
    /// let mut declare = |id: &str, method: RollupMethod| {
    ///     if let Some(node) = model.get_node_mut(id) {
    ///         node.rollup = Some(method);
    ///     }
    /// };
    /// declare("cash", RollupMethod::PeriodEnd);
    /// declare("margin", RollupMethod::Formula);
    ///
    /// let spec = RollupSpec::declared_methods(&model, PeriodKind::Annual);
    /// assert_eq!(spec.methods.get("cash"), Some(&RollupMethod::PeriodEnd));
    /// assert_eq!(spec.methods.get("margin"), Some(&RollupMethod::Formula));
    /// assert_eq!(spec.methods.get("revenue"), None);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`NodeSpec::rollup`]: crate::types::NodeSpec::rollup
    #[must_use]
    pub fn declared_methods(model: &FinancialModelSpec, frequency: PeriodKind) -> Self {
        let methods = model
            .nodes
            .iter()
            .filter_map(|(node_id, node)| Some((node_id.as_str().to_string(), node.rollup?)))
            .collect();
        Self { frequency, methods }
    }
}

/// How a rolled-up result was produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RollupProvenance {
    /// The evaluation run the source result came from. Its diagnostics,
    /// check report and capital-structure cashflows stay on that run, keyed
    /// by the source periods.
    pub source_run: RunProvenance,
    /// Target frequency of the rollup.
    pub frequency: PeriodKind,
    /// The method used for every node, in model order: the request's entry
    /// where it had one, the node's declaration otherwise.
    pub methods: IndexMap<String, RollupMethod>,
    /// The source periods combined into each target period, in timeline
    /// order.
    pub source_periods: IndexMap<PeriodId, Vec<PeriodId>>,
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
    /// Each target period combines its source periods node by node, using the
    /// node's entry in `spec.methods` or else its declared
    /// [`NodeSpec::rollup`] (see [`RollupMethod`]). Every node needs one of
    /// the two; nothing is summed by default. Every target period must be
    /// complete: a model that starts or ends mid-year cannot roll up to
    /// annual periods.
    ///
    /// - `sum` adds the source cells: money exactly, scalars with a
    ///   compensated sum. A source cell that is missing, masked or
    ///   unavailable makes the target cell unavailable.
    /// - `period_end` copies the final source cell with its status, so an
    ///   earlier gap does not hide a closing balance and a missing closing
    ///   balance stays missing.
    /// - `formula` re-evaluates the node's mask and formula on the rolled
    ///   cells and the target-period history: a lag of one reads one target
    ///   period. A false mask is a masked cell and an input without a value
    ///   leaves the result unavailable.
    ///
    /// The rolled-up result holds typed `cells` and `node_value_types` for
    /// the target periods, those periods in `periods`, any warnings raised by
    /// re-evaluated formulas, and its [`RollupProvenance`] in
    /// `rollup_provenance`. Each
    /// target period runs from its first source's start to its last source's
    /// end and is actual only when every source is. It is not itself an
    /// evaluation run: `provenance`, cell sources, capital-structure
    /// cashflows and the check report stay on the source result.
    ///
    /// # Arguments
    ///
    /// * `model` - The model these results were evaluated from. It supplies
    ///   the periods, with their dates and actual/forecast split, each line's
    ///   declared rollup method, and the formulas re-evaluated for `formula`
    ///   nodes.
    /// * `spec` - Target frequency and per-node rollup methods, which override
    ///   the methods the nodes declare.
    ///
    /// # Returns
    ///
    /// A [`StatementResult`] keyed by the target periods (`2025`, `2025H1`,
    /// `2025Q1`, with the `FY` prefix kept for fiscal periods).
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error if these results are not a verified run
    /// of `model` (no provenance, another model content, source snapshot or
    /// period plan), the target frequency is not a whole multiple of the
    /// model's frequency, the model mixes frequencies, a target period is
    /// incomplete, these results hold a published adjusted metric,
    /// `spec.methods` names an unknown node, a node has no
    /// method, or a node resolving to `formula` has no formula or reads
    /// capital-structure (`cs.*`) values, which are not rolled up.
    /// Returns an evaluation error for a sum that mixes units or cannot be
    /// represented, and for a re-evaluated formula that is invalid at the
    /// target cadence or in its arithmetic.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::{PeriodId, PeriodKind};
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_statements::evaluator::{Evaluator, RollupSpec};
    /// use finstack_quant_statements::types::{AmountOrScalar, RollupMethod};
    /// use indexmap::IndexMap;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let quarterly = |values: [f64; 4]| -> Result<Vec<(PeriodId, AmountOrScalar)>, Box<dyn std::error::Error>> {
    ///     (1..=4)
    ///         .zip(values)
    ///         .map(|(q, v)| Ok((PeriodId::quarter(2025, q)?, AmountOrScalar::scalar(v))))
    ///         .collect()
    /// };
    /// let model = {
    ///     let mut model_builder = ModelBuilder::new("rollup");
    ///     model_builder.periods("2025Q1..Q4", None)?;
    ///     model_builder.value("revenue", &quarterly([100.0; 4])?);
    ///     model_builder.value("cash", &quarterly([10.0, 20.0, 30.0, 40.0])?);
    ///     model_builder.compute("ebitda", "revenue * 0.25")?;
    ///     model_builder.compute("margin", "ebitda / revenue")?;
    ///     model_builder.build()?
    /// };
    /// let quarterly = Evaluator::new().evaluate(&model)?;
    ///
    /// let spec = RollupSpec {
    ///     frequency: PeriodKind::Annual,
    ///     methods: IndexMap::from([
    ///         ("revenue".to_string(), RollupMethod::Sum),
    ///         ("cash".to_string(), RollupMethod::PeriodEnd),
    ///         ("ebitda".to_string(), RollupMethod::Sum),
    ///         ("margin".to_string(), RollupMethod::Formula),
    ///     ]),
    /// };
    /// let annual = quarterly.rollup(&model, &spec)?;
    /// let year = PeriodId::annual(2025).expect("valid annual period");
    /// assert_eq!(annual.get("revenue", &year), Some(400.0));
    /// assert_eq!(annual.get("cash", &year), Some(40.0));
    /// assert_eq!(annual.get("margin", &year), Some(0.25));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`NodeSpec::rollup`]: crate::types::NodeSpec::rollup
    pub fn rollup(&self, model: &FinancialModelSpec, spec: &RollupSpec) -> Result<StatementResult> {
        if let Some(published) = self.normalizations.keys().next() {
            return Err(Error::invalid_input(format!(
                "Cannot roll up results that hold the published adjusted metric '{published}': \
                 it is not a model node and has no rollup method. Roll up the evaluated \
                 results and normalize the rolled-up result"
            )));
        }
        let buckets = bucket_periods(&model.periods, spec.frequency)?;
        let methods = resolve_methods(model, spec)?;
        let source_run = self.verified_run_of(model, None)?.clone();

        let prepared = Evaluator::new().prepare(model)?;
        let eval_order = &prepared.eval_order;
        let node_to_column = Arc::clone(&prepared.node_to_column);
        let target_periods: Vec<Period> = buckets.iter().map(|b| b.period.clone()).collect();
        let target_plan = PeriodPlan::try_new(model.periods.calendar, target_periods)?;
        let mut history = Arc::new(PeriodHistory::with_periods(
            Arc::clone(&node_to_column),
            &target_plan,
        ));
        let value_types: IndexMap<NodeId, NodeValueType> = eval_order
            .iter()
            .filter_map(|node_id| {
                self.node_value_types
                    .get(node_id.as_str())
                    .copied()
                    .or_else(|| prepared.node_value_types.get(node_id).copied())
                    .map(|value_type| (node_id.clone(), value_type))
            })
            .collect();

        let mut rolled = StatementResult::new();
        rolled.periods = Some(target_plan);
        let mut warnings = Vec::new();
        for bucket in &buckets {
            let target = bucket.period.id;
            let mut context =
                EvaluationContext::new_with_history(target, Arc::clone(&history), Arc::default());
            for node_id in eval_order {
                let node = node_id.as_str();
                let method = methods.get(node).copied().ok_or_else(|| {
                    Error::eval(format!("No rollup method was resolved for node '{node}'"))
                })?;
                let at = At {
                    node_id: Some(node),
                    period: target,
                };
                let source_cell = |period: &PeriodId| {
                    self.get_cell(node, period)
                        .map_or(Cell::Missing, |cell| cell.to_cell())
                };
                let cell = match method {
                    RollupMethod::Sum => {
                        let values: Vec<Value> = bucket
                            .sources
                            .iter()
                            .map(|period| Value::from_cell(source_cell(period)))
                            .collect();
                        let total = value::aggregate(Aggregate::Sum, &values, at)?;
                        value::into_cell(total, value_types.get(node_id).copied(), at)?
                    }
                    RollupMethod::PeriodEnd => {
                        bucket.sources.last().map_or(Cell::Missing, source_cell)
                    }
                    RollupMethod::Formula => {
                        let where_key = NodeId::new(format!("__where__{node}"));
                        let mask = match prepared.compiled_cache.get(&where_key) {
                            Some(condition) => {
                                let mask = evaluate_value(condition, &mut context, Some(node))?;
                                value::control(mask, "where clause", at)?
                            }
                            None => Some(true),
                        };
                        match mask {
                            Some(false) => Cell::Masked,
                            None => Cell::Unavailable(UnavailableReason::MissingMask),
                            Some(true) => {
                                let formula =
                                    prepared.compiled_cache.get(node_id).ok_or_else(|| {
                                        Error::eval(format!(
                                            "Formula for node '{node}' was not compiled"
                                        ))
                                    })?;
                                let result = evaluate_value(formula, &mut context, Some(node))?;
                                value::into_cell(result, value_types.get(node_id).copied(), at)?
                            }
                        }
                    }
                };
                context.set_cell(node, cell)?;
                rolled.set_cell(node, target, ResultCell::from_cell(cell, None));
            }
            warnings.append(&mut context.warnings);
            Arc::make_mut(&mut history).push_typed_row(
                target,
                context.current_values,
                context.current_cells,
            );
        }

        rolled.meta = EvalStats {
            eval_time_ms: None,
            num_nodes: eval_order.len(),
            num_periods: buckets.len(),
            numeric_mode: NumericMode::Float64,
            parallel: false,
            warnings,
        };
        rolled.populate_value_types(&value_types);
        rolled.rollup_provenance = Some(RollupProvenance {
            source_run,
            frequency: spec.frequency,
            methods: methods
                .iter()
                .map(|(node, method)| ((*node).to_string(), *method))
                .collect(),
            source_periods: buckets
                .iter()
                .map(|bucket| (bucket.period.id, bucket.sources.clone()))
                .collect(),
        });
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
    let (Some(source_slots), Some(target_slots)) =
        (source.slots_per_year(), target.slots_per_year())
    else {
        return Err(Error::invalid_input(format!(
            "Cannot roll {source} periods up to {target} periods: only months, quarters, halves \
             and years nest exactly"
        )));
    };
    if target_slots >= source_slots {
        return Err(Error::invalid_input(format!(
            "Cannot roll {source} periods up to {target} periods: the target must be coarser \
             and hold a whole number of source periods (monthly into quarterly, semi-annual or \
             annual; quarterly into semi-annual or annual; semi-annual into annual)"
        )));
    }
    let per_bucket = source_slots / target_slots;

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

/// Resolve each node's method (its `spec` entry, then its declaration),
/// rejecting spec entries for unknown nodes, nodes with no method and
/// resolved `formula` methods that cannot be re-evaluated.
fn resolve_methods<'model>(
    model: &'model FinancialModelSpec,
    spec: &RollupSpec,
) -> Result<IndexMap<&'model str, RollupMethod>> {
    if let Some(node_id) = spec.methods.keys().find(|id| model.get_node(id).is_none()) {
        return Err(Error::invalid_input(format!(
            "Rollup method names unknown node '{node_id}'"
        )));
    }
    let unresolved: Vec<&str> = model
        .nodes
        .iter()
        .filter(|(node_id, node)| {
            node.rollup.is_none() && !spec.methods.contains_key(node_id.as_str())
        })
        .map(|(node_id, _)| node_id.as_str())
        .collect();
    if !unresolved.is_empty() {
        return Err(Error::invalid_input(format!(
            "Nodes [{}] have no rollup method: declare sum, period_end or formula on each node, \
             or name it in the rollup request. Nothing is summed by default",
            unresolved.join(", ")
        )));
    }
    let mut methods = IndexMap::with_capacity(model.nodes.len());
    let mut unrecomputable = Vec::new();
    for (node_id, node) in &model.nodes {
        let node_id = node_id.as_str();
        let Some(method) = spec.methods.get(node_id).copied().or(node.rollup) else {
            continue;
        };
        methods.insert(node_id, method);
        if method != RollupMethod::Formula {
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
            unrecomputable.push(node_id);
        }
    }
    if !unrecomputable.is_empty() {
        return Err(Error::invalid_input(format!(
            "Nodes [{}] read capital-structure (cs.*) values, which are not rolled up, so their \
             formulas cannot be re-evaluated; roll them up with sum or period_end",
            unrecomputable.join(", ")
        )));
    }
    Ok(methods)
}
