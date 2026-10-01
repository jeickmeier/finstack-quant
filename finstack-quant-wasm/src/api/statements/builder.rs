//! WASM wrapper for the type-state `ModelBuilder`.
//!
//! JavaScript cannot model the Rust type-state, so the two builder states are
//! one class and readiness is tracked at runtime, exactly as in the Python
//! binding. Methods update the builder in place and return nothing
//! (wasm-bindgen cannot return `this`), so calls are written as statements
//! rather than a chain. Fallible steps use the crate's non-consuming `try_*`
//! twins, so a bad formula or period range throws without discarding the
//! model built so far.

use super::handles::JsRegistry;
use crate::utils::input::{from_js_json, js_f64, js_opt_string, js_string, json_text};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::dates::{Date, Period, PeriodId};
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::{MixedNodeBuilder, ModelBuilder, NeedPeriods, Ready};
use finstack_quant_statements::capital_structure::WaterfallSpec;
use finstack_quant_statements::types::{
    AmountOrScalar, FinancialStatementInstrument, ForecastSpec, NodeSpec,
};
use finstack_quant_statements::FinancialModelSpec;
use indexmap::IndexMap;
use wasm_bindgen::prelude::*;

enum BuilderState {
    NeedPeriods(ModelBuilder<NeedPeriods>),
    Ready(ModelBuilder<Ready>),
}

fn invalid(message: &str) -> JsValue {
    to_js_err(finstack_quant_core::Error::Validation(message.to_string()))
}

fn consumed() -> JsValue {
    invalid(
        "Builder is no longer usable: it was consumed by build()/mixed() or by a failed \
         capital-structure call. Construct a new ModelBuilder.",
    )
}

/// Any JSON value: a string is stored as a string, other primitives as
/// themselves, and arrays or plain objects through the strict JSON walker.
fn meta_value(value: &JsValue) -> Result<serde_json::Value, JsValue> {
    if let Some(text) = value.as_string() {
        return Ok(serde_json::Value::String(text));
    }
    if value.is_null() || value.is_undefined() {
        return Ok(serde_json::Value::Null);
    }
    if let Some(flag) = value.as_bool() {
        return Ok(serde_json::Value::Bool(flag));
    }
    if value.as_f64().is_some() {
        let number = js_f64(value, "value")?;
        return serde_json::Number::from_f64(number)
            .map(serde_json::Value::Number)
            .ok_or_else(|| invalid("value: a meta number must be finite"));
    }
    from_js_json(value, "value")
}

fn iso_date(value: &JsValue, label: &str) -> Result<Date, JsValue> {
    crate::utils::parse_iso_date(&js_string(value, label)?)
}

/// Fluent builder for `FinancialModelSpec`.
///
/// Twin of the Python `ModelBuilder` (Rust `ModelBuilder`). Call `periods`
/// first, add nodes, then `build`. Every method updates the builder in place
/// and returns `undefined`; `build` and `mixed` consume it.
///
/// @example
/// ```javascript
/// import init, { statements } from "finstack-quant-wasm";
/// await init();
/// const builder = new statements.ModelBuilder("acme");
/// builder.periods("2025Q1..Q4", "2025Q1");
/// builder.valueScalar("revenue", { "2025Q1": 100 });
/// builder.forecast("revenue", statements.forecastSpecGrowth(0.05));
/// builder.compute("gross_profit", "revenue * 0.6");
/// const model = builder.build();   // plain FinancialModelSpec object
/// statements.modelNodeIds(model);  // ["revenue", "gross_profit"]
/// ```
#[wasm_bindgen(js_name = ModelBuilder)]
pub struct JsModelBuilder {
    inner: Option<BuilderState>,
}

impl JsModelBuilder {
    fn take_any(&mut self) -> Result<BuilderState, JsValue> {
        self.inner.take().ok_or_else(consumed)
    }

    fn take_ready(&mut self) -> Result<ModelBuilder<Ready>, JsValue> {
        match self.take_any()? {
            BuilderState::Ready(builder) => Ok(builder),
            state @ BuilderState::NeedPeriods(_) => {
                self.inner = Some(state);
                Err(invalid("Must call periods() before adding nodes"))
            }
        }
    }

    fn ready_mut(&mut self) -> Result<&mut ModelBuilder<Ready>, JsValue> {
        match self.inner.as_mut() {
            Some(BuilderState::Ready(builder)) => Ok(builder),
            Some(BuilderState::NeedPeriods(_)) => {
                Err(invalid("Must call periods() before adding nodes"))
            }
            None => Err(consumed()),
        }
    }

    /// Apply a capital-structure step that exists in both builder states.
    fn capital_structure(
        &mut self,
        need: impl FnOnce(
            ModelBuilder<NeedPeriods>,
        ) -> finstack_quant_statements::Result<ModelBuilder<NeedPeriods>>,
        ready: impl FnOnce(
            ModelBuilder<Ready>,
        ) -> finstack_quant_statements::Result<ModelBuilder<Ready>>,
    ) -> Result<(), JsValue> {
        let next = match self.take_any()? {
            BuilderState::NeedPeriods(builder) => {
                BuilderState::NeedPeriods(need(builder).map_err(to_js_err)?)
            }
            BuilderState::Ready(builder) => BuilderState::Ready(ready(builder).map_err(to_js_err)?),
        };
        self.inner = Some(next);
        Ok(())
    }
}

