//! Dependency tracing and formula explanation Python bindings.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::extract::{extract_model_ref, extract_results_ref};
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::{tabular_methods, wire_methods};
use crate::bindings::statements::parse_period_id;
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::{core_to_py, statements_to_py};
use finstack_quant_statements_analytics::analysis::{DependencyTree, Explanation, ExplanationStep};
use pyo3::prelude::*;

/// Column schema for [`PyExplanation`] tabular exports.
const EXPLANATION_COLUMNS: [(&str, &str); 3] = [
    ("component", "str"),
    ("value", "float64"),
    ("operation", "str"),
];

/// Cached dependency tracer that builds the model graph once.
///
/// Construct from a ``FinancialModelSpec`` (or JSON string) and reuse for
/// multiple introspection queries without rebuilding the dependency graph.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import DependencyTracer
/// >>> b = ModelBuilder("m"); b.periods("2025Q1..Q1", None); b.value("revenue", [("2025Q1", 100.0)])
/// >>> b.compute("profit", "revenue * 0.5")
/// >>> DependencyTracer(b.build()).direct_dependencies("profit")
/// ['revenue']
#[pyclass(
    name = "DependencyTracer",
    module = "finstack_quant.statements_analytics",
    skip_from_py_object
)]
pub(super) struct PyDependencyTracer {
    model: finstack_quant_statements::types::FinancialModelSpec,
    graph: finstack_quant_statements::evaluator::DependencyGraph,
}

#[pymethods]
impl PyDependencyTracer {
    /// Build a tracer from a model (typed object or JSON string).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the model JSON is malformed or the graph contains a cycle.
    #[new]
    fn new(model: &Bound<'_, PyAny>) -> PyResult<Self> {
        let model = extract_model_ref(model)?.into_owned();
        let graph = finstack_quant_statements::evaluator::DependencyGraph::from_model(&model)
            .map_err(statements_to_py)?;
        Ok(Self { model, graph })
    }

    /// Dependency tree for a node (the Rust ``DependencyTracer::dependency_tree``).
    ///
    /// Returns
    /// -------
    /// DependencyTree
    ///     Typed tree with ``node_id``, ``formula`` and ``children``; its
    ///     ``to_json()`` matches the WASM ``DependencyTracer.dependencyTree`` result.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``node_id`` or a reachable dependency is not in the model.
    fn dependency_tree(&self, node_id: &str) -> PyResult<PyDependencyTree> {
        finstack_quant_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        )
        .dependency_tree(node_id)
        .map(|inner| PyDependencyTree { inner })
        .map_err(statements_to_py)
    }

    /// ASCII rendering of a node's dependency tree (the Rust
    /// ``DependencyTracer::dependency_tree_text``).
    ///
    /// Returns
    /// -------
    /// str
    ///     Root on the first line, then one line per dependency drawn with
    ///     ``├──`` / ``└──`` connectors and indented by depth; identical to the
    ///     WASM ``DependencyTracer.dependencyTreeText``.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``node_id`` or a reachable dependency is not in the model.
    fn dependency_tree_text(&self, node_id: &str) -> PyResult<String> {
        finstack_quant_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        )
        .dependency_tree_text(node_id)
        .map_err(statements_to_py)
    }

    /// ASCII tree with node values for a given period.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``node_id`` is not in the model.
    /// ValueError
    ///     If ``period`` does not parse.
    fn dependency_tree_detailed(
        &self,
        results: &Bound<'_, PyAny>,
        node_id: &str,
        period: &str,
    ) -> PyResult<String> {
        let results = extract_results_ref(results)?;
        let pid = parse_period_id(period)?;
        let tracer = finstack_quant_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        );
        let tree = tracer.dependency_tree(node_id).map_err(statements_to_py)?;
        Ok(
            finstack_quant_statements_analytics::analysis::render_tree_detailed(
                &tree, &results, &pid,
            ),
        )
    }

    /// Direct dependency node IDs.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``node_id`` is not in the model.
    fn direct_dependencies(&self, node_id: &str) -> PyResult<Vec<String>> {
        let tracer = finstack_quant_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        );
        let deps = tracer
            .direct_dependencies(node_id)
            .map_err(statements_to_py)?;
        Ok(deps.into_iter().map(String::from).collect())
    }

    /// All transitive dependency node IDs in dependency order.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``node_id`` is not in the model.
    fn all_dependencies(&self, node_id: &str) -> PyResult<Vec<String>> {
        let tracer = finstack_quant_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        );
        tracer.all_dependencies(node_id).map_err(statements_to_py)
    }

    /// Node IDs that depend on this node.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``node_id`` is not in the model.
    fn dependents(&self, node_id: &str) -> PyResult<Vec<String>> {
        let tracer = finstack_quant_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        );
        let deps = tracer.dependents(node_id).map_err(statements_to_py)?;
        Ok(deps.into_iter().map(String::from).collect())
    }

    fn __repr__(&self) -> String {
        format!("DependencyTracer(nodes={})", self.model.nodes.len())
    }
}

