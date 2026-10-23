//! Flat, deduplicated execution plans for the scalar expression evaluator.

use super::ast::*;
use crate::FxHashMap;

/// Scalar operation without recursively owned child expressions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Operation {
    /// Read a named input column.
    Column(String),
    /// Literal f64 bits, preserving signed zero and NaN payload identity.
    Literal(u64),
    /// Invoke an elementwise function.
    Call(Function),
    /// Apply a binary operator to two dependencies.
    BinOp(BinOp),
    /// Apply a unary operator to one dependency.
    UnaryOp(UnaryOp),
    /// Select between two branches using a condition dependency.
    IfThenElse,
}

/// One unique operation, emitted after all its dependencies.
#[derive(Debug, Clone)]
pub(crate) struct DagNode {
    /// Dense index of this node in the plan.
    pub id: u64,
    /// Scalar operation applied by this node.
    pub operation: Operation,
    /// Child node indices in the original operand order.
    pub dependencies: Vec<u64>,
}

const MAX_DAG_RECURSION_DEPTH: usize = 512;

/// Flat execution plan whose last node is the expression result.
#[derive(Debug, Clone)]
pub(crate) struct ExecutionPlan {
    /// Unique operations in dependency order.
    pub nodes: Vec<DagNode>,
    /// Index of the root operation.
    pub root: u64,
}

enum Frame {
    Visit(Expr, usize),
    Finish(Operation, usize),
}

/// Compile by iterative, left-to-right postorder traversal.
///
/// Depth and scalar-support validation happen before visiting children; neither
/// deduplication nor cleanup hashes, clones, or drops a recursive subtree.
pub(crate) fn build_plan(expr: Expr) -> crate::Result<ExecutionPlan> {
    let mut pending = vec![Frame::Visit(expr, 0)];
    let mut results = Vec::new();
    let mut nodes = Vec::new();
    let mut cache = FxHashMap::default();

    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Visit(expr, depth) => {
                if depth >= MAX_DAG_RECURSION_DEPTH {
                    pending.push(Frame::Visit(expr, depth));
                    discard_pending(pending);
                    return Err(crate::Error::Validation(format!(
                        "Execution plan exceeded maximum DAG recursion depth of {MAX_DAG_RECURSION_DEPTH}"
                    )));
                }

                let (operation, children) = match expr.node {
                    ExprNode::Column(name) => (Operation::Column(name), Vec::new()),
                    ExprNode::Literal(value) => (Operation::Literal(value.to_bits()), Vec::new()),
                    ExprNode::Call(function, children) => {
                        if !function.is_scalar_evaluable() {
                            pending
                                .extend(children.into_iter().map(|child| Frame::Visit(child, 0)));
                            discard_pending(pending);
                            return Err(crate::Error::Validation(format!(
                                "Expression function '{function}' is a statements-layer function; \
                                 compile it via the statements crate instead of core::expr::CompiledExpr::new"
                            )));
                        }
                        (Operation::Call(function), children)
                    }
                    ExprNode::BinOp { op, left, right } => {
                        (Operation::BinOp(op), vec![*left, *right])
                    }
                    ExprNode::UnaryOp { op, operand } => (Operation::UnaryOp(op), vec![*operand]),
                    ExprNode::IfThenElse {
                        condition,
                        then_expr,
                        else_expr,
                    } => (
                        Operation::IfThenElse,
                        vec![*condition, *then_expr, *else_expr],
                    ),
                    ExprNode::CsRef { .. } => {
                        discard_pending(pending);
                        return Err(crate::Error::Validation(
                            "capital-structure references require the statements evaluator"
                                .to_string(),
                        ));
                    }
                };
                pending.push(Frame::Finish(operation, children.len()));
                pending.extend(
                    children
                        .into_iter()
                        .rev()
                        .map(|child| Frame::Visit(child, depth + 1)),
                );
            }
            Frame::Finish(operation, child_count) => {
                let dependencies = results.split_off(results.len() - child_count);
                let key = (operation, dependencies);
                let id = if let Some(&id) = cache.get(&key) {
                    id
                } else {
                    let id = nodes.len() as u64;
                    nodes.push(DagNode {
                        id,
                        operation: key.0.clone(),
                        dependencies: key.1.clone(),
                    });
                    cache.insert(key, id);
                    id
                };
                results.push(id);
            }
        }
    }

    let root = results[0];
    debug_assert!(
        nodes
            .iter()
            .enumerate()
            .all(|(index, node)| node.id == index as u64
                && node.dependencies.iter().all(|&dep| dep < node.id)),
        "plan ids must be dense positions with dependencies emitted first"
    );
    debug_assert_eq!(
        root as usize,
        nodes.len() - 1,
        "the root must be the last emitted node"
    );
    Ok(ExecutionPlan { nodes, root })
}

