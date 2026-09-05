//! Type-state builder pattern for financial models.

use crate::error::{Error, Result};
use crate::registry::dynamic::StoredMetric;
use crate::types::{AmountOrScalar, FinancialModelSpec, NodeId, NodeSpec, NodeType};
use finstack_quant_core::dates::{build_periods, Period, PeriodId};
use indexmap::{IndexMap, IndexSet};
use std::marker::PhantomData;

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
/// language primitive.
///
/// Exposed so binding layers can pre-validate a node id without consuming a
/// (move-based) builder — mirroring the check `compute` runs internally.
///
/// # Arguments
///
/// * `node_id` - Candidate statement-node identifier to validate against
///   internal prefixes and reserved formula-language keywords.
///
/// # Errors
///
/// Returns an error if `node_id` uses a reserved prefix or collides with a DSL
/// keyword.
pub fn validate_node_id(node_id: &str) -> Result<()> {
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

/// Type-state marker: Periods not yet defined
#[derive(Debug)]
pub struct NeedPeriods;

/// Type-state marker: Ready to add nodes
#[derive(Debug)]
pub struct Ready;

/// Builder for financial models with compile-time type-state enforcement.
///
/// The builder uses a type-state pattern to ensure correct usage:
/// 1. Start with [`FinancialModelSpec::builder`](crate::types::FinancialModelSpec::builder)
///    or `ModelBuilder::new()` → `ModelBuilder<NeedPeriods>`
/// 2. Call `.periods()` → `ModelBuilder<Ready>`
/// 3. Add nodes, forecasts, etc.
/// 4. Call `.build()` → `FinancialModelSpec`
///
/// # Example
///
/// ```rust
/// use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec, NodeType};
/// use finstack_quant_core::dates::PeriodId;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let model = FinancialModelSpec::builder("test_model")
///     .periods("2025Q1..Q4", None)?
///     .value("revenue", &[
///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100.0)),
///     ])
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct ModelBuilder<State> {
    id: String,
    pub(crate) periods: Vec<Period>,
    pub(crate) nodes: IndexMap<NodeId, NodeSpec>,
    meta: IndexMap<String, serde_json::Value>,
    pub(crate) capital_structure: Option<crate::types::CapitalStructureSpec>,
    _state: PhantomData<State>,
}

impl<State> ModelBuilder<State> {
    /// Insert a pre-built node into the model.
    ///
    /// This is an advanced API for template builders that need to construct
    /// nodes programmatically. Prefer `.compute()` and `.value()` for
    /// standard model construction.
    pub fn insert_node(&mut self, id: NodeId, spec: NodeSpec) -> &mut Self {
        self.nodes.insert(id, spec);
        self
    }

    /// Warn when a node-creating builder method would overwrite an existing
    /// node with a fresh definition (silently discarding the prior values /
    /// forecast / formula). The legitimate `value()` + `forecast()` mixed-node
    /// pattern goes through the node-*modifying* methods, which do not call
    /// this. `insert_node` is the explicit overwrite escape hatch.
    fn warn_if_redefining(&self, node_id: &NodeId, method: &str) {
        if self.nodes.contains_key(node_id) {
            tracing::warn!(
                node = node_id.as_str(),
                method,
                "node '{}' redefined by {}(); the previous definition is discarded. Use a \
                 distinct id, or build a mixed node via value() + forecast().",
                node_id.as_str(),
                method
            );
        }
    }

    /// Return a read-only slice of the model's periods.
    ///
    /// This is an advanced API primarily for template builders in external crates
    /// that need to iterate over periods to generate per-period value nodes.
    pub fn periods_slice(&self) -> &[Period] {
        &self.periods
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

impl ModelBuilder<NeedPeriods> {
    /// Create a new model builder.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the model
    ///
    /// You must call `.periods()` before adding nodes.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            periods: Vec::new(),
            nodes: IndexMap::new(),
            meta: IndexMap::new(),
            capital_structure: None,
            _state: PhantomData,
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
    /// The type-state transition succeeds only with a non-empty period plan.
    pub fn periods(self, range: &str, actuals_until: Option<&str>) -> Result<ModelBuilder<Ready>> {
        self.try_periods(range, actuals_until)
            .map_err(|(_, error)| error)
    }

    /// Define periods from a range expression, handing the builder back on
    /// failure.
    ///
    /// Identical to [`periods`](Self::periods) except that a rejected range
    /// returns the untouched `NeedPeriods` builder alongside the error, so a
    /// host wrapper that cannot express Rust's move semantics (Python, JS) can
    /// keep the instruments, metadata, and capital structure accumulated so
    /// far instead of forcing the caller to start over after a typo.
    ///
    /// # Arguments
    ///
    /// * `range` - Period range expression in the crate's period grammar
    ///   (`"2025Q1..Q4"`, `"2024M10..2025M03"`, `"2025..2030"`); start and end
    ///   must share one frequency family
    /// * `actuals_until` - Optional inclusive cutoff (same family as `range`)
    ///   through which periods are labelled actuals
    ///
    /// # Errors
    ///
    /// Returns `(self, error)` (the builder boxed, to keep the error variant
    /// small) when the range or cutoff cannot be parsed, the families are
    /// incompatible, the range is reversed, or it resolves to no periods.
    pub fn try_periods(
        self,
        range: &str,
        actuals_until: Option<&str>,
    ) -> std::result::Result<ModelBuilder<Ready>, (Box<Self>, Error)> {
        let period_plan = match build_periods(range, actuals_until) {
            Ok(plan) => plan,
            Err(error) => return Err((Box::new(self), error.into())),
        };

        if period_plan.periods.is_empty() {
            return Err((
                Box::new(self),
                Error::period("Period range must contain at least one period"),
            ));
        }

        Ok(ModelBuilder {
            id: self.id,
            periods: period_plan.periods,
            nodes: self.nodes,
            meta: self.meta,
            capital_structure: self.capital_structure,
            _state: PhantomData,
        })
    }

    /// Define periods explicitly (for advanced use cases).
    ///
    /// # Arguments
    /// * `periods` - Vector of [`Period`](finstack_quant_core::dates::Period) instances, typically
    ///   produced by `finstack_quant_core::dates::build_periods`
    ///
    /// The supplied sequence is retained in the given order. It is therefore
    /// appropriate for models whose actual and forecast periods cannot be
    /// expressed with the range DSL, but callers remain responsible for
    /// providing a coherent non-empty period plan.
    ///
    /// # Errors
    ///
    /// Returns a period error when `periods` is empty. No additional period
    /// ordering or continuity validation is performed at this boundary.
    pub fn periods_explicit(self, periods: Vec<Period>) -> Result<ModelBuilder<Ready>> {
        if periods.is_empty() {
            return Err(Error::period(
                "Period list must contain at least one period",
            ));
        }

        Ok(ModelBuilder {
            id: self.id,
            periods,
            nodes: self.nodes,
            meta: self.meta,
            capital_structure: self.capital_structure,
            _state: PhantomData,
        })
    }
}

impl ModelBuilder<Ready> {
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
            periods: spec.periods,
            nodes: spec.nodes,
            meta: spec.meta,
            capital_structure: spec.capital_structure,
            _state: PhantomData,
        })
    }

    fn insert_metric_node(
        &mut self,
        qualified_id: &str,
        stored_metric: &StoredMetric,
        formula: String,
    ) {
        let key = NodeId::new(qualified_id);
        let node = NodeSpec::new(key.clone(), NodeType::Calculated)
            .with_name(stored_metric.definition.name.clone())
            .with_formula(formula);
        self.nodes.insert(key, node);
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
    /// let model = ModelBuilder::new("test")
    ///     .periods("2025Q1..Q2", None)?
    ///     .value("revenue", &[
    ///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100_000.0)),
    ///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), AmountOrScalar::scalar(110_000.0)),
    ///     ])
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Duplicate periods in `values` follow [`IndexMap`] insertion semantics:
    /// the final entry for a period is retained. The values' monetary/scalar
    /// consistency is inferred and checked at [`build`](Self::build), so this
    /// fluent method can remain useful while a node is being assembled.
    /// Reusing an existing node identifier replaces its former definition and
    /// emits a warning because its values, forecast, and formula are discarded.
    #[must_use = "builder methods must be chained"]
    pub fn value(
        mut self,
        node_id: impl Into<NodeId>,
        values: &[(PeriodId, AmountOrScalar)],
    ) -> Self {
        let node_id = node_id.into();
        let values_map: IndexMap<PeriodId, AmountOrScalar> = values.iter().cloned().collect();

        let node = NodeSpec::new(node_id.clone(), NodeType::Value).with_values(values_map);

        self.warn_if_redefining(&node_id, "value");
        self.nodes.insert(node_id, node);
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
    /// let model = ModelBuilder::new("test")
    ///     .periods("2025Q1..Q2", None)?
    ///     .value_money("revenue", &[
    ///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), Money::from((100_000_i64, Currency::USD))),
    ///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), Money::from((110_000_i64, Currency::USD))),
    ///     ])
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The first value establishes the node's monetary currency when every
    /// supplied value shares it. If values mix currencies, the type is left
    /// unresolved so [`build`](Self::build) can reject the invalid series with
    /// a precise diagnostic. Duplicate periods retain their final entry.
    /// Reusing `node_id` replaces the former definition and emits a warning.
    #[must_use = "builder methods must be chained"]
    pub fn value_money(
        mut self,
        node_id: impl Into<NodeId>,
        values: &[(PeriodId, finstack_quant_core::money::Money)],
    ) -> Self {
        let node_id = node_id.into();
        let values_map: IndexMap<PeriodId, AmountOrScalar> = values
            .iter()
            .map(|(period_id, money)| (*period_id, AmountOrScalar::Amount(*money)))
            .collect();

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

        self.warn_if_redefining(&node_id, "value_money");
        self.nodes.insert(node_id, node);
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
    /// let model = ModelBuilder::new("test")
    ///     .periods("2025Q1..Q2", None)?
    ///     .value_scalar("gross_margin_pct", &[
    ///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), 0.35),
    ///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), 0.37),
    ///     ])
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Duplicate periods retain their final entry. Reusing `node_id` replaces
    /// the former definition and emits a warning; scalar consistency is already
    /// explicit in this typed convenience method.
    #[must_use = "builder methods must be chained"]
    pub fn value_scalar(mut self, node_id: impl Into<NodeId>, values: &[(PeriodId, f64)]) -> Self {
        let node_id = node_id.into();
        let values_map: IndexMap<PeriodId, AmountOrScalar> = values
            .iter()
            .map(|(period_id, value)| (*period_id, AmountOrScalar::Scalar(*value)))
            .collect();

        let mut node = NodeSpec::new(node_id.clone(), NodeType::Value).with_values(values_map);
        node.value_type = Some(crate::types::NodeValueType::Scalar);

        self.warn_if_redefining(&node_id, "value_scalar");
        self.nodes.insert(node_id, node);
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
    #[must_use = "builder methods must be chained"]
    pub fn availability_dates(
        mut self,
        node_id: &str,
        availability_dates: &[(PeriodId, finstack_quant_core::dates::Date)],
    ) -> Result<Self> {
        self.try_availability_dates(node_id, availability_dates)?;
        Ok(self)
    }

    /// Set availability dates in place, leaving the builder usable on error.
    ///
    /// The non-consuming twin of
    /// [`availability_dates`](Self::availability_dates) for host wrappers
    /// that hold the builder behind a mutable reference: an unknown node
    /// returns an error without discarding the accumulated model.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Existing value or mixed node whose explicit observations
    ///   are dated
    /// * `availability_dates` - `(period, available_on)` pairs; observations
    ///   without an entry default to the period's exclusive end date
    ///
    /// # Errors
    ///
    /// Returns a build error when `node_id` does not exist in the builder.
    pub fn try_availability_dates(
        &mut self,
        node_id: &str,
        availability_dates: &[(PeriodId, finstack_quant_core::dates::Date)],
    ) -> Result<&mut Self> {
        let node = self.nodes.get_mut(node_id).ok_or_else(|| {
            Error::build(format!(
                "Cannot set availability dates for unknown node '{node_id}'"
            ))
        })?;
        node.availability_dates = availability_dates.iter().copied().collect();
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
    /// let model = ModelBuilder::new("test")
    ///     .periods("2025Q1..Q2", None)?
    ///     .compute("gross_profit", "revenue - cogs")?
    ///     .build()?;
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
    #[must_use = "builder methods must be chained"]
    pub fn compute(
        mut self,
        node_id: impl Into<NodeId>,
        formula: impl Into<String>,
    ) -> Result<Self> {
        self.try_compute(node_id, formula)?;
        Ok(self)
    }

    /// Add a calculated node in place, leaving the builder usable on error.
    ///
    /// The non-consuming twin of [`compute`](Self::compute): the same
    /// node-id and formula validation runs *before* any state changes, so a
    /// host wrapper holding the builder behind a mutable reference keeps its
    /// accumulated nodes when a formula has a typo.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Identifier for the node to create; must not use a
    ///   reserved prefix or shadow a DSL keyword
    /// * `formula` - Statements DSL expression (e.g. `"revenue - cogs"`);
    ///   syntax-checked immediately, references checked at `build`
    ///
    /// # Errors
    ///
    /// Returns an error if `node_id` uses an internal prefix or DSL keyword,
    /// `formula` is blank, or the formula cannot be parsed and compiled.
    pub fn try_compute(
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

        self.warn_if_redefining(&node_id, "compute");
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
    /// let model = ModelBuilder::new("test")
    ///     .periods("2025Q1..Q4", Some("2025Q2"))?
    ///     .mixed("revenue")
    ///         .values(&[
    ///             (PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100_000.0)),
    ///             (PeriodId::quarter(2025, 2).expect("valid period fixture"), AmountOrScalar::scalar(110_000.0)),
    ///         ])
    ///         .forecast(ForecastSpec {
    ///             method: ForecastMethod::GrowthPct,
    ///             params: indexmap! { "rate".into() => serde_json::json!(0.05) },
    ///         })
    ///         .formula("lag(revenue, 1) * 1.05")?
    ///         .build()?
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "builder methods must be chained"]
    pub fn mixed(self, node_id: impl Into<NodeId>) -> MixedNodeBuilder {
        MixedNodeBuilder {
            parent: self,
            node_id: node_id.into(),
            values: None,
            forecast: None,
            formula: None,
            name: None,
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
    /// let model = ModelBuilder::new("test")
    ///     .periods("2025Q1..Q4", Some("2025Q2"))?
    ///     .value("revenue", &[
    ///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100_000.0)),
    ///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), AmountOrScalar::scalar(110_000.0)),
    ///     ])
    ///     .forecast("revenue", ForecastSpec {
    ///         method: ForecastMethod::GrowthPct,
    ///         params: indexmap! { "rate".into() => serde_json::json!(0.05) },
    ///     })
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "builder methods must be chained"]
    pub fn forecast(
        mut self,
        node_id: impl Into<NodeId>,
        forecast_spec: crate::types::ForecastSpec,
    ) -> Self {
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

    /// Add metadata to the model.
    ///
    /// # Arguments
    /// * `key` - Metadata key
    /// * `value` - Arbitrary JSON payload
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = ModelBuilder::new("demo")
    ///     .periods("2025Q1..Q2", None)?
    ///     .with_meta("currency", serde_json::json!({ "code": "USD" }))
    ///     .build()?;
    /// assert_eq!(model.meta["currency"]["code"], "USD");
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "builder methods must be chained"]
    pub fn with_meta(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.meta.insert(key.into(), value);
        self
    }

    /// Add a where clause to the last added node.
    ///
    /// The where clause is a conditional expression that determines whether
    /// the node should be evaluated for a given period. If the where clause
    /// evaluates to false (0.0), the node value will be set to 0.0 for that period.
    ///
    /// # Arguments
    /// * `where_clause` - DSL expression evaluated as a predicate
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::prelude::*;
    /// # use finstack_quant_core::dates::PeriodId;
    /// # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    /// let model = ModelBuilder::new("test")
    ///     .periods("2025Q1..Q4", Some("2025Q2"))?
    ///     .value("revenue", &[(PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(1500000.0))])
    ///     .compute("bonus", "revenue * 0.01")?
    ///     .where_clause("revenue > 1000000")  // Only compute bonus if revenue > 1M
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "builder methods must be chained"]
    pub fn where_clause(mut self, where_clause: impl Into<String>) -> Self {
        if let Some((_, last_node)) = self.nodes.last_mut() {
            last_node.where_text = Some(where_clause.into());
        }
        self
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
    /// let model = ModelBuilder::new("test")
    ///     .periods("2025Q1..Q2", None)?
    ///     .value("revenue", &[])
    ///     .value("cogs", &[])
    ///     .with_builtin_metrics()?
    ///     .build()?;
    ///
    /// // Now you can use metrics like fin.gross_profit
    /// assert!(model.has_node("fin.gross_profit"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`add_metric_from_registry`]: ModelBuilder::add_metric_from_registry
    ///
    /// # Errors
    ///
    /// Returns registry or formula errors if the built-in catalog cannot be
    /// loaded, a built-in metric has an unresolved dependency, or dependency
    /// formulas cannot be prepared for this builder. Existing nodes are not
    /// overwritten when their identifiers match a built-in metric or one of
    /// its dependencies.
    #[must_use = "builder methods must be chained"]
    pub fn with_builtin_metrics(mut self) -> Result<Self> {
        self.try_with_builtin_metrics()?;
        Ok(self)
    }

    /// Add the built-in metric catalog in place, leaving the builder usable
    /// on error.
    ///
    /// The non-consuming twin of
    /// [`with_builtin_metrics`](Self::with_builtin_metrics) for host wrappers.
    /// Nothing is inserted unless the whole catalog loads and resolves.
    ///
    /// # Errors
    ///
    /// Returns registry or formula errors if the built-in catalog cannot be
    /// loaded or a built-in metric has an unresolved dependency.
    pub fn try_with_builtin_metrics(&mut self) -> Result<&mut Self> {
        let registry = crate::registry::Registry::with_builtins()?;
        self.add_all_metrics_from_registry_in_place(&registry)?;
        Ok(self)
    }

    /// Add every metric from a loaded registry to the model, qualifying
    /// same-namespace references in the generated formulas.
    fn add_all_metrics_from_registry_in_place(
        &mut self,
        registry: &crate::registry::Registry,
    ) -> Result<()> {
        let mut namespace_cache: IndexMap<String, IndexSet<String>> = IndexMap::new();
        for (qualified_id, stored_metric) in registry.all_metrics() {
            let namespace = qualified_id.split('.').next().unwrap_or("");
            let formula = if namespace.is_empty() {
                stored_metric.definition.formula.clone()
            } else {
                let metrics_in_namespace = namespace_cache
                    .entry(namespace.to_string())
                    .or_insert_with(|| Self::metric_ids_in_namespace(registry, namespace));
                Self::qualify_metric_references_with_namespace_set(
                    &stored_metric.definition.formula,
                    namespace,
                    metrics_in_namespace,
                )
            };
            self.insert_metric_node(qualified_id, stored_metric, formula);
        }
        Ok(())
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
    /// let model = ModelBuilder::new("test")
    ///     .periods("2025Q1..Q2", None)?
    ///     .value("revenue", &[])
    ///     .value("cogs", &[])
    ///     .add_metric_from_registry("fin.gross_profit", &registry)?
    ///     .add_metric_from_registry("fin.gross_margin", &registry)?
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Dependencies are added before the requested metric. References to
    /// metrics in the same namespace are qualified in the generated formulas,
    /// preventing an unqualified dependency from accidentally resolving to a
    /// caller-defined node with the same short name. Nodes already present in
    /// the builder take precedence and are left unchanged.
    ///
    /// # Errors
    ///
    /// Returns a registry error if `qualified_id` is not in
    /// `namespace.metric_id` form, is unknown to `registry`, or any dependency
    /// is unavailable. It also propagates errors while obtaining the
    /// dependency order. The builder is consumed, so callers should retain
    /// validation at [`build`](Self::build) as the final model-level check.
    pub fn add_metric_from_registry(
        mut self,
        qualified_id: &str,
        registry: &crate::registry::Registry,
    ) -> Result<Self> {
        self.try_add_metric_from_registry(qualified_id, registry)?;
        Ok(self)
    }

    /// Add one registry metric in place, leaving the builder usable on error.
    ///
    /// The non-consuming twin of
    /// [`add_metric_from_registry`](Self::add_metric_from_registry): the
    /// metric and its dependency order are resolved before any node is
    /// inserted, so an unknown `qualified_id` (a routine typo) leaves the
    /// builder exactly as it was.
    ///
    /// # Arguments
    ///
    /// * `qualified_id` - Fully qualified metric identifier in
    ///   `namespace.metric_id` form (e.g. `"fin.gross_margin"`)
    /// * `registry` - Registry the metric and its dependencies are read from
    ///
    /// # Errors
    ///
    /// Returns a registry-not-found error if `qualified_id` is unknown to
    /// `registry`, or a registry error if it is malformed or a dependency
    /// cannot be resolved.
    pub fn try_add_metric_from_registry(
        &mut self,
        qualified_id: &str,
        registry: &crate::registry::Registry,
    ) -> Result<&mut Self> {
        let dependencies = registry.get_metric_dependencies(qualified_id)?;

        let namespace = qualified_id
            .split('.')
            .next()
            .ok_or_else(|| Error::registry(format!(
                "Invalid qualified ID '{}'. Expected format: 'namespace.metric_id' (e.g., 'fin.gross_margin')",
                qualified_id
            )))?;
        let metrics_in_namespace = Self::metric_ids_in_namespace(registry, namespace);

        for dep_id in dependencies {
            if !self.nodes.contains_key(dep_id.as_str()) {
                let dep_metric = registry.get(&dep_id)?;

                let formula = Self::qualify_metric_references_with_namespace_set(
                    &dep_metric.definition.formula,
                    namespace,
                    &metrics_in_namespace,
                );

                self.insert_metric_node(&dep_id, dep_metric, formula);
            }
        }

        if !self.nodes.contains_key(qualified_id) {
            let stored_metric = registry.get(qualified_id)?;

            let formula = Self::qualify_metric_references_with_namespace_set(
                &stored_metric.definition.formula,
                namespace,
                &metrics_in_namespace,
            );

            self.insert_metric_node(qualified_id, stored_metric, formula);
        }

        Ok(self)
    }

    /// Replace unqualified metric references with qualified ones in a formula.
    fn metric_ids_in_namespace(
        registry: &crate::registry::Registry,
        namespace: &str,
    ) -> IndexSet<String> {
        let prefix = format!("{namespace}.");
        registry
            .namespace(namespace)
            .map(|(id, _)| id.strip_prefix(&prefix).unwrap_or(id).to_string())
            .collect()
    }

    fn qualify_metric_references_with_namespace_set(
        formula: &str,
        namespace: &str,
        metrics_in_namespace: &IndexSet<String>,
    ) -> String {
        crate::utils::formula::qualify_identifiers(formula, metrics_in_namespace, namespace)
    }

    /// Build and validate the final financial-model specification.
    ///
    /// The returned [`FinancialModelSpec`] is the immutable input to the
    /// evaluator. Formula and `where` references retain the exact node IDs
    /// supplied by the caller; unknown identifiers are diagnosed by the
    /// dependency graph instead of being rewritten.
    ///
    /// # Errors
    ///
    /// Returns an error if the completed specification violates semantic
    /// invariants such as invalid node definitions or incompatible values.
    /// Builder calls may accept intermediate state, so this is the authoritative
    /// whole-model validation boundary. Unknown formula references are surfaced
    /// when the dependency graph is prepared for evaluation.
    pub fn build(self) -> Result<FinancialModelSpec> {
        let _span = tracing::info_span!(
            "statements.build",
            model_id = self.id.as_str(),
            periods = self.periods.len(),
            nodes = self.nodes.len(),
        )
        .entered();

        let mut spec = FinancialModelSpec::new(self.id, self.periods);
        spec.nodes = self.nodes;
        spec.meta = self.meta;
        spec.capital_structure = self.capital_structure;
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
    parent: ModelBuilder<Ready>,
    node_id: NodeId,
    values: Option<IndexMap<PeriodId, AmountOrScalar>>,
    forecast: Option<crate::types::ForecastSpec>,
    formula: Option<String>,
    name: Option<String>,
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
    /// let builder = ModelBuilder::new("demo")
    ///     .periods("2025Q1..Q2", None)?
    ///     .mixed("revenue")
    ///     .values(&[(PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100.0))])
    ///     .build()?;
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
    #[must_use = "builder methods must be chained"]
    pub fn values(mut self, values: &[(PeriodId, AmountOrScalar)]) -> Self {
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
    /// let builder = ModelBuilder::new("demo")
    ///     .periods("2025Q1..Q2", None)?
    ///     .mixed("revenue")
    ///     .forecast(ForecastSpec::forward_fill())
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "builder methods must be chained"]
    pub fn forecast(mut self, forecast_spec: crate::types::ForecastSpec) -> Self {
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
    /// let builder = ModelBuilder::new("demo")
    ///     .periods("2025Q1..Q2", None)?
    ///     .mixed("revenue")
    ///     .values(&[(PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100.0))])
    ///     .formula("lag(revenue, 1)")?
    ///     .build()?;
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
    #[must_use = "builder methods must be chained"]
    pub fn formula(mut self, formula: impl Into<String>) -> Result<Self> {
        self.try_formula(formula)?;
        Ok(self)
    }

    /// Set the fallback formula in place, leaving the builder usable on error.
    ///
    /// The non-consuming twin of [`formula`](Self::formula): the formula is
    /// syntax-checked before it is stored, so a host wrapper holding this
    /// builder behind a mutable reference keeps its values, forecast, and
    /// parent model when the formula has a typo.
    ///
    /// # Arguments
    ///
    /// * `formula` - DSL expression evaluated when neither an explicit value
    ///   nor a forecast resolves for a period
    ///
    /// # Errors
    ///
    /// Returns a formula parse or compilation error if `formula` is blank or
    /// invalid Statements DSL.
    pub fn try_formula(&mut self, formula: impl Into<String>) -> Result<&mut Self> {
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
    /// let builder = ModelBuilder::new("demo")
    ///     .periods("2025Q1..Q2", None)?
    ///     .mixed("revenue")
    ///     .name("Revenue (actual + forecast)")
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "builder methods must be chained"]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
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
    /// let model = ModelBuilder::new("demo")
    ///     .periods("2025Q1..Q2", None)?
    ///     .mixed("revenue")
    ///         .values(&[(PeriodId::quarter(2025, 1).expect("valid period fixture"), 100.0.into())])
    ///         .forecast(ForecastSpec::forward_fill())
    ///         .formula("lag(revenue, 1)")?
    ///         .build()?
    ///     .build()?;
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
    #[must_use = "builder methods must be chained"]
    pub fn build(mut self) -> Result<ModelBuilder<Ready>> {
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

        self.parent.nodes.insert(self.node_id, node);
        Ok(self.parent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluator::Evaluator;

    #[test]
    fn test_formula_references_require_exact_node_ids() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let model = ModelBuilder::new("alias-test")
            .periods("2025Q1..Q1", None)
            .expect("valid periods")
            .value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))])
            .value("cogs", &[(period, AmountOrScalar::scalar(40_000.0))])
            .compute("gross_profit", "rev - cogs")
            .expect("valid formula")
            .build()
            .expect("formula references are checked when preparing evaluation");

        let mut evaluator = Evaluator::new();
        let error = evaluator
            .evaluate(&model)
            .expect_err("the alias must not be rewritten to revenue");
        let message = error.to_string();
        assert!(message.contains("Unknown identifier 'rev'"));
        assert!(message.contains("Did you mean one of: revenue"));
    }

    #[test]
    fn test_build_rejects_cross_currency_addition() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let result = ModelBuilder::new("dimension-test")
            .periods("2025Q1..Q1", None)
            .expect("valid periods")
            .value_money(
                "usd_revenue",
                &[(
                    period,
                    finstack_quant_core::money::Money::from((
                        100_i64,
                        finstack_quant_core::currency::Currency::USD,
                    )),
                )],
            )
            .value_money(
                "eur_cost",
                &[(
                    period,
                    finstack_quant_core::money::Money::from((
                        40_i64,
                        finstack_quant_core::currency::Currency::EUR,
                    )),
                )],
            )
            .compute("bad_total", "usd_revenue + eur_cost")
            .expect("formula syntax is valid")
            .build();

        assert!(result.is_err());
        assert!(result
            .expect_err("cross-currency addition should fail")
            .to_string()
            .contains("Dimensional mismatch"));
    }
}
