//! Stateful statements-analytics handles: the dependency tracer and the
//! corkscrew and credit-scorecard extensions.

use super::parse_model;
use crate::utils::input::{from_js_json, js_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_statements::evaluator::{DependencyGraph, StatementResult};
use finstack_quant_statements::types::FinancialModelSpec;
use finstack_quant_statements_analytics::analysis::{DependencyTracer, render_tree_detailed};
use finstack_quant_statements_analytics::extensions::{
    CorkscrewConfig, CorkscrewExtension, CreditScorecardExtension, ScorecardConfig,
};
use wasm_bindgen::prelude::*;

/// Reusable dependency tracer for a financial model.
///
/// Builds the model's dependency graph once and answers dependency questions
/// against it. Twin of the Python `DependencyTracer` (Rust `DependencyTracer`
/// over `DependencyGraph::from_model`).
///
/// @example
/// ```javascript
/// import init, { statements, statements_analytics } from "finstack-quant-wasm";
/// await init();
/// const builder = new statements.ModelBuilder("demo");
/// builder.periods("2025Q1..Q1");
/// builder.valueScalar("revenue", { "2025Q1": 100 });
/// builder.compute("profit", "revenue * 0.5");
/// const tracer = new statements_analytics.DependencyTracer(builder.build());
/// tracer.directDependencies("profit");  // ["revenue"]
/// tracer.dependents("revenue");         // ["profit"]
/// tracer.free();
/// ```
#[wasm_bindgen(js_name = DependencyTracer)]
pub struct JsDependencyTracer {
    model: FinancialModelSpec,
    graph: DependencyGraph,
}

impl JsDependencyTracer {
    fn tracer(&self) -> DependencyTracer<'_> {
        DependencyTracer::new(&self.model, &self.graph)
    }
}

