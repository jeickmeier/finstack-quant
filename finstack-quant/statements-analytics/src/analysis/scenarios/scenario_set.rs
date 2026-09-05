//! Multi-scenario management and comparison for statement models.
//!
//! This module provides a lightweight registry for named scenarios built on
//! top of a single [`FinancialModelSpec`].
//!
//! A `ScenarioSet` stores a map of scenario name → [`ScenarioDefinition`],
//! supports simple parent chaining with override merging, and can:
//! - Evaluate all scenarios into a [`ScenarioResults`] envelope.
//! - Compute variance-style diffs between two scenarios using
//!   [`VarianceAnalyzer`].
//! - Export wide comparison tables as serializable table envelopes.
//!
//! The wire format is intentionally simple and mirrors the design docs:
//!
//! ```json
//! {
//!   "scenario_set": {
//!     "base": { "overrides": {} },
//!     "downside": {
//!       "parent": "base",
//!       "overrides": { "revenue_growth": -0.05, "margin": -0.02 }
//!     },
//!     "stress": {
//!       "parent": "downside",
//!       "overrides": { "revenue_growth": -0.15 }
//!     }
//!   }
//! }
//! ```
//!
//! In the first implementation, `overrides` is interpreted as a map of
//! `node_id → scalar`, where the scalar is broadcast as an explicit value
//! for **all periods** of the given node. This leverages the existing
//! precedence rules (`Value > Forecast > Formula`) to override model drivers
//! in a deterministic way without introducing new forecast semantics.

use crate::analysis::{VarianceAnalyzer, VarianceConfig, VarianceReport};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::math::ZERO_TOLERANCE;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};
use finstack_quant_statements::error::{Error, Result};
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec};
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Definition for a single named scenario.
///
/// Scenarios are attached to a base [`FinancialModelSpec`] and specify an
/// optional parent plus typed scalar or monetary overrides for named nodes.
///
/// The JSON representation mirrors the design docs example:
///
/// ```json
/// "downside": {
///   "parent": "base",
///   "overrides": { "revenue_growth": -0.05, "margin": -0.02 }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioDefinition {
    /// Optional parent scenario to inherit overrides from.
    ///
    /// Parent chains can be arbitrarily deep but must be acyclic. Later
    /// scenarios in the chain override earlier ones for the same `node_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,

    /// Typed overrides for model nodes.
    ///
    /// Overrides apply to forecast periods only and must match the target
    /// node's scalar or monetary type and currency.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub overrides: IndexMap<String, AmountOrScalar>,

    /// Typed per-period overrides for model nodes, keyed `node_id` then
    /// period id.
    ///
    /// A per-period override applies only to the named forecast period and
    /// takes precedence over a model-wide `overrides` entry for the same node
    /// in that period. Across a parent chain the child's per-period value wins
    /// for that (node, period), but a child's model-wide override does not
    /// clear a parent's per-period overrides for other periods.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub period_overrides: IndexMap<String, IndexMap<PeriodId, AmountOrScalar>>,
}

/// Registry of named scenarios built on top of a base model.
///
/// The `scenarios` map preserves insertion order and is the primary serialized
/// surface. Overrides preserve scalar/monetary units while broadcasting across
/// forecast periods.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioSet {
    /// Map of scenario name → definition.
    pub scenarios: IndexMap<String, ScenarioDefinition>,
}

/// Evaluated results for all scenarios in a [`ScenarioSet`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScenarioResults {
    /// Map of scenario name → evaluated [`StatementResult`] for that scenario.
    pub scenarios: IndexMap<String, StatementResult>,
}

/// Variance-style diff between two evaluated scenarios.
///
/// This is a thin wrapper around [`VarianceReport`] that keeps track of the
/// baseline and comparison scenario names.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioDiff {
    /// Baseline scenario name.
    pub baseline: String,
    /// Comparison scenario name.
    pub comparison: String,
    /// Underlying variance report.
    pub variance: VarianceReport,
}

