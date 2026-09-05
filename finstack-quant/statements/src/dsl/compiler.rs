//! Compiler from Statements DSL AST to core Expr.

use crate::dsl::ast::{BinOp as StmtBinOp, StmtExpr, UnaryOp as StmtUnaryOp};
use crate::error::Result;
use crate::types::{NodeId, NodeValueType};
use finstack_quant_core::expr::{BinOp as CoreBinOp, Expr, Function, UnaryOp as CoreUnaryOp};
use indexmap::IndexMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dimension {
    Unknown,
    Scalar,
    Monetary(finstack_quant_core::currency::Currency),
}

/// Compile a [`StmtExpr`] into a core [`Expr`].
///
/// Converts the statements DSL syntax into the shared expression engine
/// representation used by the evaluator.
/// Capital-structure references are checked against the fixed component
/// vocabulary at this boundary and encoded as the core expression engine's
/// dedicated `cs` reference form.
///
/// # Arguments
///
/// * `ast` - Parsed statements DSL expression to lower into the core expression
///   representation.
///
/// # Errors
///
/// Returns an evaluation error when a capital-structure reference names an
/// unsupported component, or when a nested operator/function expression is
/// invalid for compilation. Parsing is a separate step; use
/// [`crate::dsl::parse_and_compile`] when starting from source text.
///
pub fn compile(ast: &StmtExpr) -> Result<Expr> {
    match ast {
        StmtExpr::Literal(val) => Ok(Expr::literal(*val)),

        StmtExpr::NodeRef(name) => Ok(Expr::column(name.as_str().to_string())),

        // Capital structure references are encoded as special column names
        // Format: __cs__component__instrument_or_total
        StmtExpr::CsRef {
            component,
            instrument_or_total,
        } => {
            // Validate component name at compile time to catch typos early
            const VALID_CS_COMPONENTS: &[&str] = &[
                "interest_expense",
                "interest_expense_cash",
                "interest_expense_pik",
                "interest_income",
                "principal_payment",
                "debt_balance",
                "fees",
                "accrued_interest",
            ];
            if !VALID_CS_COMPONENTS.contains(&component.as_str()) {
                return Err(crate::error::Error::eval(format!(
                    "Unknown capital structure component: '{}'. Valid components: {}",
                    component,
                    VALID_CS_COMPONENTS.join(", ")
                )));
            }

            Ok(Expr::cs_ref(component.clone(), instrument_or_total.clone()))
        }

        StmtExpr::BinOp { op, left, right } => compile_bin_op(*op, left, right),

        StmtExpr::UnaryOp { op, operand } => compile_unary_op(*op, operand),

        StmtExpr::Call { func, args } => compile_function_call(func, args),

        StmtExpr::IfThenElse {
            condition,
            then_expr,
            else_expr,
        } => compile_if_then_else(condition, then_expr, else_expr),
    }
}

/// Validate monetary/scalar dimensional compatibility for a formula AST.
///
/// `node_types` describes known model outputs. Unknown references remain
/// dimension-unknown so they can be resolved later; known monetary operands
/// must be compatible for addition, subtraction, comparison, and conditional
/// branches. This catches currency-unit mistakes before numerical evaluation.
///
/// `FinancialModelSpec::validate_semantics` repeatedly applies the companion
/// inference routine through formula dependencies, so calculated monetary
/// nodes retain currency and indirect cross-currency operations are rejected.
/// This standalone validator checks only the `node_types` supplied by its
/// caller; references absent from that map remain unknown. Variance-like
/// squared units also remain unknown because `NodeValueType` intentionally
/// represents only scalar and monetary outputs.
///
/// # Arguments
///
/// * `ast` - Parsed statements DSL expression whose known value dimensions are
///   checked before evaluation.
/// * `node_types` - Known node output types keyed by node ID; references absent
///   from this map remain dimension-unknown.
///
/// # Errors
///
/// Returns an error when the expression combines incompatible known dimensions
/// (for example, currencies that cannot be added), uses a monetary value where
/// a scalar-only operation is required, or supplies an invalid function
/// dimension. It does not prove the units of references absent from
/// `node_types`.
pub fn validate_dimensions(
    ast: &StmtExpr,
    node_types: &IndexMap<NodeId, NodeValueType>,
) -> Result<()> {
    infer_dimension(ast, node_types, None)?;
    Ok(())
}