#[wasm_bindgen(js_class = ModelBuilder)]
impl JsModelBuilder {
    /// Start a model with the given identifier.
    ///
    /// @param id - Model identifier stored on the built `FinancialModelSpec`.
    /// @returns A builder waiting for `periods`.
    /// @throws TypeError with kind `invalid_type` if `id` is not a string.
    #[wasm_bindgen(constructor)]
    pub fn new(id: JsValue) -> Result<JsModelBuilder, JsValue> {
        Ok(JsModelBuilder {
            inner: Some(BuilderState::NeedPeriods(ModelBuilder::new(js_string(
                &id, "id",
            )?))),
        })
    }

    /// Resume building from an existing model specification.
    ///
    /// Twin of Python `ModelBuilder.from_spec` (Rust `ModelBuilder::from_spec`).
    ///
    /// @param spec - `FinancialModelSpec` whose periods, nodes and capital structure seed the builder (object or JSON).
    /// @returns A ready builder holding the model's periods and nodes.
    /// @throws Error with kind `validation` if the model is malformed or fails semantic validation.
    #[wasm_bindgen(js_name = fromSpec)]
    pub fn from_spec(spec: JsValue) -> Result<JsModelBuilder, JsValue> {
        let spec = FinancialModelSpec::from_json(&json_text(&spec, "spec")?).map_err(to_js_err)?;
        let builder = ModelBuilder::from_spec(spec).map_err(to_js_err)?;
        Ok(JsModelBuilder {
            inner: Some(BuilderState::Ready(builder)),
        })
    }

    /// Define the model periods from a range expression.
    ///
    /// Twin of Python `ModelBuilder.periods` (Rust `ModelBuilder::try_periods`).
    /// A failed call leaves the builder usable.
    ///
    /// @param range - Period range `<start>..<end>` in one period kind, e.g. `"2025Q1..Q4"`, `"2024M10..2025M03"` or `"2025..2030"`.
    /// @param actuals_until - Last actual period (inclusive), e.g. `"2025Q2"`; later periods are forecast. Omit to treat every period as forecast.
    /// @throws Error with kind `validation` if the range or cutoff cannot be parsed, the range is reversed or empty, periods were already set, or the builder was consumed.
    pub fn periods(
        &mut self,
        range: JsValue,
        actuals_until: Option<JsValue>,
    ) -> Result<(), JsValue> {
        let range = js_string(&range, "range")?;
        let actuals_until = js_opt_string(actuals_until.as_ref(), "actualsUntil")?;
        match self.take_any()? {
            BuilderState::NeedPeriods(builder) => {
                match builder.try_periods(&range, actuals_until.as_deref()) {
                    Ok(ready) => {
                        self.inner = Some(BuilderState::Ready(ready));
                        Ok(())
                    }
                    Err((builder, error)) => {
                        self.inner = Some(BuilderState::NeedPeriods(*builder));
                        Err(to_js_err(error))
                    }
                }
            }
            state @ BuilderState::Ready(_) => {
                self.inner = Some(state);
                Err(invalid("Periods already set"))
            }
        }
    }

    /// Define the model periods from an explicit list.
    ///
    /// Twin of Python `ModelBuilder.periods_explicit` (Rust
    /// `ModelBuilder::periods_explicit`).
    ///
    /// @param periods - Array of `Period` objects (`id`, ISO `start`, exclusive ISO `end`, `is_actual`) in timeline order (array or JSON).
    /// @throws Error with kind `validation` if `periods` is malformed, empty, unordered or overlapping, periods were already set, or the builder was consumed.
    #[wasm_bindgen(js_name = periodsExplicit)]
    pub fn periods_explicit(&mut self, periods: JsValue) -> Result<(), JsValue> {
        let periods: Vec<Period> = from_js_json(&periods, "periods")?;
        match self.take_any()? {
            BuilderState::NeedPeriods(builder) => {
                let ready = builder.periods_explicit(periods).map_err(to_js_err)?;
                self.inner = Some(BuilderState::Ready(ready));
                Ok(())
            }
            state @ BuilderState::Ready(_) => {
                self.inner = Some(state);
                Err(invalid("Periods already set"))
            }
        }
    }

    /// Add a value node holding explicit per-period values.
    ///
    /// Twin of Python `ModelBuilder.value` (Rust `ModelBuilder::value`).
    /// Reusing a node identifier replaces its former definition.
    ///
    /// @param node_id - Node identifier, unique within the model.
    /// @param values - Object mapping period id (e.g. `"2025Q1"`) to a number or a `Money` wire object (`{amount, currency}`).
    /// @throws Error with kind `validation` if `values` is malformed, periods were not set, or the builder was consumed.
    pub fn value(&mut self, node_id: JsValue, values: JsValue) -> Result<(), JsValue> {
        let node_id = js_string(&node_id, "nodeId")?;
        let values: IndexMap<PeriodId, AmountOrScalar> = from_js_json(&values, "values")?;
        let values: Vec<(PeriodId, AmountOrScalar)> = values.into_iter().collect();
        let builder = self.take_ready()?;
        self.inner = Some(BuilderState::Ready(builder.value(node_id, &values)));
        Ok(())
    }