impl ScenarioSet {
    /// Evaluate all scenarios against a base financial model.
    ///
    /// Parent scenarios are resolved first, with child overrides applied
    /// last. Each scenario is evaluated independently starting from the
    /// provided `base_model`.
    ///
    /// # Arguments
    ///
    /// * `base_model` - Baseline model that each scenario overrides
    ///
    /// # Returns
    ///
    /// Returns [`ScenarioResults`] keyed by scenario name in insertion order.
    ///
    /// # Errors
    ///
    /// Returns an error if the scenario set is empty, if parent chains are
    /// invalid, if overrides reference missing nodes, or if model evaluation
    /// fails for any scenario.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::PeriodId;
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_statements::types::AmountOrScalar;
    /// use finstack_quant_statements_analytics::analysis::{ScenarioDefinition, ScenarioSet};
    /// use indexmap::IndexMap;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    /// let model = ModelBuilder::new("scenario-model")
    ///     .periods("2025Q1..Q1", None)?
    ///     .value("revenue", &[(period, AmountOrScalar::scalar(100.0))])
    ///     .build()?;
    ///
    /// let mut scenarios = IndexMap::new();
    /// scenarios.insert(
    ///     "base".to_string(),
    ///     ScenarioDefinition {
    ///         parent: None,
    ///         period_overrides: IndexMap::new(),
    ///         overrides: IndexMap::new(),
    ///     },
    /// );
    /// scenarios.insert(
    ///     "downside".to_string(),
    ///     ScenarioDefinition {
    ///         parent: Some("base".to_string()),
    ///         period_overrides: IndexMap::new(),
    ///         overrides: IndexMap::from([(
    ///             "revenue".to_string(),
    ///             AmountOrScalar::scalar(90.0),
    ///         )]),
    ///     },
    /// );
    ///
    /// let results = ScenarioSet { scenarios }.evaluate_all(&model)?;
    /// assert_eq!(results.len(), 2);
    /// # Ok(())
    /// # }
    /// ```
    pub fn evaluate_all(&self, base_model: &FinancialModelSpec) -> Result<ScenarioResults> {
        if self.scenarios.is_empty() {
            return Err(Error::invalid_input(
                "ScenarioSet.scenarios cannot be empty",
            ));
        }

        let mut out = IndexMap::new();

        for (name, _) in &self.scenarios {
            let merged_overrides = self.resolve_overrides(name)?;
            let mut model = base_model.clone();
            apply_overrides(&mut model, &merged_overrides)?;

            let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
            let results = evaluator.evaluate(&model)?;
            out.insert(name.clone(), results);
        }

        Ok(ScenarioResults { scenarios: out })
    }

    /// Compute a variance-style diff between two evaluated scenarios.
    ///
    /// This delegates to [`VarianceAnalyzer`] under the hood so that
    /// scenario diffs share the same semantics as other variance reports.
    ///
    /// # Arguments
    ///
    /// * `results` - Previously evaluated scenario outputs
    /// * `baseline` - Scenario name to treat as the base case
    /// * `comparison` - Scenario name to treat as the comparison case
    /// * `metrics` - Node ids to compare
    /// * `periods` - Periods to include in the diff
    ///
    /// # Returns
    ///
    /// Returns a [`ScenarioDiff`] wrapping the underlying [`VarianceReport`].
    ///
    /// # Errors
    ///
    /// Returns an error if either scenario is missing, or if `metrics` or
    /// `periods` is empty, or if the variance calculation fails.
    ///
    /// # References
    ///
    /// - One-pass variance decomposition context: `docs/REFERENCES.md#welford-1962`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::PeriodId;
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_statements::types::AmountOrScalar;
    /// use finstack_quant_statements_analytics::analysis::{ScenarioDefinition, ScenarioSet};
    /// use indexmap::IndexMap;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    /// let model = ModelBuilder::new("scenario-model")
    ///     .periods("2025Q1..Q1", None)?
    ///     .value("revenue", &[(period, AmountOrScalar::scalar(100.0))])
    ///     .build()?;
    ///
    /// let scenarios = IndexMap::from([
    ///     ("base".to_string(), ScenarioDefinition {
    ///         parent: None,
    ///         period_overrides: IndexMap::new(),
    ///         overrides: IndexMap::new(),
    ///     }),
    ///     ("downside".to_string(), ScenarioDefinition {
    ///         parent: Some("base".to_string()),
    ///         period_overrides: IndexMap::new(),
    ///         overrides: IndexMap::from([(
    ///             "revenue".to_string(),
    ///             AmountOrScalar::scalar(90.0),
    ///         )]),
    ///     }),
    /// ]);
    ///
    /// let set = ScenarioSet { scenarios };
    /// let results = set.evaluate_all(&model)?;
    /// let diff = set.diff(&results, "base", "downside", &["revenue".to_string()], &[period])?;
    /// assert_eq!(diff.baseline, "base");
    /// # Ok(())
    /// # }
    /// ```
    pub fn diff(
        &self,
        results: &ScenarioResults,
        baseline: &str,
        comparison: &str,
        metrics: &[String],
        periods: &[PeriodId],
    ) -> Result<ScenarioDiff> {
        if metrics.is_empty() {
            return Err(Error::invalid_input("metrics cannot be empty"));
        }

        if periods.is_empty() {
            return Err(Error::invalid_input("periods cannot be empty"));
        }

        let baseline_results = results.scenarios.get(baseline).ok_or_else(|| {
            Error::invalid_input(format!("Unknown baseline scenario '{baseline}'"))
        })?;

        let comparison_results = results.scenarios.get(comparison).ok_or_else(|| {
            Error::invalid_input(format!("Unknown comparison scenario '{comparison}'"))
        })?;

        let analyzer = VarianceAnalyzer::new(baseline_results, comparison_results);
        let config = VarianceConfig::new(
            baseline.to_string(),
            comparison.to_string(),
            metrics.to_vec(),
            periods.to_vec(),
        );

        let variance = analyzer.compute(&config)?;

        Ok(ScenarioDiff {
            baseline: baseline.to_string(),
            comparison: comparison.to_string(),
            variance,
        })
    }

