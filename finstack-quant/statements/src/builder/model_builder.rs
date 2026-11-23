//! Mutable builder for financial models.

use super::validation::validate_node_id;
use crate::error::{Error, Result};
use crate::types::{AmountOrScalar, FinancialModelSpec, NodeId, NodeSpec, NodeType};
use finstack_quant_core::dates::{Period, PeriodId, PeriodPlan, build_periods};
use indexmap::{IndexMap, IndexSet};

/// Builder for financial models.
///
/// Configuration methods borrow this builder mutably. Rejected setters leave
/// the accumulated model unchanged. `build` consumes the completed builder,
/// and `mixed` transfers ownership to a mixed-node builder.
///
/// 1. Start with `ModelBuilder::new()`.
/// 2. Set the timeline once with `.periods()` or `.periods_explicit()`.
/// 3. Add nodes, forecasts, capital structure, etc. (in any order).
/// 4. Call `.build()` → `FinancialModelSpec`, which rejects a model without
///    periods along with every other semantic error.
///
/// # Example
///
/// ```rust
/// use finstack_quant_statements::builder::ModelBuilder;
/// use finstack_quant_statements::types::{AmountOrScalar, NodeType};
/// use finstack_quant_core::dates::PeriodId;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let model = {
///     let mut model_builder = ModelBuilder::new("test_model");
///     model_builder.periods("2025Q1..Q4", None)?;
///     model_builder.value("revenue", &[
///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100.0)),
///     ]);
///     model_builder.build()?
/// };
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct ModelBuilder {
    pub(super) id: String,
    pub(crate) periods: Option<PeriodPlan>,
    pub(crate) nodes: IndexMap<NodeId, NodeSpec>,
    pub(super) meta: IndexMap<String, serde_json::Value>,
    pub(crate) capital_structure: Option<crate::types::CapitalStructureSpec>,
    pub(super) source_snapshot: Option<String>,
    /// First duplicate definition or observation seen by an infallible
    /// setter; [`ModelBuilder::build`] returns it.
    pub(super) rejected: Option<Error>,
}

impl ModelBuilder {
    /// Insert a pre-built node into the model.
    ///
    /// This is an advanced API for template builders that need to construct
    /// nodes programmatically. Prefer `.compute()` and `.value()` for
    /// standard model construction.
    ///
    /// A node with the same id is a duplicate definition:
    /// [`build`](Self::build) rejects it. Use
    /// [`replace_node`](Self::replace_node) to change an existing node.
    ///
    /// # Arguments
    ///
    /// * `spec` - Node to insert under its own `node_id`.
    pub fn insert_node(&mut self, spec: NodeSpec) -> &mut Self {
        self.define_node(spec.node_id.clone(), spec);
        self
    }

    /// Replace an existing node with a complete new definition.
    ///
    /// This is the named edit for changing a node already in the builder;
    /// the defining setters reject a repeated id instead.
    ///
    /// # Arguments
    ///
    /// * `spec` - Complete replacement stored under its own `node_id`, in the
    ///   original node's position.
    ///
    /// # Errors
    ///
    /// Returns a build error if no node with that id exists. The builder is
    /// unchanged.
    pub fn replace_node(&mut self, spec: NodeSpec) -> Result<&mut Self> {
        let Some(node) = self.nodes.get_mut(&spec.node_id) else {
            return Err(Error::build(format!(
                "Cannot replace unknown node '{}'",
                spec.node_id
            )));
        };
        *node = spec;
        Ok(self)
    }

    /// Remove an existing node.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Identifier of the node to remove. Formulas that still
    ///   reference it fail [`build`](Self::build).
    ///
    /// # Errors
    ///
    /// Returns a build error if no node with that id exists.
    pub fn remove_node(&mut self, node_id: &str) -> Result<&mut Self> {
        if self.nodes.shift_remove(node_id).is_none() {
            return Err(Error::build(format!(
                "Cannot remove unknown node '{node_id}'"
            )));
        }
        Ok(self)
    }

