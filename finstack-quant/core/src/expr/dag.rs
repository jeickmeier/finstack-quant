//! DAG planning and execution optimization for expressions.
//!
//! Detects shared sub-expressions in complex expression trees and builds an
//! optimized execution plan that evaluates each unique sub-expression only
//! once. Critical for performance in financial statement models with hundreds
//! of interdependent formulas.

use super::ast::*;
use crate::{FxHashMap, FxHashSet};
use std::vec::Vec;

/// A node in the execution DAG.
#[derive(Debug, Clone)]
pub(crate) struct DagNode {
    /// Unique identifier for this node.
    pub id: u64,
    /// The expression this node represents.
    pub expr: Expr,
    /// Dependencies (other DAG nodes this depends on).
    pub dependencies: Vec<u64>,
}

const MAX_DAG_RECURSION_DEPTH: usize = 512;

/// Execution plan for one expression.
#[derive(Debug, Clone)]
pub(crate) struct ExecutionPlan {
    /// All nodes in topological order.
    pub nodes: Vec<DagNode>,
    /// Root node id (the final output).
    pub root: u64,
}

/// DAG builder that detects shared sub-expressions and builds optimized execution plans.
#[derive(Default)]
pub(crate) struct DagBuilder {
    /// Expression cache for deduplication.
    expr_cache: FxHashMap<Expr, u64>,
    /// Node storage.
    nodes: FxHashMap<u64, DagNode>,
    /// Next available node ID.
    next_id: u64,
}

impl DagBuilder {
    /// Build an execution plan for one root expression.
    pub(crate) fn build_plan(&mut self, expr: Expr) -> crate::Result<ExecutionPlan> {
        self.expr_cache.clear();
        self.nodes.clear();
        self.next_id = 0;

        let root = self.process_expression(expr, 0)?;
        let nodes = self.topological_sort(root)?;

        Ok(ExecutionPlan { nodes, root })
    }

    /// Process an expression tree, deduplicating shared sub-expressions.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] when the expression nests deeper
    /// than [`MAX_DAG_RECURSION_DEPTH`]. The same guard protects
    /// `topological_sort`; without it here, a
    /// deeply nested programmatic AST would overflow the stack during plan
    /// construction before that guard is reached.
    fn process_expression(&mut self, expr: Expr, depth: usize) -> crate::Result<u64> {
        if depth >= MAX_DAG_RECURSION_DEPTH {
            return Err(crate::Error::Validation(format!(
                "Execution plan exceeded maximum DAG recursion depth of {MAX_DAG_RECURSION_DEPTH}"
            )));
        }

        if let Some(&existing_id) = self.expr_cache.get(&expr) {
            return Ok(existing_id);
        }

        let id = self.next_id;
        self.next_id += 1;

        let dependencies = match &expr.node {
            ExprNode::Column(_) | ExprNode::CsRef { .. } | ExprNode::Literal(_) => Vec::new(),
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

        let node = DagNode {
            id,
            expr: expr.clone(),
            dependencies,
        };

        self.nodes.insert(id, node);
        self.expr_cache.insert(expr, id);

        Ok(id)
    }

    /// Build topological ordering of nodes.
    fn topological_sort(&self, root: u64) -> crate::Result<Vec<DagNode>> {
        let mut visited = FxHashSet::default();
        let mut result = Vec::new();
        let mut visiting = FxHashSet::default();

        fn visit(
            node_id: u64,
            nodes: &FxHashMap<u64, DagNode>,
            visited: &mut FxHashSet<u64>,
            visiting: &mut FxHashSet<u64>,
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
                return Err(crate::Error::CircularDependency {
                    path: vec![format!("expr_node_{node_id}")],
                });
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

        visit(
            root,
            &self.nodes,
            &mut visited,
            &mut visiting,
            &mut result,
            0,
        )?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dag_builder_deduplicates_structurally_identical_nodes() {
        let mut builder = DagBuilder::default();
        let a = Expr::call(Function::Pow, vec![Expr::column("x"), Expr::literal(3.0)]);
        let b = Expr::call(Function::Pow, vec![Expr::column("x"), Expr::literal(3.0)]);

        let plan = builder
            .build_plan(Expr::bin_op(BinOp::Add, a, b))
            .expect("valid expressions should build a DAG plan");
        assert_eq!(plan.nodes.len(), 4, "column, literal, pow, add");
        assert_eq!(plan.nodes.last().map(|node| node.id), Some(plan.root));
    }

    #[test]
    fn dag_builder_orders_dependencies_topologically() {
        let mut builder = DagBuilder::default();
        let col = Expr::column("x");
        let lit = Expr::literal(2.0);
        let pow = Expr::call(Function::Pow, vec![col.clone(), lit.clone()]);
        let round = Expr::call(Function::Round, vec![col, lit]);

        let plan = builder
            .build_plan(Expr::bin_op(BinOp::Mul, pow, round))
            .expect("valid expressions should build a DAG plan");
        assert_eq!(plan.nodes.len(), 5);

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
        let mut builder = DagBuilder::default();
        let mut prev = 0_u64;
        for id in 1..=MAX_DAG_RECURSION_DEPTH as u64 + 1 {
            let dependencies = if prev == 0 { vec![] } else { vec![prev] };
            builder.nodes.insert(
                id,
                DagNode {
                    id,
                    expr: Expr::literal(id as f64),
                    dependencies,
                },
            );
            prev = id;
        }

        let err = builder
            .topological_sort(prev)
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

        let mut builder = DagBuilder::default();
        let err = builder
            .build_plan(expr)
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

        let mut builder = DagBuilder::default();
        let plan = builder
            .build_plan(expr)
            .expect("AST below the depth limit should build a plan");
        assert_eq!(plan.nodes.len(), MAX_DAG_RECURSION_DEPTH - 1);
    }

    #[test]
    fn dag_builder_keeps_dependency_chain_in_topological_order() {
        let mut builder = DagBuilder::default();
        let col_x = Expr::column("x");
        let abs_x = Expr::call(Function::Abs, vec![col_x]);
        let sqrt_abs = Expr::call(Function::Sqrt, vec![abs_x]);

        let plan = builder
            .build_plan(sqrt_abs)
            .expect("valid expressions should build a DAG plan");

        let mut found_column = false;
        let mut found_abs = false;
        for node in &plan.nodes {
            match &node.expr.node {
                ExprNode::Column(_) => {
                    assert!(!found_column && !found_abs);
                    found_column = true;
                }
                ExprNode::Call(Function::Abs, _) => {
                    assert!(found_column && !found_abs);
                    found_abs = true;
                }
                ExprNode::Call(Function::Sqrt, _) => {
                    assert!(found_column && found_abs);
                }
                _ => {}
            }
        }
    }
}