    /// Return the lineage of a scenario from root ancestor to the given name.
    ///
    /// This is useful for explainability and debugging of nested overrides.
    ///
    /// # Arguments
    ///
    /// * `scenario` - Scenario name to trace
    ///
    /// # Returns
    ///
    /// Returns scenario names ordered from the root ancestor to `scenario`.
    ///
    /// # Errors
    ///
    /// Returns an error if the named scenario does not exist or the parent
    /// chain is cyclic.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_statements_analytics::analysis::{ScenarioDefinition, ScenarioSet};
    /// use indexmap::IndexMap;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let scenarios = IndexMap::from([
    ///     ("base".to_string(), ScenarioDefinition {
    ///         parent: None,
    ///         period_overrides: IndexMap::new(),
    ///         overrides: IndexMap::new(),
    ///     }),
    ///     ("stress".to_string(), ScenarioDefinition {
    ///         parent: Some("base".to_string()),
    ///         period_overrides: IndexMap::new(),
    ///         overrides: IndexMap::new(),
    ///     }),
    /// ]);
    ///
    /// let set = ScenarioSet { scenarios };
    /// assert_eq!(set.trace("stress")?, vec!["base".to_string(), "stress".to_string()]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn trace(&self, scenario: &str) -> Result<Vec<String>> {
        let mut lineage = Vec::new();
        let mut seen = IndexSet::new();
        let mut current = Some(scenario);
        let mut steps = 0usize;
        let max_steps = self.scenarios.len().saturating_add(1);

        while let Some(name) = current {
            steps += 1;
            if steps > max_steps {
                return Err(Error::invalid_input(
                    "Scenario parent chain exceeded maximum depth; check for cycles or corrupted data",
                ));
            }
            if !seen.insert(name.to_string()) {
                return Err(Error::invalid_input(format!(
                    "Cycle detected in scenario parents at '{name}'"
                )));
            }

            let def = self.scenarios.get(name).ok_or_else(|| {
                Error::invalid_input(format!("Unknown scenario '{name}' in trace()"))
            })?;

            lineage.push(name.to_string());
            current = def.parent.as_deref();
        }

        lineage.reverse();
        Ok(lineage)
    }

    /// Resolve the full override map for a given scenario by walking the
    /// parent chain from root to leaf.
    ///
    /// Later scenarios in the chain override earlier ones for the same
    /// `node_id`.
    fn resolve_overrides(&self, name: &str) -> Result<ResolvedOverrides> {
        let mut merged = IndexMap::new();
        let mut merged_periods: IndexMap<String, IndexMap<PeriodId, AmountOrScalar>> =
            IndexMap::new();
        let mut stack = Vec::new();
        let mut seen = IndexSet::new();
        let mut current = Some(name);
        let mut steps = 0usize;
        let max_steps = self.scenarios.len().saturating_add(1);

        while let Some(scenario_name) = current {
            steps += 1;
            if steps > max_steps {
                return Err(Error::invalid_input(
                    "Scenario parent chain exceeded maximum depth; check for cycles or corrupted data",
                ));
            }
            if !seen.insert(scenario_name.to_string()) {
                return Err(Error::invalid_input(format!(
                    "Cycle detected in scenario parents at '{scenario_name}'"
                )));
            }

            let def = self.scenarios.get(scenario_name).ok_or_else(|| {
                Error::invalid_input(format!("Unknown scenario '{scenario_name}'"))
            })?;
            stack.push(scenario_name);
            current = def.parent.as_deref();
        }

        // Merge from oldest ancestor → target scenario so that later overrides win.
        while let Some(scenario_name) = stack.pop() {
            // Scenario was verified to exist when pushed onto the stack
            if let Some(def) = self.scenarios.get(scenario_name) {
                for (node_id, value) in &def.overrides {
                    merged.insert(node_id.clone(), *value);
                }
                for (node_id, by_period) in &def.period_overrides {
                    let entry = merged_periods.entry(node_id.clone()).or_default();
                    for (period, value) in by_period {
                        entry.insert(*period, *value);
                    }
                }
            }
        }

        Ok(ResolvedOverrides {
            model_wide: merged,
            per_period: merged_periods,
        })
    }
}

