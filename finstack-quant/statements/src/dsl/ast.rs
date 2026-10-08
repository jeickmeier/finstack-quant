//! Abstract Syntax Tree for the Statements DSL.

use crate::types::NodeId;
use serde::{Deserialize, Serialize};

/// Statements DSL expression AST.
///
/// Represents parsed formula syntax before compilation to the core expression
/// engine. Each variant captures a syntactic construct in the DSL.
// NOTE: `StmtExpr` deliberately does NOT derive `Serialize`/`Deserialize`.
// serde cannot represent newtype variants wrapping primitives (`Literal(f64)`,
// `NodeRef(String)`) inside an internally-tagged enum — it fails at runtime for
// essentially every real formula AST. Nothing serializes `StmtExpr` (formulas
// are stored as their source string and re-parsed), so the derives were a
// latent wire-format trap and have been removed.
#[derive(Debug, Clone, PartialEq)]
pub enum StmtExpr {
    /// Literal value (integer or float)
    Literal(f64),

    /// Node reference (e.g., "revenue", "cogs")
    NodeRef(NodeId),

    /// Binary operation
    BinOp {
        /// Operator
        op: BinOp,
        /// Left operand
        left: Box<StmtExpr>,
        /// Right operand
        right: Box<StmtExpr>,
    },

    /// Unary operation
    UnaryOp {
        /// Operator
        op: UnaryOp,
        /// Operand
        operand: Box<StmtExpr>,
    },

    /// Function call
    Call {
        /// Function name
        func: String,
        /// Arguments
        args: Vec<StmtExpr>,
    },

    /// If-then-else conditional
    IfThenElse {
        /// Condition expression
        condition: Box<StmtExpr>,
        /// Then branch
        then_expr: Box<StmtExpr>,
        /// Else branch
        else_expr: Box<StmtExpr>,
    },

    /// Capital structure reference (e.g., cs.interest_expense.total)
    ///
    /// Keeps the component/instrument tokens separate so the compiler can
    /// rewrite them into encoded column names understood by the evaluator.
    CsRef {
        /// Component (interest_expense, principal_payment, debt_balance)
        component: String,
        /// Instrument ID or "total" for aggregate
        instrument_or_total: String,
    },
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinOp {
    /// Addition (+)
    Add,
    /// Subtraction (-)
    Sub,
    /// Multiplication (*)
    Mul,
    /// Division (/)
    Div,
    /// Modulo (%)
    Mod,

    /// Equal (==)
    Eq,
    /// Not equal (!=)
    Ne,
    /// Less than (<)
    Lt,
    /// Less than or equal (<=)
    Le,
    /// Greater than (>)
    Gt,
    /// Greater than or equal (>=)
    Ge,

    /// Logical AND
    And,
    /// Logical OR
    Or,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnaryOp {
    /// Negation (-)
    Neg,
    /// Logical NOT
    Not,
}

/// Binding strength used when rendering an expression back to source text.
///
/// Higher binds tighter. Mirrors the parser's precedence ladder so the
/// rendered text re-parses to an identical AST.
fn precedence(expr: &StmtExpr) -> u8 {
    match expr {
        StmtExpr::BinOp { op, .. } => match op {
            BinOp::Or => 1,
            BinOp::And => 2,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 3,
            BinOp::Add | BinOp::Sub => 4,
            BinOp::Mul | BinOp::Div | BinOp::Mod => 5,
        },
        StmtExpr::UnaryOp { .. } => 6,
        StmtExpr::Literal(_)
        | StmtExpr::NodeRef(_)
        | StmtExpr::Call { .. }
        | StmtExpr::IfThenElse { .. }
        | StmtExpr::CsRef { .. } => 7,
    }
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::And => "and",
            Self::Or => "or",
        })
    }
}

