//! Model introspection: dependency tracing, formula explanation, and tree visualization.

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::error::{Error, Result};
use finstack_quant_statements::evaluator::{DependencyGraph, StatementResult};
use finstack_quant_statements::types::{FinancialModelSpec, NodeType};
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};

// Dependency tracing

/// Traces dependencies between nodes in a financial model.
///
/// The tracer uses the dependency graph to identify which nodes a given node
/// depends on (direct and transitive) and which nodes depend on it.
///
/// # Examples
///
/// ```rust
/// # use finstack_quant_statements::builder::ModelBuilder;
/// # use finstack_quant_statements::evaluator::DependencyGraph;
/// # use finstack_quant_statements_analytics::analysis::DependencyTracer;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let model = ModelBuilder::new("demo")
///     .periods("2025Q1..Q2", None)?
///     .compute("revenue", "100000")?
///     .compute("cogs", "revenue * 0.4")?
///     .compute("gross_profit", "revenue - cogs")?
///     .build()?;
///
/// let graph = DependencyGraph::from_model(&model)?;
/// let tracer = DependencyTracer::new(&model, &graph);
///
/// // Get direct dependencies
/// let deps = tracer.direct_dependencies("gross_profit")?;
/// assert_eq!(deps.len(), 2);
/// assert!(deps.contains(&"revenue"));
/// assert!(deps.contains(&"cogs"));
/// # Ok(())
/// # }
/// ```
pub struct DependencyTracer<'a> {
    model: &'a FinancialModelSpec,
    graph: &'a DependencyGraph,
}

impl<'a> DependencyTracer<'a> {
    /// Create a new dependency tracer.
    ///
    /// # Arguments
    ///
    /// * `model` - Financial model specification
    /// * `graph` - Pre-built dependency graph
    pub fn new(model: &'a FinancialModelSpec, graph: &'a DependencyGraph) -> Self {
        Self { model, graph }
    }

    /// Get all direct dependencies for a node.
    ///
    /// Returns the nodes that this node directly references in its formula.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Node identifier to inspect
    ///
    /// # Returns
    ///
    /// Vector of node IDs that are direct dependencies
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error when `node_id` is absent from the
    /// dependency graph.
    pub fn direct_dependencies(&self, node_id: &str) -> Result<Vec<&str>> {
        let deps = self
            .graph
            .get_dependencies(node_id)
            .ok_or_else(|| Error::node_not_found(node_id))?;

        Ok(deps.iter().map(|s| s.as_str()).collect())
    }

    /// Get all transitive dependencies (recursive).
    ///
    /// Returns all nodes that this node depends on, directly or indirectly.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Node identifier to inspect
    ///
    /// # Returns
    ///
    /// Vector of node IDs in dependency order (dependencies before dependents)
    ///
    /// # Errors
    ///
    /// Returns an error when `node_id` or a traversed dependency is absent, or
    /// when the graph contains a dependency cycle.
    pub fn all_dependencies(&self, node_id: &str) -> Result<Vec<String>> {
        let mut all_deps = IndexSet::new();
        let mut visited = IndexSet::new();
        self.collect_transitive_deps_owned(node_id, &mut all_deps, &mut visited)?;
        Ok(all_deps.into_iter().collect())
    }

    /// Get dependency tree as hierarchical structure.
    ///
    /// Builds a tree showing the complete dependency hierarchy for a node.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Root node for the dependency tree
    ///
    /// # Returns
    ///
    /// Dependency tree structure suitable for visualization
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_statements::evaluator::DependencyGraph;
    /// # use finstack_quant_statements_analytics::analysis::DependencyTracer;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let model = ModelBuilder::new("demo")
    /// #     .periods("2025Q1..Q2", None)?
    /// #     .compute("a", "10")?
    /// #     .compute("b", "a * 2")?
    /// #     .compute("c", "a + b")?
    /// #     .build()?;
    /// # let graph = DependencyGraph::from_model(&model)?;
    /// let tracer = DependencyTracer::new(&model, &graph);
    /// let tree = tracer.dependency_tree("c")?;
    ///
    /// assert_eq!(tree.node_id, "c");
    /// assert_eq!(tree.children.len(), 2);
    /// assert_eq!(tree.depth(), 2);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when `node_id` or a reachable dependency is absent, or
    /// when a cycle prevents construction of a finite tree.
    pub fn dependency_tree(&self, node_id: &str) -> Result<DependencyTree> {
        let mut visited = IndexSet::new();
        self.build_tree(node_id, &mut visited)
    }

