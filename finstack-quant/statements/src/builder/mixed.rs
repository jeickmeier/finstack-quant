//! Fluent builder for mixed statement nodes.

use super::model_builder::ModelBuilder;
use super::validation::validate_node_id;
use crate::error::{Error, Result};
use crate::types::{AmountOrScalar, NodeId, NodeSpec, NodeType};
use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;

/// Fluent builder for mixed nodes.
///
/// This builder allows configuring all aspects of a mixed node (values, forecast, formula)
/// in a fluent manner before adding it to the model.
#[derive(Debug)]
pub struct MixedNodeBuilder {
    pub(super) parent: ModelBuilder,
    pub(super) node_id: NodeId,
    pub(super) values: Option<IndexMap<PeriodId, AmountOrScalar>>,
    pub(super) forecast: Option<crate::types::ForecastSpec>,
    pub(super) formula: Option<String>,
    pub(super) name: Option<String>,
    pub(super) rollup: Option<crate::types::RollupMethod>,
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
