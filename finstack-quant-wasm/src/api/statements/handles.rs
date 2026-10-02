//! Stateful statements handles: the evaluator and the metric registry.

use crate::utils::input::{from_js_json, js_string, json_text};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_statements::checks::CheckSuiteSpec;
use finstack_quant_statements::evaluator::{Evaluator, MonteCarloConfig};
use finstack_quant_statements::registry::Registry;
use finstack_quant_statements::FinancialModelSpec;
use wasm_bindgen::prelude::*;

/// Evaluator for financial statement models.
///
/// Compiles formulas, resolves dependencies and evaluates a model period by
/// period under the Value > Forecast > Formula precedence. Twin of the Python
/// `Evaluator` (Rust `Evaluator`). Results are plain objects.
///
/// @example
/// ```javascript
/// import init, { statements } from "finstack-quant-wasm";
/// await init();
/// const builder = new statements.ModelBuilder("demo");
/// builder.periods("2025Q1..Q2");
/// builder.valueScalar("revenue", { "2025Q1": 100, "2025Q2": 110 });
/// builder.compute("margin", "revenue * 0.4");
/// const evaluator = new statements.Evaluator();
/// const result = evaluator.evaluate(builder.build());
/// result.nodes.margin["2025Q2"];  // 44
/// evaluator.free();
/// ```
#[wasm_bindgen(js_name = Evaluator)]
pub struct JsEvaluator {
    pub(crate) inner: Evaluator,
}