    /// Render a node's dependency tree as ASCII text.
    ///
    /// Equivalent to [`render_tree_ascii`] applied to [`Self::dependency_tree`]:
    /// the root on the first line, then one line per dependency drawn with
    /// `├──` / `└──` connectors and indented by depth, each followed by its
    /// formula in parentheses when it has one.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Root node for the dependency tree
    ///
    /// # Returns
    ///
    /// Multi-line ASCII tree, one node per line, ending with a newline.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_statements::evaluator::DependencyGraph;
    /// # use finstack_quant_statements_analytics::analysis::DependencyTracer;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let model = ModelBuilder::new("demo")
    /// #     .periods("2025Q1..Q2", None)?
    /// #     .compute("a", "10")?
    /// #     .compute("b", "a * 2")?
    /// #     .build()?;
    /// # let graph = DependencyGraph::from_model(&model)?;
    /// let tracer = DependencyTracer::new(&model, &graph);
    /// assert_eq!(tracer.dependency_tree_text("b")?, "b (a * 2)\n└── a (10)\n");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when `node_id` or a reachable dependency is absent, or
    /// when a cycle prevents construction of a finite tree.
    pub fn dependency_tree_text(&self, node_id: &str) -> Result<String> {
        Ok(render_tree_ascii(&self.dependency_tree(node_id)?))
    }

    /// Get nodes that depend on this node (reverse dependencies).
    ///
    /// # Arguments
    ///
    /// * `node_id` - Node identifier to inspect
    ///
    /// # Returns
    ///
    /// Vector of node IDs that depend on this node
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error when `node_id` is absent from the graph.
    pub fn dependents(&self, node_id: &str) -> Result<Vec<&str>> {
        let deps = self
            .graph
            .dependents
            .get(node_id)
            .ok_or_else(|| Error::node_not_found(node_id))?;

        Ok(deps.iter().map(|s| s.as_str()).collect())
    }

    fn collect_transitive_deps_owned(
        &self,
        node_id: &str,
        all_deps: &mut IndexSet<String>,
        visited: &mut IndexSet<String>,
    ) -> Result<()> {
        if visited.contains(node_id) {
            return Ok(());
        }
        visited.insert(node_id.to_string());

        let direct_deps = self.direct_dependencies(node_id)?;

        for dep_id in direct_deps {
            self.collect_transitive_deps_owned(dep_id, all_deps, visited)?;
            all_deps.insert(dep_id.to_string());
        }

        Ok(())
    }

    fn build_tree(&self, node_id: &str, visited: &mut IndexSet<String>) -> Result<DependencyTree> {
        let node_spec = self
            .model
            .nodes
            .get(node_id)
            .ok_or_else(|| Error::node_not_found(node_id))?;

        let formula = node_spec.formula_text.clone();
        let deps = self.direct_dependencies(node_id)?;

        let mut children = Vec::new();
        for dep_id in deps {
            if visited.contains(dep_id) {
                children.push(DependencyTree {
                    node_id: format!("{} (cycle)", dep_id),
                    formula: None,
                    children: Vec::new(),
                });
            } else {
                visited.insert(dep_id.to_string());
                children.push(self.build_tree(dep_id, visited)?);
                visited.shift_remove(dep_id);
            }
        }

        Ok(DependencyTree {
            node_id: node_id.to_string(),
            formula,
            children,
        })
    }
}

