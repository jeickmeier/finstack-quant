//! Model introspection: dependency tracing, formula explanation, and tree visualization.

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::error::{Error, Result};
use finstack_quant_statements::evaluator::{DependencyGraph, StatementResult};
use finstack_quant_statements::types::{FinancialModelSpec, NodeType};
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Dependency tracing
// ---------------------------------------------------------------------------

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
            .ok_or_else(|| Error::invalid_input(format!("Node '{}' not found", node_id)))?;

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
            .ok_or_else(|| Error::invalid_input(format!("Node '{}' not found", node_id)))?;

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
            .ok_or_else(|| Error::invalid_input(format!("Node '{}' not found", node_id)))?;

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Convert tree to ASCII representation.
    ///
    /// # Returns
    ///
    /// ASCII tree string suitable for console output
    pub fn to_string_ascii(&self) -> String {
        render_tree_ascii(self)
    }
}

// ---------------------------------------------------------------------------
// Tree visualization
// ---------------------------------------------------------------------------

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
/// println!("{}", ascii);
/// // Output:
/// // gross_profit
/// // ├── revenue
/// // └── cogs
/// //     └── revenue
/// # Ok(())
/// # }
/// ```
pub fn render_tree_ascii(tree: &DependencyTree) -> String {
    let mut output = String::new();
    render_tree_recursive(tree, &mut output, "", true);
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
/// let period = PeriodId::quarter(2025, 1);
/// let detailed = render_tree_detailed(&tree, &results, &period);
/// println!("{}", detailed);
/// // Output:
/// // gross_profit = 60,000.00
/// // ├── revenue = 100,000.00
/// // └── cogs = 40,000.00
/// //     └── revenue = 100,000.00
/// # Ok(())
/// # }
/// ```
pub fn render_tree_detailed(
    tree: &DependencyTree,
    results: &StatementResult,
    period: &PeriodId,
) -> String {
    let mut output = String::new();
    render_tree_with_values(tree, results, period, &mut output, "", true);
    output
}

fn render_tree_recursive(tree: &DependencyTree, output: &mut String, prefix: &str, is_last: bool) {
    let connector = if is_last { "└── " } else { "├── " };
    let node_name = if prefix.is_empty() {
        tree.node_id.clone()
    } else {
        format!("{}{}", connector, tree.node_id)
    };

    output.push_str(&node_name);
    if let Some(formula) = &tree.formula {
        output.push_str(&format!(" ({})", formula));
    }
    output.push('\n');

    let child_count = tree.children.len();
    for (i, child) in tree.children.iter().enumerate() {
        let is_last_child = i == child_count - 1;
        let new_prefix = if prefix.is_empty() {
            String::new()
        } else {
            format!("{}{}", prefix, if is_last { "    " } else { "│   " })
        };

        render_tree_recursive(child, output, &new_prefix, is_last_child);
    }
}

fn render_tree_with_values(
    tree: &DependencyTree,
    results: &StatementResult,
    period: &PeriodId,
    output: &mut String,
    prefix: &str,
    is_last: bool,
) {
    let value = results.get(&tree.node_id, period);

    let connector = if is_last { "└── " } else { "├── " };
    let node_display = if prefix.is_empty() {
        if let Some(v) = value {
            format!("{} = {:.2}", tree.node_id, v)
        } else {
            tree.node_id.clone()
        }
    } else {
        let base = format!("{}{}", connector, tree.node_id);
        if let Some(v) = value {
            format!("{} = {:.2}", base, v)
        } else {
            base
        }
    };

    output.push_str(&node_display);
    output.push('\n');

    let child_count = tree.children.len();
    for (i, child) in tree.children.iter().enumerate() {
        let is_last_child = i == child_count - 1;
        let new_prefix = if prefix.is_empty() {
            String::new()
        } else {
            format!("{}{}", prefix, if is_last { "    " } else { "│   " })
        };

        render_tree_with_values(child, results, period, output, &new_prefix, is_last_child);
    }
}

// ---------------------------------------------------------------------------
// Formula explanation
// ---------------------------------------------------------------------------

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
/// let period = PeriodId::quarter(2025, 1);
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
            .ok_or_else(|| Error::invalid_input(format!("Node '{}' not found", node_id)))?;

        let final_value = self.results.get(node_id, period).ok_or_else(|| {
            Error::invalid_input(format!(
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
pub struct Explanation {
    /// Node identifier
    pub node_id: String,

    /// Period being explained
    pub period_id: PeriodId,

    /// Final calculated value
    pub final_value: f64,

    /// Type of node (Value, Calculated, etc.)
    pub node_type: NodeType,

    /// Formula text (if calculated)
    pub formula_text: Option<String>,

    /// Breakdown of calculation components
    pub breakdown: Vec<ExplanationStep>,
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

    /// Convert explanation to compact string format.
    ///
    /// # Returns
    ///
    /// Compact single-line summary
    pub fn to_string_compact(&self) -> String {
        if let Some(formula) = &self.formula_text {
            format!(
                "{} [{}] = {:.2} ({})",
                self.node_id, self.period_id, self.final_value, formula
            )
        } else {
            format!(
                "{} [{}] = {:.2}",
                self.node_id, self.period_id, self.final_value
            )
        }
    }
}

/// Step in a calculation breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplanationStep {
    /// Component identifier (e.g., "revenue")
    pub component: String,

    /// Value of the component
    pub value: f64,

    /// Operation applied (e.g., "+", "-", "*", "/")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::evaluator::Evaluator;
    use finstack_quant_statements::types::AmountOrScalar;

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
        assert!(ascii.contains("c"));
        assert!(ascii.contains("a"));
        assert!(ascii.contains("b"));
        assert_eq!(tree.children.len(), 2);
    }

    #[test]
    fn test_render_tree_detailed() {
        let period = PeriodId::quarter(2025, 1);
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
        let period = PeriodId::quarter(2025, 1);
        let period2 = PeriodId::quarter(2025, 2);
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
        let period = PeriodId::quarter(2025, 1);
        let period2 = PeriodId::quarter(2025, 2);
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

    #[test]
    fn test_explain_to_string_detailed() {
        let period = PeriodId::quarter(2025, 1);
        let period2 = PeriodId::quarter(2025, 2);
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
        let period = PeriodId::quarter(2025, 1);
        let period2 = PeriodId::quarter(2025, 2);
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