/// Infer a formula's output type from known node types and capital-structure
/// reporting currency.
pub(crate) fn infer_value_type(
    ast: &StmtExpr,
    node_types: &IndexMap<NodeId, NodeValueType>,
    capital_structure_currency: Option<finstack_quant_core::currency::Currency>,
) -> Result<Option<NodeValueType>> {
    Ok(
        match infer_dimension(ast, node_types, capital_structure_currency)? {
            Dimension::Unknown => None,
            Dimension::Scalar => Some(NodeValueType::Scalar),
            Dimension::Monetary(currency) => Some(NodeValueType::Monetary { currency }),
        },
    )
}

fn infer_dimension(
    ast: &StmtExpr,
    node_types: &IndexMap<NodeId, NodeValueType>,
    capital_structure_currency: Option<finstack_quant_core::currency::Currency>,
) -> Result<Dimension> {
    match ast {
        StmtExpr::Literal(_) => Ok(Dimension::Scalar),
        StmtExpr::NodeRef(name) => Ok(node_types
            .get(name)
            .map(node_value_type_to_dimension)
            .unwrap_or(Dimension::Unknown)),
        StmtExpr::CsRef { .. } => Ok(capital_structure_currency
            .map(Dimension::Monetary)
            .unwrap_or(Dimension::Unknown)),
        StmtExpr::UnaryOp { op, operand } => {
            let dim = infer_dimension(operand, node_types, capital_structure_currency)?;
            match op {
                StmtUnaryOp::Neg => Ok(dim),
                StmtUnaryOp::Not => Ok(Dimension::Scalar),
            }
        }
        StmtExpr::BinOp { op, left, right } => {
            let left_dim = infer_dimension(left, node_types, capital_structure_currency)?;
            let right_dim = infer_dimension(right, node_types, capital_structure_currency)?;
            infer_bin_op_dimension(*op, left_dim, right_dim)
        }
        StmtExpr::Call { func, args } => {
            infer_call_dimension(func, args, node_types, capital_structure_currency)
        }
        StmtExpr::IfThenElse {
            condition,
            then_expr,
            else_expr,
        } => {
            infer_dimension(condition, node_types, capital_structure_currency)?;
            let then_dim = infer_dimension(then_expr, node_types, capital_structure_currency)?;
            let else_dim = infer_dimension(else_expr, node_types, capital_structure_currency)?;
            compatible_dimensions("if branches", then_dim, else_dim)
        }
    }
}

fn node_value_type_to_dimension(value_type: &NodeValueType) -> Dimension {
    match value_type {
        NodeValueType::Monetary { currency } => Dimension::Monetary(*currency),
        NodeValueType::Scalar => Dimension::Scalar,
    }
}

fn infer_bin_op_dimension(op: StmtBinOp, left: Dimension, right: Dimension) -> Result<Dimension> {
    match op {
        StmtBinOp::Add | StmtBinOp::Sub => {
            compatible_dimensions("arithmetic operands", left, right)
        }
        StmtBinOp::Mul => multiply_dimensions(left, right),
        StmtBinOp::Div => divide_dimensions(left, right),
        StmtBinOp::Mod => require_scalar_operands("modulo", left, right),
        StmtBinOp::Eq
        | StmtBinOp::Ne
        | StmtBinOp::Lt
        | StmtBinOp::Le
        | StmtBinOp::Gt
        | StmtBinOp::Ge => {
            compatible_dimensions("comparison operands", left, right)?;
            Ok(Dimension::Scalar)
        }
        StmtBinOp::And | StmtBinOp::Or => require_scalar_operands("logical operands", left, right),
    }
}

fn compatible_dimensions(context: &str, left: Dimension, right: Dimension) -> Result<Dimension> {
    match (left, right) {
        (Dimension::Unknown, _) | (_, Dimension::Unknown) => Ok(Dimension::Unknown),
        (Dimension::Scalar, Dimension::Scalar) => Ok(Dimension::Scalar),
        (Dimension::Monetary(lhs), Dimension::Monetary(rhs)) if lhs == rhs => {
            Ok(Dimension::Monetary(lhs))
        }
        (Dimension::Monetary(lhs), Dimension::Monetary(rhs)) => Err(crate::error::Error::build(
            format!("Dimensional mismatch in {context}: cannot combine {lhs} and {rhs}"),
        )),
        (Dimension::Scalar, Dimension::Monetary(ccy))
        | (Dimension::Monetary(ccy), Dimension::Scalar) => Err(crate::error::Error::build(
            format!("Dimensional mismatch in {context}: cannot combine scalar and {ccy}"),
        )),
    }
}