/// Hierarchical dependency tree structure.
///
/// Represents the complete dependency hierarchy for a node, suitable for
/// visualization and analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DependencyTree {
    /// Node identifier
    pub node_id: String,

    /// Formula text (if node is calculated)
    pub formula: Option<String>,

    /// Child dependencies
    pub children: Vec<DependencyTree>,
}

impl DependencyTree {
    /// Get the maximum depth of the tree.
    ///
    /// # Returns
    ///
    /// Maximum depth (0 for a leaf node, 1 for a node with children, etc.)
    pub fn depth(&self) -> usize {
        if self.children.is_empty() {
            0
        } else {
            1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
        }
    }

    /// Count total number of nodes in the tree.
    ///
    /// # Returns
    ///
    /// Total node count including this node and all descendants
    pub fn node_count(&self) -> usize {
        1 + self.children.iter().map(|c| c.node_count()).sum::<usize>()
    }
}

// Tree visualization

/// Render dependency tree as ASCII art.
///
/// # Arguments
///
/// * `tree` - Dependency tree to render
///
/// # Returns
///
/// ASCII representation suitable for console output
///
/// # Examples
///
/// ```rust
/// # use finstack_quant_statements::builder::ModelBuilder;
/// # use finstack_quant_statements::evaluator::DependencyGraph;
/// # use finstack_quant_statements_analytics::analysis::{DependencyTracer, render_tree_ascii};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let model = ModelBuilder::new("demo")
/// #     .periods("2025Q1..Q2", None)?
/// #     .compute("revenue", "100000")?
/// #     .compute("cogs", "revenue * 0.4")?
/// #     .compute("gross_profit", "revenue - cogs")?
/// #     .build()?;
/// # let graph = DependencyGraph::from_model(&model)?;
/// let tracer = DependencyTracer::new(&model, &graph);
/// let tree = tracer.dependency_tree("gross_profit")?;
///
/// let ascii = render_tree_ascii(&tree);
/// assert_eq!(
///     ascii,
///     "gross_profit (revenue - cogs)\n\
///      ├── revenue (100000)\n\
///      └── cogs (revenue * 0.4)\n    \
///      └── revenue (100000)\n"
/// );
/// # Ok(())
/// # }
/// ```
pub fn render_tree_ascii(tree: &DependencyTree) -> String {
    let mut output = String::new();
    render_tree_lines(tree, &mut output, "", None, &|node| match &node.formula {
        Some(formula) => format!("{} ({})", node.node_id, formula),
        None => node.node_id.clone(),
    });
    output
}

/// Render dependency tree with values from results.
///
/// # Arguments
///
/// * `tree` - Dependency tree to render
/// * `results` - Evaluation results containing node values
/// * `period` - Period to display values for
///
/// # Returns
///
/// ASCII tree with values
///
/// # Examples
///
/// ```rust
/// # use finstack_quant_statements::builder::ModelBuilder;
/// # use finstack_quant_statements::evaluator::{DependencyGraph, Evaluator};
/// # use finstack_quant_statements_analytics::analysis::{DependencyTracer, render_tree_detailed};
/// # use finstack_quant_core::dates::PeriodId;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let model = ModelBuilder::new("demo")
/// #     .periods("2025Q1..Q2", None)?
/// #     .compute("revenue", "100000")?
/// #     .compute("cogs", "revenue * 0.4")?
/// #     .compute("gross_profit", "revenue - cogs")?
/// #     .build()?;
/// # let mut evaluator = Evaluator::new();
/// # let results = evaluator.evaluate(&model)?;
/// # let graph = DependencyGraph::from_model(&model)?;
/// let tracer = DependencyTracer::new(&model, &graph);
/// let tree = tracer.dependency_tree("gross_profit")?;
///
/// let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
/// let detailed = render_tree_detailed(&tree, &results, &period);
/// assert_eq!(
///     detailed,
///     "gross_profit = 60000.00\n\
///      ├── revenue = 100000.00\n\
///      └── cogs = 40000.00\n    \
///      └── revenue = 100000.00\n"
/// );
/// # Ok(())
/// # }
/// ```
pub fn render_tree_detailed(
    tree: &DependencyTree,
    results: &StatementResult,
    period: &PeriodId,
) -> String {
    let mut output = String::new();
    render_tree_lines(
        tree,
        &mut output,
        "",
        None,
        &|node| match results.get(&node.node_id, period) {
            Some(value) => format!("{} = {:.2}", node.node_id, value),
            None => node.node_id.clone(),
        },
    );
    output
}

