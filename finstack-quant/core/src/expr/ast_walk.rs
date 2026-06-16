//! AST walks used to enforce layering invariants on the expression engine.
//!
//! The core scalar evaluator (`core::expr::eval`) rejects a small set of
//! period-aware functions (`Ttm`, `Ytd`, `Qtd`, `FiscalYtd`, `GrowthRate`,
//! plus the multi-arg reducers `Sum`/`Mean`/`Coalesce` and the annualization
//! helpers) that the `statements` crate evaluates with knowledge of the
//! period grid. Historically that rejection only fired at `eval()` time.
//!
//! This module exposes a parser/compile-time check that callers can use to
//! reject those functions earlier with a typed validation error.

use super::ast::{Expr, ExprNode};

/// Returns `Err(Error::Validation)` if `ast` (or any subexpression) calls a
/// function that the core scalar evaluator does not support.
///
/// The single source of truth for which functions belong in the statements
/// layer is [`super::ast::Function::is_scalar_evaluable`].
pub(crate) fn ensure_scalar_evaluable(ast: &Expr) -> crate::Result<()> {
    match &ast.node {
        ExprNode::Column(_) | ExprNode::CSRef { .. } | ExprNode::Literal(_) => Ok(()),
        ExprNode::Call(func, args) => {
            if !func.is_scalar_evaluable() {
                return Err(crate::Error::Validation(format!(
                    "Expression function '{func}' is a statements-layer function; \
                     compile it via the statements crate instead of core::expr::CompiledExpr::try_new_scalar"
                )));
            }
            for arg in args {
                ensure_scalar_evaluable(arg)?;
            }
            Ok(())
        }
        ExprNode::BinOp { left, right, .. } => {
            ensure_scalar_evaluable(left)?;
            ensure_scalar_evaluable(right)
        }
        ExprNode::UnaryOp { operand, .. } => ensure_scalar_evaluable(operand),
        ExprNode::IfThenElse {
            condition,
            then_expr,
            else_expr,
        } => {
            ensure_scalar_evaluable(condition)?;
            ensure_scalar_evaluable(then_expr)?;
            ensure_scalar_evaluable(else_expr)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::ast::{Expr, Function};

    #[test]
    fn scalar_functions_are_accepted() {
        let expr = Expr::call(
            Function::RollingMean,
            vec![Expr::column("x"), Expr::literal(3.0)],
        );
        assert!(ensure_scalar_evaluable(&expr).is_ok());
    }

    #[test]
    fn statements_functions_are_rejected_at_compile_time() {
        for func in [
            Function::Sum,
            Function::Mean,
            Function::Ttm,
            Function::Ytd,
            Function::Qtd,
            Function::FiscalYtd,
            Function::Annualize,
            Function::AnnualizeRate,
            Function::Coalesce,
            Function::GrowthRate,
        ] {
            let expr = Expr::call(func, vec![Expr::column("x")]);
            let err = ensure_scalar_evaluable(&expr)
                .expect_err("statements-layer functions must be rejected");
            assert!(matches!(err, crate::Error::Validation(_)));
        }
    }

    #[test]
    fn nested_statements_function_is_caught() {
        let inner = Expr::call(Function::Ttm, vec![Expr::column("x")]);
        let outer = Expr::bin_op(super::super::ast::BinOp::Add, inner, Expr::literal(1.0));
        assert!(ensure_scalar_evaluable(&outer).is_err());
    }
}