fn multiply_dimensions(left: Dimension, right: Dimension) -> Result<Dimension> {
    match (left, right) {
        (Dimension::Unknown, _) | (_, Dimension::Unknown) => Ok(Dimension::Unknown),
        (Dimension::Scalar, Dimension::Scalar) => Ok(Dimension::Scalar),
        (Dimension::Scalar, Dimension::Monetary(ccy))
        | (Dimension::Monetary(ccy), Dimension::Scalar) => Ok(Dimension::Monetary(ccy)),
        (Dimension::Monetary(lhs), Dimension::Monetary(rhs)) => Err(crate::error::Error::build(
            format!("Dimensional mismatch in multiplication: cannot multiply {lhs} by {rhs}"),
        )),
    }
}

fn divide_dimensions(left: Dimension, right: Dimension) -> Result<Dimension> {
    match (left, right) {
        (Dimension::Unknown, _) | (_, Dimension::Unknown) => Ok(Dimension::Unknown),
        (Dimension::Scalar, Dimension::Scalar) => Ok(Dimension::Scalar),
        (Dimension::Monetary(ccy), Dimension::Scalar) => Ok(Dimension::Monetary(ccy)),
        (Dimension::Monetary(lhs), Dimension::Monetary(rhs)) if lhs == rhs => Ok(Dimension::Scalar),
        (Dimension::Monetary(lhs), Dimension::Monetary(rhs)) => Err(crate::error::Error::build(
            format!("Dimensional mismatch in division: cannot divide {lhs} by {rhs}"),
        )),
        (Dimension::Scalar, Dimension::Monetary(ccy)) => Err(crate::error::Error::build(format!(
            "Dimensional mismatch in division: cannot divide scalar by {ccy}"
        ))),
    }
}

fn require_scalar_operands(context: &str, left: Dimension, right: Dimension) -> Result<Dimension> {
    match (left, right) {
        (Dimension::Unknown, _)
        | (_, Dimension::Unknown)
        | (Dimension::Scalar, Dimension::Scalar) => Ok(Dimension::Scalar),
        _ => Err(crate::error::Error::build(format!(
            "Dimensional mismatch in {context}: operands must be scalar"
        ))),
    }
}

fn infer_call_dimension(
    func: &str,
    args: &[StmtExpr],
    node_types: &IndexMap<NodeId, NodeValueType>,
    capital_structure_currency: Option<finstack_quant_core::currency::Currency>,
) -> Result<Dimension> {
    let arg_dims: Vec<_> = args
        .iter()
        .map(|arg| infer_dimension(arg, node_types, capital_structure_currency))
        .collect::<Result<Vec<_>>>()?;
    match func {
        // Dimension-preserving: the result carries the same units as its
        // value-bearing argument(s). Includes same-unit transforms such as
        // `diff` (difference of amounts), `std`/`rolling_std`/`ewm_std`
        // (dispersion in the series' own units), and `min`/`max`/`median`.
        "abs" | "lag" | "shift" | "cumsum" | "cummin" | "cummax" | "rolling_mean"
        | "rolling_sum" | "rolling_min" | "rolling_max" | "mean" | "sum" | "min" | "max"
        | "median" | "rolling_median" | "diff" | "std" | "rolling_std" | "ewm_mean" | "ewm_std"
        | "ttm" | "ltm" | "ytd" | "qtd" | "fiscal_ytd" | "annualize" | "coalesce" | "clamp" => {
            combine_arg_dimensions(func, arg_dims)
        }
        // Dimension-preserving in the *first* argument only. `floor`/`ceil`
        // preserve their single argument's units; for `round`, the trailing
        // `digits` argument is a dimensionless count, so folding it with
        // `combine_arg_dimensions` would wrongly reject `round(usd_amount, 2)`
        // as a monetary/scalar mix.
        "round" | "floor" | "ceil" => Ok(arg_dims.first().copied().unwrap_or(Dimension::Unknown)),
        // Genuinely scalar: ratios, counts, signs, and rates carry no currency
        // unit regardless of input. Transcendentals (`pow`, `ln`, `exp`,
        // `log10`, `sqrt`) only make dimensional sense on scalars, and the
        // missing-value predicate returns a 0/1 flag.
        "sign" | "pct_change" | "cumprod" | "rolling_count" | "rank" | "quantile"
        | "annualize_rate" | "growth_rate" | "pow" | "ln" | "exp" | "log10" | "sqrt"
        | "is_missing" => Ok(Dimension::Scalar),
        // Everything else defers to `Unknown`. This deliberately includes the
        // variance family (`var` / `rolling_var` / `ewm_var`), which yields
        // squared units that this three-valued dimension system (Unknown /
        // Scalar / Monetary) cannot represent — deferring neither falsely
        // rejects nor falsely passes downstream combinations.
        _ => Ok(Dimension::Unknown),
    }
}