    /// Add a value node holding unitless scalar values.
    ///
    /// Twin of Python `ModelBuilder.value_scalar` (Rust
    /// `ModelBuilder::value_scalar`).
    ///
    /// @param node_id - Node identifier, unique within the model.
    /// @param values - Object mapping period id (e.g. `"2025Q1"`) to a number.
    /// @throws Error with kind `validation` if `values` is malformed, periods were not set, or the builder was consumed.
    #[wasm_bindgen(js_name = valueScalar)]
    pub fn value_scalar(&mut self, node_id: JsValue, values: JsValue) -> Result<(), JsValue> {
        let node_id = js_string(&node_id, "nodeId")?;
        let values: IndexMap<PeriodId, f64> = from_js_json(&values, "values")?;
        let values: Vec<(PeriodId, f64)> = values.into_iter().collect();
        let builder = self.take_ready()?;
        self.inner = Some(BuilderState::Ready(builder.value_scalar(node_id, &values)));
        Ok(())
    }

    /// Add a value node holding monetary values.
    ///
    /// Twin of Python `ModelBuilder.value_money` (Rust
    /// `ModelBuilder::value_money`).
    ///
    /// @param node_id - Node identifier, unique within the model.
    /// @param values - Object mapping period id (e.g. `"2025Q1"`) to a `Money` wire object (`{amount, currency}`); all amounts share one currency.
    /// @throws Error with kind `validation` if `values` is malformed, periods were not set, or the builder was consumed.
    #[wasm_bindgen(js_name = valueMoney)]
    pub fn value_money(&mut self, node_id: JsValue, values: JsValue) -> Result<(), JsValue> {
        let node_id = js_string(&node_id, "nodeId")?;
        let values: IndexMap<PeriodId, Money> = from_js_json(&values, "values")?;
        let values: Vec<(PeriodId, Money)> = values.into_iter().collect();
        let builder = self.take_ready()?;
        self.inner = Some(BuilderState::Ready(builder.value_money(node_id, &values)));
        Ok(())
    }

    /// Record the dates on which a node's period values became available.
    ///
    /// Twin of Python `ModelBuilder.availability_dates` (Rust
    /// `ModelBuilder::try_availability_dates`). A failed call leaves the
    /// builder usable.
    ///
    /// @param node_id - Existing node whose values the dates describe.
    /// @param availability_dates - Object mapping period id (e.g. `"2025Q1"`) to the ISO 8601 date its value was published.
    /// @throws Error with kind `validation` if `availabilityDates` is malformed, the node does not exist, periods were not set, or the builder was consumed.
    #[wasm_bindgen(js_name = availabilityDates)]
    pub fn availability_dates(
        &mut self,
        node_id: JsValue,
        availability_dates: JsValue,
    ) -> Result<(), JsValue> {
        let node_id = js_string(&node_id, "nodeId")?;
        let dates: IndexMap<PeriodId, String> =
            from_js_json(&availability_dates, "availabilityDates")?;
        let dates = dates
            .into_iter()
            .map(|(period, date)| Ok((period, crate::utils::parse_iso_date(&date)?)))
            .collect::<Result<Vec<(PeriodId, Date)>, JsValue>>()?;
        self.ready_mut()?
            .try_availability_dates(&node_id, &dates)
            .map_err(to_js_err)?;
        Ok(())
    }

    /// Add a calculated node defined by a DSL formula.
    ///
    /// Twin of Python `ModelBuilder.compute` (Rust `ModelBuilder::try_compute`).
    /// A failed call leaves the builder usable.
    ///
    /// @param node_id - Node identifier, unique within the model.
    /// @param formula - Statements DSL expression, e.g. `"revenue - cogs"`.
    /// @throws Error with kind `validation` if the identifier is reserved, the formula is blank or does not parse and compile, periods were not set, or the builder was consumed.
    pub fn compute(&mut self, node_id: JsValue, formula: JsValue) -> Result<(), JsValue> {
        let node_id = js_string(&node_id, "nodeId")?;
        let formula = js_string(&formula, "formula")?;
        self.ready_mut()?
            .try_compute(node_id, formula)
            .map_err(to_js_err)?;
        Ok(())
    }

    /// Insert a fully specified node, replacing any node with the same id.
    ///
    /// Twin of Python `ModelBuilder.insert_node` (Rust
    /// `ModelBuilder::insert_node`). Allowed before `periods`.
    ///
    /// @param node - `NodeSpec` to insert under its own `node_id` (object or JSON).
    /// @throws Error with kind `validation` if `node` is malformed or the builder was consumed.
    #[wasm_bindgen(js_name = insertNode)]
    pub fn insert_node(&mut self, node: JsValue) -> Result<(), JsValue> {
        let spec: NodeSpec = from_js_json(&node, "node")?;
        let id = spec.node_id.clone();
        match self.inner.as_mut() {
            Some(BuilderState::NeedPeriods(builder)) => {
                builder.insert_node(id, spec);
            }
            Some(BuilderState::Ready(builder)) => {
                builder.insert_node(id, spec);
            }
            None => return Err(consumed()),
        }
        Ok(())
    }