/// Overrides merged along a scenario's parent chain.
struct ResolvedOverrides {
    model_wide: IndexMap<String, AmountOrScalar>,
    per_period: IndexMap<String, IndexMap<PeriodId, AmountOrScalar>>,
}

impl ScenarioResults {
    /// Return the number of scenarios.
    ///
    /// # Returns
    ///
    /// The number of evaluated scenarios held by this result envelope.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_statements_analytics::analysis::ScenarioResults;
    /// use indexmap::IndexMap;
    ///
    /// let results = ScenarioResults { scenarios: IndexMap::new() };
    /// assert_eq!(results.len(), 0);
    /// ```
    pub fn len(&self) -> usize {
        self.scenarios.len()
    }

    /// Check if there are no scenarios.
    ///
    /// # Returns
    ///
    /// `true` when the envelope contains no evaluated scenarios.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_statements_analytics::analysis::ScenarioResults;
    /// use indexmap::IndexMap;
    ///
    /// let results = ScenarioResults { scenarios: IndexMap::new() };
    /// assert!(results.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.scenarios.is_empty()
    }
}

impl ScenarioResults {
    /// Export a wide comparison table to a serializable table envelope.
    ///
    /// Columns:
    /// - `period` (Utf8)
    /// - `metric` (Utf8)
    /// - One Float64 column per scenario (named by scenario).
    /// - For each non-baseline scenario, an additional Float64 column
    ///   `<scenario>_vs_<baseline>_frac` computed as:
    ///   `(scenario - baseline) / baseline`, with `0.0` used when the
    ///   baseline is effectively zero to avoid infinities/NaNs.
    ///
    /// The baseline scenario is chosen as:
    /// - `"base"` if present, otherwise
    /// - the first scenario in insertion order.
    ///
    /// # Arguments
    ///
    /// * `metrics` - Node ids to include in the comparison table.
    ///
    /// # Returns
    ///
    /// A [`TableEnvelope`] with one row per `(period, metric)` pair and one
    /// value column per scenario.
    ///
    /// # Errors
    ///
    /// Returns an error if the result set or metric list is empty, or if table
    /// construction invariants fail.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::PeriodId;
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_statements::types::AmountOrScalar;
    /// use finstack_quant_statements_analytics::analysis::{ScenarioDefinition, ScenarioSet};
    /// use indexmap::IndexMap;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    /// let model = ModelBuilder::new("scenario-model")
    ///     .periods("2025Q1..Q1", None)?
    ///     .value("revenue", &[(period, AmountOrScalar::scalar(100.0))])
    ///     .build()?;
    /// let scenarios = IndexMap::from([
    ///     ("base".to_string(), ScenarioDefinition {
    ///         parent: None,
    ///         period_overrides: IndexMap::new(),
    ///         overrides: IndexMap::new(),
    ///     }),
    ///     ("downside".to_string(), ScenarioDefinition {
    ///         parent: Some("base".to_string()),
    ///         period_overrides: IndexMap::new(),
    ///         overrides: IndexMap::from([(
    ///             "revenue".to_string(),
    ///             AmountOrScalar::scalar(90.0),
    ///         )]),
    ///     }),
    /// ]);
    /// let results = ScenarioSet { scenarios }.evaluate_all(&model)?;
    ///
    /// let table = results.to_comparison_table(&["revenue"])?;
    /// assert!(!table.columns.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_comparison_table(&self, metrics: &[&str]) -> Result<TableEnvelope> {
        if self.scenarios.is_empty() {
            return Err(Error::invalid_input(
                "ScenarioResults.scenarios cannot be empty",
            ));
        }