fn combine_arg_dimensions(context: &str, arg_dims: Vec<Dimension>) -> Result<Dimension> {
    // Seed the fold with the FIRST argument's dimension rather than
    // `Dimension::Unknown`. `Unknown` is absorbing in `compatible_dimensions`
    // (`(Unknown, _) => Unknown`), so seeding with it would make the fold
    // permanently `Unknown` and silently accept e.g. `min(usd, eur)`.
    let mut iter = arg_dims.into_iter();
    let Some(first) = iter.next() else {
        return Ok(Dimension::Unknown);
    };
    iter.try_fold(first, |acc, dim| compatible_dimensions(context, acc, dim))
}

/// Compile binary operations.
fn compile_bin_op(op: StmtBinOp, left: &StmtExpr, right: &StmtExpr) -> Result<Expr> {
    let left_expr = compile(left)?;
    let right_expr = compile(right)?;

    // Map statement BinOp to core BinOp
    let core_op = match op {
        StmtBinOp::Add => CoreBinOp::Add,
        StmtBinOp::Sub => CoreBinOp::Sub,
        StmtBinOp::Mul => CoreBinOp::Mul,
        StmtBinOp::Div => CoreBinOp::Div,
        StmtBinOp::Mod => CoreBinOp::Mod,
        StmtBinOp::Eq => CoreBinOp::Eq,
        StmtBinOp::Ne => CoreBinOp::Ne,
        StmtBinOp::Lt => CoreBinOp::Lt,
        StmtBinOp::Le => CoreBinOp::Le,
        StmtBinOp::Gt => CoreBinOp::Gt,
        StmtBinOp::Ge => CoreBinOp::Ge,
        StmtBinOp::And => CoreBinOp::And,
        StmtBinOp::Or => CoreBinOp::Or,
    };

    Ok(Expr::bin_op(core_op, left_expr, right_expr))
}

/// Compile unary operations.
fn compile_unary_op(op: StmtUnaryOp, operand: &StmtExpr) -> Result<Expr> {
    let operand_expr = compile(operand)?;

    let core_op = match op {
        StmtUnaryOp::Neg => CoreUnaryOp::Neg,
        StmtUnaryOp::Not => CoreUnaryOp::Not,
    };

    Ok(Expr::unary_op(core_op, operand_expr))
}