/// Write `tree` as one line per node: the root bare, every descendant behind
/// its parent's `prefix` plus a `├── ` / `└── ` connector. `is_last` is `None`
/// for the root and `Some(last)` for a child.
fn render_tree_lines(
    tree: &DependencyTree,
    output: &mut String,
    prefix: &str,
    is_last: Option<bool>,
    label: &dyn Fn(&DependencyTree) -> String,
) {
    if let Some(last) = is_last {
        output.push_str(prefix);
        output.push_str(if last { "└── " } else { "├── " });
    }
    output.push_str(&label(tree));
    output.push('\n');

    let child_prefix = match is_last {
        None => String::new(),
        Some(last) => format!("{prefix}{}", if last { "    " } else { "│   " }),
    };
    let child_count = tree.children.len();
    for (i, child) in tree.children.iter().enumerate() {
        render_tree_lines(
            child,
            output,
            &child_prefix,
            Some(i + 1 == child_count),
            label,
        );
    }
}

// Formula explanation

/// Explains how formulas are calculated.
///
/// The explainer breaks down formula calculations to show how a node's value
/// was derived from its dependencies.
///
/// # Examples
///
/// ```rust
/// # use finstack_quant_statements::builder::ModelBuilder;
/// # use finstack_quant_statements::evaluator::Evaluator;
/// # use finstack_quant_statements_analytics::analysis::FormulaExplainer;
/// # use finstack_quant_core::dates::PeriodId;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let model = ModelBuilder::new("demo")
///     .periods("2025Q1..Q2", None)?
///     .compute("revenue", "100000")?
///     .compute("cogs", "revenue * 0.4")?
///     .compute("gross_profit", "revenue - cogs")?
///     .build()?;
///
/// let mut evaluator = Evaluator::new();
/// let results = evaluator.evaluate(&model)?;
///
/// let explainer = FormulaExplainer::new(&model, &results);
/// let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
/// let explanation = explainer.explain("gross_profit", &period)?;
///
/// println!("{}", explanation.to_string_detailed());
/// // Output:
/// // gross_profit [2025Q1] = 60,000
/// // Formula: revenue - cogs
/// // Type: Calculated
/// # Ok(())
/// # }
/// ```
pub struct FormulaExplainer<'a> {
    model: &'a FinancialModelSpec,
    results: &'a StatementResult,
}

impl<'a> FormulaExplainer<'a> {
    /// Create a new formula explainer.
    ///
    /// # Arguments
    ///
    /// * `model` - Financial model specification
    /// * `results` - Evaluation results
    pub fn new(model: &'a FinancialModelSpec, results: &'a StatementResult) -> Self {
        Self { model, results }
    }

    /// Explain how a node's value was calculated for a specific period.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Node identifier
    /// * `period` - Period to explain
    ///
    /// # Returns
    ///
    /// Detailed explanation of the calculation
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error when the model has no such node or the
    /// supplied evaluation results contain no value for it in `period`.
    pub fn explain(&self, node_id: &str, period: &PeriodId) -> Result<Explanation> {
        let node_spec = self
            .model
            .nodes
            .get(node_id)
            .ok_or_else(|| Error::node_not_found(node_id))?;

        let final_value = self.results.get(node_id, period).ok_or_else(|| {
            Error::missing_data(format!(
                "No result for node '{}' in period '{}'",
                node_id, period
            ))
        })?;

        let breakdown = self.build_breakdown(node_id, period, &node_spec.formula_text)?;

        Ok(Explanation {
            node_id: node_id.to_string(),
            period_id: *period,
            final_value,
            node_type: node_spec.node_type,
            formula_text: node_spec.formula_text.clone(),
            breakdown,
        })
    }

