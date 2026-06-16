//! Abstract Syntax Tree (AST) nodes and function registry for the expression engine.
//!
//! This module defines the core AST types used throughout the expression evaluation
//! system, including expression nodes, operators, and the function registry.
//!
//! # Components
//!
//! - [`Expr`]: Top-level expression with optional ID for DAG planning
//! - [`ExprNode`]: Core expression variants (columns, literals, calls, operators)
//! - [`Function`]: Built-in function registry (lag, diff, rolling operations)
//! - [`BinOp`], [`UnaryOp`]: Arithmetic, comparison, and logical operators
//!
//! # Design
//!
//! The AST is designed for:
//! - **Efficient evaluation**: Minimal allocations during evaluation
//! - **DAG optimization**: Shared subexpression detection via IDs
//! - **Serialization**: Full serde support for persistence
//! - **Type safety**: Strong typing prevents runtime type errors

use core::hash::{Hash, Hasher};

// DurationSpec removed: time-window API was unused in evaluation

/// Expression AST with optional unique ID for DAG planning.
///
/// Deserialization is strict (`deny_unknown_fields`): unknown fields on
/// inbound payloads are rejected rather than silently ignored.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expr {
    /// Unique identifier for this expression node (for DAG planning).
    pub id: Option<u64>,
    /// The actual expression node.
    pub node: ExprNode,
}

/// The core expression node types.
///
/// Deserialization is strict (`deny_unknown_fields`): unknown fields inside
/// struct variants (`CSRef`, `BinOp`, `UnaryOp`, `IfThenElse`) are rejected.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ExprNode {
    /// Reference a column by name.
    Column(String),
    /// Domain-specific capital-structure reference.
    CSRef {
        /// Capital-structure component.
        component: String,
        /// Instrument id or `total`.
        instrument_or_total: String,
    },
    /// Literal scalar value using the crate's numeric type alias.
    Literal(f64),
    /// Call a registered function with positional arguments.
    Call(Function, Vec<Expr>),
    /// Binary operation (arithmetic, comparison, logical).
    BinOp {
        /// Operator
        op: BinOp,
        /// Left operand
        left: Box<Expr>,
        /// Right operand
        right: Box<Expr>,
    },
    /// Unary operation.
    UnaryOp {
        /// Operator
        op: UnaryOp,
        /// Operand
        operand: Box<Expr>,
    },
    /// If-then-else conditional.
    IfThenElse {
        /// Condition expression
        condition: Box<Expr>,
        /// Then branch
        then_expr: Box<Expr>,
        /// Else branch
        else_expr: Box<Expr>,
    },
}

/// Binary operators for expressions.
#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

/// Unary operators for expressions.
#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum UnaryOp {
    /// Negation (-)
    Neg,
    /// Logical NOT
    Not,
}

impl Expr {
    /// Create a new column reference.
    pub fn column(name: impl Into<String>) -> Self {
        Self {
            id: None,
            node: ExprNode::Column(name.into()),
        }
    }

    /// Create a new capital-structure reference.
    pub fn cs_ref(component: impl Into<String>, instrument_or_total: impl Into<String>) -> Self {
        Self {
            id: None,
            node: ExprNode::CSRef {
                component: component.into(),
                instrument_or_total: instrument_or_total.into(),
            },
        }
    }

    /// Create a new literal value.
    pub fn literal(value: f64) -> Self {
        Self {
            id: None,
            node: ExprNode::Literal(value),
        }
    }

    /// Create a new function call.
    pub fn call(func: Function, args: Vec<Expr>) -> Self {
        Self {
            id: None,
            node: ExprNode::Call(func, args),
        }
    }

    /// Create a new binary operation.
    pub fn bin_op(op: BinOp, left: Expr, right: Expr) -> Self {
        Self {
            id: None,
            node: ExprNode::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
        }
    }

