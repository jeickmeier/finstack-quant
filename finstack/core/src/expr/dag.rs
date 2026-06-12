//! DAG planning and execution optimization for expressions.
//!
//! Detects shared sub-expressions in complex expression trees and builds an
//! optimized execution plan that evaluates each unique sub-expression only
//! once. Critical for performance in financial statement models with hundreds
//! of interdependent formulas.

use super::ast::*;
use crate::collections::{HashMap, HashSet};
use std::vec::Vec;

/// A node in the execution DAG.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct DagNode {
    /// Unique identifier for this node.
    pub id: u64,
    /// The expression this node represents.
    pub expr: Expr,
    /// Dependencies (other DAG nodes this depends on).
    pub dependencies: Vec<u64>,
    /// Reference count (how many other nodes depend on this).
    pub ref_count: usize,
    /// Estimated cost of computing this node.
    pub cost: usize,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{NumericMode, ResultsMeta, RoundingContext, RoundingMode, ToleranceConfig};

    fn meta() -> ResultsMeta {
        crate::config::results_meta(&crate::config::FinstackConfig::default())
    }

    fn explicit_meta() -> ResultsMeta {
        ResultsMeta {
            numeric_mode: NumericMode::F64,
            rounding: RoundingContext {
                mode: RoundingMode::Bankers,
                ingest_scale_by_ccy: Default::default(),
                output_scale_by_ccy: Default::default(),
                tolerances: ToleranceConfig::default(),
                version: 1,
            },
            fx_policy_applied: None,
            timestamp: None,
            version: None,
        }
    }

    #[test]
    fn dag_builder_deduplicates_structurally_identical_nodes() {
        let mut builder = DagBuilder::new();
        let a = Expr::call(
            Function::RollingMean,
            vec![Expr::column("x"), Expr::literal(3.0)],
        )
        .with_id(7);
        let b = Expr::call(
            Function::RollingMean,
            vec![Expr::column("x"), Expr::literal(3.0)],
        )
        .with_id(42);

        let plan = builder
            .build_plan(vec![a, b], meta())
            .expect("valid expressions should build a DAG plan");
        assert_eq!(plan.nodes.len(), 3, "column, literal, rolling mean");
        assert_eq!(plan.roots.len(), 2);
        assert_eq!(plan.roots[0], plan.roots[1]);
    }

    #[test]
    fn dag_builder_orders_dependencies_topologically() {
        let mut builder = DagBuilder::new();
        let col = Expr::column("x");
        let lit = Expr::literal(2.0);
        let sum = Expr::call(Function::RollingSum, vec![col.clone(), lit.clone()]);
        let mean = Expr::call(Function::RollingMean, vec![col, lit]);

        let plan = builder
            .build_plan(vec![sum, mean], meta())
            .expect("valid expressions should build a DAG plan");
        assert_eq!(plan.nodes.len(), 4);

        // Ensure dependencies come before dependents in node order.
        for node in &plan.nodes {
            for &dep in &node.dependencies {
                let dep_index = plan
                    .nodes
                    .iter()
                    .position(|n| n.id == dep)
                    .expect("dependency must exist");
                let node_index = plan
                    .nodes
                    .iter()
                    .position(|n| n.id == node.id)
                    .expect("node must exist in plan");
                assert!(dep_index < node_index);
            }
        }
    }

    #[test]
    fn dag_topological_sort_rejects_excessive_depth() {
        let mut builder = DagBuilder::new();
        let mut prev = 0_u64;
        for id in 1..=MAX_DAG_RECURSION_DEPTH as u64 + 1 {
            let dependencies = if prev == 0 { vec![] } else { vec![prev] };
            builder.nodes.insert(
                id,
                DagNode {
                    id,
                    expr: Expr::literal(id as f64),
                    dependencies,
                    ref_count: 0,
                    cost: 1,
                },
            );
            prev = id;
        }

        let err = builder
            .topological_sort(&[prev])
            .expect_err("deep recursion should fail with a guard");
        assert!(
            err.to_string().contains("maximum DAG recursion depth"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn dag_build_plan_rejects_deeply_nested_ast() {
        // A programmatic AST nested just past the limit must return the
        // structured depth error from process_expression instead of
        // overflowing the stack during plan construction.
        let mut expr = Expr::column("x");
        for _ in 0..MAX_DAG_RECURSION_DEPTH + 1 {
            expr = Expr::unary_op(UnaryOp::Neg, expr);
        }

        let mut builder = DagBuilder::new();
        let err = builder
            .build_plan(vec![expr], meta())
            .expect_err("deeply nested AST should fail with a depth guard");
        assert!(
            err.to_string().contains("maximum DAG recursion depth"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn dag_build_plan_accepts_ast_below_depth_limit() {
        let mut expr = Expr::column("x");
        for _ in 0..MAX_DAG_RECURSION_DEPTH - 2 {
            expr = Expr::unary_op(UnaryOp::Neg, expr);
        }

        let mut builder = DagBuilder::new();
        let plan = builder
            .build_plan(vec![expr], meta())
            .expect("AST below the depth limit should build a plan");
        assert_eq!(plan.nodes.len(), MAX_DAG_RECURSION_DEPTH - 1);
    }

    #[test]
    fn dag_builder_tracks_shared_subexpressions() {
        let mut builder = DagBuilder::new();
        let col_x = Expr::column("x");
        let lit_3 = Expr::literal(3.0);
        let rolling_mean = Expr::call(Function::RollingMean, vec![col_x.clone(), lit_3.clone()]);
        let rolling_sum = Expr::call(Function::RollingSum, vec![col_x, lit_3]);

        let plan = builder
            .build_plan(vec![rolling_mean, rolling_sum], explicit_meta())
            .expect("valid expressions should build a DAG plan");

        assert_eq!(plan.nodes.len(), 4);
        assert_eq!(plan.roots.len(), 2);

        let col_node = plan
            .nodes
            .iter()
            .find(|node| matches!(node.expr.node, ExprNode::Column(_)))
            .expect("column node should be present");
        assert!(
            col_node.ref_count > 1,
            "shared column should be referenced twice"
        );
        assert!(plan.cache_strategy.expected_hit_rate >= 0.0);
    }

    #[test]
    fn dag_builder_costs_reflect_function_complexity() {
        let mut builder = DagBuilder::new();
        let col_x = Expr::column("x");
        let lit_5 = Expr::literal(5.0);
        let lag = Expr::call(Function::Lag, vec![col_x.clone(), lit_5.clone()]);
        let rolling_std = Expr::call(Function::RollingStd, vec![col_x, lit_5]);

        let plan = builder
            .build_plan(vec![lag, rolling_std], explicit_meta())
            .expect("valid expressions should build a DAG plan");

        let lag_node = plan
            .nodes
            .iter()
            .find(|node| matches!(node.expr.node, ExprNode::Call(Function::Lag, _)))
            .expect("lag node should be present");
        let rolling_std_node = plan
            .nodes
            .iter()
            .find(|node| matches!(node.expr.node, ExprNode::Call(Function::RollingStd, _)))
            .expect("rolling std node should be present");

        assert!(rolling_std_node.cost > lag_node.cost);
    }

    #[test]
    fn dag_builder_prefers_caching_expensive_shared_nodes() {
        let mut builder = DagBuilder::new();
        let col_x = Expr::column("x");
        let rolling_std = Expr::call(Function::RollingStd, vec![col_x, Expr::literal(10.0)]);
        let expr1 = Expr::call(
            Function::RollingMean,
            vec![rolling_std.clone(), Expr::literal(5.0)],
        );
        let expr2 = Expr::call(Function::RollingSum, vec![rolling_std, Expr::literal(3.0)]);

        let plan = builder
            .build_plan(vec![expr1, expr2], explicit_meta())
            .expect("valid expressions should build a DAG plan");

        let rolling_std_node = plan
            .nodes
            .iter()
            .find(|node| matches!(node.expr.node, ExprNode::Call(Function::RollingStd, _)))
            .expect("shared rolling std node should be present");
        assert!(rolling_std_node.ref_count > 1);
        assert!(plan.cache_strategy.expected_hit_rate > 0.0);
    }

    #[test]
    fn dag_builder_counts_shared_node_dependencies_once() {
        let mut builder = DagBuilder::new();
        let shared = Expr::call(
            Function::RollingStd,
            vec![Expr::column("x"), Expr::literal(10.0)],
        );
        let derived = Expr::call(
            Function::RollingMean,
            vec![shared.clone(), Expr::literal(5.0)],
        );

        let plan = builder
            .build_plan(vec![shared, derived], explicit_meta())
            .expect("valid expressions should build a DAG plan");

        let col_node = plan
            .nodes
            .iter()
            .find(|node| matches!(node.expr.node, ExprNode::Column(_)))
            .expect("column node should be present");

        assert_eq!(
            col_node.ref_count, 1,
            "shared-node dependencies should be counted once per edge"
        );
    }

    #[test]
    fn dag_builder_keeps_dependency_chain_in_topological_order() {
        let mut builder = DagBuilder::new();
        let col_x = Expr::column("x");
        let lag_x = Expr::call(Function::Lag, vec![col_x, Expr::literal(1.0)]);
        let diff_lag = Expr::call(Function::Diff, vec![lag_x, Expr::literal(1.0)]);

        let plan = builder
            .build_plan(vec![diff_lag], explicit_meta())
            .expect("valid expressions should build a DAG plan");

        let mut found_column = false;
        let mut found_lag = false;
        for node in &plan.nodes {
            match &node.expr.node {
                ExprNode::Column(_) => {
                    assert!(!found_column && !found_lag);
                    found_column = true;
                }
                ExprNode::Call(Function::Lag, _) => {
                    assert!(found_column && !found_lag);
                    found_lag = true;
                }
                ExprNode::Call(Function::Diff, _) => {
                    assert!(found_column && found_lag);
                }
                _ => {}
            }
        }
    }

    #[test]
    fn dag_builder_allows_empty_plans() {
        let mut builder = DagBuilder::new();
        let plan = builder
            .build_plan(vec![], explicit_meta())
            .expect("empty root list should still produce a valid plan");

        assert!(plan.nodes.is_empty());
        assert!(plan.roots.is_empty());
        assert_eq!(plan.cache_strategy.expected_hit_rate, 0.0);
    }
}

const MAX_DAG_RECURSION_DEPTH: usize = 512;

/// Execution plan for a DAG of expressions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ExecutionPlan {
    /// All nodes in topological order.
    pub nodes: Vec<DagNode>,
    /// Root node IDs (final outputs).
    pub roots: Vec<u64>,
    /// Execution metadata.
    pub meta: crate::config::ResultsMeta,
    /// Cache strategy recommendations.
    pub cache_strategy: CacheStrategy,
}

/// Vestigial cache-strategy statistics carried by [`ExecutionPlan`].
///
/// The cross-evaluation result cache these recommendations fed was removed
/// (see `docs/reviews/2026-06-09-core-quant-review.md`, Blocker #2): nothing
/// consults any of these fields at evaluation time. The type is retained only
/// because `ExecutionPlan` remains part of the serialized form of
/// [`super::eval::CompiledExpr`]; deleting the field would silently change
/// that wire format.
///
/// Field semantics (advisory only):
/// - `cache_nodes`: nodes a per-node result cache *would* have retained
///   (high ref count or expensive).
/// - `expected_hit_rate`: estimated fraction of total node cost such a cache
///   would have deduplicated — a cost share, not a literal hit rate.
/// - `memory_budget`: rough unit-less size estimate; never consulted.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CacheStrategy {
    /// Nodes that should be cached (high ref count or expensive).
    pub cache_nodes: HashSet<u64>,
    /// Estimated fraction of total node cost a cache would deduplicate.
    pub expected_hit_rate: f64,
    /// Memory budget estimate (arbitrary units); never consulted.
    pub memory_budget: usize,
}

/// DAG builder that detects shared sub-expressions and builds optimized execution plans.
#[derive(Default)]
pub(crate) struct DagBuilder {
    /// Expression cache for deduplication.
    expr_cache: HashMap<Expr, u64>,
    /// Node storage.
    nodes: HashMap<u64, DagNode>,
    /// Next available node ID.
    next_id: u64,
}

impl DagBuilder {
    /// Create a new DAG builder.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Build an execution plan from a list of root expressions.
    pub(crate) fn build_plan(
        &mut self,
        exprs: Vec<Expr>,
        meta: crate::config::ResultsMeta,
    ) -> crate::Result<ExecutionPlan> {
        // Clear state
        self.expr_cache.clear();
        self.nodes.clear();
        self.next_id = 0;

        // Process each root expression
        let mut root_ids = Vec::new();
        for expr in exprs {
            let id = self.process_expression(expr, 0)?;
            root_ids.push(id);
        }

        // Calculate reference counts
        self.calculate_ref_counts(&root_ids);

        // Build topological order (dependencies first)
        let ordered_nodes = self.topological_sort(&root_ids)?;

        // Generate cache strategy
        let cache_strategy = self.generate_cache_strategy(&ordered_nodes);

        Ok(ExecutionPlan {
            nodes: ordered_nodes,
            roots: root_ids,
            meta,
            cache_strategy,
        })
    }

    /// Process an expression tree, deduplicating shared sub-expressions.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] when the expression nests deeper
    /// than [`MAX_DAG_RECURSION_DEPTH`]. The same guard protects
    /// `topological_sort` and `calculate_ref_counts`; without it here, a
    /// deeply nested programmatic AST would overflow the stack during plan
    /// construction before either of those guards is reached.
    fn process_expression(&mut self, expr: Expr, depth: usize) -> crate::Result<u64> {
        if depth >= MAX_DAG_RECURSION_DEPTH {
            return Err(crate::Error::Validation(format!(
                "Execution plan exceeded maximum DAG recursion depth of {MAX_DAG_RECURSION_DEPTH}"
            )));
        }

        // Check if we've already seen this expression
        if let Some(&existing_id) = self.expr_cache.get(&expr) {
            return Ok(existing_id);
        }

        // Generate new ID and process dependencies
        let id = self.next_id;
        self.next_id += 1;

        let dependencies = match &expr.node {
            ExprNode::Column(_) | ExprNode::CSRef { .. } | ExprNode::Literal(_) => Vec::new(),
            ExprNode::Call(_, args) => args
                .iter()
                .map(|arg| self.process_expression(arg.clone(), depth + 1))
                .collect::<crate::Result<Vec<_>>>()?,
            ExprNode::BinOp { left, right, .. } => {
                vec![
                    self.process_expression((**left).clone(), depth + 1)?,
                    self.process_expression((**right).clone(), depth + 1)?,
                ]
            }
            ExprNode::UnaryOp { operand, .. } => {
                vec![self.process_expression((**operand).clone(), depth + 1)?]
            }
            ExprNode::IfThenElse {
                condition,
                then_expr,
                else_expr,
            } => {
                vec![
                    self.process_expression((**condition).clone(), depth + 1)?,
                    self.process_expression((**then_expr).clone(), depth + 1)?,
                    self.process_expression((**else_expr).clone(), depth + 1)?,
                ]
            }
        };

        // Estimate cost
        let cost = self.estimate_cost(&expr);

        // Create DAG node
        let node = DagNode {
            id,
            expr: expr.clone(),
            dependencies,
            ref_count: 0, // Will be calculated later
            cost,
        };

        // Store node and cache expression
        self.nodes.insert(id, node);
        self.expr_cache.insert(expr, id);

        Ok(id)
    }

    /// Calculate reference counts for all nodes.
    fn calculate_ref_counts(&mut self, root_ids: &[u64]) {
        let mut ref_counts: HashMap<u64, usize> = HashMap::default();
        let mut visited = HashSet::default();

        fn count_refs(
            node_id: u64,
            nodes: &HashMap<u64, DagNode>,
            ref_counts: &mut HashMap<u64, usize>,
            visited: &mut HashSet<u64>,
            depth: usize,
        ) {
            if depth >= MAX_DAG_RECURSION_DEPTH {
                return;
            }
            let first_visit = visited.insert(node_id);

            if let Some(node) = nodes.get(&node_id) {
                for &dep_id in &node.dependencies {
                    if first_visit {
                        *ref_counts.entry(dep_id).or_insert(0) += 1;
                        count_refs(dep_id, nodes, ref_counts, visited, depth + 1);
                    }
                }
            }
        }

        for &root_id in root_ids {
            count_refs(root_id, &self.nodes, &mut ref_counts, &mut visited, 0);
        }

        // Update nodes with reference counts
        for (id, count) in ref_counts {
            if let Some(node) = self.nodes.get_mut(&id) {
                node.ref_count = count;
            }
        }
    }

    /// Estimate the computational cost of an expression.
    fn estimate_cost(&self, expr: &Expr) -> usize {
        match &expr.node {
            ExprNode::Column(_) | ExprNode::CSRef { .. } | ExprNode::Literal(_) => 1,
            ExprNode::BinOp { .. } | ExprNode::UnaryOp { .. } => 2, // Basic arithmetic/comparison/logical operations
            ExprNode::IfThenElse { .. } => 3,                       // Conditional evaluation
            ExprNode::Call(func, args) => {
                let base_cost = match func {
                    Function::Lag
                    | Function::Lead
                    | Function::Shift
                    | Function::Sum
                    | Function::Mean => 5,
                    Function::Diff | Function::PctChange => 10,
                    Function::CumSum
                    | Function::CumProd
                    | Function::CumMin
                    | Function::CumMax
                    | Function::RollingCount => 20,
                    Function::RollingMean
                    | Function::RollingSum
                    | Function::RollingMin
                    | Function::RollingMax
                    | Function::Ttm
                    | Function::Ytd
                    | Function::Qtd
                    | Function::FiscalYtd => 30,
                    Function::RollingStd | Function::RollingVar | Function::RollingMedian => 50,
                    Function::EwmMean => 25,
                    Function::Std | Function::Var => 40,
                    Function::Median => 60,

                    // New functions
                    Function::Rank => 80,
                    Function::Quantile => 90,
                    Function::EwmStd | Function::EwmVar => 45,
                    // Custom financial functions
                    Function::Annualize | Function::Abs | Function::Sign => 2,
                    Function::AnnualizeRate | Function::Coalesce => 3, // Slightly more expensive due to powf
                    Function::GrowthRate => 35,
                };
                base_cost + args.len() * 5
            }
        }
    }

    /// Build topological ordering of nodes.
    fn topological_sort(&self, root_ids: &[u64]) -> crate::Result<Vec<DagNode>> {
        let mut visited = HashSet::default();
        let mut result = Vec::new();
        let mut visiting = HashSet::default();

        fn visit(
            node_id: u64,
            nodes: &HashMap<u64, DagNode>,
            visited: &mut HashSet<u64>,
            visiting: &mut HashSet<u64>,
            result: &mut Vec<DagNode>,
            depth: usize,
        ) -> crate::Result<()> {
            if depth >= MAX_DAG_RECURSION_DEPTH {
                return Err(crate::Error::Validation(format!(
                    "Execution plan exceeded maximum DAG recursion depth of {MAX_DAG_RECURSION_DEPTH}"
                )));
            }
            if visited.contains(&node_id) {
                return Ok(());
            }
            if visiting.contains(&node_id) {
                return Err(crate::Error::circular_dependency(vec![format!(
                    "expr_node_{node_id}"
                )]));
            }

            visiting.insert(node_id);

            if let Some(node) = nodes.get(&node_id) {
                for &dep_id in &node.dependencies {
                    visit(dep_id, nodes, visited, visiting, result, depth + 1)?;
                }
                result.push(node.clone());
            } else {
                return Err(crate::Error::Validation(format!(
                    "Execution plan references missing DAG node {node_id}"
                )));
            }

            visiting.remove(&node_id);
            visited.insert(node_id);
            Ok(())
        }

        for &root_id in root_ids {
            visit(
                root_id,
                &self.nodes,
                &mut visited,
                &mut visiting,
                &mut result,
                0,
            )?;
        }

        Ok(result)
    }

    /// Generate cache strategy based on node characteristics.
    fn generate_cache_strategy(&self, nodes: &[DagNode]) -> CacheStrategy {
        let mut cache_nodes = HashSet::default();
        let mut total_cost = 0;
        let mut cacheable_cost = 0;

        for node in nodes {
            total_cost += node.cost;

            // Cache nodes with high reference count or high cost
            let should_cache = node.ref_count > 1 && (node.cost > 30 || node.ref_count > 2);

            if should_cache {
                cache_nodes.insert(node.id);
                cacheable_cost += node.cost * (node.ref_count - 1);
            }
        }

        let expected_hit_rate = if total_cost > 0 {
            cacheable_cost as f64 / total_cost as f64
        } else {
            0.0
        };

        CacheStrategy {
            cache_nodes,
            expected_hit_rate,
            memory_budget: nodes.len() * 100, // Rough estimate
        }
    }
}