/// Drain rejected owned trees without recursive `Expr` destruction.
fn discard_pending(mut pending: Vec<Frame>) {
    while let Some(frame) = pending.pop() {
        let Frame::Visit(expr, _) = frame else {
            continue;
        };
        match expr.node {
            ExprNode::Call(_, args) => {
                pending.extend(args.into_iter().map(|arg| Frame::Visit(arg, 0)))
            }
            ExprNode::BinOp { left, right, .. } => {
                pending.push(Frame::Visit(*left, 0));
                pending.push(Frame::Visit(*right, 0));
            }
            ExprNode::UnaryOp { operand, .. } => pending.push(Frame::Visit(*operand, 0)),
            ExprNode::IfThenElse {
                condition,
                then_expr,
                else_expr,
            } => {
                pending.push(Frame::Visit(*condition, 0));
                pending.push(Frame::Visit(*then_expr, 0));
                pending.push(Frame::Visit(*else_expr, 0));
            }
            ExprNode::Column(_) | ExprNode::Literal(_) | ExprNode::CsRef { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dag_builder_deduplicates_structurally_identical_nodes() {
        let a = Expr::call(Function::Pow, vec![Expr::column("x"), Expr::literal(3.0)]);
        let b = Expr::call(Function::Pow, vec![Expr::column("x"), Expr::literal(3.0)]);

        let plan = build_plan(Expr::bin_op(BinOp::Add, a, b))
            .expect("valid expressions should build a DAG plan");
        assert_eq!(plan.nodes.len(), 4, "column, literal, pow, add");
        assert_eq!(plan.nodes.last().map(|node| node.id), Some(plan.root));
    }

    #[test]
    fn dag_builder_orders_dependencies_topologically() {
        let col = Expr::column("x");
        let lit = Expr::literal(2.0);
        let pow = Expr::call(Function::Pow, vec![col.clone(), lit.clone()]);
        let round = Expr::call(Function::Round, vec![col, lit]);

        let plan = build_plan(Expr::bin_op(BinOp::Mul, pow, round))
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
    fn public_compilation_rejects_deep_trees_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                for unsupported_root in [false, true] {
                    let mut expr = Expr::column("x");
                    for _ in 0..100_000 {
                        expr = Expr::unary_op(UnaryOp::Neg, expr);
                    }
                    if unsupported_root {
                        expr = Expr::call(Function::Lag, vec![expr]);
                    }
                    let error = crate::expr::CompiledExpr::new(expr).expect_err("must reject");
                    let expected = if unsupported_root {
                        "statements-layer"
                    } else {
                        "maximum DAG recursion depth"
                    };
                    assert!(error.to_string().contains(expected), "{error}");
                }
            })
            .expect("spawn small-stack test")
            .join()
            .expect("compilation and error cleanup must not overflow");
    }

    #[test]
    fn public_compilation_rejects_the_depth_limit() {
        let mut expr = Expr::column("x");
        for _ in 0..MAX_DAG_RECURSION_DEPTH {
            expr = Expr::unary_op(UnaryOp::Neg, expr);
        }
        assert!(crate::expr::CompiledExpr::new(expr).is_err());
    }

    #[test]
    fn literal_identity_preserves_zero_sign_and_nan_payloads() {
        for (left, right) in [
            (0.0, -0.0),
            (
                f64::from_bits(0x7ff8_0000_0000_0001),
                f64::from_bits(0x7ff8_0000_0000_0002),
            ),
        ] {
            let plan = build_plan(Expr::bin_op(
                BinOp::Add,
                Expr::literal(left),
                Expr::literal(right),
            ))
            .expect("valid literals");
            assert_eq!(plan.nodes.len(), 3);
            assert_ne!(plan.nodes[0].operation, plan.nodes[1].operation);
        }
    }

    #[test]
    fn dag_build_plan_accepts_ast_below_depth_limit() {
        let mut expr = Expr::column("x");
        for _ in 0..MAX_DAG_RECURSION_DEPTH - 1 {
            expr = Expr::unary_op(UnaryOp::Neg, expr);
        }

        let plan = build_plan(expr).expect("AST below the depth limit should build a plan");
        assert_eq!(plan.nodes.len(), MAX_DAG_RECURSION_DEPTH);
    }

    #[test]
    fn dag_builder_keeps_dependency_chain_in_topological_order() {
        let col_x = Expr::column("x");
        let abs_x = Expr::call(Function::Abs, vec![col_x]);
        let sqrt_abs = Expr::call(Function::Sqrt, vec![abs_x]);

        let plan = build_plan(sqrt_abs).expect("valid expressions should build a DAG plan");

        let mut found_column = false;
        let mut found_abs = false;
        for node in &plan.nodes {
            match &node.operation {
                Operation::Column(_) => {
                    assert!(!found_column && !found_abs);
                    found_column = true;
                }
                Operation::Call(Function::Abs) => {
                    assert!(found_column && !found_abs);
                    found_abs = true;
                }
                Operation::Call(Function::Sqrt) => {
                    assert!(found_column && found_abs);
                }
                _ => {}
            }
        }
    }
}