    /// Create a new unary operation.
    pub fn unary_op(op: UnaryOp, operand: Expr) -> Self {
        Self {
            id: None,
            node: ExprNode::UnaryOp {
                op,
                operand: Box::new(operand),
            },
        }
    }

    /// Create a new if-then-else conditional.
    pub fn if_then_else(condition: Expr, then_expr: Expr, else_expr: Expr) -> Self {
        Self {
            id: None,
            node: ExprNode::IfThenElse {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            },
        }
    }

    /// Assign a unique ID to this expression for DAG planning purposes.
    pub fn with_id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }
}

/// Hash implementation for Expr to support deduplication in DAG planning.
///
/// Note: Structural identity only. The opaque `id` field is intentionally
/// excluded from both `Hash` and `Eq` so that DAG deduplication and caches
/// consider two expressions identical if their `node` matches, regardless of
/// their runtime-assigned ids.
impl Hash for Expr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.node {
            ExprNode::Column(name) => {
                0u8.hash(state);
                name.hash(state);
            }
            ExprNode::CSRef {
                component,
                instrument_or_total,
            } => {
                6u8.hash(state);
                component.hash(state);
                instrument_or_total.hash(state);
            }
            ExprNode::Literal(val) => {
                1u8.hash(state);
                // Hash via raw f64 bits for determinism (covers NaN payloads)
                (*val).to_bits().hash(state);
            }
            ExprNode::Call(func, args) => {
                2u8.hash(state);
                (*func as u8).hash(state);
                args.hash(state);
            }
            ExprNode::BinOp { op, left, right } => {
                3u8.hash(state);
                op.hash(state);
                left.hash(state);
                right.hash(state);
            }
            ExprNode::UnaryOp { op, operand } => {
                4u8.hash(state);
                op.hash(state);
                operand.hash(state);
            }
            ExprNode::IfThenElse {
                condition,
                then_expr,
                else_expr,
            } => {
                5u8.hash(state);
                condition.hash(state);
                then_expr.hash(state);
                else_expr.hash(state);
            }
        }
    }
}

impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        match (&self.node, &other.node) {
            (ExprNode::Column(a), ExprNode::Column(b)) => a == b,
            (
                ExprNode::CSRef {
                    component: a_component,
                    instrument_or_total: a_instrument,
                },
                ExprNode::CSRef {
                    component: b_component,
                    instrument_or_total: b_instrument,
                },
            ) => a_component == b_component && a_instrument == b_instrument,
            (ExprNode::Literal(a), ExprNode::Literal(b)) => {
                // f64 equality via raw bits for deterministic NaN handling
                (*a).to_bits() == (*b).to_bits()
            }
            (ExprNode::Call(f1, a1), ExprNode::Call(f2, a2)) => f1 == f2 && a1 == a2,
            (
                ExprNode::BinOp {
                    op: op1,
                    left: l1,
                    right: r1,
                },
                ExprNode::BinOp {
                    op: op2,
                    left: l2,
                    right: r2,
                },
            ) => op1 == op2 && l1 == l2 && r1 == r2,
            (
                ExprNode::UnaryOp {
                    op: op1,
                    operand: o1,
                },
                ExprNode::UnaryOp {
                    op: op2,
                    operand: o2,
                },
            ) => op1 == op2 && o1 == o2,
            (
                ExprNode::IfThenElse {
                    condition: c1,
                    then_expr: t1,
                    else_expr: e1,
                },
                ExprNode::IfThenElse {
                    condition: c2,
                    then_expr: t2,
                    else_expr: e2,
                },
            ) => c1 == c2 && t1 == t2 && e1 == e2,
            _ => false,
        }
    }
}

impl Eq for Expr {}