    /// Start configuring a mixed node (values, forecast and formula together).
    ///
    /// Twin of Python `ModelBuilder.mixed` (Rust `ModelBuilder::mixed`). This
    /// builder is consumed; `MixedNodeBuilder.build` returns the builder to
    /// continue with.
    ///
    /// @param node_id - Node identifier, unique within the model.
    /// @returns A `MixedNodeBuilder` for the node.
    /// @throws Error with kind `validation` if periods were not set or the builder was consumed.
    pub fn mixed(&mut self, node_id: JsValue) -> Result<JsMixedNodeBuilder, JsValue> {
        let node_id = js_string(&node_id, "nodeId")?;
        let builder = self.take_ready()?;
        Ok(JsMixedNodeBuilder {
            inner: Some(builder.mixed(node_id)),
        })
    }

    /// Attach a forecast to a node, creating the node when it does not exist.
    ///
    /// Twin of Python `ModelBuilder.forecast` (Rust `ModelBuilder::forecast`).
    ///
    /// @param node_id - Node whose forecast periods the spec fills.
    /// @param forecast_spec - `ForecastSpec` (`method` plus `params`), e.g. from `forecastSpecGrowth` (object or JSON).
    /// @throws Error with kind `validation` if `forecastSpec` is malformed, periods were not set, or the builder was consumed.
    pub fn forecast(&mut self, node_id: JsValue, forecast_spec: JsValue) -> Result<(), JsValue> {
        let node_id = js_string(&node_id, "nodeId")?;
        let spec: ForecastSpec = from_js_json(&forecast_spec, "forecastSpec")?;
        let builder = self.take_ready()?;
        self.inner = Some(BuilderState::Ready(builder.forecast(node_id, spec)));
        Ok(())
    }

    /// Attach a where clause to the most recently added node.
    ///
    /// Twin of Python `ModelBuilder.where_clause` (Rust
    /// `ModelBuilder::where_clause`).
    ///
    /// @param where_clause - Boolean DSL expression; the node evaluates only in periods where it is true.
    /// @throws Error with kind `validation` if periods were not set or the builder was consumed.
    #[wasm_bindgen(js_name = whereClause)]
    pub fn where_clause(&mut self, where_clause: JsValue) -> Result<(), JsValue> {
        let where_clause = js_string(&where_clause, "whereClause")?;
        let builder = self.take_ready()?;
        self.inner = Some(BuilderState::Ready(builder.where_clause(where_clause)));
        Ok(())
    }

    /// Add model-level metadata.
    ///
    /// Twin of Python `ModelBuilder.with_meta` (Rust `ModelBuilder::with_meta`).
    ///
    /// @param key - Metadata key; `"currency"` is read by the evaluator and the DCF to infer the reporting currency.
    /// @param value - Any JSON value (string, number, boolean, array, plain object or `null`), stored as given: a string is not parsed as JSON. For `key = "currency"` pass a three-letter code such as `"USD"`.
    /// @throws Error with kind `invalid_type` if `value` is not JSON-representable, and kind `validation` if a number is not finite, periods were not set, or the builder was consumed.
    #[wasm_bindgen(js_name = withMeta)]
    pub fn with_meta(&mut self, key: JsValue, value: JsValue) -> Result<(), JsValue> {
        let key = js_string(&key, "key")?;
        let value = meta_value(&value)?;
        let builder = self.take_ready()?;
        self.inner = Some(BuilderState::Ready(builder.with_meta(key, value)));
        Ok(())
    }

    /// Add every built-in statement metric (`fin.*` namespace).
    ///
    /// Twin of Python `ModelBuilder.with_builtin_metrics` (Rust
    /// `ModelBuilder::try_with_builtin_metrics`). A failed call leaves the
    /// builder usable.
    ///
    /// @throws Error with kind `validation` if the built-in catalog cannot be loaded, periods were not set, or the builder was consumed.
    #[wasm_bindgen(js_name = withBuiltinMetrics)]
    pub fn with_builtin_metrics(&mut self) -> Result<(), JsValue> {
        self.ready_mut()?
            .try_with_builtin_metrics()
            .map_err(to_js_err)?;
        Ok(())
    }

    /// Add one metric and its dependencies from a registry.
    ///
    /// Twin of Python `ModelBuilder.add_metric_from_registry` (Rust
    /// `ModelBuilder::try_add_metric_from_registry`). A failed call leaves the
    /// builder usable.
    ///
    /// @param qualified_id - Metric identifier as `namespace.metric`, e.g. `"fin.gross_margin"`.
    /// @param registry - `Registry` holding the metric and its dependencies.
    /// @throws Error with kind `not_found` if the metric or a dependency is not in `registry`, and kind `validation` if the identifier is not `namespace.metric` shaped, periods were not set, or the builder was consumed.
    #[wasm_bindgen(js_name = addMetricFromRegistry)]
    pub fn add_metric_from_registry(
        &mut self,
        qualified_id: JsValue,
        registry: &JsRegistry,
    ) -> Result<(), JsValue> {
        let qualified_id = js_string(&qualified_id, "qualifiedId")?;
        self.ready_mut()?
            .try_add_metric_from_registry(&qualified_id, &registry.inner)
            .map_err(to_js_err)?;
        Ok(())
    }