    /// Store a new node, or record a duplicate definition for `build`.
    pub(super) fn define_node(&mut self, id: NodeId, spec: NodeSpec) {
        if self.nodes.contains_key(&id) {
            self.reject(Error::build(format!(
                "Node '{id}' is defined more than once; change an existing node with \
                 replace_node or a model edit"
            )));
        } else {
            self.nodes.insert(id, spec);
        }
    }

    /// Keep the first rejection for [`ModelBuilder::build`] to return.
    pub(super) fn reject(&mut self, error: Error) {
        self.rejected.get_or_insert(error);
    }

    /// Collect one node's observations, recording a repeated period.
    pub(super) fn unique_observations(
        &mut self,
        node_id: &NodeId,
        values: impl Iterator<Item = (PeriodId, AmountOrScalar)>,
    ) -> IndexMap<PeriodId, AmountOrScalar> {
        let mut unique = IndexMap::new();
        for (period_id, value) in values {
            if unique.insert(period_id, value).is_some() {
                self.reject(Error::build(format!(
                    "Node '{node_id}' has more than one observation for period {period_id}"
                )));
            }
        }
        unique
    }

    /// Return a read-only slice of the model's periods.
    ///
    /// This is an advanced API primarily for template builders in external crates
    /// that need to iterate over periods to generate per-period value nodes.
    pub fn periods_slice(&self) -> &[Period] {
        self.periods.as_deref().unwrap_or(&[])
    }

    /// Import period-labelled observations as new value nodes.
    ///
    /// The model timeline must already be set: an import never infers a
    /// plan. Columns map to periods by label, in any order. A `None` cell is
    /// a missing observation and is left out of the series; a zero is kept as
    /// a known zero. The import is atomic: any rejection leaves the builder
    /// unchanged.
    ///
    /// # Arguments
    ///
    /// * `columns` - Period labels (for example `"2025Q1"`), one per column.
    ///   Labels must be unique and belong to the model timeline, including a
    ///   column whose cells are all missing. Order is free.
    /// * `rows` - One `(node id, cells)` pair per new value node. `cells`
    ///   holds one entry per column, in column order. A node's observations
    ///   must be finite and share one unit: all scalar, or `Money` in one
    ///   currency.
    ///
    /// # Errors
    ///
    /// Returns a period error if the timeline is not set, `columns` is empty,
    /// or a label is invalid, repeated or outside the timeline. Returns a
    /// build error for an invalid or repeated node id, an id the builder
    /// already defines, a row whose length differs from the column count, a
    /// non-finite observation, or mixed units within a row.
    pub fn import_observations(
        &mut self,
        columns: &[&str],
        rows: &[(NodeId, Vec<Option<AmountOrScalar>>)],
    ) -> Result<&mut Self> {
        let Some(plan) = &self.periods else {
            return Err(Error::period(
                "Set the model timeline before importing observations",
            ));
        };
        if columns.is_empty() {
            return Err(Error::period(
                "Observation import requires at least one period column",
            ));
        }
        let plan_ids: IndexSet<_> = plan.iter().map(|period| period.id).collect();
        let mut column_ids = IndexSet::new();
        for label in columns {
            let period_id: PeriodId = label
                .parse()
                .map_err(|error| Error::period(format!("Period column '{label}': {error}")))?;
            if !plan_ids.contains(&period_id) {
                return Err(Error::period(format!(
                    "Period column {period_id} is not present in the model timeline"
                )));
            }
            if !column_ids.insert(period_id) {
                return Err(Error::period(format!(
                    "Period column {period_id} appears more than once"
                )));
            }
        }

        let mut imported: IndexMap<NodeId, NodeSpec> = IndexMap::new();
        for (node_id, cells) in rows {
            validate_node_id(node_id.as_str())?;
            if self.nodes.contains_key(node_id) || imported.contains_key(node_id) {
                return Err(Error::build(format!(
                    "Node '{node_id}' is defined more than once; change an existing node with \
                     replace_node or a model edit"
                )));
            }
            if cells.len() != column_ids.len() {
                return Err(Error::build(format!(
                    "Import row '{node_id}' has {} cells for {} period columns",
                    cells.len(),
                    column_ids.len()
                )));
            }
            let mut values = IndexMap::new();
            for (period_id, cell) in column_ids.iter().zip(cells) {
                let Some(value) = cell else { continue };
                if !value.value().is_finite() {
                    return Err(Error::build(format!(
                        "Import cell ('{node_id}', {period_id}) is not finite; leave a \
                         missing cell empty"
                    )));
                }
                values.insert(*period_id, *value);
            }
            let mut node = NodeSpec::new(node_id.clone(), NodeType::Value).with_values(values);
            node.value_type = crate::types::validated_explicit_value_type(&node)?;
            imported.insert(node_id.clone(), node);
        }
        self.nodes.extend(imported);
        Ok(self)
    }