/// Built-in function identifiers.
#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Function {
    /// Previous N values (shift down).
    Lag,
    /// Next N values (shift up).
    Lead,
    /// First/lagged difference with step N (default 1).
    Diff,
    /// Percentage change over step N (default 1).
    PctChange,
    /// Cumulative sum.
    CumSum,
    /// Cumulative product.
    CumProd,
    /// Cumulative minimum.
    CumMin,
    /// Cumulative maximum.
    CumMax,
    /// Rolling arithmetic mean over a fixed row window size.
    RollingMean,
    /// Rolling sum over a fixed row window size.
    RollingSum,
    /// Exponentially weighted moving average with alpha and adjust flag.
    EwmMean,
    /// Population standard deviation.
    Std,
    /// Population variance.
    Var,
    /// Median.
    Median,
    /// Rolling standard deviation over a fixed row window size.
    RollingStd,
    /// Rolling variance over a fixed row window size.
    RollingVar,
    /// Rolling median over a fixed row window size.
    RollingMedian,

    /// Shift values by N positions (positive = shift down, negative = shift up).
    Shift,
    /// Rank values (dense ranking).
    Rank,
    /// Calculate quantile/percentile of values.
    ///
    /// Note: This function reduces the entire series to a single scalar value
    /// and broadcasts that scalar across the output length. It is not a
    /// rolling/windowed quantile. For windowed behavior, prefer `RollingMedian`
    /// or a domain-specific rolling estimator.
    Quantile,
    /// Rolling minimum over a fixed row window size.
    RollingMin,
    /// Rolling maximum over a fixed row window size.
    RollingMax,
    /// Count non-null values in rolling window.
    RollingCount,
    /// Exponentially weighted moving standard deviation.
    EwmStd,
    /// Exponentially weighted moving variance.
    EwmVar,

    // Custom financial functions
    /// Sum multiple values, skipping NaN values.
    Sum,
    /// Average of multiple values, skipping NaN values.
    Mean,
    /// Annualize a flow value by multiplying by periods per year.
    ///
    /// Use this for cash flows, income, and expense items.
    /// For periodic rates, use `AnnualizeRate` instead.
    Annualize,
    /// Annualize a periodic rate with simple or compound methodology.
    ///
    /// Arguments: (rate, periods_per_year, compounding)
    /// - compounding = 0.0: Simple (rate * periods_per_year)
    /// - compounding = 1.0: Compound ((1 + rate)^periods_per_year - 1)
    ///
    /// Use this for interest rates, returns, and growth rates.
    /// For cash flows, use `Annualize` instead.
    AnnualizeRate,
    /// Trailing twelve months over a window of `periods_per_year` periods.
    ///
    /// Evaluated in the `statements` layer (not in `core::expr`), where the
    /// period frequency (monthly, quarterly, etc.) is known.
    Ttm,
    /// Calendar year-to-date sum.
    ///
    /// Evaluated in the `statements` layer, using the period's calendar year
    /// and frequency (monthly, quarterly, weekly, semi-annual, annual).
    Ytd,
    /// Quarter-to-date sum for monthly statement models.
    ///
    /// Evaluated in the `statements` layer; only valid for monthly periods.
    Qtd,
    /// Fiscal year-to-date sum with configurable fiscal start month.
    ///
    /// Evaluated in the `statements` layer for monthly periods. The fiscal
    /// start month is provided as a numeric argument (1-12).
    FiscalYtd,
    /// Return first non-NaN/non-zero value (coalesce).
    Coalesce,
    /// Absolute value helper (`abs(expr)`).
    Abs,
    /// Sign helper returning -1, 0, or 1 (`sign(expr)`).
    Sign,
    /// Compound annual growth rate between current and lagged periods.
    ///
    /// Evaluated in the statements layer where period spacing is known.
    GrowthRate,
}