#[wasm_bindgen(js_class = DependencyTracer)]
impl JsDependencyTracer {
    /// Build the dependency graph of a model.
    ///
    /// @param model - `FinancialModelSpec` to trace (object or JSON).
    /// @returns A tracer bound to the model.
    /// @throws Error with kind `validation` if the model is malformed, fails semantic validation, or a formula's dependencies cannot be parsed; kind `not_found` if a formula references an unknown node; kind `computation` for a dependency cycle.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "JsonInput")] model: JsValue,
    ) -> Result<JsDependencyTracer, JsValue> {
        let model = parse_model(&model)?;
        let graph = DependencyGraph::from_model(&model).map_err(to_js_err)?;
        Ok(JsDependencyTracer { model, graph })
    }

    /// Dependency tree of a node.
    ///
    /// Twin of Python `DependencyTracer.dependency_tree` (Rust
    /// `DependencyTracer::dependency_tree`). A dependency already on the
    /// current path appears once more as a leaf named `"<id> (cycle)"`.
    ///
    /// @param nodeId - Root node whose dependencies are traced.
    /// @returns `DependencyTree` plain object: `node_id`, `formula` (`null` for a value node) and `children`, one tree per direct dependency.
    /// @throws Error with kind `not_found` if `nodeId` or a reachable dependency is not in the model.
    #[wasm_bindgen(js_name = dependencyTree, unchecked_return_type = "DependencyTree")]
    pub fn dependency_tree(
        &self,
        #[wasm_bindgen(js_name = "nodeId", unchecked_param_type = "string")] node_id: JsValue,
    ) -> Result<JsValue, JsValue> {
        let tree = self
            .tracer()
            .dependency_tree(&js_string(&node_id, "nodeId")?)
            .map_err(to_js_err)?;
        to_js_value(&tree)
    }

    /// Dependency tree of a node as indented ASCII text.
    ///
    /// Twin of Python `DependencyTracer.dependency_tree_text` (Rust
    /// `DependencyTracer::dependency_tree_text`).
    ///
    /// @param nodeId - Root node whose dependencies are traced.
    /// @returns Multi-line tree, one node per line.
    /// @throws Error with kind `not_found` if `nodeId` or a reachable dependency is not in the model.
    #[wasm_bindgen(js_name = dependencyTreeText)]
    pub fn dependency_tree_text(
        &self,
        #[wasm_bindgen(js_name = "nodeId", unchecked_param_type = "string")] node_id: JsValue,
    ) -> Result<String, JsValue> {
        self.tracer()
            .dependency_tree_text(&js_string(&node_id, "nodeId")?)
            .map_err(to_js_err)
    }

    /// Dependency tree of a node as text, annotated with evaluated values.
    ///
    /// Twin of Python `DependencyTracer.dependency_tree_detailed` (Rust
    /// `render_tree_detailed` over `DependencyTracer::dependency_tree`).
    ///
    /// @param results - Evaluated `StatementResult` supplying the values (object or JSON).
    /// @param nodeId - Root node whose dependencies are traced.
    /// @param period - Period whose values annotate each node, e.g. `"2025Q1"`.
    /// @returns Multi-line tree with `node = value` (two decimals) on each line that has a value.
    /// @throws Error with kind `not_found` if `nodeId` or a reachable dependency is not in the model, and kind `validation` if `results` is malformed or `period` is not a valid period identifier.
    #[wasm_bindgen(js_name = dependencyTreeDetailedText)]
    pub fn dependency_tree_detailed_text(
        &self,
        #[wasm_bindgen(unchecked_param_type = "StatementResult | string")] results: JsValue,
        #[wasm_bindgen(js_name = "nodeId", unchecked_param_type = "string")] node_id: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] period: JsValue,
    ) -> Result<String, JsValue> {
        let results: StatementResult = from_js_json(&results, "results")?;
        let period: finstack_quant_core::dates::PeriodId =
            js_string(&period, "period")?.parse().map_err(to_js_err)?;
        let tree = self
            .tracer()
            .dependency_tree(&js_string(&node_id, "nodeId")?)
            .map_err(to_js_err)?;
        Ok(render_tree_detailed(&tree, &results, &period))
    }

    /// Nodes a node's formula references directly.
    ///
    /// Twin of Python `DependencyTracer.direct_dependencies` (Rust
    /// `DependencyTracer::direct_dependencies`).
    ///
    /// @param nodeId - Node to inspect.
    /// @returns Identifiers of the direct dependencies; empty for a value node.
    /// @throws Error with kind `not_found` if `nodeId` is not in the model.
    #[wasm_bindgen(js_name = directDependencies, unchecked_return_type = "string[]")]
    pub fn direct_dependencies(
        &self,
        #[wasm_bindgen(js_name = "nodeId", unchecked_param_type = "string")] node_id: JsValue,
    ) -> Result<JsValue, JsValue> {
        let tracer = self.tracer();
        let dependencies = tracer
            .direct_dependencies(&js_string(&node_id, "nodeId")?)
            .map_err(to_js_err)?;
        to_js_value(&dependencies)
    }

    /// Every node a node depends on, directly or transitively.
    ///
    /// Twin of Python `DependencyTracer.all_dependencies` (Rust
    /// `DependencyTracer::all_dependencies`).
    ///
    /// @param nodeId - Node to inspect.
    /// @returns Identifiers of all upstream nodes in dependency order (inputs before the nodes that use them).
    /// @throws Error with kind `not_found` if `nodeId` is not in the model.
    #[wasm_bindgen(js_name = allDependencies, unchecked_return_type = "string[]")]
    pub fn all_dependencies(
        &self,
        #[wasm_bindgen(js_name = "nodeId", unchecked_param_type = "string")] node_id: JsValue,
    ) -> Result<JsValue, JsValue> {
        let dependencies = self
            .tracer()
            .all_dependencies(&js_string(&node_id, "nodeId")?)
            .map_err(to_js_err)?;
        to_js_value(&dependencies)
    }

    /// Nodes whose formulas reference a node directly.
    ///
    /// Twin of Python `DependencyTracer.dependents` (Rust
    /// `DependencyTracer::dependents`).
    ///
    /// @param nodeId - Node to inspect.
    /// @returns Identifiers of the nodes that read `nodeId`.
    /// @throws Error with kind `not_found` if `nodeId` is not in the model.
    #[wasm_bindgen(unchecked_return_type = "string[]")]
    pub fn dependents(
        &self,
        #[wasm_bindgen(js_name = "nodeId", unchecked_param_type = "string")] node_id: JsValue,
    ) -> Result<JsValue, JsValue> {
        let tracer = self.tracer();
        let dependents = tracer
            .dependents(&js_string(&node_id, "nodeId")?)
            .map_err(to_js_err)?;
        to_js_value(&dependents)
    }
}

/// Corkscrew (roll-forward) validation extension.
///
/// Checks that each configured balance account rolls forward:
/// `ending = beginning + increases - decreases`, within the configured
/// tolerance. Twin of the Python `CorkscrewExtension` (Rust
/// `CorkscrewExtension`).
///
/// @example
/// ```javascript
/// import init, { statements, statements_analytics } from "finstack-quant-wasm";
/// await init();
/// const builder = new statements.ModelBuilder("demo");
/// builder.periods("2025Q1..Q2");
/// builder.valueScalar("cash", { "2025Q1": 100, "2025Q2": 120 });
/// builder.valueScalar("inflow", { "2025Q1": 0, "2025Q2": 20 });
/// const model = builder.build();
/// const results = new statements.Evaluator().evaluate(model);
/// const extension = new statements_analytics.CorkscrewExtension({
///   accounts: [{ node_id: "cash", account_type: "asset", changes: ["inflow"] }],
/// });
/// extension.execute(model, results).status;  // "success"
/// extension.free();
/// ```
#[wasm_bindgen(js_name = CorkscrewExtension)]
pub struct JsCorkscrewExtension {
    pub(crate) inner: CorkscrewExtension,
}