/// Render the expression as canonical DSL source text.
///
/// Parentheses are emitted only where the operator precedence requires them,
/// so `parse_formula(expr.to_string())` yields an AST equal to `expr`. This
/// is the stable, host-facing rendering of a parsed formula (the `Debug`
/// form is not a contract).
impl std::fmt::Display for StmtExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn child(
            f: &mut std::fmt::Formatter<'_>,
            expr: &StmtExpr,
            needs_parens: bool,
        ) -> std::fmt::Result {
            if needs_parens {
                write!(f, "({expr})")
            } else {
                write!(f, "{expr}")
            }
        }

        match self {
            Self::Literal(value) => write!(f, "{value}"),
            Self::NodeRef(id) => f.write_str(id.as_str()),
            Self::BinOp { op, left, right } => {
                let level = precedence(self);
                child(f, left, precedence(left) < level)?;
                write!(f, " {op} ")?;
                // Left-associative: a right operand at the same level needs
                // parentheses to preserve `a - (b - c)`.
                child(f, right, precedence(right) <= level)
            }
            Self::UnaryOp { op, operand } => {
                match op {
                    UnaryOp::Neg => f.write_str("-")?,
                    UnaryOp::Not => f.write_str("not ")?,
                }
                child(f, operand, precedence(operand) < precedence(self))
            }
            Self::Call { func, args } => {
                write!(f, "{func}(")?;
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                f.write_str(")")
            }
            Self::IfThenElse {
                condition,
                then_expr,
                else_expr,
            } => write!(f, "if({condition}, {then_expr}, {else_expr})"),
            Self::CsRef {
                component,
                instrument_or_total,
            } => write!(f, "cs.{component}.{instrument_or_total}"),
        }
    }
}

impl StmtExpr {
    /// Create a literal expression.
    pub fn literal(value: f64) -> Self {
        Self::Literal(value)
    }

    /// Create a node reference.
    pub fn node_ref(name: impl Into<NodeId>) -> Self {
        Self::NodeRef(name.into())
    }

    /// Create a binary operation.
    pub fn bin_op(op: BinOp, left: Self, right: Self) -> Self {
        Self::BinOp {
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Create a unary operation.
    pub fn unary_op(op: UnaryOp, operand: Self) -> Self {
        Self::UnaryOp {
            op,
            operand: Box::new(operand),
        }
    }

    /// Create a function call.
    pub fn call(func: impl Into<String>, args: Vec<Self>) -> Self {
        Self::Call {
            func: func.into(),
            args,
        }
    }

    /// Create an if-then-else expression.
    pub fn if_then_else(condition: Self, then_expr: Self, else_expr: Self) -> Self {
        Self::IfThenElse {
            condition: Box::new(condition),
            then_expr: Box::new(then_expr),
            else_expr: Box::new(else_expr),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal() {
        let expr = StmtExpr::literal(42.0);
        assert_eq!(expr, StmtExpr::Literal(42.0));
    }

    #[test]
    fn test_node_ref() {
        let expr = StmtExpr::node_ref("revenue");
        assert_eq!(expr, StmtExpr::NodeRef(NodeId::new("revenue")));
    }

    #[test]
    fn test_bin_op() {
        let expr = StmtExpr::bin_op(BinOp::Add, StmtExpr::literal(1.0), StmtExpr::literal(2.0));

        match expr {
            StmtExpr::BinOp { op, .. } => assert_eq!(op, BinOp::Add),
            _ => panic!("Expected BinOp"),
        }
    }

    /// The canonical rendering must re-parse to the same AST, with
    /// parentheses only where precedence demands them.
    #[test]
    fn display_round_trips_through_parser() {
        for (source, expected) in [
            ("revenue - cogs", "revenue - cogs"),
            ("(revenue - cogs) / revenue", "(revenue - cogs) / revenue"),
            ("a - (b - c)", "a - (b - c)"),
            ("a - b - c", "a - b - c"),
            ("a * (b + c)", "a * (b + c)"),
            ("-(a + b)", "-(a + b)"),
            ("not (a > 1) and b <= 2", "not (a > 1) and b <= 2"),
            ("if(a > 0, a, 0)", "if(a > 0, a, 0)"),
            ("lag(revenue, 1) * 1.05", "lag(revenue, 1) * 1.05"),
            (
                "cs.interest_expense.total / ebitda",
                "cs.interest_expense.total / ebitda",
            ),
            ("a or b and c", "a or b and c"),
            ("(a or b) and c", "(a or b) and c"),
        ] {
            let ast = crate::dsl::parse_formula(source).expect("valid source");
            let rendered = ast.to_string();
            assert_eq!(rendered, expected, "rendering of {source:?}");
            let reparsed = crate::dsl::parse_formula(&rendered).expect("rendered text re-parses");
            assert_eq!(reparsed, ast, "round trip of {source:?}");
        }
    }

    #[test]
    fn test_function_call() {
        let expr = StmtExpr::call(
            "lag",
            vec![StmtExpr::node_ref("revenue"), StmtExpr::literal(1.0)],
        );

        match expr {
            StmtExpr::Call { func, args } => {
                assert_eq!(func, "lag");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected Call"),
        }
    }
}