    fn build_breakdown(
        &self,
        _node_id: &str,
        period: &PeriodId,
        formula: &Option<String>,
    ) -> Result<Vec<ExplanationStep>> {
        let mut breakdown = Vec::new();

        if let Some(formula_text) = formula {
            let identifiers =
                finstack_quant_statements::formula::extract_all_identifiers(formula_text)?;

            for identifier in identifiers {
                if identifier.starts_with("cs.") {
                    continue;
                }

                if let Some(value) = self.results.get(&identifier, period) {
                    breakdown.push(ExplanationStep {
                        component: identifier.clone(),
                        value,
                        operation: None,
                    });
                }
            }
        }

        Ok(breakdown)
    }
}

/// Detailed explanation of a node's calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct Explanation {
    /// Node identifier
    pub node_id: String,

    /// Period being explained
    pub period_id: PeriodId,

    /// Final calculated value; non-finite values serialize as `"nan"`,
    /// `"inf"` or `"-inf"`.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub final_value: f64,

    /// Type of node (Value, Calculated, etc.)
    pub node_type: NodeType,

    /// Formula text (if calculated)
    pub formula_text: Option<String>,

    /// Breakdown of calculation components
    pub breakdown: Vec<ExplanationStep>,
}

impl finstack_quant_core::wire::NonFiniteFields for Explanation {
    const NON_FINITE_FIELDS: &'static [&'static str] = &["final_value"];
}

impl Explanation {
    /// Convert explanation to detailed string format.
    ///
    /// # Returns
    ///
    /// Human-readable explanation of the calculation
    pub fn to_string_detailed(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!(
            "{} [{}] = {:.2}\n",
            self.node_id, self.period_id, self.final_value
        ));

        if let Some(formula) = &self.formula_text {
            output.push_str(&format!("Formula: {}\n", formula));
        }

        output.push_str(&format!("Type: {:?}\n", self.node_type));

        if !self.breakdown.is_empty() {
            output.push_str("\nComponents:\n");
            for step in &self.breakdown {
                output.push_str(&format!("  {} = {:.2}\n", step.component, step.value));
            }
        }

        output
    }
}

/// Step in a calculation breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct ExplanationStep {
    /// Component identifier (e.g., "revenue")
    pub component: String,

    /// Value of the component; non-finite values serialize as `"nan"`,
    /// `"inf"` or `"-inf"`.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub value: f64,

    /// Operation applied (e.g., "+", "-", "*", "/")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
}

