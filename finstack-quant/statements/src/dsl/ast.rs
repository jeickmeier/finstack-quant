//! Abstract Syntax Tree for the Statements DSL.

use crate::types::NodeId;
use finstack_quant_core::expr::{BinOp, UnaryOp};

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

    /// Explicit monetary denomination of a unitless scalar.
    AsMoney {
        /// Scalar-valued expression in units of the selected currency.
        value: Box<StmtExpr>,
        /// Currency that denominates the scalar, with no FX conversion.
        currency: finstack_quant_core::currency::Currency,
    },

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

    /// Capital structure reference: `cs.<component>.<scope>`, for example
    /// `cs.interest_expense.total`, `cs.debt_balance.TL-A` or
    /// `cs.debt_balance.total.EUR`.
    CsRef {
        /// The reported amount read.
        component: crate::capital_structure::CsComponent,
        /// Whose amount it is: one instrument, one native currency's total
        /// or the consolidated total.
        scope: crate::capital_structure::CsScope,
    },
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
        | StmtExpr::AsMoney { .. }
        | StmtExpr::IfThenElse { .. }
        | StmtExpr::CsRef { .. } => 7,
    }
}

fn binary_symbol(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "and",
        BinOp::Or => "or",
    }
}

/// Render the expression as canonical DSL source text.
///
/// Parentheses are emitted only where the operator precedence requires them,
/// and logical negation is always written `!`, so
/// `parse_formula(expr.to_string())` yields an AST equal to a parsed `expr`. This
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
            Self::AsMoney { value, currency } => write!(f, "as_money({value}, {currency})"),
            Self::BinOp { op, left, right } => {
                let level = precedence(self);
                // Comparisons do not chain: `(a > b) > c` keeps its
                // parentheses on either side.
                let comparison = level == 3;
                let left_level = precedence(left);
                child(
                    f,
                    left,
                    left_level < level || (comparison && left_level == level),
                )?;
                write!(f, " {} ", binary_symbol(*op))?;
                // Left-associative: a right operand at the same level needs
                // parentheses to preserve `a - (b - c)`.
                child(f, right, precedence(right) <= level)
            }
            Self::UnaryOp { op, operand } => {
                // `!` binds at unary precedence, so `(!a) > b` renders as
                // `!a > b`. Keyword `not` binds outside a comparison and
                // would regroup it.
                f.write_str(match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                })?;
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
            Self::CsRef { component, scope } => write!(f, "cs.{component}.{scope}"),
        }
    }
}

impl StmtExpr {
    /// Create a literal expression.
    pub fn literal(value: f64) -> Self {
        Self::Literal(value)
    }

    /// The same expression with every node reference renamed by `rename`.
    ///
    /// Function names, literals and capital-structure references are left
    /// untouched: only [`StmtExpr::NodeRef`] names pass through `rename`.
    ///
    /// # Arguments
    ///
    /// * `rename` - Maps a referenced node identifier to the identifier the
    ///   returned expression should reference instead.
    #[must_use]
    pub fn map_node_refs(&self, rename: &mut impl FnMut(&str) -> String) -> Self {
        match self {
            Self::Literal(_) | Self::CsRef { .. } => self.clone(),
            Self::NodeRef(name) => Self::NodeRef(NodeId::new(rename(name.as_str()))),
            Self::AsMoney { value, currency } => Self::AsMoney {
                value: Box::new(value.map_node_refs(rename)),
                currency: *currency,
            },
            Self::BinOp { op, left, right } => Self::BinOp {
                op: *op,
                left: Box::new(left.map_node_refs(rename)),
                right: Box::new(right.map_node_refs(rename)),
            },
            Self::UnaryOp { op, operand } => Self::UnaryOp {
                op: *op,
                operand: Box::new(operand.map_node_refs(rename)),
            },
            Self::Call { func, args } => Self::Call {
                func: func.clone(),
                args: args.iter().map(|arg| arg.map_node_refs(rename)).collect(),
            },
            Self::IfThenElse {
                condition,
                then_expr,
                else_expr,
            } => Self::IfThenElse {
                condition: Box::new(condition.map_node_refs(rename)),
                then_expr: Box::new(then_expr.map_node_refs(rename)),
                else_expr: Box::new(else_expr.map_node_refs(rename)),
            },
        }
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
            ("not (a > 1) and b <= 2", "!(a > 1) and b <= 2"),
            ("not a > b", "!(a > b)"),
            ("!a > b", "!a > b"),
            ("-!a", "-!a"),
            ("!-a", "!-a"),
            ("(a > b) == (c > d)", "(a > b) == (c > d)"),
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