    /// Add a fixed-rate bond to the capital structure (US conventions: 30/360, semi-annual).
    ///
    /// Twin of Python `ModelBuilder.add_bond` (Rust `ModelBuilder::add_bond`).
    /// A failed call consumes the builder.
    ///
    /// @param id - Unique instrument identifier.
    /// @param notional - Principal as a `Money` wire object (`{amount, currency}`).
    /// @param coupon_rate - Annual coupon rate as a decimal (`0.05` = 5%).
    /// @param issue_date - ISO 8601 issue date.
    /// @param maturity_date - ISO 8601 maturity date.
    /// @param discount_curve_id - Identifier of the discount curve used for pricing.
    /// @throws Error with kind `validation` if an argument is malformed, the bond cannot be constructed, or the builder was consumed.
    #[wasm_bindgen(js_name = addBond)]
    pub fn add_bond(
        &mut self,
        id: JsValue,
        notional: JsValue,
        coupon_rate: JsValue,
        issue_date: JsValue,
        maturity_date: JsValue,
        discount_curve_id: JsValue,
    ) -> Result<(), JsValue> {
        let id = js_string(&id, "id")?;
        let notional: Money = from_js_json(&notional, "notional")?;
        let coupon_rate = js_f64(&coupon_rate, "couponRate")?;
        let issue = iso_date(&issue_date, "issueDate")?;
        let maturity = iso_date(&maturity_date, "maturityDate")?;
        let curve = js_string(&discount_curve_id, "discountCurveId")?;
        let (id2, curve2) = (id.clone(), curve.clone());
        self.capital_structure(
            |b| b.add_bond(id, notional, coupon_rate, issue, maturity, curve),
            |b| b.add_bond(id2, notional, coupon_rate, issue, maturity, curve2),
        )
    }

    /// Add a fixed-rate bond with a market convention preset.
    ///
    /// Twin of Python `ModelBuilder.add_bond_with_convention` (Rust
    /// `ModelBuilder::add_bond_with_convention`). A failed call consumes the
    /// builder.
    ///
    /// @param id - Unique instrument identifier.
    /// @param notional - Principal as a `Money` wire object (`{amount, currency}`).
    /// @param coupon_rate - Annual coupon rate as a decimal (`0.03` = 3%).
    /// @param issue_date - ISO 8601 issue date.
    /// @param maturity_date - ISO 8601 maturity date.
    /// @param convention - Regional preset: `"us_treasury"`, `"us_agency"`, `"german_bund"`, `"uk_gilt"`, `"french_oat"`, `"jgb"`, `"us_corporate"` or `"eur_corporate"`; sets day count, coupon frequency and calendar.
    /// @param discount_curve_id - Identifier of the discount curve used for pricing.
    /// @throws Error with kind `validation` if an argument is malformed, `convention` is unknown, the bond cannot be constructed, or the builder was consumed.
    #[wasm_bindgen(js_name = addBondWithConvention)]
    #[allow(clippy::too_many_arguments)]
    pub fn add_bond_with_convention(
        &mut self,
        id: JsValue,
        notional: JsValue,
        coupon_rate: JsValue,
        issue_date: JsValue,
        maturity_date: JsValue,
        convention: JsValue,
        discount_curve_id: JsValue,
    ) -> Result<(), JsValue> {
        let id = js_string(&id, "id")?;
        let notional: Money = from_js_json(&notional, "notional")?;
        let rate =
            finstack_quant_core::types::Rate::from_decimal(js_f64(&coupon_rate, "couponRate")?)
                .map_err(to_js_err)?;
        let issue = iso_date(&issue_date, "issueDate")?;
        let maturity = iso_date(&maturity_date, "maturityDate")?;
        let convention: finstack_quant_valuations::instruments::BondConvention =
            finstack_quant_core::wire::serde_parse(&js_string(&convention, "convention")?)
                .map_err(to_js_err)?;
        let curve = js_string(&discount_curve_id, "discountCurveId")?;
        let (id2, curve2) = (id.clone(), curve.clone());
        self.capital_structure(
            |b| b.add_bond_with_convention(id, notional, rate, issue, maturity, convention, curve),
            |b| {
                b.add_bond_with_convention(id2, notional, rate, issue, maturity, convention, curve2)
            },
        )
    }

    /// Add a pay-fixed interest rate swap to the capital structure (US conventions).
    ///
    /// Twin of Python `ModelBuilder.add_swap` (Rust `ModelBuilder::add_swap`).
    /// A failed call consumes the builder.
    ///
    /// @param id - Unique instrument identifier.
    /// @param notional - Swap notional as a `Money` wire object (`{amount, currency}`).
    /// @param fixed_rate - Fixed leg rate as a decimal (`0.04` = 4%).
    /// @param start_date - ISO 8601 effective date.
    /// @param maturity_date - ISO 8601 maturity date.
    /// @param discount_curve_id - Identifier of the discount curve.
    /// @param forward_curve_id - Identifier of the floating-leg forward curve.
    /// @throws Error with kind `validation` if an argument is malformed, the swap cannot be constructed, or the builder was consumed.
    #[wasm_bindgen(js_name = addSwap)]
    #[allow(clippy::too_many_arguments)]
    pub fn add_swap(
        &mut self,
        id: JsValue,
        notional: JsValue,
        fixed_rate: JsValue,
        start_date: JsValue,
        maturity_date: JsValue,
        discount_curve_id: JsValue,
        forward_curve_id: JsValue,
    ) -> Result<(), JsValue> {
        let id = js_string(&id, "id")?;
        let notional: Money = from_js_json(&notional, "notional")?;
        let fixed_rate = js_f64(&fixed_rate, "fixedRate")?;
        let start = iso_date(&start_date, "startDate")?;
        let maturity = iso_date(&maturity_date, "maturityDate")?;
        let discount = js_string(&discount_curve_id, "discountCurveId")?;
        let forward = js_string(&forward_curve_id, "forwardCurveId")?;
        let (id2, discount2, forward2) = (id.clone(), discount.clone(), forward.clone());
        self.capital_structure(
            |b| b.add_swap(id, notional, fixed_rate, start, maturity, discount, forward),
            |b| {
                b.add_swap(
                    id2, notional, fixed_rate, start, maturity, discount2, forward2,
                )
            },
        )
    }