impl finstack_quant_core::wire::NonFiniteFields for ExplanationStep {
    const NON_FINITE_FIELDS: &'static [&'static str] = &["value"];
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::wire::NonFiniteFields;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::evaluator::Evaluator;
    use finstack_quant_statements::types::AmountOrScalar;

    /// Keys of `value` that carry the `"nan"` sentinel string.
    fn sentinel_keys(value: &serde_json::Value) -> Vec<String> {
        let mut keys: Vec<String> = value
            .as_object()
            .expect("object")
            .iter()
            .filter(|(_, field)| field.as_str() == Some("nan"))
            .map(|(key, _)| key.clone())
            .collect();
        keys.sort_unstable();
        keys
    }

    /// `NON_FINITE_FIELDS` names exactly the NaN-carrying keys that
    /// serialize as sentinel strings, for the explanation and its steps.
    #[test]
    fn non_finite_fields_match_the_serde_attributes() {
        let step = ExplanationStep {
            component: "revenue".into(),
            value: f64::NAN,
            operation: Some("+".into()),
        };
        let explanation = Explanation {
            node_id: "ebitda".into(),
            period_id: "2025Q1".parse().expect("period"),
            final_value: f64::NAN,
            node_type: NodeType::Calculated,
            formula_text: None,
            breakdown: vec![step.clone()],
        };
        let json = serde_json::to_value(&explanation).expect("serialize");
        assert_eq!(sentinel_keys(&json), Explanation::NON_FINITE_FIELDS);
        let json = serde_json::to_value(&step).expect("serialize");
        assert_eq!(sentinel_keys(&json), ExplanationStep::NON_FINITE_FIELDS);
    }

    #[test]
    fn test_direct_dependencies() {
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("valid operation")
            .compute("a", "10")
            .expect("valid operation")
            .compute("b", "a * 2")
            .expect("valid operation")
            .compute("c", "a + b")
            .expect("valid operation")
            .build()
            .expect("valid operation");

        let graph = DependencyGraph::from_model(&model).expect("should build dependency graph");
        let tracer = DependencyTracer::new(&model, &graph);

        let deps_a = tracer
            .direct_dependencies("a")
            .expect("should get dependencies");
        assert_eq!(deps_a.len(), 0);

        let deps_b = tracer
            .direct_dependencies("b")
            .expect("should get dependencies");
        assert_eq!(deps_b.len(), 1);
        assert!(deps_b.contains(&"a"));

        let deps_c = tracer
            .direct_dependencies("c")
            .expect("should get dependencies");
        assert_eq!(deps_c.len(), 2);
        assert!(deps_c.contains(&"a"));
        assert!(deps_c.contains(&"b"));
    }

    #[test]
    fn test_all_dependencies() {
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("valid operation")
            .compute("a", "10")
            .expect("valid operation")
            .compute("b", "a * 2")
            .expect("valid operation")
            .compute("c", "b + 5")
            .expect("valid operation")
            .compute("d", "c - a")
            .expect("valid operation")
            .build()
            .expect("valid operation");

        let graph = DependencyGraph::from_model(&model).expect("should build dependency graph");
        let tracer = DependencyTracer::new(&model, &graph);

        let deps = tracer
            .all_dependencies("d")
            .expect("should get all dependencies");
        assert_eq!(deps.len(), 3);
        assert!(deps.contains(&"a".to_string()));
        assert!(deps.contains(&"b".to_string()));
        assert!(deps.contains(&"c".to_string()));
    }

    #[test]
    fn test_dependency_tree() {
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("valid operation")
            .compute("revenue", "100000")
            .expect("valid operation")
            .compute("cogs", "revenue * 0.4")
            .expect("valid operation")
            .compute("gross_profit", "revenue - cogs")
            .expect("valid operation")
            .build()
            .expect("valid operation");

        let graph = DependencyGraph::from_model(&model).expect("should build dependency graph");
        let tracer = DependencyTracer::new(&model, &graph);

        let tree = tracer
            .dependency_tree("gross_profit")
            .expect("should build dependency tree");
        assert_eq!(tree.node_id, "gross_profit");
        assert_eq!(tree.children.len(), 2);
        assert_eq!(tree.depth(), 2);
        assert_eq!(tree.node_count(), 4);
    }

    #[test]
    fn test_dependents() {
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("valid operation")
            .compute("a", "10")
            .expect("valid operation")
            .compute("b", "a * 2")
            .expect("valid operation")
            .compute("c", "a + 5")
            .expect("valid operation")
            .build()
            .expect("valid operation");

        let graph = DependencyGraph::from_model(&model).expect("should build dependency graph");
        let tracer = DependencyTracer::new(&model, &graph);

        let dependents = tracer.dependents("a").expect("should get dependents");
        assert_eq!(dependents.len(), 2);
        assert!(dependents.contains(&"b"));
        assert!(dependents.contains(&"c"));

        let dependents_b = tracer.dependents("b").expect("should get dependents");
        assert_eq!(dependents_b.len(), 0);
    }

    #[test]
    fn test_node_not_found() {
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("valid operation")
            .compute("a", "10")
            .expect("valid operation")
            .build()
            .expect("valid operation");

        let graph = DependencyGraph::from_model(&model).expect("should build dependency graph");
        let tracer = DependencyTracer::new(&model, &graph);

        let result = tracer.direct_dependencies("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_render_tree_ascii() {
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("test should succeed")
            .compute("a", "10")
            .expect("test should succeed")
            .compute("b", "a * 2")
            .expect("test should succeed")
            .compute("c", "a + b")
            .expect("test should succeed")
            .build()
            .expect("test should succeed");

        let graph = DependencyGraph::from_model(&model).expect("test should succeed");
        let tracer = DependencyTracer::new(&model, &graph);
        let tree = tracer.dependency_tree("c").expect("test should succeed");

        let ascii = render_tree_ascii(&tree);
        assert_eq!(
            ascii, "c (a + b)\n├── a (10)\n└── b (a * 2)\n    └── a (10)\n",
            "children carry connectors and grandchildren are indented"
        );
        assert_eq!(tracer.dependency_tree_text("c").expect("text"), ascii);
        assert_eq!(tree.children.len(), 2);
    }

    /// A deeper tree keeps a `│` rail under a non-last branch.
    #[test]
    fn test_render_tree_ascii_rails_under_open_branches() {
        let tree = DependencyTree {
            node_id: "root".into(),
            formula: None,
            children: vec![
                DependencyTree {
                    node_id: "left".into(),
                    formula: None,
                    children: vec![DependencyTree {
                        node_id: "leaf".into(),
                        formula: None,
                        children: Vec::new(),
                    }],
                },
                DependencyTree {
                    node_id: "right".into(),
                    formula: None,
                    children: Vec::new(),
                },
            ],
        };
        assert_eq!(
            render_tree_ascii(&tree),
            "root\n├── left\n│   └── leaf\n└── right\n"
        );
        let json = serde_json::to_string(&tree).expect("serialize");
        let back: DependencyTree = serde_json::from_str(&json).expect("round trip");
        assert_eq!(back, tree);
    }

    #[test]
    fn test_render_tree_detailed() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("test should succeed")
            .compute("revenue", "100000")
            .expect("test should succeed")
            .compute("cogs", "revenue * 0.4")
            .expect("test should succeed")
            .compute("gross_profit", "revenue - cogs")
            .expect("test should succeed")
            .build()
            .expect("test should succeed");

        let mut evaluator = Evaluator::new();
        let results = evaluator.evaluate(&model).expect("test should succeed");

        let graph = DependencyGraph::from_model(&model).expect("test should succeed");
        let tracer = DependencyTracer::new(&model, &graph);
        let tree = tracer
            .dependency_tree("gross_profit")
            .expect("test should succeed");

        let detailed = render_tree_detailed(&tree, &results, &period);
        assert!(detailed.contains("gross_profit = 60000.00"));
        assert!(detailed.contains("revenue = 100000.00"));
        assert!(detailed.contains("cogs = 40000.00"));
    }

    #[test]
    fn test_render_empty_tree() {
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("test should succeed")
            .compute("a", "10")
            .expect("test should succeed")
            .build()
            .expect("test should succeed");

        let graph = DependencyGraph::from_model(&model).expect("test should succeed");
        let tracer = DependencyTracer::new(&model, &graph);
        let tree = tracer.dependency_tree("a").expect("test should succeed");

        let ascii = render_tree_ascii(&tree);
        assert!(ascii.contains("a"));
        assert_eq!(ascii.lines().count(), 1);
    }

    #[test]
    fn test_explain_value_node() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let period2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("test should succeed")
            .value(
                "revenue",
                &[
                    (period, AmountOrScalar::scalar(100_000.0)),
                    (period2, AmountOrScalar::scalar(110_000.0)),
                ],
            )
            .build()
            .expect("test should succeed");

        let mut evaluator = Evaluator::new();
        let results = evaluator.evaluate(&model).expect("test should succeed");

        let explainer = FormulaExplainer::new(&model, &results);
        let explanation = explainer
            .explain("revenue", &period)
            .expect("test should succeed");

        assert_eq!(explanation.node_id, "revenue");
        assert_eq!(explanation.final_value, 100_000.0);
        assert!(matches!(explanation.node_type, NodeType::Value));
        assert!(explanation.formula_text.is_none());
        assert!(explanation.breakdown.is_empty());
    }

    #[test]
    fn test_explain_calculated_node() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let period2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("test should succeed")
            .value(
                "revenue",
                &[
                    (period, AmountOrScalar::scalar(100_000.0)),
                    (period2, AmountOrScalar::scalar(110_000.0)),
                ],
            )
            .compute("cogs", "revenue * 0.4")
            .expect("test should succeed")
            .compute("gross_profit", "revenue - cogs")
            .expect("test should succeed")
            .build()
            .expect("test should succeed");

        let mut evaluator = Evaluator::new();
        let results = evaluator.evaluate(&model).expect("test should succeed");

        let explainer = FormulaExplainer::new(&model, &results);
        let explanation = explainer
            .explain("gross_profit", &period)
            .expect("test should succeed");

        assert_eq!(explanation.node_id, "gross_profit");
        assert_eq!(explanation.final_value, 60_000.0);
        assert!(matches!(explanation.node_type, NodeType::Calculated));
        assert_eq!(explanation.formula_text, Some("revenue - cogs".to_string()));
        assert_eq!(explanation.breakdown.len(), 2);
    }

    /// A `lag` node has no value at the first period; the explanation keeps
    /// the NaN as the `"nan"` sentinel so its serde form round-trips.
    #[test]
    fn test_explain_non_finite_value_round_trips() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let period2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("test should succeed")
            .value(
                "revenue",
                &[
                    (period, AmountOrScalar::scalar(100.0)),
                    (period2, AmountOrScalar::scalar(110.0)),
                ],
            )
            .compute("lagged", "lag(revenue, 1)")
            .expect("test should succeed")
            .build()
            .expect("test should succeed");
        let results = Evaluator::new()
            .evaluate(&model)
            .expect("test should succeed");
        let explanation = FormulaExplainer::new(&model, &results)
            .explain("lagged", &period)
            .expect("test should succeed");
        assert!(explanation.final_value.is_nan());

        let json = serde_json::to_value(&explanation).expect("serialize");
        assert_eq!(json["final_value"], serde_json::json!("nan"));
        let back: Explanation = serde_json::from_value(json).expect("sentinel round-trips");
        assert!(back.final_value.is_nan());

        let step: ExplanationStep =
            serde_json::from_str(r#"{"component": "x", "value": "inf"}"#).expect("sentinel step");
        assert_eq!(step.value, f64::INFINITY);
    }

    #[test]
    fn test_explain_to_string_detailed() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let period2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("test should succeed")
            .value(
                "revenue",
                &[
                    (period, AmountOrScalar::scalar(100_000.0)),
                    (period2, AmountOrScalar::scalar(110_000.0)),
                ],
            )
            .compute("cogs", "revenue * 0.4")
            .expect("test should succeed")
            .build()
            .expect("test should succeed");

        let mut evaluator = Evaluator::new();
        let results = evaluator.evaluate(&model).expect("test should succeed");

        let explainer = FormulaExplainer::new(&model, &results);
        let explanation = explainer
            .explain("cogs", &period)
            .expect("test should succeed");

        let detailed = explanation.to_string_detailed();
        assert!(detailed.contains("cogs [2025Q1]"));
        assert!(detailed.contains("Formula: revenue * 0.4"));
        assert!(detailed.contains("revenue = 100000.00"));
    }

    #[test]
    fn test_explain_nonexistent_node() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let period2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("test should succeed")
            .value(
                "revenue",
                &[
                    (period, AmountOrScalar::scalar(100_000.0)),
                    (period2, AmountOrScalar::scalar(110_000.0)),
                ],
            )
            .build()
            .expect("test should succeed");

        let mut evaluator = Evaluator::new();
        let results = evaluator.evaluate(&model).expect("test should succeed");

        let explainer = FormulaExplainer::new(&model, &results);
        let result = explainer.explain("nonexistent", &period);

        assert!(result.is_err());
    }
}