/// Dependency tree of one statement node (the Rust ``DependencyTree``).
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import DependencyTree
/// >>> tree = DependencyTree.from_json(
/// ...     '{"node_id":"profit","formula":"revenue * 0.5","children":'
/// ...     '[{"node_id":"revenue","formula":null,"children":[]}]}')
/// >>> tree.node_id, tree.formula, [child.node_id for child in tree.children]
/// ('profit', 'revenue * 0.5', ['revenue'])
#[pyclass(
    name = "DependencyTree",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyDependencyTree {
    pub(crate) inner: DependencyTree,
}

#[pymethods]
impl PyDependencyTree {
    /// Node identifier; a dependency already on the current path appears as
    /// a leaf named ``"<id> (cycle)"``.
    #[getter]
    fn node_id(&self) -> &str {
        &self.inner.node_id
    }

    /// Formula text when the node is calculated, ``None`` for a value node.
    #[getter]
    fn formula(&self) -> Option<&str> {
        self.inner.formula.as_deref()
    }

    /// One tree per direct dependency, in formula reference order.
    #[getter]
    fn children(&self) -> Vec<PyDependencyTree> {
        self.inner
            .children
            .iter()
            .cloned()
            .map(|inner| PyDependencyTree { inner })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "DependencyTree(node_id={:?}, children={})",
            self.inner.node_id,
            self.inner.children.len()
        )
    }
}

wire_methods!(PyDependencyTree, DependencyTree, "DependencyTree");

/// One component of a formula explanation.
#[pyclass(
    name = "ExplanationStep",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyExplanationStep {
    pub(crate) inner: ExplanationStep,
}

#[pymethods]
impl PyExplanationStep {
    /// Component node id or literal text.
    #[getter]
    fn component(&self) -> &str {
        &self.inner.component
    }

    /// Component value at the explained period.
    #[getter]
    fn value(&self) -> f64 {
        self.inner.value
    }

    /// Sign with which the component enters the formula (``"+"`` or ``"-"``)
    /// when the formula is a pure sum/difference of references, else ``None``.
    #[getter]
    fn operation(&self) -> Option<&str> {
        self.inner.operation.as_deref()
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ExplanationStep", &self.inner)
    }
}

/// Explanation of how a node's value was derived at one period.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import Explanation
/// >>> e = Explanation.from_json('{"node_id":"profit","period_id":"2025Q1","final_value":50.0,'
/// ...     '"node_type":"calculated","formula_text":"revenue * 0.5","breakdown":[]}')
/// >>> e.node_id, e.final_value
/// ('profit', 50.0)
#[pyclass(
    name = "Explanation",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyExplanation {
    pub(crate) inner: Explanation,
}

#[pymethods]
impl PyExplanation {
    /// Explained node id.
    #[getter]
    fn node_id(&self) -> &str {
        &self.inner.node_id
    }

