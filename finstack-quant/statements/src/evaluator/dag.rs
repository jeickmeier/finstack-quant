//! Dependency graph construction and topological sorting.

use crate::error::{Error, Result};
use crate::types::{FinancialModelSpec, NodeId};
use indexmap::{IndexMap, IndexSet};

/// Dependency graph for nodes in a financial model.
///
/// The graph stores both incoming and outgoing edges so that consumers can
/// traverse dependencies and dependents efficiently. It is primarily used by
/// the evaluator to derive a topological execution order and detect cycles.
#[derive(Debug)]
pub struct DependencyGraph {
    /// Map of node_id → set of dependencies (nodes it depends on)
    pub dependencies: IndexMap<NodeId, IndexSet<NodeId>>,

    /// Map of node_id → set of dependents (nodes that depend on it)
    pub dependents: IndexMap<NodeId, IndexSet<NodeId>>,
}

impl DependencyGraph {
    /// Build a dependency graph from a model specification.
    ///
    /// # Arguments
    /// * `model` - Fully configured [`FinancialModelSpec`](crate::types::FinancialModelSpec)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::builder::ModelBuilder;
    /// # use finstack_quant_statements::evaluator::DependencyGraph;
    /// let model = ModelBuilder::new("demo")
    ///     .periods("2025Q1..Q2", None)?
    ///     .compute("a", "10")?
    ///     .compute("b", "a * 2")?
    ///     .build()?;
    ///
    /// let graph = DependencyGraph::from_model(&model)?;
    /// assert!(graph.dependencies["b"].contains("a"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_model(model: &FinancialModelSpec) -> Result<Self> {
        // Validate all formula references before building graph
        Self::validate_formula_references(model)?;

        let mut dependencies = IndexMap::new();
        let mut dependents = IndexMap::new();

        // Initialize empty sets for all nodes
        for node_id in model.nodes.keys() {
            dependencies.insert(node_id.clone(), IndexSet::new());
            dependents.insert(node_id.clone(), IndexSet::new());
        }

        let all_node_ids: IndexSet<NodeId> = model.nodes.keys().cloned().collect();

        // Extract dependencies from formulas and where clauses
        for (node_id, node_spec) in &model.nodes {
            if let Some(formula) = &node_spec.formula_text {
                let node_deps = extract_dependencies(formula, &all_node_ids)?;
                add_dependency_edges(node_id, &node_deps, &mut dependencies, &mut dependents);
            }

            if let Some(where_clause) = &node_spec.where_text {
                let node_deps = extract_dependencies(where_clause, &all_node_ids)?;
                add_dependency_edges(node_id, &node_deps, &mut dependencies, &mut dependents);
            }
        }