        if metrics.is_empty() {
            return Err(Error::invalid_input("metrics cannot be empty"));
        }

        let mut scenario_iter = self.scenarios.iter();
        let baseline_name = if self.scenarios.contains_key("base") {
            "base"
        } else {
            scenario_iter
                .next()
                .map(|(name, _)| name.as_str())
                .ok_or_else(|| Error::invalid_input("ScenarioResults.scenarios cannot be empty"))?
        };

        let baseline_results = self.scenarios.get(baseline_name).ok_or_else(|| {
            Error::invalid_input(format!(
                "Baseline scenario '{}' not found in ScenarioResults",
                baseline_name
            ))
        })?;

        let scenario_names: Vec<&str> = self.scenarios.keys().map(|k| k.as_str()).collect();
        let non_baseline_names: Vec<&str> = scenario_names
            .iter()
            .copied()
            .filter(|name| *name != baseline_name)
            .collect();

        let mut periods_col: Vec<String> = Vec::new();
        let mut metrics_col: Vec<String> = Vec::new();

        let mut scenario_values: Vec<Vec<Option<f64>>> = vec![Vec::new(); scenario_names.len()];
        let mut pct_values: Vec<Vec<Option<f64>>> = vec![Vec::new(); non_baseline_names.len()];

        for metric in metrics {
            let metric_nodes = baseline_results.nodes.get(*metric).ok_or_else(|| {
                Error::missing_data(format!(
                    "Metric '{}' not found in baseline scenario '{}'",
                    metric, baseline_name
                ))
            })?;

            for (&period, _) in metric_nodes {
                periods_col.push(period.to_string());
                metrics_col.push((*metric).to_string());

                let baseline_value = baseline_results.get(metric, &period);

                for (idx, scenario_name) in scenario_names.iter().enumerate() {
                    if let Some(results) = self.scenarios.get(*scenario_name) {
                        let value = results.get(metric, &period);
                        scenario_values[idx].push(value);
                    } else {
                        scenario_values[idx].push(None);
                    }
                }

                for (pct_idx, scenario_name) in non_baseline_names.iter().enumerate() {
                    if let Some(results) = self.scenarios.get(*scenario_name) {
                        let value = results.get(metric, &period);

                        // Percent change is undefined on a (near-)zero
                        // baseline; emit a null instead of a misleading 0%
                        // (consistent with `VarianceRow::pct_var` semantics).
                        let pct = match (baseline_value, value) {
                            (Some(base), Some(v)) => {
                                if base.abs() < ZERO_TOLERANCE {
                                    None
                                } else {
                                    Some((v - base) / base)
                                }
                            }
                            _ => None,
                        };

                        pct_values[pct_idx].push(pct);
                    } else {
                        pct_values[pct_idx].push(None);
                    }
                }
            }
        }

        let mut columns = vec![
            TableColumn::new("period", TableColumnData::String(periods_col))
                .with_role(TableColumnRole::Index),
            TableColumn::new("metric", TableColumnData::String(metrics_col))
                .with_role(TableColumnRole::Dimension),
        ];

        for (idx, scenario_name) in scenario_names.iter().enumerate() {
            columns.push(
                TableColumn::new(
                    (*scenario_name).to_string(),
                    TableColumnData::NullableFloat64(scenario_values[idx].clone()),
                )
                .with_role(TableColumnRole::Measure),
            );

            if *scenario_name != baseline_name {
                if let Some(pct_idx) = non_baseline_names
                    .iter()
                    .position(|name| *name == *scenario_name)
                {
                    let frac_col_name = format!("{}_vs_{}_frac", scenario_name, baseline_name);
                    columns.push(
                        TableColumn::new(
                            frac_col_name,
                            TableColumnData::NullableFloat64(pct_values[pct_idx].clone()),
                        )
                        .with_role(TableColumnRole::Measure),
                    );
                }
            }
        }

        let mut metadata = IndexMap::new();
        metadata.insert("layout".to_string(), json!("long"));
        metadata.insert("source".to_string(), json!("scenario_results"));
        metadata.insert("baseline".to_string(), json!(baseline_name));

        TableEnvelope::new_with_metadata(columns, metadata).map_err(Into::into)
    }
}

