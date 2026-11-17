//! Mutable builder for financial models.

use crate::error::{Error, Result};
use crate::registry::dynamic::StoredMetric;
use crate::types::{AmountOrScalar, FinancialModelSpec, NodeId, NodeSpec, NodeType};
use finstack_quant_core::dates::{Period, PeriodId, PeriodPlan, build_periods};
use indexmap::{IndexMap, IndexSet};

/// DSL keywords that must not be used as node IDs.
///
/// A node named `if`, `and`, `or`, `not`, `true`, or `false` would shadow the
/// language keyword: the parser treats these tokens specially (as control flow
/// or logical operators / boolean literals) and a bare identifier reference
/// cannot reach such a node. Reject at model build time instead of producing
/// confusing downstream parse errors.
///
/// Built-in function names (`lag`, `sum`, `median`, ...) are intentionally not
/// on this list: the DSL disambiguates call sites (`median(x)`) from bare
/// references (`median`), so a node named after a function is legal.
const RESERVED_DSL_IDENTIFIERS: &[&str] = &["if", "and", "or", "not", "true", "false"];

/// Validate that a node ID does not use reserved prefixes or shadow a DSL
/// keyword.
///
/// The `__cs__` prefix is reserved for internal capital structure references,
/// `__` prefix is reserved for other internal use, and identifiers that
/// collide with a DSL keyword (see `RESERVED_DSL_IDENTIFIERS`) would shadow the
/// language primitive. `period_id` is reserved for the timeline column in
/// wide result exports.
///
/// Shared by mutable model construction and complete-specification validation.
/// The same identifier rules apply at both boundaries.
///
/// # Arguments
///
/// * `node_id` - Candidate statement-node identifier to validate against
///   internal prefixes and reserved formula-language keywords.
///
/// # Errors
///
/// Returns an error if `node_id` is not a formula identifier (nonempty
/// dot-separated segments of letters, digits and underscores, starting with a
/// letter or underscore), uses a reserved prefix, is `period_id`, or collides
/// with a DSL keyword.
pub fn validate_node_id(node_id: &str) -> Result<()> {
    if !crate::dsl::signatures::is_valid_identifier(node_id) {
        return Err(Error::build(format!(
            "Node ID '{node_id}' is not a valid identifier: use nonempty dot-separated \
             segments of letters, digits and underscores, starting with a letter or underscore"
        )));
    }
    if node_id == "period_id" {
        return Err(Error::build(
            "Node ID 'period_id' is reserved for the timeline column in result exports",
        ));
    }
    if node_id.contains("__cs__") {
        return Err(Error::build(format!(
            "Node ID '{}' contains reserved prefix '__cs__'. \
             This prefix is used internally for capital structure references.",
            node_id
        )));
    }
    if node_id.starts_with("__") {
        return Err(Error::build(format!(
            "Node ID '{}' cannot start with '__' (reserved for internal use)",
            node_id
        )));
    }
    if RESERVED_DSL_IDENTIFIERS.contains(&node_id) {
        return Err(Error::build(format!(
            "Node ID '{}' collides with a reserved DSL keyword or built-in function; \
             rename the node to avoid shadowing formula primitives.",
            node_id
        )));
    }
    Ok(())
}

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
#[derive(Debug)]
pub struct ModelBuilder {
    id: String,
    pub(crate) periods: Option<PeriodPlan>,
    pub(crate) nodes: IndexMap<NodeId, NodeSpec>,
    meta: IndexMap<String, serde_json::Value>,
    pub(crate) capital_structure: Option<crate::types::CapitalStructureSpec>,
    source_snapshot: Option<String>,
    /// First duplicate definition or observation seen by an infallible
    /// setter; [`ModelBuilder::build`] returns it.
    rejected: Option<Error>,
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
    fn define_node(&mut self, id: NodeId, spec: NodeSpec) {
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
    fn reject(&mut self, error: Error) {
        self.rejected.get_or_insert(error);
    }

    /// Collect one node's observations, recording a repeated period.
    fn unique_observations(
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
        if period_plan.periods.is_empty() {
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
        if plan.periods.is_empty() {
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

    /// The model node a registered metric imports as: its pinned, qualified
    /// formula under its qualified identifier.
    fn metric_node(qualified_id: &str, stored_metric: &StoredMetric) -> NodeSpec {
        let mut node = NodeSpec::new(NodeId::new(qualified_id), NodeType::Calculated)
            .with_name(stored_metric.definition.name.clone())
            .with_formula(stored_metric.qualified_formula.clone());
        node.rollup = stored_metric.definition.rollup;
        node
    }

    /// Import registered metrics atomically.
    ///
    /// A node that already holds the metric's unchanged template is shared. A
    /// node with the same identifier and a different definition is a conflict:
    /// nothing is imported.
    fn import_metrics<'a>(
        &mut self,
        registry: &crate::registry::Registry,
        qualified_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<()> {
        let mut pending = Vec::new();
        for qualified_id in qualified_ids {
            let node = Self::metric_node(qualified_id, registry.get(qualified_id)?);
            // Units resolved by an earlier build are not part of the template.
            let unchanged = |existing: &NodeSpec| {
                let mut template = node.clone();
                template.value_type = existing.value_type;
                *existing == template
            };
            match self.nodes.get(qualified_id) {
                Some(existing) if unchanged(existing) => {}
                Some(_) => {
                    return Err(Error::build(format!(
                        "Node '{qualified_id}' is already defined differently from the registry \
                         metric of that name; import the metric first and change it with \
                         replace_node, or rename the node"
                    )));
                }
                None => pending.push(node),
            }
        }
        for node in pending {
            self.nodes.insert(node.node_id.clone(), node);
        }
        Ok(())
    }

    /// Add a value node with explicit period values.
    ///
    /// Value nodes contain only explicit data (actuals or assumptions).
    ///
    /// # Arguments
    /// * `node_id` - Identifier for the node to create
    /// * `values` - Slice of `(PeriodId, AmountOrScalar)` tuples representing actual values
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_statements::types::AmountOrScalar;
    /// # use finstack_quant_core::dates::PeriodId;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = {
    ///     let mut model_builder = ModelBuilder::new("test");
    ///     model_builder.periods("2025Q1..Q2", None)?;
    ///     model_builder.value("revenue", &[
    ///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100_000.0)),
    ///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), AmountOrScalar::scalar(110_000.0)),
    ///     ]);
    ///     model_builder.build()?
    /// };
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Duplicate periods in `values` follow [`IndexMap`] insertion semantics:
    /// the final entry for a period is retained. The values' monetary/scalar
    /// consistency is inferred and checked at [`build`](Self::build), so this
    /// fluent method can remain useful while a node is being assembled.
    /// Reusing an existing node identifier, or giving one period two
    /// observations, is a duplicate that [`build`](Self::build) rejects.
    pub fn value(
        &mut self,
        node_id: impl Into<NodeId>,
        values: &[(PeriodId, AmountOrScalar)],
    ) -> &mut Self {
        let node_id = node_id.into();
        let values_map = self.unique_observations(&node_id, values.iter().cloned());

        let node = NodeSpec::new(node_id.clone(), NodeType::Value).with_values(values_map);

        self.define_node(node_id, node);
        self
    }

    /// Add a monetary value node with Money values.
    ///
    /// This is a type-safe way to create value nodes that explicitly represent
    /// monetary amounts with currency. The node will be tracked as a Monetary type.
    ///
    /// # Arguments
    /// * `node_id` - Identifier for the node to create
    /// * `values` - Slice of `(PeriodId, Money)` tuples representing monetary values
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::dates::PeriodId;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = {
    ///     let mut model_builder = ModelBuilder::new("test");
    ///     model_builder.periods("2025Q1..Q2", None)?;
    ///     model_builder.value_money("revenue", &[
    ///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), Money::from((100_000_i64, Currency::USD))),
    ///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), Money::from((110_000_i64, Currency::USD))),
    ///     ]);
    ///     model_builder.build()?
    /// };
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The first value establishes the node's monetary currency when every
    /// supplied value shares it. If values mix currencies, the type is left
    /// unresolved so [`build`](Self::build) can reject the invalid series with
    /// a precise diagnostic. Duplicate periods retain their final entry.
    /// Reusing `node_id` replaces the former definition and emits a warning.
    pub fn value_money(
        &mut self,
        node_id: impl Into<NodeId>,
        values: &[(PeriodId, finstack_quant_core::money::Money)],
    ) -> &mut Self {
        let node_id = node_id.into();
        let values_map = self.unique_observations(
            &node_id,
            values
                .iter()
                .map(|(period_id, money)| (*period_id, AmountOrScalar::Amount(*money))),
        );

        let value_type = values.first().and_then(|(_, first_money)| {
            let first_currency = first_money.currency();
            values
                .iter()
                .all(|(_, money)| money.currency() == first_currency)
                .then_some(crate::types::NodeValueType::Monetary {
                    currency: first_currency,
                })
        });

        let mut node = NodeSpec::new(node_id.clone(), NodeType::Value).with_values(values_map);
        node.value_type = value_type;

        self.define_node(node_id, node);
        self
    }

    /// Add a scalar value node.
    ///
    /// This is a convenience method for creating value nodes that represent
    /// non-monetary scalars (ratios, percentages, counts, etc.).
    ///
    /// # Arguments
    /// * `node_id` - Identifier for the node to create
    /// * `values` - Slice of `(PeriodId, f64)` tuples representing scalar values
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_core::dates::PeriodId;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = {
    ///     let mut model_builder = ModelBuilder::new("test");
    ///     model_builder.periods("2025Q1..Q2", None)?;
    ///     model_builder.value_scalar("gross_margin_pct", &[
    ///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), 0.35),
    ///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), 0.37),
    ///     ]);
    ///     model_builder.build()?
    /// };
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Duplicate periods retain their final entry. Reusing `node_id` replaces
    /// the former definition and emits a warning; scalar consistency is already
    /// explicit in this typed convenience method.
    pub fn value_scalar(
        &mut self,
        node_id: impl Into<NodeId>,
        values: &[(PeriodId, f64)],
    ) -> &mut Self {
        let node_id = node_id.into();
        let values_map = self.unique_observations(
            &node_id,
            values
                .iter()
                .map(|(period_id, value)| (*period_id, AmountOrScalar::Scalar(*value))),
        );

        let mut node = NodeSpec::new(node_id.clone(), NodeType::Value).with_values(values_map);
        node.value_type = Some(crate::types::NodeValueType::Scalar);

        self.define_node(node_id, node);
        self
    }

    /// Set point-in-time availability dates for an existing value or mixed node.
    ///
    /// Explicit observations without an entry use the reporting period's
    /// exclusive end date. Use this method for filing/release dates or
    /// operational observations whose availability differs from period end.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Existing node whose explicit observations are dated
    /// * `availability_dates` - Slice of `(period, available_on)` pairs
    ///
    /// # Errors
    ///
    /// Returns an error when `node_id` does not exist. Whole-model validation
    /// later rejects dates for periods that have no explicit observation.
    pub fn availability_dates(
        &mut self,
        node_id: &str,
        availability_dates: &[(PeriodId, finstack_quant_core::dates::Date)],
    ) -> Result<&mut Self> {
        let mut unique = IndexMap::new();
        for (period_id, date) in availability_dates {
            if unique.insert(*period_id, *date).is_some() {
                return Err(Error::build(format!(
                    "Node '{node_id}' has more than one availability date for period {period_id}"
                )));
            }
        }
        let node = self.nodes.get_mut(node_id).ok_or_else(|| {
            Error::build(format!(
                "Cannot set availability dates for unknown node '{node_id}'"
            ))
        })?;
        node.availability_dates = unique;
        Ok(self)
    }

    /// Declare how an existing line rolls up to a coarser reporting frequency.
    ///
    /// Call it after the method that creates the line, whichever one that is
    /// (`value`, `value_money`, `value_scalar`, `compute`, `forecast`,
    /// `add_metric_from_registry`). Redefining `node_id` afterwards discards
    /// the declaration along with the rest of the old definition.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Existing node to declare the method on
    /// * `rollup` - `Sum` for flows, `PeriodEnd` for balances and trailing
    ///   metrics, `Formula` for ratios. A rollup rejects `Formula` on a node
    ///   without a formula. A [`RollupSpec`](crate::evaluator::RollupSpec)
    ///   entry for the node overrides this declaration.
    ///
    /// # Errors
    ///
    /// Returns an error when `node_id` does not exist.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_statements::types::{AmountOrScalar, RollupMethod};
    /// # use finstack_quant_core::dates::PeriodId;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut model_builder = ModelBuilder::new("demo");
    /// model_builder.periods("2025Q1..Q1", None)?;
    /// model_builder.value("cash", &[(PeriodId::quarter(2025, 1)?, AmountOrScalar::scalar(40.0))]);
    /// model_builder.rollup("cash", RollupMethod::PeriodEnd)?;
    /// let model = model_builder.build()?;
    /// assert_eq!(model.get_node("cash").and_then(|node| node.rollup), Some(RollupMethod::PeriodEnd));
    /// # Ok(())
    /// # }
    /// ```
    pub fn rollup(
        &mut self,
        node_id: &str,
        rollup: crate::types::RollupMethod,
    ) -> Result<&mut Self> {
        let node = self.nodes.get_mut(node_id).ok_or_else(|| {
            Error::build(format!(
                "Cannot declare a rollup method for unknown node '{node_id}'"
            ))
        })?;
        node.rollup = Some(rollup);
        Ok(self)
    }

    /// Add a calculated node with a formula.
    ///
    /// Calculated nodes derive their values from formulas only.
    ///
    /// # Arguments
    /// * `node_id` - Identifier for the node to create
    /// * `formula` - Statements DSL expression (e.g., `"revenue - cogs"`)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = {
    ///     let mut model_builder = ModelBuilder::new("test");
    ///     model_builder.periods("2025Q1..Q2", None)?;
    ///     model_builder.value_scalar("revenue", &[]);
    ///     model_builder.value_scalar("cogs", &[]);
    ///     model_builder.compute("gross_profit", "revenue - cogs")?;
    ///     model_builder.build()?
    /// };
    /// # let _ = model;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Formula syntax is checked immediately, while cross-node references,
    /// period coverage, and type compatibility are checked when the completed
    /// model is built. Reusing `node_id` intentionally replaces the previous
    /// node definition and emits a warning because prior values, forecasts, and
    /// formulas are discarded.
    ///
    /// # Errors
    ///
    /// Returns an error if `node_id` uses an internal prefix or DSL keyword,
    /// `formula` is blank, or the formula cannot be parsed and compiled.
    pub fn compute(
        &mut self,
        node_id: impl Into<NodeId>,
        formula: impl Into<String>,
    ) -> Result<&mut Self> {
        let node_id = node_id.into();
        let formula = formula.into();

        validate_node_id(node_id.as_str())?;

        if formula.trim().is_empty() {
            return Err(Error::formula_parse("Formula cannot be empty"));
        }

        crate::dsl::parse_and_compile(&formula)?;

        let node = NodeSpec::new(node_id.clone(), NodeType::Calculated).with_formula(formula);

        if self.nodes.contains_key(&node_id) {
            return Err(Error::build(format!(
                "Node '{node_id}' is defined more than once; change an existing node with \
                 replace_node or a model edit"
            )));
        }
        self.nodes.insert(node_id, node);
        Ok(self)
    }

    /// Create a mixed node with explicit configuration.
    ///
    /// Mixed nodes support Value, Forecast, and Formula with precedence: Value > Forecast > Formula.
    /// This method returns a fluent builder for configuring all aspects of a mixed node.
    ///
    /// # Arguments
    /// * `node_id` - Identifier for the mixed node being configured
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_statements::types::{AmountOrScalar, ForecastSpec, ForecastMethod};
    /// # use finstack_quant_core::dates::PeriodId;
    /// # use indexmap::indexmap;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = {
    ///     let mut model_builder = ModelBuilder::new("test");
    ///     model_builder.periods("2025Q1..Q4", Some("2025Q2"))?;
    ///     let mut model_builder = model_builder.mixed("revenue");
    ///     model_builder.values(&[
    ///             (PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100_000.0)),
    ///             (PeriodId::quarter(2025, 2).expect("valid period fixture"), AmountOrScalar::scalar(110_000.0)),
    ///         ]);
    ///     model_builder.forecast(ForecastSpec {
    ///             method: ForecastMethod::GrowthPct,
    ///             params: indexmap! { "rate".into() => serde_json::json!(0.05) },
    ///         });
    ///     model_builder.formula("lag(revenue, 1) * 1.05")?;
    ///     let model_builder = model_builder.build()?;
    ///     model_builder.build()?
    /// };
    /// # Ok(())
    /// # }
    /// ```
    pub fn mixed(self, node_id: impl Into<NodeId>) -> MixedNodeBuilder {
        MixedNodeBuilder {
            parent: self,
            node_id: node_id.into(),
            values: None,
            forecast: None,
            formula: None,
            name: None,
            rollup: None,
        }
    }

    /// Add a forecast specification to an existing node.
    ///
    /// This allows forecasting values into future periods using various methods.
    ///
    /// # Arguments
    /// * `node_id` - Identifier of the node to augment (created previously)
    /// * `forecast_spec` - Forecast configuration created with [`ForecastSpec`](crate::types::ForecastSpec)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_statements::types::{AmountOrScalar, ForecastSpec, ForecastMethod};
    /// # use finstack_quant_core::dates::PeriodId;
    /// # use indexmap::indexmap;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = {
    ///     let mut model_builder = ModelBuilder::new("test");
    ///     model_builder.periods("2025Q1..Q4", Some("2025Q2"))?;
    ///     model_builder.value("revenue", &[
    ///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100_000.0)),
    ///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), AmountOrScalar::scalar(110_000.0)),
    ///     ]);
    ///     model_builder.forecast("revenue", ForecastSpec {
    ///         method: ForecastMethod::GrowthPct,
    ///         params: indexmap! { "rate".into() => serde_json::json!(0.05) },
    ///     });
    ///     model_builder.build()?
    /// };
    /// # Ok(())
    /// # }
    /// ```
    pub fn forecast(
        &mut self,
        node_id: impl Into<NodeId>,
        forecast_spec: crate::types::ForecastSpec,
    ) -> &mut Self {
        let node_id = node_id.into();

        if let Some(node) = self.nodes.get_mut(node_id.as_str()) {
            node.forecast = Some(forecast_spec);

            // A node carrying a forecast must be Mixed so precedence resolves
            // Value > Forecast > Formula. Upgrade both Value AND Calculated:
            // leaving a Calculated node with a forecast would let the forecast
            // override the formula in forecast periods while the node still
            // claims to be formula-only.
            if matches!(node.node_type, NodeType::Value | NodeType::Calculated) {
                node.node_type = NodeType::Mixed;
            }
        } else {
            let node = NodeSpec::new(node_id.clone(), NodeType::Mixed).with_forecast(forecast_spec);
            self.nodes.insert(node_id, node);
        }

        self
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

    /// Attach a where clause to a named node.
    ///
    /// The where clause is a scalar predicate evaluated before the node's
    /// value source. Where it is false for a period, the node's value,
    /// forecast and formula are not consulted.
    ///
    /// # Arguments
    /// * `node_id` - Existing node the predicate masks
    /// * `where_clause` - DSL expression evaluated as a scalar predicate
    ///
    /// # Errors
    ///
    /// Returns a formula error if the predicate does not parse and compile,
    /// or a build error if `node_id` is not defined. The builder is unchanged.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_statements::types::AmountOrScalar;
    /// # use finstack_quant_core::dates::PeriodId;
    /// # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    /// let mut model_builder = ModelBuilder::new("test");
    /// model_builder.periods("2025Q1..Q4", Some("2025Q2"))?;
    /// model_builder.value("revenue", &[(PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(1500000.0))]);
    /// model_builder.compute("bonus", "revenue * 0.01")?;
    /// // Only compute bonus if revenue > 1M
    /// model_builder.where_clause("bonus", "revenue > 1000000")?;
    /// let model = model_builder.build()?;
    /// # let _ = model;
    /// # Ok(())
    /// # }
    /// ```
    pub fn where_clause(
        &mut self,
        node_id: &str,
        where_clause: impl Into<String>,
    ) -> Result<&mut Self> {
        let where_clause = where_clause.into();
        crate::dsl::parse_and_compile(&where_clause)?;
        let node = self.nodes.get_mut(node_id).ok_or_else(|| {
            Error::build(format!(
                "Cannot attach a where clause to unknown node '{node_id}'"
            ))
        })?;
        node.where_text = Some(where_clause);
        Ok(self)
    }

    /// Load built-in metrics (fin.* namespace) and add them to the model.
    ///
    /// Convenience wrapper over
    /// [`Registry::with_builtins`](crate::registry::Registry::with_builtins).
    ///
    /// For selective loading, build a [`Registry`](crate::registry::Registry)
    /// yourself and call [`add_metric_from_registry`] for each metric.
    ///
    /// # Example
    ///
    /// ```
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # fn main() -> finstack_quant_statements::Result<()> {
    /// let model = {
    ///     let mut model_builder = ModelBuilder::new("test");
    ///     model_builder.periods("2025Q1..Q2", None)?;
    ///     model_builder.value_scalar("revenue", &[]);
    ///     model_builder.value_scalar("cogs", &[]);
    ///     model_builder.with_builtin_metrics()?;
    ///     model_builder
    /// };
    ///
    /// // The catalogue is staged; `build` needs every input it reads.
    /// assert!(model.nodes().contains_key("fin.gross_profit"));
    /// assert!(model.build().is_err());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The whole catalogue reads many input nodes. [`build`](Self::build)
    /// resolves every reference, so it succeeds only when the model defines
    /// all of them; add a subset with [`add_metric_from_registry`].
    ///
    /// [`add_metric_from_registry`]: ModelBuilder::add_metric_from_registry
    ///
    /// # Errors
    ///
    /// Returns a registry error if the built-in catalog cannot be loaded, and
    /// a build error if a node with a built-in metric's name is already
    /// defined differently. A failed import leaves the builder unchanged; a
    /// node holding the same unchanged template is shared.
    pub fn with_builtin_metrics(&mut self) -> Result<&mut Self> {
        let registry = crate::registry::Registry::with_builtins()?;
        self.import_metrics(&registry, registry.all_metrics().map(|(id, _)| id))?;
        Ok(self)
    }

    /// Add a specific metric from a registry.
    ///
    /// This allows selectively adding metrics from a registry instead of
    /// adding all of them.
    ///
    /// # Arguments
    /// * `qualified_id` - Fully qualified metric identifier to add
    /// * `registry` - Registry loaded by the caller (allows reuse across builders)
    ///
    /// # Example
    ///
    /// ```
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_statements::registry::Registry;
    /// # fn main() -> finstack_quant_statements::Result<()> {
    /// let mut registry = Registry::new();
    /// registry.load_builtins()?;
    ///
    /// let model = {
    ///     let mut model_builder = ModelBuilder::new("test");
    ///     model_builder.periods("2025Q1..Q2", None)?;
    ///     model_builder.value_scalar("revenue", &[]);
    ///     model_builder.value_scalar("cogs", &[]);
    ///     model_builder.add_metric_from_registry("fin.gross_profit", &registry)?;
    ///     model_builder.add_metric_from_registry("fin.gross_margin", &registry)?;
    ///     model_builder.build()?
    /// };
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The metric and its dependency closure are imported atomically, with
    /// dependencies first. Each node carries the formula pinned at
    /// registration, in which references to other metrics are qualified, so
    /// an unqualified dependency cannot resolve to a caller-defined node with
    /// the same short name. A node that already holds the same unchanged
    /// template is shared. A node of that name with another definition is a
    /// conflict: override a metric with [`replace_node`](Self::replace_node)
    /// after importing it.
    ///
    /// # Errors
    ///
    /// Returns a registry error if `qualified_id` is unknown to `registry`,
    /// and a build error if a node of the metric's or a dependency's name is
    /// already defined differently. A failed import leaves the builder
    /// unchanged.
    /// [`build`](Self::build) performs the final model-level validation.
    pub fn add_metric_from_registry(
        &mut self,
        qualified_id: &str,
        registry: &crate::registry::Registry,
    ) -> Result<&mut Self> {
        let closure = registry.get_dependencies(qualified_id)?;
        self.import_metrics(
            registry,
            closure
                .iter()
                .map(String::as_str)
                .chain(std::iter::once(qualified_id)),
        )?;
        Ok(self)
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

/// Fluent builder for mixed nodes.
///
/// This builder allows configuring all aspects of a mixed node (values, forecast, formula)
/// in a fluent manner before adding it to the model.
#[derive(Debug)]
pub struct MixedNodeBuilder {
    parent: ModelBuilder,
    node_id: NodeId,
    values: Option<IndexMap<PeriodId, AmountOrScalar>>,
    forecast: Option<crate::types::ForecastSpec>,
    formula: Option<String>,
    name: Option<String>,
    rollup: Option<crate::types::RollupMethod>,
}

impl MixedNodeBuilder {
    /// Set explicit values for the mixed node.
    ///
    /// # Arguments
    /// * `values` - Slice of `(PeriodId, AmountOrScalar)` tuples to seed actual periods
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_core::dates::PeriodId;
    /// # use finstack_quant_statements::types::AmountOrScalar;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let builder = {
    ///     let mut model_builder = ModelBuilder::new("demo");
    ///     model_builder.periods("2025Q1..Q2", None)?;
    ///     let mut model_builder = model_builder.mixed("revenue");
    ///     model_builder.values(&[(PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100.0))]);
    ///     model_builder.build()?
    /// };
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a formula parse or compilation error if `formula` is blank or
    /// is not valid Statements DSL. The formula is checked immediately, but
    /// reference resolution and type compatibility remain part of the parent
    /// model's final [`ModelBuilder::build`] validation.
    pub fn values(&mut self, values: &[(PeriodId, AmountOrScalar)]) -> &mut Self {
        self.values = Some(values.iter().cloned().collect());
        self
    }

    /// Set the forecast specification.
    ///
    /// # Arguments
    /// * `forecast_spec` - Forecast configuration created with [`ForecastSpec`](crate::types::ForecastSpec)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_statements::types::ForecastSpec;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let builder = {
    ///     let mut model_builder = ModelBuilder::new("demo");
    ///     model_builder.periods("2025Q1..Q2", None)?;
    ///     let mut model_builder = model_builder.mixed("revenue");
    ///     model_builder.forecast(ForecastSpec::forward_fill());
    ///     model_builder.build()?
    /// };
    /// # Ok(())
    /// # }
    /// ```
    pub fn forecast(&mut self, forecast_spec: crate::types::ForecastSpec) -> &mut Self {
        self.forecast = Some(forecast_spec);
        self
    }

    /// Set the fallback formula.
    ///
    /// # Arguments
    /// * `formula` - DSL expression evaluated when explicit values or forecasts are absent
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_core::dates::PeriodId;
    /// # use finstack_quant_statements::types::AmountOrScalar;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let builder = {
    ///     let mut model_builder = ModelBuilder::new("demo");
    ///     model_builder.periods("2025Q1..Q2", None)?;
    ///     let mut model_builder = model_builder.mixed("revenue");
    ///     model_builder.values(&[(PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100.0))]);
    ///     model_builder.formula("lag(revenue, 1)")?;
    ///     model_builder.build()?
    /// };
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The fallback is used only when neither an explicit value nor a forecast
    /// resolves for a period. It is syntax-checked immediately; references and
    /// dimensions are checked when the parent [`ModelBuilder`] is finalized.
    ///
    /// # Errors
    ///
    /// Returns a formula parse or compilation error if `formula` is blank or
    /// invalid Statements DSL. It does not yet verify that referenced nodes
    /// exist or that their units are compatible.
    pub fn formula(&mut self, formula: impl Into<String>) -> Result<&mut Self> {
        let formula = formula.into();

        if formula.trim().is_empty() {
            return Err(Error::formula_parse("Formula cannot be empty"));
        }
        crate::dsl::parse_and_compile(&formula)?;

        self.formula = Some(formula);
        Ok(self)
    }

    /// Identifier of the mixed node being configured.
    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    /// Set the human-readable name.
    ///
    /// # Arguments
    /// * `name` - Display label used in reports or exports
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let builder = {
    ///     let mut model_builder = ModelBuilder::new("demo");
    ///     model_builder.periods("2025Q1..Q2", None)?;
    ///     let mut model_builder = model_builder.mixed("revenue");
    ///     model_builder.name("Revenue (actual + forecast)");
    ///     model_builder.build()?
    /// };
    /// # Ok(())
    /// # }
    /// ```
    pub fn name(&mut self, name: impl Into<String>) -> &mut Self {
        self.name = Some(name.into());
        self
    }

    /// Declare how this line rolls up to a coarser reporting frequency.
    ///
    /// # Arguments
    /// * `rollup` - `Sum` for flows, `PeriodEnd` for balances and trailing
    ///   metrics, `Formula` for ratios. A rollup rejects `Formula` on a node
    ///   without a formula. A [`RollupSpec`](crate::evaluator::RollupSpec)
    ///   entry for the node overrides this declaration.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_statements::types::{AmountOrScalar, RollupMethod};
    /// # use finstack_quant_core::dates::PeriodId;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut model_builder = ModelBuilder::new("demo");
    /// model_builder.periods("2025Q1..Q2", None)?;
    /// let mut mixed = model_builder.mixed("cash");
    /// mixed.values(&[(PeriodId::quarter(2025, 1)?, AmountOrScalar::scalar(10.0))]);
    /// mixed.rollup(RollupMethod::PeriodEnd);
    /// let model = mixed.build()?.build()?;
    /// assert_eq!(model.get_node("cash").and_then(|node| node.rollup), Some(RollupMethod::PeriodEnd));
    /// # Ok(())
    /// # }
    /// ```
    pub fn rollup(&mut self, rollup: crate::types::RollupMethod) -> &mut Self {
        self.rollup = Some(rollup);
        self
    }

    /// Build the mixed node and return to the parent model builder.
    ///
    /// This validates the mixed node ID eagerly before attaching it to the
    /// parent builder so there is a single completion path for mixed nodes.
    /// Values, forecasts, and formulas are retained together; evaluation uses
    /// the mixed-node precedence documented on [`ModelBuilder::mixed`].
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_statements::types::ForecastSpec;
    /// # use finstack_quant_core::dates::PeriodId;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = {
    ///     let mut model_builder = ModelBuilder::new("demo");
    ///     model_builder.periods("2025Q1..Q2", None)?;
    ///     let mut model_builder = model_builder.mixed("revenue");
    ///     model_builder.values(&[(PeriodId::quarter(2025, 1).expect("valid period fixture"), 100.0.into())]);
    ///     model_builder.forecast(ForecastSpec::forward_fill());
    ///     model_builder.formula("lag(revenue, 1)")?;
    ///     let model_builder = model_builder.build()?;
    ///     model_builder.build()?
    /// };
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the mixed node identifier uses a reserved internal
    /// prefix or shadows a Statements DSL keyword. Formula syntax, forecast
    /// compatibility, period coverage, and cross-node references are validated
    /// later when the parent builder is finalized.
    pub fn build(mut self) -> Result<ModelBuilder> {
        validate_node_id(self.node_id.as_str())?;

        let mut node = NodeSpec::new(self.node_id.clone(), NodeType::Mixed);

        if let Some(name) = self.name {
            node.name = Some(name);
        }
        if let Some(values) = self.values {
            node.values = Some(values);
        }
        if let Some(forecast) = self.forecast {
            node.forecast = Some(forecast);
        }
        if let Some(formula) = self.formula {
            node.formula_text = Some(formula);
        }
        node.rollup = self.rollup;

        if self.parent.nodes.contains_key(&self.node_id) {
            return Err(Error::build(format!(
                "Node '{}' is defined more than once; change an existing node with \
                 replace_node or a model edit",
                self.node_id
            )));
        }
        self.parent.nodes.insert(self.node_id, node);
        Ok(self.parent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// STATEMENTS-CONTRACTS-006 #4: an import shares an unchanged template,
    /// rejects a conflicting definition without importing anything, and an
    /// intentional override is an explicit edit after the import.
    #[test]
    fn metric_imports_share_templates_and_reject_conflicts() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let registry = crate::registry::Registry::with_builtins().expect("catalogue");
        let base = || {
            let mut builder = ModelBuilder::new("imports");
            builder.periods("2025Q1..Q1", None).expect("valid periods");
            builder
        };

        // A node of the same name with another definition is a conflict, and
        // the import is atomic: its dependency is not added either.
        let mut conflicting = base();
        conflicting.value("fin.gross_margin", &[(period, AmountOrScalar::scalar(0.4))]);
        let error = conflicting
            .add_metric_from_registry("fin.gross_margin", &registry)
            .map(|_| ())
            .expect_err("conflict");
        assert!(
            error.to_string().contains("already defined differently"),
            "{error}"
        );
        assert!(!conflicting.nodes().contains_key("fin.gross_profit"));
        assert!(conflicting.with_builtin_metrics().map(|_| ()).is_err());
        assert_eq!(conflicting.nodes().len(), 1);

        // Two imports share the unchanged template of a common dependency.
        let mut shared = base();
        shared
            .add_metric_from_registry("fin.gross_margin", &registry)
            .expect("first import");
        let template = shared.nodes()["fin.gross_profit"].clone();
        shared
            .add_metric_from_registry("fin.gross_profit", &registry)
            .expect("second import shares the node");
        shared
            .with_builtin_metrics()
            .expect("catalogue shares both");
        assert_eq!(shared.nodes()["fin.gross_profit"], template);

        // An intentional override is a named edit after the import.
        let mut overridden = base();
        overridden
            .add_metric_from_registry("fin.gross_profit", &registry)
            .expect("import");
        overridden
            .replace_node(
                NodeSpec::new("fin.gross_profit", NodeType::Calculated)
                    .with_formula("revenue - cogs - freight"),
            )
            .expect("explicit override");
        assert_eq!(
            overridden.nodes()["fin.gross_profit"]
                .formula_text
                .as_deref(),
            Some("revenue - cogs - freight")
        );
    }

    /// STATEMENTS-CONTRACTS-002 #1: a repeated node id, a repeated period
    /// and a repeated availability date are duplicates, not replacements.
    #[test]
    fn duplicate_definitions_and_observations_are_rejected() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let base = || {
            let mut builder = ModelBuilder::new("duplicates");
            builder.periods("2025Q1..Q1", None).expect("valid periods");
            builder.value("revenue", &[(period, AmountOrScalar::scalar(100.0))]);
            builder
        };
        let rejected = |builder: ModelBuilder, expected: &str| {
            let error = builder.build().expect_err(expected);
            assert!(error.to_string().contains(expected), "{error}");
        };

        let mut builder = base();
        builder.value_scalar("revenue", &[]);
        rejected(builder, "defined more than once");
        let mut builder = base();
        builder.value_scalar("revenue", &[]);
        rejected(builder, "defined more than once");
        let mut builder = base();
        builder.value_money("revenue", &[]);
        rejected(builder, "defined more than once");
        let mut builder = base();
        builder.insert_node(NodeSpec::new("revenue", NodeType::Value));
        rejected(builder, "defined more than once");

        let mut builder = base();
        let error = builder
            .compute("revenue", "1")
            .expect_err("compute rejects a defined id");
        assert!(
            error.to_string().contains("defined more than once"),
            "{error}"
        );
        let error = base()
            .mixed("revenue")
            .build()
            .expect_err("a mixed node rejects a defined id");
        assert!(
            error.to_string().contains("defined more than once"),
            "{error}"
        );

        let mut builder = base();
        builder.value(
            "cost",
            &[
                (period, AmountOrScalar::scalar(1.0)),
                (period, AmountOrScalar::scalar(2.0)),
            ],
        );
        rejected(builder, "more than one observation for period 2025Q1");

        let date =
            finstack_quant_core::dates::Date::from_calendar_date(2025, time::Month::April, 25)
                .expect("date");
        let mut builder = base();
        let error = builder
            .availability_dates("revenue", &[(period, date), (period, date)])
            .expect_err("a repeated availability date is rejected");
        assert!(
            error
                .to_string()
                .contains("more than one availability date"),
            "{error}"
        );
        assert!(builder.nodes()["revenue"].availability_dates.is_empty());

        // The named edits change an existing node.
        let mut builder = base();
        builder
            .replace_node(
                NodeSpec::new("revenue", NodeType::Value)
                    .with_values(IndexMap::from([(period, AmountOrScalar::scalar(110.0))])),
            )
            .expect("replace an existing node");
        builder
            .replace_node(NodeSpec::new("absent", NodeType::Value))
            .expect_err("replace needs an existing node");
        builder
            .remove_node("absent")
            .expect_err("remove needs an existing node");
        let model = builder.build().expect("model builds");
        assert_eq!(
            model.nodes["revenue"].values.as_ref().expect("values")[&period].value(),
            110.0
        );
    }

    /// STATEMENTS-CONTRACTS-002 #1: a `where` clause needs a valid named
    /// target and a predicate that compiles.
    #[test]
    fn where_clause_needs_a_named_target() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let mut builder = ModelBuilder::new("where-target");
        builder.periods("2025Q1..Q1", None).expect("valid periods");
        builder.value("revenue", &[(period, AmountOrScalar::scalar(100.0))]);
        builder.compute("bonus", "revenue * 0.1").expect("formula");
        builder.compute("tax", "revenue * 0.2").expect("formula");
        builder
            .where_clause("bonus", "revenue > 1000")
            .expect("named target");
        builder
            .where_clause("absent", "revenue > 1000")
            .expect_err("an unknown target is rejected");
        builder
            .where_clause("tax", "revenue >")
            .expect_err("an invalid predicate is rejected");
        let model = builder.build().expect("model builds");

        let where_text = |id: &str| model.get_node(id).expect("node exists").where_text.clone();
        assert_eq!(where_text("bonus").as_deref(), Some("revenue > 1000"));
        assert_eq!(where_text("tax"), None);
    }

    #[test]
    fn test_formula_references_require_exact_node_ids() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let model = {
            let mut model_builder_1 = ModelBuilder::new("alias-test");
            model_builder_1
                .periods("2025Q1..Q1", None)
                .expect("valid periods");
            model_builder_1.value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))]);
            model_builder_1.value("cogs", &[(period, AmountOrScalar::scalar(40_000.0))]);
            model_builder_1
                .compute("gross_profit", "rev - cogs")
                .expect("valid formula");
            model_builder_1
        };

        let error = model
            .build()
            .expect_err("the alias must not be rewritten to revenue");
        let message = error.to_string();
        assert!(message.contains("Unknown identifier 'rev'"));
        assert!(message.contains("Did you mean one of: revenue"));
    }

    #[test]
    fn test_build_rejects_cross_currency_addition() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let result = {
            let mut model_builder_1 = ModelBuilder::new("dimension-test");
            model_builder_1
                .periods("2025Q1..Q1", None)
                .expect("valid periods");
            model_builder_1.value_money(
                "usd_revenue",
                &[(
                    period,
                    finstack_quant_core::money::Money::from((
                        100_i64,
                        finstack_quant_core::currency::Currency::USD,
                    )),
                )],
            );
            model_builder_1.value_money(
                "eur_cost",
                &[(
                    period,
                    finstack_quant_core::money::Money::from((
                        40_i64,
                        finstack_quant_core::currency::Currency::EUR,
                    )),
                )],
            );
            model_builder_1
                .compute("bad_total", "usd_revenue + eur_cost")
                .expect("formula syntax is valid");
            model_builder_1.build()
        };

        assert!(result.is_err());
        assert!(
            result
                .expect_err("cross-currency addition should fail")
                .to_string()
                .contains("Dimensional mismatch")
        );
    }
}
