//! Node, formula, forecast, and metric-registry builders for [`ModelBuilder`].

use super::mixed::MixedNodeBuilder;
use super::model_builder::ModelBuilder;
use super::validation::validate_node_id;
use crate::error::{Error, Result};
use crate::registry::dynamic::StoredMetric;
use crate::types::{AmountOrScalar, NodeId, NodeSpec, NodeType};
use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;

impl ModelBuilder {
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

    /// Add the same value observation to every model period.
    ///
    /// # Arguments
    /// * `node_id` - Unique node identifier stored in the model.
    /// * `value` - A finite unitless scalar or an exact monetary amount in its
    ///   stated currency, copied to every actual and forecast period.
    ///
    /// # Errors
    /// Rejects a missing period plan or a nonfinite scalar. The standard
    /// builder validation rejects duplicate node identifiers at build time.
    pub fn constant(
        &mut self,
        node_id: impl Into<NodeId>,
        value: AmountOrScalar,
    ) -> Result<&mut Self> {
        if self.periods_slice().is_empty() {
            return Err(Error::build("Set model periods before adding a constant"));
        }
        if matches!(value, AmountOrScalar::Scalar(value) if !value.is_finite()) {
            return Err(Error::build("Constant scalar must be finite"));
        }
        let observations: Vec<_> = self
            .periods_slice()
            .iter()
            .map(|period| (period.id, value))
            .collect();
        Ok(self.value(node_id, &observations))
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
    /// # use finstack_quant_statements::types::{AmountOrScalar, ForecastSpec};
    /// # use finstack_quant_core::dates::PeriodId;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = {
    ///     let mut model_builder = ModelBuilder::new("test");
    ///     model_builder.periods("2025Q1..Q4", Some("2025Q2"))?;
    ///     let mut model_builder = model_builder.mixed("revenue");
    ///     model_builder.values(&[
    ///             (PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100_000.0)),
    ///             (PeriodId::quarter(2025, 2).expect("valid period fixture"), AmountOrScalar::scalar(110_000.0)),
    ///         ]);
    ///     model_builder.forecast(ForecastSpec::growth(0.05));
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
    /// # use finstack_quant_statements::types::{AmountOrScalar, ForecastSpec};
    /// # use finstack_quant_core::dates::PeriodId;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = {
    ///     let mut model_builder = ModelBuilder::new("test");
    ///     model_builder.periods("2025Q1..Q4", Some("2025Q2"))?;
    ///     model_builder.value("revenue", &[
    ///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100_000.0)),
    ///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), AmountOrScalar::scalar(110_000.0)),
    ///     ]);
    ///     model_builder.forecast("revenue", ForecastSpec::growth(0.05));
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

    /// Denominate a scalar formula result as an amount of currency.
    ///
    /// # Arguments
    /// * `node_id` - Unique model identifier for the new monetary node.
    /// * `source` - Scalar DSL expression or source node in units of the selected currency.
    /// * `currency` - Explicit denomination, or the currency stored in model metadata.
    ///
    /// # Errors
    /// Rejects invalid formula syntax, or absent or invalid model currency when
    /// an explicit denomination is omitted. Model validation rejects a source
    /// with monetary or Boolean units.
    pub fn as_money(
        &mut self,
        node_id: impl Into<NodeId>,
        source: &str,
        currency: Option<finstack_quant_core::currency::Currency>,
    ) -> Result<&mut Self> {
        let currency = match currency {
            Some(currency) => currency,
            None => self
                .meta
                .get("currency")
                .and_then(serde_json::Value::as_str)
                .and_then(|code| code.parse().ok())
                .ok_or_else(|| {
                    Error::build("as_money needs an explicit currency or a valid model currency")
                })?,
        };
        self.compute(node_id, format!("as_money(({source}), {currency})"))
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
}