#[wasm_bindgen(js_class = CorkscrewExtension)]
impl JsCorkscrewExtension {
    /// Create the extension from its configuration.
    ///
    /// @param config - `CorkscrewConfig`: the `accounts` to validate, the absolute `tolerance` and `fail_on_error` (object or JSON).
    /// @returns The configured extension.
    /// @throws Error with kind `validation` if `config` is malformed.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(
            unchecked_param_type = "generated.statements_analytics.CorkscrewConfig | string"
        )]
        config: JsValue,
    ) -> Result<JsCorkscrewExtension, JsValue> {
        let config: CorkscrewConfig = from_js_json(&config, "config")?;
        Ok(JsCorkscrewExtension {
            inner: CorkscrewExtension::new(config),
        })
    }

    /// Configuration the extension was created with.
    ///
    /// Twin of Python `CorkscrewExtension.config` (Rust
    /// `CorkscrewExtension::config`).
    ///
    /// @returns `CorkscrewConfig` plain object.
    /// @throws Error only if the configuration cannot be converted to a JavaScript object.
    #[wasm_bindgen(unchecked_return_type = "generated.statements_analytics.CorkscrewConfig")]
    pub fn config(&self) -> Result<JsValue, JsValue> {
        to_js_value(self.inner.config())
    }

    /// Validate the configured accounts against evaluated results.
    ///
    /// Twin of Python `CorkscrewExtension.execute` (Rust
    /// `CorkscrewExtension::execute`).
    ///
    /// @param model - `FinancialModelSpec` that produced the results (object or JSON).
    /// @param results - Evaluated `StatementResult` (object or JSON).
    /// @returns `CorkscrewReport`: `status`, `message`, per-account `data`, `warnings` and `errors`.
    /// @throws Error with kind `validation` if an input is malformed, or strict mode (`fail_on_error`) meets a missing or invalid account, change node or period value.
    #[wasm_bindgen(unchecked_return_type = "generated.statements_analytics.CorkscrewReport")]
    pub fn execute(
        &self,
        #[wasm_bindgen(unchecked_param_type = "JsonInput")] model: JsValue,
        #[wasm_bindgen(unchecked_param_type = "StatementResult | string")] results: JsValue,
    ) -> Result<JsValue, JsValue> {
        let model = parse_model(&model)?;
        let results: StatementResult = from_js_json(&results, "results")?;
        to_js_value(&self.inner.execute(&model, &results).map_err(to_js_err)?)
    }
}

/// Credit scorecard extension.
///
/// Scores configured metrics against rating thresholds and combines them
/// into a weighted rating. Twin of the Python `CreditScorecardExtension`
/// (Rust `CreditScorecardExtension`).
///
/// @example
/// ```javascript
/// import init, { statements, statements_analytics } from "finstack-quant-wasm";
/// await init();
/// const builder = new statements.ModelBuilder("demo");
/// builder.periods("2025Q1..Q1");
/// builder.valueScalar("debt", { "2025Q1": 300 });
/// builder.valueScalar("ebitda", { "2025Q1": 100 });
/// const model = builder.build();
/// const results = new statements.Evaluator().evaluate(model);
/// const scale = core.scorecardScaleTryNew("Internal", undefined, [
///   core.ratingLevelTryNew("AAA", 95, 90),
///   core.ratingLevelTryNew("BBB", 60, 0),
/// ]);
/// const registry = new core.RatingScaleRegistry([
///   core.ratingScaleEntryTryNew("internal", "credit policy 2026", scale),
/// ]);
/// const extension = new statements_analytics.CreditScorecardExtension(
///   {
///     rating_scale: "internal",
///     metrics: [{ name: "leverage", formula: "debt / ebitda", weight: 1.0, thresholds: { AAA: [0, 1], BBB: [1, 4] } }],
///   },
///   registry,
/// );
/// extension.execute(model, results).status;
/// extension.free();
/// ```
#[wasm_bindgen(js_name = CreditScorecardExtension)]
pub struct JsCreditScorecardExtension {
    pub(crate) inner: CreditScorecardExtension,
}