/// Compile function calls.
fn compile_function_call(func_name: &str, args: &[StmtExpr]) -> Result<Expr> {
    let compiled_args: Result<Vec<_>> = args.iter().map(compile).collect();
    let compiled_args = compiled_args?;

    // Map DSL function names to core Function enum
    let func = match func_name {
        "lag" => Some(Function::Lag),
        // `lead` is intentionally unsupported: forward-looking references
        // silently corrupt historical model cells and backtests.
        "diff" => Some(Function::Diff),
        "pct_change" => Some(Function::PctChange),
        "cumsum" => Some(Function::CumSum),
        "cumprod" => Some(Function::CumProd),
        "cummin" => Some(Function::CumMin),
        "cummax" => Some(Function::CumMax),
        "rolling_mean" => Some(Function::RollingMean),
        "rolling_sum" => Some(Function::RollingSum),
        "rolling_std" => Some(Function::RollingStd),
        "rolling_var" => Some(Function::RollingVar),
        "rolling_median" => Some(Function::RollingMedian),
        "rolling_min" => Some(Function::RollingMin),
        "rolling_max" => Some(Function::RollingMax),
        "rolling_count" => Some(Function::RollingCount),
        "ewm_mean" => Some(Function::EwmMean),
        "ewm_std" => Some(Function::EwmStd),
        "ewm_var" => Some(Function::EwmVar),
        "std" => Some(Function::Std),
        "var" => Some(Function::Var),
        "median" => Some(Function::Median),
        "shift" => Some(Function::Shift),
        "rank" => Some(Function::Rank),
        "quantile" => Some(Function::Quantile),
        "sum" => Some(Function::Sum),
        "mean" => Some(Function::Mean),
        "min" => Some(Function::Min),
        "max" => Some(Function::Max),
        "ttm" | "ltm" => Some(Function::Ttm),
        "ytd" => Some(Function::Ytd),
        "qtd" => Some(Function::Qtd),
        "fiscal_ytd" => Some(Function::FiscalYtd),
        "annualize" => Some(Function::Annualize),
        "annualize_rate" => Some(Function::AnnualizeRate),
        "coalesce" => Some(Function::Coalesce),
        "abs" => Some(Function::Abs),
        "sign" => Some(Function::Sign),
        "growth_rate" => Some(Function::GrowthRate),
        "pow" => Some(Function::Pow),
        "round" => Some(Function::Round),
        "floor" => Some(Function::Floor),
        "ceil" => Some(Function::Ceil),
        "ln" => Some(Function::Ln),
        "exp" => Some(Function::Exp),
        "log10" => Some(Function::Log10),
        "sqrt" => Some(Function::Sqrt),
        "clamp" => Some(Function::Clamp),
        "is_missing" => Some(Function::IsMissing),
        _ => None,
    };

    if let Some(f) = func {
        match f {
            Function::Sum | Function::Mean | Function::Min | Function::Max => {
                if compiled_args.is_empty() {
                    return Err(crate::error::Error::eval(format!(
                        "{:?} requires at least one argument",
                        f
                    )));
                }
            }
            Function::Abs
            | Function::Sign
            | Function::Floor
            | Function::Ceil
            | Function::Ln
            | Function::Exp
            | Function::Log10
            | Function::Sqrt
            | Function::IsMissing => {
                if compiled_args.len() != 1 {
                    return Err(crate::error::Error::eval(format!(
                        "{:?} requires exactly 1 argument",
                        f
                    )));
                }
            }
            Function::Pow => {
                if compiled_args.len() != 2 {
                    return Err(crate::error::Error::eval(
                        "pow() requires exactly 2 arguments (base, exponent)",
                    ));
                }
            }
            Function::Round => {
                if compiled_args.is_empty() || compiled_args.len() > 2 {
                    return Err(crate::error::Error::eval(
                        "round() requires 1 or 2 arguments (value, [digits])",
                    ));
                }
            }
            Function::Clamp => {
                if compiled_args.len() != 3 {
                    return Err(crate::error::Error::eval(
                        "clamp() requires exactly 3 arguments (value, lo, hi)",
                    ));
                }
            }
            Function::Ttm => {
                if compiled_args.len() != 1 {
                    return Err(crate::error::Error::eval(
                        "ttm()/ltm() require exactly 1 argument",
                    ));
                }
            }
            Function::Ytd => {
                if compiled_args.len() != 1 {
                    return Err(crate::error::Error::eval(
                        "ytd() requires exactly 1 argument",
                    ));
                }
            }
            Function::Qtd => {
                if compiled_args.len() != 1 {
                    return Err(crate::error::Error::eval(
                        "qtd() requires exactly 1 argument",
                    ));
                }
            }
            Function::FiscalYtd => {
                if compiled_args.len() != 2 {
                    return Err(crate::error::Error::eval(
                        "fiscal_ytd() requires 2 arguments (expr, fiscal_start_month)",
                    ));
                }
            }
            Function::Annualize => {
                if compiled_args.is_empty() || compiled_args.len() > 2 {
                    return Err(crate::error::Error::eval(
                        "annualize() requires 1 or 2 arguments (value, [periods_per_year])",
                    ));
                }
            }
            Function::AnnualizeRate => {
                if compiled_args.len() != 3 {
                    return Err(crate::error::Error::eval(
                        "annualize_rate() requires 3 arguments (rate, periods_per_year, compounding)",
                    ));
                }
            }
            Function::Coalesce => {
                if compiled_args.len() < 2 {
                    return Err(crate::error::Error::eval(
                        "coalesce() requires at least 2 arguments",
                    ));
                }
            }
            Function::GrowthRate => {
                if compiled_args.is_empty() || compiled_args.len() > 2 {
                    return Err(crate::error::Error::eval(
                        "growth_rate() requires 1 or 2 arguments (series, [periods])",
                    ));
                }
            }
            Function::Lag | Function::Shift | Function::EwmMean | Function::Quantile => {
                if compiled_args.len() != 2 {
                    return Err(crate::error::Error::eval(format!(
                        "{}() requires exactly 2 arguments",
                        func_name
                    )));
                }
            }
            Function::RollingMean
            | Function::RollingSum
            | Function::RollingStd
            | Function::RollingVar
            | Function::RollingMedian
            | Function::RollingMin
            | Function::RollingMax
            | Function::RollingCount => {
                if compiled_args.len() < 2 || compiled_args.len() > 3 {
                    return Err(crate::error::Error::eval(format!(
                        "{}() requires 2 or 3 arguments (series, window, [min_periods])",
                        func_name
                    )));
                }
            }
            Function::Diff | Function::PctChange => {
                if compiled_args.is_empty() || compiled_args.len() > 2 {
                    return Err(crate::error::Error::eval(format!(
                        "{}() requires 1 or 2 arguments",
                        func_name
                    )));
                }
            }
            Function::EwmStd | Function::EwmVar => {
                if compiled_args.len() < 2 || compiled_args.len() > 3 {
                    return Err(crate::error::Error::eval(format!(
                        "{}() requires 2 or 3 arguments",
                        func_name
                    )));
                }
            }
            Function::Rank => {
                if compiled_args.is_empty() {
                    return Err(crate::error::Error::eval(
                        "rank() requires at least 1 argument",
                    ));
                }
            }
            Function::CumSum
            | Function::CumProd
            | Function::CumMin
            | Function::CumMax
            | Function::Std
            | Function::Var
            | Function::Median => {
                if compiled_args.is_empty() {
                    return Err(crate::error::Error::eval(format!(
                        "{}() requires at least 1 argument",
                        func_name
                    )));
                }
            }
            Function::Lead => {}
        }
        Ok(Expr::call(f, compiled_args))
    } else {
        Err(crate::error::Error::eval(format!(
            "Function '{}' is not supported. \
             Supported functions include: lag, diff, pct_change, rolling_*, ewm_*, std, var, median, \
             sum, mean, min, max, ttm/ltm, ytd, qtd, fiscal_ytd, annualize, growth_rate, abs, sign, coalesce, \
             pow, round, floor, ceil, ln, exp, log10, sqrt, clamp, is_missing",
            func_name
        )))
    }
}