        Ok(Self {
            dependencies,
            dependents,
        })
    }

    /// Validate that all identifier references in formulas exist in the model.
    ///
    /// This catches typos and unknown references at build time instead of runtime.
    fn validate_formula_references(model: &FinancialModelSpec) -> Result<()> {
        let valid_identifiers: IndexSet<NodeId> = model.nodes.keys().cloned().collect();

        // Check each formula
        for (node_id, node_spec) in &model.nodes {
            if let Some(formula) = &node_spec.formula_text {
                // Extract all identifiers from the formula
                let all_identifiers = crate::utils::formula::extract_all_identifiers(formula)?;

                // Check each identifier
                for identifier in &all_identifiers {
                    // Skip cs.* references (capital structure - validated at runtime)
                    if identifier.starts_with("cs.") {
                        continue;
                    }

                    // Check if identifier exists in model nodes
                    if !valid_identifiers.contains(identifier.as_str()) {
                        return Err(Error::eval(format!(
                            "Unknown identifier '{}' in formula for node '{}'. \
                             Formula: '{}'. \
                             This identifier does not exist in the model. \
                             Did you mean one of: {}?",
                            identifier,
                            node_id,
                            formula,
                            suggest_similar_identifiers(identifier, &valid_identifiers)
                        )));
                    }
                }
            }

            // Also validate where clauses
            if let Some(where_clause) = &node_spec.where_text {
                let all_identifiers = crate::utils::formula::extract_all_identifiers(where_clause)?;

                for identifier in &all_identifiers {
                    if identifier.starts_with("cs.") {
                        continue;
                    }

                    if !valid_identifiers.contains(identifier.as_str()) {
                        return Err(Error::eval(format!(
                            "Unknown identifier '{}' in where clause for node '{}'. \
                             Where clause: '{}'. \
                             This identifier does not exist in the model. \
                             Did you mean one of: {}?",
                            identifier,
                            node_id,
                            where_clause,
                            suggest_similar_identifiers(identifier, &valid_identifiers)
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// Get dependencies for a node.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier to inspect
    ///
    /// # Returns
    /// Either an [`IndexSet`] of upstream dependencies or `None` if the node
    /// does not exist.
    pub fn get_dependencies(&self, node_id: &str) -> Option<&IndexSet<NodeId>> {
        self.dependencies.get(node_id)
    }

    /// Check for circular dependencies.
    ///
    /// Performs a depth-first search to surface a representative cycle when one
    /// exists. The `visited` set is shared across every starting node so a
    /// fully-explored subgraph is never re-walked, keeping the whole scan
    /// `O(V + E)` rather than `O(V * (V + E))`.
    pub fn detect_cycles(&self) -> Result<()> {
        let mut visited: IndexSet<NodeId> = IndexSet::new();
        for node_id in self.dependencies.keys() {
            if let Some(cycle) = self.dfs_cycle(node_id.as_str(), &mut visited) {
                return Err(Error::circular_dependency(cycle));
            }
        }
        Ok(())
    }

    /// Walk one node's subgraph, returning a representative cycle if found.
    ///
    /// The traversal is iterative with an explicit stack. A recursive walk goes
    /// `V` frames deep on a chain of dependencies, and the graph comes from a
    /// user-supplied model (JSON), so a deep chain overflows the stack — an
    /// abort (SIGABRT) that the Python / WASM bindings cannot catch, not a
    /// recoverable error. Whether it overflows depends on node *order*: the
    /// shared `visited` set short-circuits a chain entered from its root, but a
    /// chain entered from its deep end recurses its full length, and node order
    /// in a JSON document is arbitrary.
    ///
    /// `on_path` mirrors `path` as a set so the cycle test is O(1) rather than
    /// a linear scan of the path on every step (the scan made a deep walk
    /// O(V²)). `path` is kept only to reconstruct the cycle for the diagnostic,
    /// which happens at most once.
    fn dfs_cycle(&self, start: &str, visited: &mut IndexSet<NodeId>) -> Option<Vec<String>> {
        if visited.contains(start) {
            return None;
        }

        // Each frame is (node, index of the next dependency to visit).
        let mut stack: Vec<(NodeId, usize)> = vec![(NodeId::new(start), 0)];
        let mut path: Vec<NodeId> = vec![NodeId::new(start)];
        let mut on_path: IndexSet<NodeId> = IndexSet::new();
        on_path.insert(NodeId::new(start));

        while let Some((node, dep_idx)) = stack.last().map(|(n, i)| (n.clone(), *i)) {
            let next_dep = self
                .dependencies
                .get(node.as_str())
                .and_then(|deps| deps.get_index(dep_idx))
                .cloned();

            match next_dep {
                Some(dep) => {
                    if let Some(frame) = stack.last_mut() {
                        frame.1 += 1;
                    }

                    if on_path.contains(&dep) {
                        // Back edge into the active path: report it from the
                        // point the path first reached `dep`.
                        let cycle_start = path.iter().position(|n| *n == dep).unwrap_or(0);
                        let mut cycle: Vec<String> = path
                            .get(cycle_start..)
                            .unwrap_or_default()
                            .iter()
                            .map(|n| n.as_str().to_string())
                            .collect();
                        cycle.push(dep.as_str().to_string());
                        return Some(cycle);
                    }

                    if !visited.contains(&dep) {
                        stack.push((dep.clone(), 0));
                        path.push(dep.clone());
                        on_path.insert(dep);
                    }
                }
                None => {
                    // Every dependency explored: this node is fully done.
                    if let Some((done, _)) = stack.pop() {
                        path.pop();
                        on_path.shift_remove(&done);
                        visited.insert(done);
                    }
                }
            }
        }

        None
    }
}

fn add_dependency_edges(
    node_id: &NodeId,
    node_deps: &IndexSet<NodeId>,
    dependencies: &mut IndexMap<NodeId, IndexSet<NodeId>>,
    dependents: &mut IndexMap<NodeId, IndexSet<NodeId>>,
) {
    for dep in node_deps {
        if let Some(deps) = dependencies.get_mut(node_id) {
            deps.insert(dep.clone());
        }
        if let Some(dep_set) = dependents.get_mut(dep) {
            dep_set.insert(node_id.clone());
        }
    }
}

/// Compute the topological evaluation order.
///
/// Nodes are returned in an order where all dependencies appear before the
/// nodes that depend on them. The function returns an error if a cycle is
/// present.
///
/// # Arguments
/// * `graph` - Dependency graph built from a [`FinancialModelSpec`](crate::types::FinancialModelSpec)
///
/// # Example
///
/// ```rust
/// # use finstack_quant_statements::builder::ModelBuilder;
/// # use finstack_quant_statements::evaluator::{DependencyGraph, evaluate_order};
/// let model = ModelBuilder::new("demo")
///     .periods("2025Q1..Q2", None)?
///     .compute("a", "10")?
///     .compute("b", "a * 2")?
///     .build()?;
///
/// let graph = DependencyGraph::from_model(&model)?;
/// let order = evaluate_order(&graph)?;
/// assert!(order.iter().position(|n| n == "a") < order.iter().position(|n| n == "b"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn evaluate_order(graph: &DependencyGraph) -> Result<Vec<NodeId>> {
    crate::utils::graph::toposort_ids(&graph.dependencies).map_err(|unprocessed| {
        Error::eval(format!(
            "Circular dependency detected in model. Affected nodes: {}",
            unprocessed
                .iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })
}

/// Extract node dependencies from a formula string.
///
/// Uses shared formula utilities to find standalone identifier references.
/// This specifically uses `extract_direct_dependencies` which parses the AST
/// and ignores references inside `lag()` and `shift()` calls, allowing for
/// temporal cycles (like corkscrews) without blocking the DAG.
fn extract_dependencies(
    formula: &str,
    all_node_ids: &IndexSet<NodeId>,
) -> Result<IndexSet<NodeId>> {
    let direct_deps = crate::utils::formula::extract_direct_dependencies(formula).map_err(|e| {
        crate::error::Error::build(format!(
            "Failed to parse formula for dependency extraction: {e}"
        ))
    })?;
    Ok(direct_deps
        .into_iter()
        .filter(|id| all_node_ids.contains(id.as_str()))
        .collect())
}

/// Suggest similar identifiers for a typo using Levenshtein distance.
///
/// Returns a comma-separated list of up to 3 most similar identifiers.
fn suggest_similar_identifiers(typo: &str, valid: &IndexSet<NodeId>) -> String {
    let mut similarities: Vec<(usize, &NodeId)> = valid
        .iter()
        .map(|id| (strsim::levenshtein(typo, id.as_str()), id))
        .collect();

    // Sort by distance (closest first)
    similarities.sort_by_key(|(dist, _)| *dist);

    // Take top 3
    similarities
        .iter()
        .take(3)
        .map(|(_, id)| id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::ModelBuilder;

    #[test]
    fn test_simple_dag() {
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("test should succeed")
            .compute("a", "10")
            .expect("test should succeed")
            .compute("b", "a * 2")
            .expect("test should succeed")
            .compute("c", "b + a")
            .expect("test should succeed")
            .build()
            .expect("test should succeed");

        let graph = DependencyGraph::from_model(&model).expect("test should succeed");

        // Check dependencies
        assert_eq!(graph.dependencies["a"].len(), 0);
        assert!(graph.dependencies["b"].contains("a"));
        assert!(graph.dependencies["c"].contains("b"));
        assert!(graph.dependencies["c"].contains("a"));
    }

    #[test]
    fn test_topological_sort() {
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("test should succeed")
            .compute("a", "10")
            .expect("test should succeed")
            .compute("b", "a * 2")
            .expect("test should succeed")
            .compute("c", "b + a")
            .expect("test should succeed")
            .build()
            .expect("test should succeed");

        let graph = DependencyGraph::from_model(&model).expect("test should succeed");
        let order = evaluate_order(&graph).expect("test should succeed");

        // 'a' should come before 'b' and 'c'
        let a_pos = order
            .iter()
            .position(|n| n == "a")
            .expect("test should succeed");
        let b_pos = order
            .iter()
            .position(|n| n == "b")
            .expect("test should succeed");
        let c_pos = order
            .iter()
            .position(|n| n == "c")
            .expect("test should succeed");

        assert!(a_pos < b_pos);
        assert!(b_pos < c_pos);
    }

    #[test]
    fn test_cycle_detection() {
        // Cycles are now caught at build time by ModelBuilder::build()
        let result = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("test should succeed")
            .compute("a", "b + 1")
            .expect("test should succeed")
            .compute("b", "c + 1")
            .expect("test should succeed")
            .compute("c", "a + 1")
            .expect("test should succeed")
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Circular dependency"),
            "expected circular dependency error, got: {err}"
        );
    }

    /// Build a chain `a0 <- a1 <- ... <- a{n-1}`, inserted deepest-first.
    ///
    /// Insertion order matters: the shared `visited` set short-circuits a chain
    /// entered from its root, so only a deep-end entry exercises full-length
    /// traversal. Node order in a JSON model is arbitrary, so this order is
    /// just as reachable as any other.
    fn deep_chain_model(n: usize) -> crate::types::FinancialModelSpec {
        use crate::types::{FinancialModelSpec, NodeSpec, NodeType};
        let periods = finstack_quant_core::dates::build_periods("2025Q1..Q1", None)
            .expect("periods")
            .periods;
        let mut model = FinancialModelSpec::new("deep", periods);
        for i in (1..n).rev() {
            model.add_node(
                NodeSpec::new(format!("a{i}"), NodeType::Mixed)
                    .with_formula(format!("a{} + 1", i - 1)),
            );
        }
        model.add_node(NodeSpec::new("a0", NodeType::Mixed).with_formula("1"));
        model
    }

    /// Cycle detection must not overflow the stack on a deep dependency chain.
    ///
    /// The walk used to recurse once per node, so a 20k chain entered from its
    /// deep end aborted the process (SIGABRT) — uncatchable by the bindings,
    /// and reachable from a model spec supplied as JSON.
    #[test]
    fn deep_dependency_chain_does_not_overflow_the_stack() {
        let model = deep_chain_model(20_000);
        let graph = DependencyGraph::from_model(&model).expect("graph builds");
        graph
            .detect_cycles()
            .expect("a 20k-node chain is acyclic and must not abort");
    }

    /// The iterative walk must still find a cycle, and report it.
    #[test]
    fn deep_chain_with_a_cycle_is_still_detected() {
        use crate::types::{NodeSpec, NodeType};
        let mut model = deep_chain_model(2_000);
        // Close the loop: a0 now depends on the far end of the chain.
        model.add_node(NodeSpec::new("a0", NodeType::Mixed).with_formula("a1999 + 1"));

        let graph = DependencyGraph::from_model(&model).expect("graph builds");
        let err = graph
            .detect_cycles()
            .expect_err("a closed loop must be reported");
        assert!(
            err.to_string().contains("Circular dependency"),
            "expected a circular-dependency diagnostic, got: {err}"
        );
    }

    #[test]
    fn test_extract_dependencies() {
        let all_nodes: IndexSet<NodeId> = ["revenue", "cogs", "gross_profit"]
            .iter()
            .map(|s| NodeId::new(*s))
            .collect();

        let deps = extract_dependencies("revenue - cogs", &all_nodes).unwrap();
        assert_eq!(deps.len(), 2);
        assert!(deps.contains("revenue"));
        assert!(deps.contains("cogs"));
    }

    #[test]
    fn test_lag_breaks_cycle() {
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("test should succeed")
            .compute("a", "lag(b, 1)") // a depends on b (lagged)
            .expect("test should succeed")
            .compute("b", "a + 1") // b depends on a (direct)
            .expect("test should succeed")
            .build()
            .expect("test should succeed");

        let graph = DependencyGraph::from_model(&model).expect("test should succeed");

        // Should NOT detect cycle because a's dependency on b is lagged
        let result = graph.detect_cycles();
        assert!(result.is_ok());

        // Order should be a then b (since b depends on a, and a depends on nothing in current period)
        let order = evaluate_order(&graph).expect("test should succeed");
        let a_pos = order
            .iter()
            .position(|n| n == "a")
            .expect("node a should exist");
        let b_pos = order
            .iter()
            .position(|n| n == "b")
            .expect("node b should exist");
        assert!(a_pos < b_pos);
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(strsim::levenshtein("", ""), 0);
        assert_eq!(strsim::levenshtein("abc", "abc"), 0);
        assert_eq!(strsim::levenshtein("abc", ""), 3);
        assert_eq!(strsim::levenshtein("", "abc"), 3);
        assert_eq!(strsim::levenshtein("kitten", "sitting"), 3);
        assert_eq!(strsim::levenshtein("revenue", "revnue"), 1);
    }

    #[test]
    fn test_levenshtein_stress() {
        let long_a: String = "a".repeat(200);
        let long_b: String = "b".repeat(200);
        let dist = strsim::levenshtein(&long_a, &long_b);
        assert_eq!(dist, 200);

        let same: String = "x".repeat(200);
        assert_eq!(strsim::levenshtein(&same, &same), 0);
    }

    #[test]
    fn test_where_clause_adds_dependencies() {
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("test should succeed")
            .value("revenue", &[])
            .mixed("margin")
            .formula("1.0")
            .expect("test should succeed")
            .build()
            .expect("test should succeed")
            .where_clause("revenue > 0.0")
            .build()
            .expect("test should succeed");

        let graph = DependencyGraph::from_model(&model).expect("test should succeed");
        assert!(graph.dependencies["margin"].contains("revenue"));
        assert!(graph.dependents["revenue"].contains("margin"));
    }
}