#[wasm_bindgen(js_class = CreditScorecardExtension)]
impl JsCreditScorecardExtension {
    /// Create the extension on a scale of the supplied registry; the selected
    /// entry is resolved once and kept as a snapshot.
    ///
    /// @param config - `ScorecardConfig`: `rating_scale` (an id in `registry`), the weighted `metrics` with their rating thresholds, and the optional `min_rating` and `period` (object or JSON).
    /// @param registry - Caller-supplied `RatingScaleRegistry` holding `config.rating_scale`.
    /// @returns The configured extension.
    /// @throws Error with kind `validation` if `config` is malformed, names a scale the registry does not hold, or uses a threshold or minimum-rating label the selected scale does not hold or repeats.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(
            unchecked_param_type = "generated.statements_analytics.ScorecardConfig | string"
        )]
        config: JsValue,
        registry: &crate::api::core::rating_scales::JsRatingScaleRegistry,
    ) -> Result<JsCreditScorecardExtension, JsValue> {
        let config: ScorecardConfig = from_js_json(&config, "config")?;
        CreditScorecardExtension::new(config, &registry.inner)
            .map(|inner| JsCreditScorecardExtension { inner })
            .map_err(to_js_err)
    }

    /// Snapshot of the selected registry entry.
    ///
    /// Twin of Python `CreditScorecardExtension.rating_scale` (Rust
    /// `CreditScorecardExtension::rating_scale`).
    ///
    /// @returns The `{ id, source, scale }` entry the extension was built on.
    /// @throws Error only if the entry cannot be converted to a JavaScript object.
    #[wasm_bindgen(js_name = ratingScale, unchecked_return_type = "RatingScaleEntry")]
    pub fn rating_scale(&self) -> Result<JsValue, JsValue> {
        to_js_value(self.inner.rating_scale())
    }

    /// Configuration the extension was created with.
    ///
    /// Twin of Python `CreditScorecardExtension.config` (Rust
    /// `CreditScorecardExtension::config`).
    ///
    /// @returns `ScorecardConfig` plain object.
    /// @throws Error only if the configuration cannot be converted to a JavaScript object.
    #[wasm_bindgen(unchecked_return_type = "generated.statements_analytics.ScorecardConfig")]
    pub fn config(&self) -> Result<JsValue, JsValue> {
        to_js_value(self.inner.config())
    }

    /// Score the configured metrics against evaluated results.
    ///
    /// Twin of Python `CreditScorecardExtension.execute` (Rust
    /// `CreditScorecardExtension::execute`).
    ///
    /// @param model - `FinancialModelSpec` that produced the results (object or JSON).
    /// @param results - Evaluated `StatementResult` (object or JSON).
    /// @returns `ScorecardReport`: `status`, `message`, the rating and per-metric scores in `data`, `warnings` and `errors` (per-metric evaluation failures are reported there, not thrown).
    /// @throws Error with kind `validation` if an input is malformed, the configuration is invalid, or the target period is missing.
    #[wasm_bindgen(unchecked_return_type = "generated.statements_analytics.ScorecardReport")]
    pub fn execute(
        &self,
        #[wasm_bindgen(unchecked_param_type = "JsonInput")] model: JsValue,
        #[wasm_bindgen(unchecked_param_type = "StatementResult | string")] results: JsValue,
    ) -> Result<JsValue, JsValue> {
        let model = parse_model(&model)?;
        let results: StatementResult = from_js_json(&results, "results")?;
        to_js_value(&self.inner.execute(&model, &results).map_err(to_js_err)?)
    }
}

/// Validate a scorecard configuration.
///
/// Twin of Python `ScorecardConfig.validate` (Rust
/// `CreditScorecardExtension::validate_config`). Returns `undefined` when the
/// configuration is valid.
/// @param config - `ScorecardConfig` to check (object or JSON).
/// @param registry - Caller-supplied `RatingScaleRegistry` holding `config.rating_scale`.
///
/// # Errors
///
/// Throws with kind `validation` if `config` is malformed, the registry has no
/// such rating scale, a threshold or minimum-rating label is unknown to the
/// selected scale or repeats, a metric weight is negative or non-finite, the
/// total weight is implausible, or the explicit period is not a valid period
/// identifier.
#[wasm_bindgen(js_name = validateScorecardConfig)]
pub fn validate_scorecard_config(
    #[wasm_bindgen(
        unchecked_param_type = "generated.statements_analytics.ScorecardConfig | string"
    )]
    config: JsValue,
    registry: &crate::api::core::rating_scales::JsRatingScaleRegistry,
) -> Result<(), JsValue> {
    let config: ScorecardConfig = from_js_json(&config, "config")?;
    CreditScorecardExtension::validate_config(&config, &registry.inner).map_err(to_js_err)
}