/// Compile if-then-else expressions.
fn compile_if_then_else(
    condition: &StmtExpr,
    then_expr: &StmtExpr,
    else_expr: &StmtExpr,
) -> Result<Expr> {
    let cond = compile(condition)?;
    let then_branch = compile(then_expr)?;
    let else_branch = compile(else_expr)?;

    Ok(Expr::if_then_else(cond, then_branch, else_branch))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::parse_formula;
    use finstack_quant_core::expr::ExprNode;

    #[test]
    fn test_compile_literal() {
        let ast = StmtExpr::literal(42.0);
        let expr = compile(&ast).expect("should compile successfully");

        match expr.node {
            ExprNode::Literal(v) => assert_eq!(v, 42.0),
            _ => panic!("Expected Literal"),
        }
    }

    #[test]
    fn test_compile_node_ref() {
        let ast = StmtExpr::node_ref("revenue");
        let expr = compile(&ast).expect("should compile successfully");

        match expr.node {
            ExprNode::Column(ref name) => assert_eq!(name, "revenue"),
            _ => panic!("Expected Column"),
        }
    }

    #[test]
    fn test_compile_addition() {
        let ast = StmtExpr::bin_op(
            StmtBinOp::Add,
            StmtExpr::literal(1.0),
            StmtExpr::literal(2.0),
        );

        let expr = compile(&ast).expect("should compile successfully");

        // Should compile to a BinOp expression
        match expr.node {
            ExprNode::BinOp { .. } => {}
            _ => panic!("Expected BinOp for arithmetic"),
        }
    }

    #[test]
    fn test_compile_function_lag() {
        let ast = StmtExpr::call(
            "lag",
            vec![StmtExpr::node_ref("revenue"), StmtExpr::literal(1.0)],
        );

        let expr = compile(&ast).expect("should compile successfully");

        match expr.node {
            ExprNode::Call(Function::Lag, args) => {
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected Lag function call"),
        }
    }

    #[test]
    fn test_compile_from_parse() {
        let ast = parse_formula("revenue - cogs").expect("should parse successfully");
        let expr = compile(&ast).expect("should compile successfully");

        // Should compile successfully to a BinOp
        match expr.node {
            ExprNode::BinOp { .. } => {}
            _ => panic!("Expected BinOp for subtraction"),
        }
    }

    #[test]
    fn test_compile_complex_expression() {
        let ast = parse_formula("(revenue - cogs) / revenue").expect("should parse successfully");
        let expr = compile(&ast);

        assert!(expr.is_ok());
    }

    /// Rolling functions accept an optional third `min_periods` argument
    /// (pandas parity); one or four-plus arguments stay rejected.
    #[test]
    fn rolling_functions_accept_optional_min_periods() {
        for formula in ["rolling_mean(x, 4)", "rolling_mean(x, 4, 2)"] {
            let ast = parse_formula(formula).expect("should parse");
            assert!(compile(&ast).is_ok(), "should compile: {formula}");
        }
        for formula in ["rolling_mean(x)", "rolling_mean(x, 4, 2, 1)"] {
            let ast = parse_formula(formula).expect("should parse");
            assert!(compile(&ast).is_err(), "should reject: {formula}");
        }
    }

    /// Element-wise math helpers compile with the expected arities, and the
    /// dimension checker treats `round`'s `digits` argument as a unit-free
    /// count while still folding `clamp`'s bounds with its value.
    #[test]
    fn elementwise_math_arities_and_dimensions() {
        for formula in [
            "pow(1 + growth, 4)",
            "round(revenue, 2)",
            "round(revenue)",
            "floor(revenue)",
            "ceil(revenue)",
            "ln(revenue)",
            "exp(growth)",
            "log10(revenue)",
            "sqrt(revenue)",
            "clamp(margin, 0, 1)",
            "is_missing(revenue)",
        ] {
            let ast = parse_formula(formula).expect("should parse");
            assert!(compile(&ast).is_ok(), "should compile: {formula}");
        }

        let usd = NodeValueType::Monetary {
            currency: finstack_quant_core::currency::Currency::USD,
        };
        let eur = NodeValueType::Monetary {
            currency: finstack_quant_core::currency::Currency::EUR,
        };
        let mut node_types = IndexMap::new();
        node_types.insert(NodeId::new("usd_amount"), usd);
        node_types.insert(NodeId::new("eur_amount"), eur);

        // `round(monetary, digits)` must not be rejected as a monetary/scalar
        // mix: the digits argument is a dimensionless count.
        let ast = parse_formula("round(usd_amount, 2) + usd_amount").expect("should parse");
        assert!(validate_dimensions(&ast, &node_types).is_ok());

        // `clamp` folds value and bounds: mixing currencies is an error.
        let ast = parse_formula("clamp(usd_amount, eur_amount, usd_amount)").expect("should parse");
        assert!(validate_dimensions(&ast, &node_types).is_err());

        // ...while same-currency bounds are fine and preserve the unit.
        let ast = parse_formula("clamp(usd_amount, 0 * usd_amount, usd_amount) + usd_amount")
            .expect("should parse");
        assert!(validate_dimensions(&ast, &node_types).is_ok());
    }

    // Regression (C8): `min`/`max` now compile to a single n-ary
    // `Function::Min`/`Max` node rather than a nested if-then-else tree whose
    // size doubled per argument (an O(2^n) memory-exhaustion DoS reachable from
    // any inbound formula). A flat `min` over many arguments must compile to one
    // small call node, not an exponential tree.
    #[test]
    fn test_minmax_compiles_to_single_narity_node_not_exponential_tree() {
        let args = (0..40).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let formula = format!("min({args})");
        let ast = parse_formula(&formula).expect("should parse");
        let expr = compile(&ast).expect("should compile");
        match &expr.node {
            ExprNode::Call(Function::Min, call_args) => {
                assert_eq!(call_args.len(), 40, "all 40 args preserved on one node");
            }
            other => panic!("expected a single Function::Min call node, got {other:?}"),
        }
    }
}