    /// Add a pay-fixed interest rate swap with explicit leg conventions.
    ///
    /// Twin of Python `ModelBuilder.add_swap_with_conventions` (Rust
    /// `ModelBuilder::add_swap_with_conventions`). A failed call consumes the
    /// builder.
    ///
    /// @param id - Unique instrument identifier.
    /// @param notional - Swap notional as a `Money` wire object (`{amount, currency}`).
    /// @param fixed_rate - Fixed leg rate as a decimal (`0.04` = 4%).
    /// @param start_date - ISO 8601 effective date.
    /// @param maturity_date - ISO 8601 maturity date.
    /// @param discount_curve_id - Identifier of the discount curve.
    /// @param forward_curve_id - Identifier of the floating-leg forward curve.
    /// @param fixed_frequency - Fixed leg payment frequency as a tenor, e.g. `"1Y"`.
    /// @param fixed_day_count - Fixed leg day-count convention, e.g. `"act_360"` or `"30_360"`.
    /// @param float_frequency - Floating leg payment and fixing frequency as a tenor, e.g. `"3M"`.
    /// @param float_day_count - Floating leg day-count convention, e.g. `"act_360"`.
    /// @param business_day_convention - Schedule roll convention, e.g. `"modified_following"`; omitted uses the Rust default, Modified Following.
    /// @throws Error with kind `validation` if an argument is malformed, a tenor, day count or roll convention is unknown, the swap cannot be constructed, or the builder was consumed.
    #[wasm_bindgen(js_name = addSwapWithConventions)]
    #[allow(clippy::too_many_arguments)]
    pub fn add_swap_with_conventions(
        &mut self,
        id: JsValue,
        notional: JsValue,
        fixed_rate: JsValue,
        start_date: JsValue,
        maturity_date: JsValue,
        discount_curve_id: JsValue,
        forward_curve_id: JsValue,
        fixed_frequency: JsValue,
        fixed_day_count: JsValue,
        float_frequency: JsValue,
        float_day_count: JsValue,
        business_day_convention: Option<JsValue>,
    ) -> Result<(), JsValue> {
        use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
        let id = js_string(&id, "id")?;
        let notional: Money = from_js_json(&notional, "notional")?;
        let fixed_rate = js_f64(&fixed_rate, "fixedRate")?;
        let start = iso_date(&start_date, "startDate")?;
        let maturity = iso_date(&maturity_date, "maturityDate")?;
        let discount = js_string(&discount_curve_id, "discountCurveId")?;
        let forward = js_string(&forward_curve_id, "forwardCurveId")?;
        let fixed_freq: Tenor = js_string(&fixed_frequency, "fixedFrequency")?
            .parse()
            .map_err(to_js_err)?;
        let fixed_dc: DayCount = js_string(&fixed_day_count, "fixedDayCount")?
            .parse()
            .map_err(to_js_err)?;
        let float_freq: Tenor = js_string(&float_frequency, "floatFrequency")?
            .parse()
            .map_err(to_js_err)?;
        let float_dc: DayCount = js_string(&float_day_count, "floatDayCount")?
            .parse()
            .map_err(to_js_err)?;
        let roll_convention =
            match js_opt_string(business_day_convention.as_ref(), "businessDayConvention")? {
                Some(label) => label.parse::<BusinessDayConvention>().map_err(to_js_err)?,
                None => BusinessDayConvention::default(),
            };
        let (id2, discount2, forward2) = (id.clone(), discount.clone(), forward.clone());
        self.capital_structure(
            |b| {
                b.add_swap_with_conventions(
                    id,
                    notional,
                    fixed_rate,
                    start,
                    maturity,
                    discount,
                    forward,
                    fixed_freq,
                    fixed_dc,
                    float_freq,
                    float_dc,
                    roll_convention,
                )
            },
            |b| {
                b.add_swap_with_conventions(
                    id2,
                    notional,
                    fixed_rate,
                    start,
                    maturity,
                    discount2,
                    forward2,
                    fixed_freq,
                    fixed_dc,
                    float_freq,
                    float_dc,
                    roll_convention,
                )
            },
        )
    }