    /// Model identifier this builder will stamp on the built specification.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Nodes accumulated so far, in declaration order.
    ///
    /// Read-only view for introspection (for example a host `repr`); mutate
    /// through the builder methods so their validation still applies.
    pub fn nodes(&self) -> &IndexMap<NodeId, NodeSpec> {
        &self.nodes
    }
}

impl ModelBuilder {
    /// Create a new model builder.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the model
    ///
    /// Set the timeline with `.periods()` before calling `.build()`.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            periods: None,
            nodes: IndexMap::new(),
            meta: IndexMap::new(),
            capital_structure: None,
            source_snapshot: None,
            rejected: None,
        }
    }

    /// Define periods using a range expression.
    ///
    /// # Arguments
    ///
    /// * `range` - Period range (e.g., "2025Q1..Q4", "2025Q1..2026Q2")
    /// * `actuals_until` - Optional cutoff for actuals (e.g., Some("2025Q2"))
    ///
    /// The range DSL creates the model timeline and classification of actual
    /// versus forecast periods. `actuals_until` must use the same period family
    /// as `range`; it labels periods through that cutoff as actuals without
    /// supplying any values for them.
    ///
    /// # Errors
    ///
    /// Returns a period error if the range or actuals cutoff cannot be parsed,
    /// uses incompatible period kinds, is reversed, or resolves to no periods.
    /// Periods can be set once; a second call is rejected.
    pub fn periods(&mut self, range: &str, actuals_until: Option<&str>) -> Result<&mut Self> {
        self.ensure_periods_unset()?;
        let period_plan = build_periods(range, actuals_until)?;
        if period_plan.is_empty() {
            return Err(Error::period(
                "Period range must contain at least one period",
            ));
        }
        self.periods = Some(period_plan);
        Ok(self)
    }

    /// Reject a second timeline: node values are keyed to the first one.
    fn ensure_periods_unset(&self) -> Result<()> {
        if self.periods.is_none() {
            Ok(())
        } else {
            Err(Error::build("Periods already set"))
        }
    }

    /// Define the timeline from an explicit period plan (for advanced use
    /// cases such as fiscal plans or plans whose actual/forecast split cannot
    /// be written with the range DSL).
    ///
    /// # Arguments
    /// * `plan` - [`PeriodPlan`] with its calendar context, typically from
    ///   `finstack_quant_core::dates::build_periods` or `build_fiscal_periods`;
    ///   the model retains the plan's calendar (record CORE-CONVENTIONS-014 #4).
    ///
    /// # Errors
    ///
    /// Returns a period error when the plan has no periods. The timeline is
    /// validated as a whole by [`Self::build`].
    pub fn periods_explicit(&mut self, plan: PeriodPlan) -> Result<&mut Self> {
        self.ensure_periods_unset()?;
        if plan.is_empty() {
            return Err(Error::period(
                "Period list must contain at least one period",
            ));
        }
        self.periods = Some(plan);
        Ok(self)
    }
}