    /// Period-id string of the explanation.
    #[getter]
    fn period_id(&self) -> String {
        self.inner.period_id.to_string()
    }

    /// Node value at the period.
    #[getter]
    fn final_value(&self) -> f64 {
        self.inner.final_value
    }

    /// Node type serde name (e.g. ``"calculated"``, ``"input"``).
    #[getter]
    fn node_type(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner.node_type).map_err(core_to_py)
    }

    /// Value source selected for the cell: ``"value"``, ``"forecast"`` or
    /// ``"formula"``; ``None`` for a masked or missing cell and when the
    /// results carry no cell source.
    #[getter]
    fn source(&self) -> Option<&'static str> {
        self.inner.source.map(|source| source.as_str())
    }

    /// The node's forecast spec as ``{"method": ..., "params": {...}}`` when
    /// ``source`` is ``"forecast"``, else ``None``.
    #[getter]
    fn forecast<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .forecast
            .as_ref()
            .map(|forecast| serde_to_py(py, forecast))
            .transpose()
    }

    /// Formula text, or ``None`` for non-formula nodes.
    #[getter]
    fn formula_text(&self) -> Option<&str> {
        self.inner.formula_text.as_deref()
    }

    /// Formula components with the values they resolved to (formula order);
    /// empty when the value did not come from the formula.
    #[getter]
    fn breakdown(&self) -> Vec<PyExplanationStep> {
        self.inner
            .breakdown
            .iter()
            .cloned()
            .map(|inner| PyExplanationStep { inner })
            .collect()
    }

    /// Human-readable multi-line explanation.
    fn to_text(&self) -> String {
        self.inner.to_text()
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("Explanation", &self.inner)
    }
}

wire_methods!(PyExplanation, Explanation, "Explanation");

impl PyExplanation {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .breakdown
            .iter()
            .map(|step| {
                serde_json::json!({
                    "component": step.component,
                    "value": step.value,
                    "operation": step.operation,
                })
            })
            .collect();
        serde_rows_to_arrow_table(&rows, &EXPLANATION_COLUMNS)
    }
}

tabular_methods!(PyExplanation);

/// Explain a formula for a specific node and period.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string.
/// results : StatementResult | str
///     A ``StatementResult`` object or a JSON string.
/// node_id : str
///     Node whose formula to explain.
/// period : str
///     Period string.
///
/// Returns
/// -------
/// Explanation
///     Typed explanation with the cell ``source``, the ``forecast`` spec of a
///     forecast cell, ``breakdown`` steps, ``to_text()``, ``to_arrow()``,
///     and ``to_pandas()``; ``to_json()`` matches the WASM ``explainFormula``.
///
/// Raises
/// ------
/// KeyError
///     If ``node_id`` is not in the model or has no value at ``period``.
/// ValueError
///     If ``period`` does not parse or a payload is malformed JSON.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder, Evaluator
/// >>> from finstack_quant.statements_analytics import explain_formula
/// >>> b = ModelBuilder("m"); b.periods("2025Q1..Q1", None); b.value("revenue", [("2025Q1", 100.0)])
/// >>> b.compute("profit", "revenue * 0.5")
/// >>> model = b.build()
/// >>> explain_formula(model, Evaluator().evaluate(model), "profit", "2025Q1").final_value
/// 50.0
#[pyfunction]
pub(super) fn explain_formula(
    model: &Bound<'_, PyAny>,
    results: &Bound<'_, PyAny>,
    node_id: &str,
    period: &str,
) -> PyResult<PyExplanation> {
    let model = extract_model_ref(model)?;
    let results = extract_results_ref(results)?;
    let pid = parse_period_id(period)?;

    let inner = finstack_quant_statements_analytics::analysis::explain_formula(
        &model, &results, node_id, &pid,
    )
    .map_err(statements_to_py)?;
    Ok(PyExplanation { inner })
}
