//! Flat, deduplicated execution plans for the scalar expression evaluator.

use super::ast::*;
use crate::FxHashMap;
use smallvec::{SmallVec, smallvec};

/// Scalar operation without recursively owned child expressions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Operation {
    /// Read a named input column.
    Column(String),
    /// Literal f64 bits, preserving signed zero identity.
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
    pub dependencies: SmallVec<[u64; 3]>,
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

enum Frame<'a> {
    Visit(&'a Expr, usize),
    Finish(Operation, usize),
}

fn allocation_error(error: std::collections::TryReserveError) -> crate::Error {
    crate::Error::Validation(format!(
        "could not allocate scalar expression plan: {error}"
    ))
}

fn call_operation(function: Function, children: &[Expr]) -> crate::Result<Operation> {
    if !function.is_scalar_evaluable() {
        return Err(crate::Error::Validation(format!(
            "Expression function '{function}' is a statements-layer function"
        )));
    }
    let arity = match function {
        Function::Pow => 2..=2,
        Function::Clamp => 3..=3,
        Function::Round => 1..=2,
        _ => 1..=1,
    };
    if !arity.contains(&children.len()) {
        return Err(crate::Error::Validation(format!(
            "{function} received {} arguments, expected {arity:?}",
            children.len()
        )));
    }
    if function == Function::Round && children.len() == 2 {
        match children[1].node {
            ExprNode::Literal(d)
                if (i32::MIN as f64..=i32::MAX as f64).contains(&d) && d.fract() == 0.0 => {}
            _ => {
                return Err(crate::Error::Validation(
                    "round digits must be a direct integer literal in the i32 range".into(),
                ));
            }
        }
    }
    Ok(Operation::Call(function))
}

/// Drain an owned tree iteratively on both successful compilation and rejection.
pub(crate) fn build_plan(expr: Expr) -> crate::Result<ExecutionPlan> {
    let result = build_borrowed_plan(&expr);
    discard_tree(expr);
    result
}

/// Compile a borrowed AST without recursively cloning it.
pub(crate) fn build_borrowed_plan(expr: &Expr) -> crate::Result<ExecutionPlan> {
    let mut pending = vec![Frame::Visit(expr, 0)];
    let mut results = Vec::new();
    let mut nodes = Vec::new();
    let mut cache = FxHashMap::default();
    let mut visited = 0;

    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Visit(expr, depth) => {
                visited += 1;
                if visited > 100_000 {
                    return Err(crate::Error::Validation(
                        "expression exceeds 100,000 syntax nodes".into(),
                    ));
                }
                if depth >= MAX_DAG_RECURSION_DEPTH {
                    return Err(crate::Error::Validation(format!(
                        "Execution plan exceeded maximum DAG recursion depth of {MAX_DAG_RECURSION_DEPTH}"
                    )));
                }
                let (operation, children): (_, SmallVec<[&Expr; 3]>) = match &expr.node {
                    ExprNode::Column(name) => (Operation::Column(name.clone()), smallvec![]),
                    ExprNode::Literal(value) if value.is_finite() => {
                        (Operation::Literal(value.to_bits()), smallvec![])
                    }
                    ExprNode::Literal(_) => {
                        return Err(crate::Error::Validation(
                            "scalar expression literal must be finite".into(),
                        ));
                    }
                    ExprNode::Call(function, children) => (
                        call_operation(*function, children)?,
                        children.iter().collect(),
                    ),
                    ExprNode::BinOp { op, left, right } => (
                        Operation::BinOp(*op),
                        smallvec![left.as_ref(), right.as_ref()],
                    ),
                    ExprNode::UnaryOp { op, operand } => {
                        (Operation::UnaryOp(*op), smallvec![operand.as_ref()])
                    }
                    ExprNode::IfThenElse {
                        condition,
                        then_expr,
                        else_expr,
                    } => (
                        Operation::IfThenElse,
                        smallvec![condition.as_ref(), then_expr.as_ref(), else_expr.as_ref()],
                    ),
                    ExprNode::CsRef { .. } => {
                        return Err(crate::Error::Validation(
                            "capital-structure references require the statements evaluator".into(),
                        ));
                    }
                };
                pending
                    .try_reserve(1 + children.len())
                    .map_err(allocation_error)?;
                pending.push(Frame::Finish(operation, children.len()));
                pending.extend(
                    children
                        .into_iter()
                        .rev()
                        .map(|child| Frame::Visit(child, depth + 1)),
                );
            }
            Frame::Finish(operation, child_count) => {
                // Scalar operations have at most three inputs. Their IDs stay
                // inline in both the DAG node and its structural deduplication key.
                let dependencies: SmallVec<[u64; 3]> =
                    results.drain(results.len() - child_count..).collect();
                let key = (operation, dependencies);
                let id = if let Some(&id) = cache.get(&key) {
                    id
                } else {
                    let id = nodes.len() as u64;
                    nodes.try_reserve(1).map_err(allocation_error)?;
                    cache.try_reserve(1).map_err(allocation_error)?;
                    nodes.push(DagNode {
                        id,
                        operation: key.0.clone(),
                        dependencies: key.1.clone(),
                    });
                    cache.insert(key, id);
                    id
                };
                results.try_reserve(1).map_err(allocation_error)?;
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
fn discard_tree(expr: Expr) {
    let mut pending = vec![expr];
    while let Some(expr) = pending.pop() {
        match expr.node {
            ExprNode::Call(_, args) => pending.extend(args),
            ExprNode::BinOp { left, right, .. } => {
                pending.push(*left);
                pending.push(*right);
            }
            ExprNode::UnaryOp { operand, .. } => pending.push(*operand),
            ExprNode::IfThenElse {
                condition,
                then_expr,
                else_expr,
            } => {
                pending.push(*condition);
                pending.push(*then_expr);
                pending.push(*else_expr);
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
    fn literal_identity_preserves_signed_zero_and_rejects_nonfinite_numbers() {
        let plan = build_plan(Expr::bin_op(
            BinOp::Add,
            Expr::literal(0.0),
            Expr::literal(-0.0),
        ))
        .unwrap();
        assert_eq!(plan.nodes.len(), 3);
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(build_plan(Expr::literal(invalid)).is_err());
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
    fn occurrence_limit_applies_before_subexpression_deduplication() {
        fn repeated(leaves: usize) -> Expr {
            if leaves == 1 {
                Expr::literal(1.0)
            } else {
                Expr::bin_op(
                    BinOp::Add,
                    repeated(leaves / 2),
                    repeated(leaves - leaves / 2),
                )
            }
        }
        // 50,000 leaves and 49,999 binary operators; the unary root makes 100,000.
        let admitted = Expr::unary_op(UnaryOp::Neg, repeated(50_000));
        assert!(build_borrowed_plan(&admitted).is_ok());
        let rejected = Expr::unary_op(UnaryOp::Neg, admitted);
        assert!(
            build_plan(rejected)
                .unwrap_err()
                .to_string()
                .contains("100,000")
        );
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
