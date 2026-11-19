//! Preflight validation of a formula AST.
//!
//! The text parser bounds what it builds, but [`StmtExpr`] has public
//! constructors, so a caller can hand [`crate::dsl::compile`] a tree the
//! parser would never produce. Every later consumer walks the tree
//! recursively, so this pass runs first and is itself iterative: it must not
//! overflow the stack on the trees it exists to reject.

use crate::dsl::ast::StmtExpr;
use crate::dsl::parser::MAX_FORMULA_TERMS;
use crate::dsl::signatures::is_valid_identifier;
use crate::dsl::signatures::{FUNCTION_SIGNATURES, FunctionSignature};
use crate::error::{Error, Result};
use finstack_quant_core::expr::UnaryOp;

/// Deepest formula tree accepted.
///
/// The text parser cannot build a deeper tree: a path holds at most
/// [`MAX_FORMULA_TERMS`] operator, call and conditional levels plus the 64
/// nesting levels the parser allows for unary chains. `dsl::stack_safety`
/// compiles a tree of exactly this depth on a 512 KiB stack.
pub(crate) const MAX_AST_DEPTH: usize = MAX_FORMULA_TERMS + 64;

/// Functions whose second argument is a period offset into history.
const OFFSET_FUNCTIONS: &[&str] = &["lag", "shift", "diff", "pct_change", "growth_rate"];

/// Validate shape, literals, names, signatures and size of a formula tree.
///
/// Checks every node, including branches evaluation would skip.
pub(crate) fn validate_ast(root: &StmtExpr) -> Result<()> {
    let mut terms = 0usize;
    let mut pending = vec![(root, 1usize)];
    while let Some((expr, depth)) = pending.pop() {
        if depth > MAX_AST_DEPTH {
            return Err(Error::formula_parse(format!(
                "formula nesting exceeds the maximum depth of {MAX_AST_DEPTH}. \
                 Split it into intermediate model nodes."
            )));
        }
        match expr {
            StmtExpr::BinOp { left, right, .. } => {
                pending.push((left, depth + 1));
                pending.push((right, depth + 1));
                continue;
            }
            StmtExpr::UnaryOp { operand, .. } => {
                pending.push((operand, depth + 1));
                continue;
            }
            StmtExpr::Literal(value) => {
                if !value.is_finite() {
                    return Err(Error::formula_parse(format!(
                        "formula literal must be finite, got {value}"
                    )));
                }
            }
            StmtExpr::NodeRef(id) => validate_node_reference(id.as_str())?,
            // A capital reference is typed: its component and scope were
            // checked when it was built.
            StmtExpr::CsRef { .. } => {}
            StmtExpr::AsMoney { value, .. } => {
                pending.push((value, depth + 1));
            }
            StmtExpr::Call { func, args } => {
                validate_call(func, args)?;
                pending.extend(args.iter().map(|arg| (arg, depth + 1)));
            }
            StmtExpr::IfThenElse {
                condition,
                then_expr,
                else_expr,
            } => {
                pending.push((condition, depth + 1));
                pending.push((then_expr, depth + 1));
                pending.push((else_expr, depth + 1));
            }
        }
        terms += 1;
        if terms > MAX_FORMULA_TERMS {
            return Err(Error::formula_parse(format!(
                "formula exceeds the maximum of {MAX_FORMULA_TERMS} terms. \
                 Split it into intermediate model nodes."
            )));
        }
    }
    Ok(())
}

fn validate_node_reference(id: &str) -> Result<()> {
    if !is_valid_identifier(id) {
        return Err(Error::formula_parse(format!(
            "invalid identifier '{id}': use nonempty dot-separated segments of letters, \
             digits and underscores, starting with a letter or underscore"
        )));
    }
    if id.starts_with("cs.") {
        return Err(Error::formula_parse(format!(
            "'{id}' is in the reserved capital-structure namespace; \
             use the form cs.<component>.<instrument, total or total.CCY>"
        )));
    }
    Ok(())
}

fn validate_call(func: &str, args: &[StmtExpr]) -> Result<()> {
    let Some(signature) = FunctionSignature::lookup(func) else {
        let supported = FUNCTION_SIGNATURES
            .iter()
            .map(|signature| signature.name)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Error::eval(format!(
            "Function '{func}' is not supported. Supported functions: {supported}"
        )));
    };
    if !signature.accepts(args.len()) {
        return Err(Error::eval(format!(
            "{func}() requires {}, got {}",
            signature.arity_text(),
            args.len()
        )));
    }
    if OFFSET_FUNCTIONS.contains(&func) && args.get(1).is_some_and(is_negative_literal) {
        return Err(Error::eval(format!(
            "{func}() offset must not be negative: a formula cannot read a future period"
        )));
    }
    Ok(())
}

/// Whether `expr` is a literal below zero, written directly or as a negation.
fn is_negative_literal(expr: &StmtExpr) -> bool {
    match expr {
        StmtExpr::Literal(value) => *value < 0.0,
        StmtExpr::UnaryOp {
            op: UnaryOp::Neg,
            operand,
        } => matches!(operand.as_ref(), StmtExpr::Literal(value) if *value > 0.0),
        _ => false,
    }
}