    /// Add a debt instrument to the capital structure.
    ///
    /// Twin of Python `ModelBuilder.add_debt` (Rust `ModelBuilder::add_debt`).
    /// Supported instruments are bonds, convertible bonds, term loans,
    /// revolving credit facilities, interest-rate swaps, caps/floors and
    /// swaptions.
    ///
    /// @param id - Unique instrument identifier.
    /// @param instrument - The instrument's `finstack_quant.instrument/1` envelope (object or JSON); bare payloads without the envelope are rejected.
    /// @throws Error with kind `validation` if the envelope is invalid, the instrument type is not supported in a statement capital structure, or the builder was consumed.
    #[wasm_bindgen(js_name = addDebt)]
    pub fn add_debt(&mut self, id: JsValue, instrument: JsValue) -> Result<(), JsValue> {
        let id = js_string(&id, "id")?;
        let spec = finstack_quant_valuations::pricer::json::parse_instrument_from_json(&json_text(
            &instrument,
            "instrument",
        )?)
        .map_err(to_js_err)?;
        let spec = FinancialStatementInstrument::try_from(spec).map_err(to_js_err)?;
        let (id2, spec2) = (id.clone(), spec.clone());
        self.capital_structure(|b| Ok(b.add_debt(id, spec)), |b| Ok(b.add_debt(id2, spec2)))
    }

    /// Set the reporting currency used for capital-structure totals.
    ///
    /// Twin of Python `ModelBuilder.reporting_currency` (Rust
    /// `ModelBuilder::reporting_currency`).
    ///
    /// @param currency - ISO 4217 currency code, e.g. `"USD"`.
    /// @throws Error with kind `validation` if `currency` is not a known currency code or the builder was consumed.
    #[wasm_bindgen(js_name = reportingCurrency)]
    pub fn reporting_currency(&mut self, currency: JsValue) -> Result<(), JsValue> {
        let currency: finstack_quant_core::currency::Currency = js_string(&currency, "currency")?
            .parse()
            .map_err(to_js_err)?;
        self.capital_structure(
            |b| Ok(b.reporting_currency(currency)),
            |b| Ok(b.reporting_currency(currency)),
        )
    }

    /// Set the FX conversion policy for capital-structure cashflows.
    ///
    /// Twin of Python `ModelBuilder.fx_policy` (Rust `ModelBuilder::fx_policy`).
    ///
    /// @param policy - `"cashflow_date"`, `"period_end"` or `"period_average"`.
    /// @throws Error with kind `validation` if `policy` is not a known policy or the builder was consumed.
    #[wasm_bindgen(js_name = fxPolicy)]
    pub fn fx_policy(&mut self, policy: JsValue) -> Result<(), JsValue> {
        let policy: finstack_quant_core::money::fx::FxConversionPolicy =
            finstack_quant_core::wire::serde_parse(&js_string(&policy, "policy")?)
                .map_err(to_js_err)?;
        self.capital_structure(|b| Ok(b.fx_policy(policy)), |b| Ok(b.fx_policy(policy)))
    }

    /// Attach a waterfall (priority of payments, ECF sweep, PIK toggle, payment classes).
    ///
    /// Twin of Python `ModelBuilder.waterfall` (Rust `ModelBuilder::waterfall`).
    /// The waterfall is validated against the instruments at `build`.
    ///
    /// @param waterfall_spec - `WaterfallSpec` attached to the capital structure (object or JSON).
    /// @throws Error with kind `validation` if `waterfallSpec` is malformed or the builder was consumed.
    pub fn waterfall(&mut self, waterfall_spec: JsValue) -> Result<(), JsValue> {
        let spec: WaterfallSpec = from_js_json(&waterfall_spec, "waterfallSpec")?;
        let spec2 = spec.clone();
        self.capital_structure(|b| Ok(b.waterfall(spec)), |b| Ok(b.waterfall(spec2)))
    }

    /// Build the model specification.
    ///
    /// Twin of Python `ModelBuilder.build` (Rust `ModelBuilder::build`). The
    /// builder is consumed.
    ///
    /// @returns The completed `FinancialModelSpec` plain object, accepted by every model-taking function.
    /// @throws Error with kind `validation` if the model fails semantic validation, periods were not set, or the builder was consumed.
    pub fn build(&mut self) -> Result<JsValue, JsValue> {
        let builder = self.take_ready()?;
        to_js_value(&builder.build().map_err(to_js_err)?)
    }
}

/// Builder for one mixed node (explicit values, a forecast and a formula).
///
/// Twin of the Python `MixedNodeBuilder` (Rust `MixedNodeBuilder`), obtained
/// from `ModelBuilder.mixed`. Methods update it in place; `build` returns the
/// `ModelBuilder` to continue with.
///
/// @example
/// ```javascript
/// import init, { statements } from "finstack-quant-wasm";
/// await init();
/// const builder = new statements.ModelBuilder("acme");
/// builder.periods("2025Q1..Q2", "2025Q1");
/// const mixed = builder.mixed("revenue");
/// mixed.values({ "2025Q1": 100 });
/// mixed.forecast(statements.forecastSpecGrowth(0.05));
/// const model = mixed.build().build();
/// new statements.Evaluator().evaluate(model).nodes.revenue["2025Q2"];  // 105
/// ```
#[wasm_bindgen(js_name = MixedNodeBuilder)]
pub struct JsMixedNodeBuilder {
    inner: Option<MixedNodeBuilder>,
}

impl JsMixedNodeBuilder {
    fn take(&mut self) -> Result<MixedNodeBuilder, JsValue> {
        self.inner
            .take()
            .ok_or_else(|| invalid("MixedNodeBuilder has already been consumed"))
    }
}

