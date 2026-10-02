//! Stateful statements-analytics handles: the dependency tracer and the
//! corkscrew and credit-scorecard extensions.

use crate::utils::input::{from_js_json, js_string, json_text};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_statements::evaluator::{DependencyGraph, StatementResult};
use finstack_quant_statements::FinancialModelSpec;
use finstack_quant_statements_analytics::analysis::{render_tree_detailed, DependencyTracer};
use finstack_quant_statements_analytics::extensions::{
    CorkscrewConfig, CorkscrewExtension, CreditScorecardExtension, ScorecardConfig,
};
use wasm_bindgen::prelude::*;

fn parse_model(model: &JsValue) -> Result<FinancialModelSpec, JsValue> {
    FinancialModelSpec::from_json(&json_text(model, "model")?).map_err(to_js_err)
}

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
    pub fn new(model: JsValue) -> Result<JsDependencyTracer, JsValue> {
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
    /// @param node_id - Root node whose dependencies are traced.
    /// @returns `DependencyTree` plain object: `node_id`, `formula` (`null` for a value node) and `children`, one tree per direct dependency.
    /// @throws Error with kind `not_found` if `nodeId` or a reachable dependency is not in the model.
    #[wasm_bindgen(js_name = dependencyTree)]
    pub fn dependency_tree(&self, node_id: JsValue) -> Result<JsValue, JsValue> {
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
    /// @param node_id - Root node whose dependencies are traced.
    /// @returns Multi-line tree, one node per line.
    /// @throws Error with kind `not_found` if `nodeId` or a reachable dependency is not in the model.
    #[wasm_bindgen(js_name = dependencyTreeText)]
    pub fn dependency_tree_text(&self, node_id: JsValue) -> Result<String, JsValue> {
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
    /// @param node_id - Root node whose dependencies are traced.
    /// @param period - Period whose values annotate each node, e.g. `"2025Q1"`.
    /// @returns Multi-line tree with `node = value` (two decimals) on each line that has a value.
    /// @throws Error with kind `not_found` if `nodeId` or a reachable dependency is not in the model, and kind `validation` if `results` is malformed or `period` is not a valid period identifier.
    #[wasm_bindgen(js_name = dependencyTreeDetailedText)]
    pub fn dependency_tree_detailed_text(
        &self,
        results: JsValue,
        node_id: JsValue,
        period: JsValue,
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
    /// @param node_id - Node to inspect.
    /// @returns Identifiers of the direct dependencies; empty for a value node.
    /// @throws Error with kind `not_found` if `nodeId` is not in the model.
    #[wasm_bindgen(js_name = directDependencies)]
    pub fn direct_dependencies(&self, node_id: JsValue) -> Result<JsValue, JsValue> {
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
    /// @param node_id - Node to inspect.
    /// @returns Identifiers of all upstream nodes in dependency order (inputs before the nodes that use them).
    /// @throws Error with kind `not_found` if `nodeId` is not in the model.
    #[wasm_bindgen(js_name = allDependencies)]
    pub fn all_dependencies(&self, node_id: JsValue) -> Result<JsValue, JsValue> {
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
    /// @param node_id - Node to inspect.
    /// @returns Identifiers of the nodes that read `nodeId`.
    /// @throws Error with kind `not_found` if `nodeId` is not in the model.
    pub fn dependents(&self, node_id: JsValue) -> Result<JsValue, JsValue> {
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
    pub fn new(config: JsValue) -> Result<JsCorkscrewExtension, JsValue> {
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
    pub fn execute(&mut self, model: JsValue, results: JsValue) -> Result<JsValue, JsValue> {
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
/// const extension = new statements_analytics.CreditScorecardExtension({
///   rating_scale: "S&P",
///   metrics: [{ name: "leverage", formula: "debt / ebitda", weight: 1.0, thresholds: { AAA: [0, 1], BBB: [1, 4] } }],
/// });
/// extension.execute(model, results).status;
/// extension.free();
/// ```
#[wasm_bindgen(js_name = CreditScorecardExtension)]
pub struct JsCreditScorecardExtension {
    pub(crate) inner: CreditScorecardExtension,
}

#[wasm_bindgen(js_class = CreditScorecardExtension)]
impl JsCreditScorecardExtension {
    /// Create the extension from its configuration.
    ///
    /// @param config - `ScorecardConfig`: `rating_scale`, the weighted `metrics` with their rating thresholds, and the optional `min_rating` and `period` (object or JSON).
    /// @returns The configured extension.
    /// @throws Error with kind `validation` if `config` is malformed.
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<JsCreditScorecardExtension, JsValue> {
        let config: ScorecardConfig = from_js_json(&config, "config")?;
        Ok(JsCreditScorecardExtension {
            inner: CreditScorecardExtension::new(config),
        })
    }

    /// Configuration the extension was created with.
    ///
    /// Twin of Python `CreditScorecardExtension.config` (Rust
    /// `CreditScorecardExtension::config`).
    ///
    /// @returns `ScorecardConfig` plain object.
    /// @throws Error only if the configuration cannot be converted to a JavaScript object.
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
    pub fn execute(&mut self, model: JsValue, results: JsValue) -> Result<JsValue, JsValue> {
        let model = parse_model(&model)?;
        let results: StatementResult = from_js_json(&results, "results")?;
        to_js_value(&self.inner.execute(&model, &results).map_err(to_js_err)?)
    }
}

/// Validate a scorecard configuration.
///
/// Twin of Python `validate_scorecard_config` and `ScorecardConfig.validate`
/// (Rust `CreditScorecardExtension::validate_config`). Returns `undefined`
/// when the configuration is valid.
/// @param config - `ScorecardConfig` to check (object or JSON).
///
/// # Errors
///
/// Throws with kind `validation` if `config` is malformed, the rating scale is
/// unsupported, a metric weight is negative or non-finite, the total weight is
/// implausible, a threshold range is invalid, or the explicit period is not a
/// valid period identifier.
#[wasm_bindgen(js_name = validateScorecardConfig)]
pub fn validate_scorecard_config(config: JsValue) -> Result<(), JsValue> {
    let config: ScorecardConfig = from_js_json(&config, "config")?;
    CreditScorecardExtension::validate_config(&config).map_err(to_js_err)
}