impl Function {
    /// Whether this function can be evaluated by the core scalar evaluator.
    ///
    /// A small set of period-aware variants (`Ttm`, `Ytd`, `Qtd`, `FiscalYtd`,
    /// `GrowthRate`, and the multi-arg reducers `Sum`/`Mean`/`Coalesce`,
    /// plus the annualization helpers) live in the AST so that the
    /// `statements` crate can parse and evaluate them with knowledge of the
    /// period grid; they are deliberately rejected by `core::expr::eval`.
    ///
    /// This is the single source of truth for that layering boundary.
    /// Callers in `dag.rs` and `eval_functions.rs` consult it to avoid
    /// duplicating the list.
    #[must_use]
    pub fn is_scalar_evaluable(&self) -> bool {
        !matches!(
            self,
            Function::Sum
                | Function::Mean
                | Function::Ttm
                | Function::Ytd
                | Function::Qtd
                | Function::FiscalYtd
                | Function::Annualize
                | Function::AnnualizeRate
                | Function::Coalesce
                | Function::GrowthRate
        )
    }
}

impl std::fmt::Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Lag => "lag",
            Self::Lead => "lead",
            Self::Diff => "diff",
            Self::PctChange => "pct_change",
            Self::CumSum => "cumsum",
            Self::CumProd => "cumprod",
            Self::CumMin => "cummin",
            Self::CumMax => "cummax",
            Self::RollingMean => "rolling_mean",
            Self::RollingSum => "rolling_sum",
            Self::RollingStd => "rolling_std",
            Self::RollingVar => "rolling_var",
            Self::RollingMedian => "rolling_median",
            Self::RollingMin => "rolling_min",
            Self::RollingMax => "rolling_max",
            Self::RollingCount => "rolling_count",
            Self::EwmMean => "ewm_mean",
            Self::EwmStd => "ewm_std",
            Self::EwmVar => "ewm_var",
            Self::Std => "std",
            Self::Var => "var",
            Self::Median => "median",
            Self::Shift => "shift",
            Self::Rank => "rank",
            Self::Quantile => "quantile",
            Self::Sum => "sum",
            Self::Mean => "mean",
            Self::Annualize => "annualize",
            Self::AnnualizeRate => "annualize_rate",
            Self::Ttm => "ttm",
            Self::Ytd => "ytd",
            Self::Qtd => "qtd",
            Self::FiscalYtd => "fiscal_ytd",
            Self::Coalesce => "coalesce",
            Self::Abs => "abs",
            Self::Sign => "sign",
            Self::GrowthRate => "growth_rate",
        };
        f.write_str(name)
    }
}

// WindowSpec removed with time-window API cleanup

// ExecMeta removed in favor of unified config::ResultsMeta

/// Result envelope that includes execution metadata.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvaluationResult {
    /// The computed values.
    pub values: Vec<f64>,
    /// Execution metadata stamped into result.
    ///
    /// This carries the standard [`crate::config::ResultsMeta`] payload used by
    /// the rest of the crate for provenance, evaluation options, and other
    /// run-level annotations.
    pub metadata: crate::config::ResultsMeta,
}

// ResultMetadata removed in favor of unified config::ResultsMeta

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expr_builder_variants_match_expected_nodes() {
        let col = Expr::column("price");
        assert!(matches!(col.node, ExprNode::Column(ref name) if name == "price"));

        let lit = Expr::literal(42.0);
        assert!(matches!(lit.node, ExprNode::Literal(v) if (v - 42.0).abs() < 1e-12));

        let call = Expr::call(
            Function::Lag,
            vec![Expr::column("price"), Expr::literal(1.0)],
        );
        assert!(matches!(call.node, ExprNode::Call(Function::Lag, _)));

        let bin = Expr::bin_op(BinOp::Add, Expr::column("a"), Expr::column("b"));
        assert!(matches!(bin.node, ExprNode::BinOp { op: BinOp::Add, .. }));

        let unary = Expr::unary_op(UnaryOp::Neg, Expr::literal(1.0));
        assert!(matches!(
            unary.node,
            ExprNode::UnaryOp {
                op: UnaryOp::Neg,
                ..
            }
        ));

        let conditional =
            Expr::if_then_else(Expr::column("flag"), Expr::literal(1.0), Expr::literal(0.0));
        assert!(matches!(conditional.node, ExprNode::IfThenElse { .. }));
    }
}
