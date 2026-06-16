//! Abstract Syntax Tree for the Statements DSL.

use crate::types::NodeId;
use serde::{Deserialize, Serialize};

/// Statements DSL expression AST.
///
/// Represents parsed formula syntax before compilation to the core expression
/// engine. Each variant captures a syntactic construct in the DSL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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
    CSRef {
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
    // Arithmetic
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

    // Comparison
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

    // Logical
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

    /// Create a capital structure reference.
    pub fn cs_ref(component: impl Into<String>, instrument_or_total: impl Into<String>) -> Self {
        Self::CSRef {
            component: component.into(),
            instrument_or_total: instrument_or_total.into(),
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