impl Default for JsEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen(js_class = Evaluator)]
impl JsEvaluator {
    /// Create an evaluator with no check suite attached.
    ///
    /// @returns A fresh `Evaluator`.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsEvaluator {
        JsEvaluator {
            inner: Evaluator::new(),
        }
    }

    /// Attach a check suite that runs after every evaluation.
    ///
    /// Twin of Python `Evaluator.with_checks` (Rust `Evaluator::with_checks`
    /// over `CheckSuiteSpec::resolve`). The suite replaces any suite already
    /// attached, and each later result carries its report as `check_report`.
    /// The evaluator is updated in place.
    ///
    /// @param suite_spec - `CheckSuiteSpec` naming the built-in and formula checks to run (object or JSON).
    /// @throws Error with kind `validation` if `suiteSpec` is malformed or cannot be resolved.
    #[wasm_bindgen(js_name = withChecks)]
    pub fn with_checks(&mut self, suite_spec: JsValue) -> Result<(), JsValue> {
        let spec: CheckSuiteSpec = from_js_json(&suite_spec, "suiteSpec")?;
        let suite = spec.resolve().map_err(to_js_err)?;
        let evaluator = std::mem::replace(&mut self.inner, Evaluator::new());
        self.inner = evaluator.with_checks(suite);
        Ok(())
    }

    /// Evaluate a model over all of its periods.
    ///
    /// Twin of Python `Evaluator.evaluate` (Rust `Evaluator::evaluate`).
    /// Non-finite node values and warning values use the canonical strings
    /// `"nan"`, `"inf"` and `"-inf"`, so a `JSON.stringify`/parse round trip
    /// preserves missing-data semantics.
    ///
    /// @param model - `FinancialModelSpec` to evaluate (object or JSON).
    /// @returns `StatementResult` plain object: node values per period, value types, evaluation metadata and the optional check report.
    /// @throws Error with kind `validation` if the model is malformed, fails semantic validation, or a formula fails to evaluate; kind `not_found` if a formula references a missing node; kind `computation` for a dependency cycle or a capital-structure failure.
    pub fn evaluate(&mut self, model: JsValue) -> Result<JsValue, JsValue> {
        let model =
            FinancialModelSpec::from_json(&json_text(&model, "model")?).map_err(to_js_err)?;
        to_js_value(&self.inner.evaluate(&model).map_err(to_js_err)?)
    }

    /// Evaluate a model against a market context as of a valuation date.
    ///
    /// Twin of Python `Evaluator.evaluate_with_market` (Rust
    /// `Evaluator::evaluate_with_market`). Required for models with a capital
    /// structure, whose instruments are priced from the market.
    ///
    /// @param model - `FinancialModelSpec` to evaluate (object or JSON).
    /// @param market - `MarketContext` state supplying curves, quotes and FX (object or JSON).
    /// @param as_of - ISO 8601 date (`"2025-01-15"`): the pricing date for capital-structure instruments and the cutoff for explicit-value visibility. An actual whose availability date falls after `asOf` is hidden, so the node falls back to its forecast or formula, and a value-only node then fails.
    /// @returns `StatementResult` plain object, including `cs_cashflows` for capital-structure models.
    /// @throws Error with kind `validation` if an input is malformed, `asOf` is not an ISO date, or a formula fails to evaluate; kind `not_found` if a node or market datum is missing; kind `computation` for a dependency cycle or a capital-structure failure.
    #[wasm_bindgen(js_name = evaluateWithMarket)]
    pub fn evaluate_with_market(
        &mut self,
        model: JsValue,
        market: JsValue,
        as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let model =
            FinancialModelSpec::from_json(&json_text(&model, "model")?).map_err(to_js_err)?;
        let market: finstack_quant_core::market_data::context::MarketContext =
            from_js_json(&market, "market")?;
        let as_of = crate::utils::parse_iso_date(&js_string(&as_of, "asOf")?)?;
        to_js_value(
            &self
                .inner
                .evaluate_with_market(&model, &market, as_of)
                .map_err(to_js_err)?,
        )
    }

    /// Evaluate a model under Monte Carlo simulation of its stochastic forecasts.
    ///
    /// Twin of Python `Evaluator.evaluate_monte_carlo` (Rust
    /// `Evaluator::evaluate_monte_carlo`).
    ///
    /// @param model - `FinancialModelSpec` whose stochastic forecast nodes are simulated (object or JSON).
    /// @param config - `MonteCarloConfig`: `n_paths`, `seed`, optional `percentiles` (fractions in `[0, 1]`) and `include_path_data` (object or JSON).
    /// @returns `MonteCarloResults` plain object with percentile summaries per metric and, when requested, the per-path table.
    /// @throws Error with kind `validation` if an input is malformed, `n_paths` is zero, the model has a capital structure, or a path fails to evaluate; kind `not_found` if a formula references a missing node.
    #[wasm_bindgen(js_name = evaluateMonteCarlo)]
    pub fn evaluate_monte_carlo(
        &mut self,
        model: JsValue,
        config: JsValue,
    ) -> Result<JsValue, JsValue> {
        let model =
            FinancialModelSpec::from_json(&json_text(&model, "model")?).map_err(to_js_err)?;
        let config: MonteCarloConfig = from_js_json(&config, "config")?;
        to_js_value(
            &self
                .inner
                .evaluate_monte_carlo(&model, &config)
                .map_err(to_js_err)?,
        )
    }
}

/// Registry of reusable, namespaced metric definitions.
///
/// Holds metrics such as `fin.gross_margin` and resolves their dependencies
/// so a `ModelBuilder` can pull one in by its qualified identifier. Twin of
/// the Python `Registry` (Rust `Registry`).
///
/// @example
/// ```javascript
/// import init, { statements } from "finstack-quant-wasm";
/// await init();
/// const registry = statements.Registry.withBuiltins();
/// registry.has("fin.gross_margin");            // true
/// registry.get("fin.gross_margin").formula;    // the metric's formula text
/// registry.free();
/// ```
#[wasm_bindgen(js_name = Registry)]
pub struct JsRegistry {
    pub(crate) inner: Registry,
}