#[wasm_bindgen(js_class = MixedNodeBuilder)]
impl JsMixedNodeBuilder {
    /// Set explicit values, which take precedence over forecast and formula.
    ///
    /// Twin of Python `MixedNodeBuilder.values` (Rust `MixedNodeBuilder::values`).
    ///
    /// @param values - Object mapping period id (e.g. `"2025Q1"`) to a number or a `Money` wire object (`{amount, currency}`).
    /// @throws Error with kind `validation` if `values` is malformed or the builder was consumed.
    pub fn values(&mut self, values: JsValue) -> Result<(), JsValue> {
        let values: IndexMap<PeriodId, AmountOrScalar> = from_js_json(&values, "values")?;
        let values: Vec<(PeriodId, AmountOrScalar)> = values.into_iter().collect();
        let builder = self.take()?;
        self.inner = Some(builder.values(&values));
        Ok(())
    }

    /// Set explicit monetary values.
    ///
    /// Twin of Python `MixedNodeBuilder.values_money` (Rust
    /// `MixedNodeBuilder::values` over `Money` amounts).
    ///
    /// @param values - Object mapping period id (e.g. `"2025Q1"`) to a `Money` wire object (`{amount, currency}`).
    /// @throws Error with kind `validation` if `values` is malformed or the builder was consumed.
    #[wasm_bindgen(js_name = valuesMoney)]
    pub fn values_money(&mut self, values: JsValue) -> Result<(), JsValue> {
        let values: IndexMap<PeriodId, Money> = from_js_json(&values, "values")?;
        let values: Vec<(PeriodId, AmountOrScalar)> = values
            .into_iter()
            .map(|(period, money)| (period, AmountOrScalar::Amount(money)))
            .collect();
        let builder = self.take()?;
        self.inner = Some(builder.values(&values));
        Ok(())
    }

    /// Set the forecast used in forecast periods without an explicit value.
    ///
    /// Twin of Python `MixedNodeBuilder.forecast` (Rust
    /// `MixedNodeBuilder::forecast`).
    ///
    /// @param forecast_spec - `ForecastSpec` (`method` plus `params`), e.g. from `forecastSpecGrowth` (object or JSON).
    /// @throws Error with kind `validation` if `forecastSpec` is malformed or the builder was consumed.
    pub fn forecast(&mut self, forecast_spec: JsValue) -> Result<(), JsValue> {
        let spec: ForecastSpec = from_js_json(&forecast_spec, "forecastSpec")?;
        let builder = self.take()?;
        self.inner = Some(builder.forecast(spec));
        Ok(())
    }

    /// Set the fallback formula used when no value or forecast applies.
    ///
    /// Twin of Python `MixedNodeBuilder.formula` (Rust
    /// `MixedNodeBuilder::try_formula`). A failed call leaves the builder
    /// usable.
    ///
    /// @param formula - Statements DSL expression, e.g. `"lag(revenue, 1) * 1.05"`.
    /// @throws Error with kind `validation` if the formula is blank or does not parse and compile, or the builder was consumed.
    pub fn formula(&mut self, formula: JsValue) -> Result<(), JsValue> {
        let formula = js_string(&formula, "formula")?;
        self.inner
            .as_mut()
            .ok_or_else(|| invalid("MixedNodeBuilder has already been consumed"))?
            .try_formula(formula)
            .map_err(to_js_err)?;
        Ok(())
    }

    /// Set the human-readable node name.
    ///
    /// Twin of Python `MixedNodeBuilder.name` (Rust `MixedNodeBuilder::name`).
    ///
    /// @param name - Display name shown in reports.
    /// @throws Error with kind `validation` if the builder was consumed.
    pub fn name(&mut self, name: JsValue) -> Result<(), JsValue> {
        let name = js_string(&name, "name")?;
        let builder = self.take()?;
        self.inner = Some(builder.name(name));
        Ok(())
    }

    /// Finish the node and return the model builder.
    ///
    /// Twin of Python `MixedNodeBuilder.build` (Rust `MixedNodeBuilder::build`).
    /// This builder is consumed.
    ///
    /// @returns The `ModelBuilder` holding the finished node, ready for more nodes or `build`.
    /// @throws Error with kind `validation` if the node is invalid or the builder was consumed.
    pub fn build(&mut self) -> Result<JsModelBuilder, JsValue> {
        let builder = self.take()?;
        let ready = builder.build().map_err(to_js_err)?;
        Ok(JsModelBuilder {
            inner: Some(BuilderState::Ready(ready)),
        })
    }
}

/// Start a model builder for the given identifier.
///
/// Free-function twin of Python `FinancialModelSpec.builder` (Rust
/// `FinancialModelSpec::builder`); the same as `new ModelBuilder(id)`.
/// @param id - Model identifier stored on the built `FinancialModelSpec`.
/// @returns A `ModelBuilder` waiting for `periods`.
///
/// # Errors
///
/// Throws a `TypeError` with kind `invalid_type` if `id` is not a string.
#[wasm_bindgen(js_name = financialModelBuilder)]
pub fn financial_model_builder(id: JsValue) -> Result<JsModelBuilder, JsValue> {
    JsModelBuilder::new(id)
}