impl ModelBuilder {
    /// Reconstruct a ready builder from a complete model specification.
    ///
    /// This is the canonical path for applying builder-based transformations to
    /// an existing model. It preserves period order and actual/forecast flags,
    /// node definitions, metadata, and capital-structure configuration. Calling
    /// [`Self::build`] after reconstruction revalidates the complete model.
    ///
    /// # Arguments
    ///
    /// * `spec` - Existing model specification whose owned state will seed the
    ///   reconstructed builder.
    ///
    /// # Returns
    ///
    /// A ready builder that can accept additional nodes or transformations.
    ///
    /// # Errors
    ///
    /// Returns a period error when `spec` has no periods. Other semantic model
    /// invariants are checked by [`Self::build`] after transformations finish.
    pub fn from_spec(spec: FinancialModelSpec) -> Result<Self> {
        if spec.periods.is_empty() {
            return Err(Error::period(
                "Period list must contain at least one period",
            ));
        }
        Ok(Self {
            id: spec.id,
            periods: Some(spec.periods),
            nodes: spec.nodes,
            meta: spec.meta,
            capital_structure: spec.capital_structure,
            source_snapshot: spec.source_snapshot,
            rejected: None,
        })
    }

    /// Name the source snapshot the model's observations were taken from.
    ///
    /// An evaluation with an observation cutoff needs this identity. Keep an
    /// original filing and its later revision as two models with different
    /// snapshot identities.
    ///
    /// # Arguments
    /// * `snapshot_id` - Nonempty identifier of the source snapshot, for
    ///   example `"FY2025-10K-original"`.
    pub fn source_snapshot(&mut self, snapshot_id: impl Into<String>) -> &mut Self {
        self.source_snapshot = Some(snapshot_id.into());
        self
    }

    /// Add metadata to the model.
    ///
    /// # Arguments
    /// * `key` - Metadata key; `currency` selects the model's reporting currency.
    /// * `value` - JSON payload stored under the key. For `currency`, supply a
    ///   supported three-letter currency-code string such as `"USD"`; malformed
    ///   currency metadata is rejected by [`build`](Self::build).
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = {
    ///     let mut model_builder = ModelBuilder::new("demo");
    ///     model_builder.periods("2025Q1..Q2", None)?;
    ///     model_builder.value_scalar("headcount", &[]);
    ///     model_builder.with_meta("currency", serde_json::json!("USD"));
    ///     model_builder.build()?
    /// };
    /// assert_eq!(model.meta["currency"], "USD");
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_meta(&mut self, key: impl Into<String>, value: serde_json::Value) -> &mut Self {
        self.meta.insert(key.into(), value);
        self
    }

    /// Set the authored currency of the statement model.
    ///
    /// # Arguments
    ///
    /// * `currency` - Currency used by statement analytics when interpreting
    ///   model amounts. Separate from the capital structure reporting currency.
    pub fn currency(&mut self, currency: finstack_quant_core::currency::Currency) -> &mut Self {
        self.with_meta("currency", serde_json::Value::String(currency.to_string()))
    }

    /// Build and validate the final financial-model specification.
    ///
    /// The returned [`FinancialModelSpec`] is the immutable input to the
    /// evaluator. Formula and `where` references retain the exact node IDs
    /// supplied by the caller; an unknown identifier is an error, never
    /// rewritten.
    ///
    /// # Errors
    ///
    /// Returns the [`FinancialModelSpec::validate_semantics`] error: a
    /// non-canonical period plan, a model with no node and no capital-structure
    /// instrument, an invalid node definition or observation, a reference to an
    /// unknown node, or a same-period dependency cycle. Builder calls may
    /// accept an incomplete draft, so this is the whole-model validation
    /// boundary.
    pub fn build(self) -> Result<FinancialModelSpec> {
        let _span = tracing::info_span!(
            "statements.build",
            model_id = self.id.as_str(),
            periods = self.periods_slice().len(),
            nodes = self.nodes.len(),
        )
        .entered();

        if let Some(rejected) = self.rejected {
            return Err(rejected);
        }
        let plan = self
            .periods
            .ok_or_else(|| Error::period("Periods must be set before building the model"))?;
        let mut spec = FinancialModelSpec::new(self.id, plan);
        spec.nodes = self.nodes;
        spec.meta = self.meta;
        spec.capital_structure = self.capital_structure;
        spec.source_snapshot = self.source_snapshot;
        spec.validate_semantics()?;

        Ok(spec)
    }
}