impl Default for JsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen(js_class = Registry)]
impl JsRegistry {
    /// Create an empty registry.
    ///
    /// @returns A registry holding no metrics.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsRegistry {
        JsRegistry {
            inner: Registry::new(),
        }
    }

    /// Create a registry pre-loaded with the built-in `fin.*` metrics.
    ///
    /// Twin of Python `Registry.with_builtins` (Rust `Registry::with_builtins`).
    ///
    /// @returns A registry holding every built-in metric.
    /// @throws Error with kind `validation` if the embedded catalog cannot be loaded.
    #[wasm_bindgen(js_name = withBuiltins)]
    pub fn with_builtins() -> Result<JsRegistry, JsValue> {
        Registry::with_builtins()
            .map(|inner| JsRegistry { inner })
            .map_err(to_js_err)
    }

    /// Load the built-in `fin.*` metrics into this registry.
    ///
    /// Twin of Python `Registry.load_builtins` (Rust `Registry::load_builtins`).
    ///
    /// @throws Error with kind `validation` if the embedded catalog cannot be loaded or a built-in identifier is already registered.
    #[wasm_bindgen(js_name = loadBuiltins)]
    pub fn load_builtins(&mut self) -> Result<(), JsValue> {
        self.inner.load_builtins().map_err(to_js_err)
    }

    /// Load one metric registry document into this registry.
    ///
    /// Twin of Python `Registry.load_from_json_str` (Rust
    /// `Registry::load_from_json_str`).
    ///
    /// @param json - `MetricRegistry` document: `namespace`, `schema_version` (`1`) and `metrics` (object or JSON text).
    /// @throws Error with kind `validation` if the document is malformed, a formula does not compile, a metric identifier is duplicated, or a metric depends on an unknown metric.
    #[wasm_bindgen(js_name = loadFromJsonStr)]
    pub fn load_from_json_str(&mut self, json: JsValue) -> Result<(), JsValue> {
        self.inner
            .load_from_json_str(&json_text(&json, "json")?)
            .map(|_| ())
            .map_err(to_js_err)
    }

    /// Whether a metric is registered.
    ///
    /// Twin of Python `Registry.has` (Rust `Registry::has`).
    ///
    /// @param qualified_id - Metric identifier as `namespace.metric`, e.g. `"fin.gross_margin"`.
    /// @returns `true` when the metric exists.
    /// @throws TypeError with kind `invalid_type` if `qualifiedId` is not a string.
    pub fn has(&self, qualified_id: JsValue) -> Result<bool, JsValue> {
        Ok(self.inner.has(&js_string(&qualified_id, "qualifiedId")?))
    }

    /// Qualified identifiers of every registered metric.
    ///
    /// Twin of Python `Registry.metric_ids` (Rust `Registry::all_metrics`).
    ///
    /// @returns `namespace.metric` identifiers in registration order.
    /// @throws Error only if the list cannot be converted to a JavaScript array.
    #[wasm_bindgen(js_name = metricIds)]
    pub fn metric_ids(&self) -> Result<JsValue, JsValue> {
        let ids: Vec<&str> = self.inner.all_metrics().map(|(id, _)| id).collect();
        to_js_value(&ids)
    }

    /// Definition of one registered metric.
    ///
    /// Twin of Python `Registry.get` (Rust `Registry::get`).
    ///
    /// @param qualified_id - Metric identifier as `namespace.metric`, e.g. `"fin.gross_margin"`.
    /// @returns `MetricDefinition` plain object: `id`, `name`, `formula` and the optional description, category, unit type, requirements and tags.
    /// @throws Error with kind `not_found` if the metric is not registered.
    pub fn get(&self, qualified_id: JsValue) -> Result<JsValue, JsValue> {
        let stored = self
            .inner
            .get(&js_string(&qualified_id, "qualifiedId")?)
            .map_err(to_js_err)?;
        to_js_value(&stored.definition)
    }

    /// Metrics one metric depends on, transitively.
    ///
    /// Twin of Python `Registry.dependencies` (Rust
    /// `Registry::get_metric_dependencies`).
    ///
    /// @param qualified_id - Metric identifier as `namespace.metric`, e.g. `"fin.gross_margin"`.
    /// @returns Qualified identifiers of the metrics to add before `qualifiedId`, in dependency order.
    /// @throws Error with kind `not_found` if the metric or one of its dependencies is not registered, and kind `validation` if the dependencies form a cycle.
    pub fn dependencies(&self, qualified_id: JsValue) -> Result<JsValue, JsValue> {
        to_js_value(
            &self
                .inner
                .get_metric_dependencies(&js_string(&qualified_id, "qualifiedId")?)
                .map_err(to_js_err)?,
        )
    }
}