/// Apply typed scenario overrides to forecast periods.
///
/// Model-wide overrides are written to every forecast period first; per-period
/// overrides are then written on top, so they win for their own period. A
/// per-period override naming a period that is not a forecast period of the
/// model is rejected.
fn apply_overrides(model: &mut FinancialModelSpec, overrides: &ResolvedOverrides) -> Result<()> {
    if overrides.model_wide.is_empty() && overrides.per_period.is_empty() {
        return Ok(());
    }

    let forecast_period_ids: Vec<PeriodId> = model
        .periods
        .iter()
        .filter(|p| !p.is_actual)
        .map(|p| p.id)
        .collect();

    for (node_id, value) in &overrides.model_wide {
        let node = override_target(model, node_id, value)?;
        let mut values = node.values.clone().unwrap_or_default();
        for period_id in &forecast_period_ids {
            values.insert(*period_id, *value);
        }
        node.values = Some(values);
    }

    for (node_id, by_period) in &overrides.per_period {
        for (period_id, value) in by_period {
            if !forecast_period_ids.contains(period_id) {
                return Err(Error::invalid_input(format!(
                    "Scenario override for node '{node_id}' names period '{period_id}', which is \
                     not a forecast period of the model"
                )));
            }
            let node = override_target(model, node_id, value)?;
            let mut values = node.values.clone().unwrap_or_default();
            values.insert(*period_id, *value);
            node.values = Some(values);
        }
    }

    Ok(())
}

/// Resolve the node an override targets, checking its declared value type
/// against the supplied amount or scalar.
fn override_target<'m>(
    model: &'m mut FinancialModelSpec,
    node_id: &str,
    value: &AmountOrScalar,
) -> Result<&'m mut finstack_quant_statements::types::NodeSpec> {
    let node = model
        .get_node_mut(node_id)
        .ok_or_else(|| Error::invalid_input(format!("Node '{node_id}' not found in model")))?;
    match (node.value_type, value) {
        (
            Some(finstack_quant_statements::types::NodeValueType::Monetary { currency }),
            AmountOrScalar::Amount(money),
        ) if currency == money.currency() => {}
        (
            Some(finstack_quant_statements::types::NodeValueType::Scalar),
            AmountOrScalar::Scalar(_),
        ) => {}
        (Some(expected), supplied) => {
            return Err(Error::invalid_input(format!(
                "Scenario override for node '{node_id}' is incompatible with {expected:?}: \
                 got {supplied:?}"
            )));
        }
        (None, _) => {
            return Err(Error::invalid_input(format!(
                "Scenario override for node '{node_id}' requires a declared or inferred \
                 value_type"
            )));
        }
    }
    Ok(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_overrides_respects_parent_chain() {
        let mut scenarios = IndexMap::new();

        scenarios.insert(
            "base".to_string(),
            ScenarioDefinition {
                parent: None,
                period_overrides: IndexMap::new(),
                overrides: IndexMap::new(),
            },
        );

        let mut downside_overrides = IndexMap::new();
        downside_overrides.insert("revenue".to_string(), AmountOrScalar::scalar(90_000.0));
        scenarios.insert(
            "downside".to_string(),
            ScenarioDefinition {
                parent: Some("base".to_string()),
                period_overrides: IndexMap::new(),
                overrides: downside_overrides,
            },
        );

        let mut stress_overrides = IndexMap::new();
        stress_overrides.insert("revenue".to_string(), AmountOrScalar::scalar(80_000.0));
        scenarios.insert(
            "stress".to_string(),
            ScenarioDefinition {
                parent: Some("downside".to_string()),
                period_overrides: IndexMap::new(),
                overrides: stress_overrides,
            },
        );

        let set = ScenarioSet { scenarios };

        let base = set
            .resolve_overrides("base")
            .expect("base overrides should resolve");
        assert!(base.model_wide.is_empty());

        let downside = set
            .resolve_overrides("downside")
            .expect("downside overrides should resolve");
        assert_eq!(
            downside.model_wide.get("revenue"),
            Some(&AmountOrScalar::scalar(90_000.0))
        );

        let stress = set
            .resolve_overrides("stress")
            .expect("stress overrides should resolve");
        assert_eq!(
            stress.model_wide.get("revenue"),
            Some(&AmountOrScalar::scalar(80_000.0))
        );
    }

    #[test]
    fn removed_model_id_is_rejected_as_an_unknown_field() {
        let json = r#"{"model_id":"legacy","overrides":{}}"#;
        let error = serde_json::from_str::<ScenarioDefinition>(json)
            .expect_err("removed model_id must not be accepted");

        assert!(error.to_string().contains("unknown field `model_id`"));
    }
}
