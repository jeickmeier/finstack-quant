//! Abstract Syntax Tree (AST) nodes and function registry for the expression engine.
//!
//! This module defines the core AST types used throughout the expression evaluation
//! system, including expression nodes, operators, and the function registry.
//!
//! # Components
//!
//! - [`Expr`]: Top-level expression for structural DAG planning
//! - [`ExprNode`]: Core expression variants (columns, literals, calls, operators)
//! - [`Function`]: Built-in function registry (lag, diff, rolling operations)
//! - [`BinOp`], [`UnaryOp`]: Arithmetic, comparison, and logical operators
//!
//! # Design
//!
//! The AST is designed for:
//! - **Efficient evaluation**: Minimal allocations during evaluation
//! - **DAG optimization**: Shared subexpression detection via structural identity
//! - **Serialization**: Full serde support for persistence
//! - **Type safety**: Strong typing prevents runtime type errors

use core::hash::{Hash, Hasher};

/// Expression AST for structural DAG planning.
///
/// Deserialization is strict (`deny_unknown_fields`): unknown fields on
/// inbound payloads are rejected rather than silently ignored.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Expr {
    /// The actual expression node.
    pub node: ExprNode,
}

/// The core expression node types.
///
/// Deserialization is strict (`deny_unknown_fields`): unknown fields inside
/// struct variants (`CsRef`, `BinOp`, `UnaryOp`, `IfThenElse`) are rejected.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ExprNode {
    /// Reference a column by name.
    Column(String),
    /// Domain-specific capital-structure reference.
    CsRef {
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

/// Unary operators for expressions.
#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
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
            node: ExprNode::Column(name.into()),
        }
    }

    /// Create a new capital-structure reference.
    pub fn cs_ref(component: impl Into<String>, instrument_or_total: impl Into<String>) -> Self {
        Self {
            node: ExprNode::CsRef {
                component: component.into(),
                instrument_or_total: instrument_or_total.into(),
            },
        }
    }

    /// Create a new literal value.
    pub fn literal(value: f64) -> Self {
        Self {
            node: ExprNode::Literal(value),
        }
    }

    /// Create a new function call.
    pub fn call(func: Function, args: Vec<Expr>) -> Self {
        Self {
            node: ExprNode::Call(func, args),
        }
    }

    /// Create a new binary operation.
    pub fn bin_op(op: BinOp, left: Expr, right: Expr) -> Self {
        Self {
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
            node: ExprNode::UnaryOp {
                op,
                operand: Box::new(operand),
            },
        }
    }

    /// Create a new if-then-else conditional.
    pub fn if_then_else(condition: Expr, then_expr: Expr, else_expr: Expr) -> Self {
        Self {
            node: ExprNode::IfThenElse {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            },
        }
    }
}

/// Hash implementation for Expr to support deduplication in DAG planning.
///
/// Identity depends only on expression structure.
impl Hash for Expr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.node {
            ExprNode::Column(name) => {
                0u8.hash(state);
                name.hash(state);
            }
            ExprNode::CsRef {
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
                ExprNode::CsRef {
                    component: a_component,
                    instrument_or_total: a_instrument,
                },
                ExprNode::CsRef {
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

macro_rules! for_all_functions {
    ($(
        $(#[$meta:meta])*
        $variant:ident => ($name:literal, $scalar:literal),
    )*) => {
        /// Built-in function identifiers.
        #[derive(Clone, Debug, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
        #[serde(rename_all = "snake_case")]
        pub enum Function {
            $(
                $(#[$meta])*
                $variant,
            )*
        }

        impl Function {
            /// Whether this function can be evaluated by the core scalar evaluator.
            ///
            /// Only the elementwise functions (`abs`, `sign`, `pow`, `round`,
            /// `floor`, `ceil`, `ln`, `exp`, `log10`, `sqrt`, `clamp`,
            /// `is_missing`) are. The windowed, cumulative, positional and
            /// reducing functions (`lag`, `rolling_*`, `cumsum`, `std`, `rank`,
            /// ...) and the period-aware variants (`Ttm`, `Ytd`, `GrowthRate`,
            /// the n-ary reducers `Sum`/`Mean`/`Min`/`Max`/`Coalesce`, the
            /// annualization helpers) live in the AST so that the `statements`
            /// crate can parse and evaluate them with knowledge of the period
            /// grid; `core::expr::CompiledExpr::new` rejects them.
            ///
            /// This is the single source of truth for that layering boundary.
            #[must_use]
            pub const fn is_scalar_evaluable(&self) -> bool {
                match self {
                    $(Self::$variant => $scalar,)*
                }
            }

            const fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }
        }

        impl std::fmt::Display for Function {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

for_all_functions! {
    /// Previous N values (shift down).
    Lag => ("lag", false),
    /// Next N values (shift up).
    Lead => ("lead", false),
    /// First/lagged difference with step N (default 1).
    Diff => ("diff", false),
    /// Percentage change over step N (default 1).
    PctChange => ("pct_change", false),
    /// Cumulative sum.
    CumSum => ("cumsum", false),
    /// Cumulative product.
    CumProd => ("cumprod", false),
    /// Cumulative minimum.
    CumMin => ("cummin", false),
    /// Cumulative maximum.
    CumMax => ("cummax", false),
    /// Rolling arithmetic mean over a fixed row window size.
    RollingMean => ("rolling_mean", false),
    /// Rolling sum over a fixed row window size.
    RollingSum => ("rolling_sum", false),
    /// `ewm_mean(series, alpha, adjust?)`, with `alpha` normalized to
    /// `[1e-12, 1]`. An omitted or nonzero `adjust` uses normalized weights;
    /// zero selects the recursive form. Leading and interior NaNs are preserved,
    /// and the recursion starts at the first non-NaN observation.
    EwmMean => ("ewm_mean", false),
    /// Population standard deviation (denominator `n`, pandas `ddof=0`),
    /// broadcast over the input length. NaNs are excluded; fewer than two
    /// non-NaN observations produce NaN.
    Std => ("std", false),
    /// Population variance (denominator `n`, pandas `ddof=0`), broadcast over
    /// the input length. NaNs are excluded; fewer than two non-NaN
    /// observations produce NaN.
    Var => ("var", false),
    /// Median.
    Median => ("median", false),
    /// Rolling population standard deviation (`ddof=0`) over a fixed row
    /// window size. Incomplete windows and windows containing NaN return NaN.
    RollingStd => ("rolling_std", false),
    /// Rolling population variance (`ddof=0`) over a fixed row window size.
    /// Incomplete windows and windows containing NaN return NaN.
    RollingVar => ("rolling_var", false),
    /// Rolling median over a fixed row window size.
    RollingMedian => ("rolling_median", false),
    /// Shift values by N positions (positive = shift down, negative = shift up).
    Shift => ("shift", false),
    /// Rank values (min ranking; ties share the minimum rank, pandas
    /// `method='min'`).
    Rank => ("rank", false),
    /// Calculate quantile/percentile of values.
    ///
    /// Note: This function reduces the entire series to a single scalar value
    /// and broadcasts that scalar across the output length. It is not a
    /// rolling/windowed quantile. For windowed behavior, prefer `RollingMedian`
    /// or a domain-specific rolling estimator.
    Quantile => ("quantile", false),
    /// Rolling minimum over a fixed row window size.
    RollingMin => ("rolling_min", false),
    /// Rolling maximum over a fixed row window size.
    RollingMax => ("rolling_max", false),
    /// Count non-null values in rolling window.
    RollingCount => ("rolling_count", false),
    /// `ewm_std(series, alpha, adjust?)`, with `alpha` normalized to
    /// `[1e-12, 1]`. An omitted or positive `adjust` uses adjusted weights;
    /// zero or a negative value selects the recursive form. Leading and interior
    /// NaNs are preserved, and the recursion starts at the first non-NaN value.
    /// Returns weighted population standard deviation without a sample-bias
    /// correction; the first non-NaN observation has standard deviation zero.
    EwmStd => ("ewm_std", false),
    /// `ewm_var(series, alpha, adjust?)`, with `alpha` normalized to
    /// `[1e-12, 1]`. An omitted or positive `adjust` uses adjusted weights;
    /// zero or a negative value selects the recursive form. Leading and interior
    /// NaNs are preserved, and the recursion starts at the first non-NaN value.
    /// Returns weighted population variance without a sample-bias correction;
    /// the first non-NaN observation has variance zero.
    EwmVar => ("ewm_var", false),
    /// Sum multiple values, skipping NaN values.
    Sum => ("sum", false),
    /// Average of multiple values, skipping NaN values.
    Mean => ("mean", false),
    /// Annualize a flow value by multiplying by periods per year.
    ///
    /// Use this for cash flows, income, and expense items.
    /// For periodic rates, use `AnnualizeRate` instead.
    Annualize => ("annualize", false),
    /// Annualize a periodic rate with simple or compound methodology.
    ///
    /// Arguments: (rate, periods_per_year, compounding)
    /// - compounding = 0.0: Simple (rate * periods_per_year)
    /// - compounding = 1.0: Compound ((1 + rate)^periods_per_year - 1)
    ///
    /// Use this for interest rates, returns, and growth rates.
    /// For cash flows, use `Annualize` instead.
    AnnualizeRate => ("annualize_rate", false),
    /// Trailing twelve months over a window of `periods_per_year` periods.
    ///
    /// Evaluated in the `statements` layer (not in `core::expr`), where the
    /// period frequency (monthly, quarterly, etc.) is known.
    Ttm => ("ttm", false),
    /// Calendar year-to-date sum.
    ///
    /// Evaluated in the `statements` layer, using the period's calendar year
    /// and frequency (monthly, quarterly, weekly, semi-annual, annual).
    Ytd => ("ytd", false),
    /// Quarter-to-date sum for monthly statement models.
    ///
    /// Evaluated in the `statements` layer; only valid for monthly periods.
    Qtd => ("qtd", false),
    /// Fiscal year-to-date sum with configurable fiscal start month.
    ///
    /// Evaluated in the `statements` layer for monthly periods. The fiscal
    /// start month is provided as a numeric argument (1-12).
    FiscalYtd => ("fiscal_ytd", false),
    /// Return first non-NaN/non-zero value (coalesce).
    Coalesce => ("coalesce", false),
    /// Minimum of two or more values.
    ///
    /// Evaluated in the `statements` layer as an n-ary reducer. NaN handling
    /// is order-dependent and follows IEEE 754 comparison semantics: with a
    /// left-to-right fold of `if acc < arg { acc } else { arg }`, a leading
    /// `NaN` is dropped in favor of a finite peer while a trailing `NaN`
    /// propagates.
    Min => ("min", false),
    /// Maximum of two or more values.
    ///
    /// Evaluated in the `statements` layer as an n-ary reducer. NaN handling
    /// is order-dependent and follows IEEE 754 comparison semantics (see
    /// [`Function::Min`]).
    Max => ("max", false),
    /// Absolute value helper (`abs(expr)`).
    Abs => ("abs", true),
    /// Sign helper returning -1, 0, or 1 (`sign(expr)`).
    Sign => ("sign", true),
    /// Compound annual growth rate between current and lagged periods.
    ///
    /// Evaluated in the statements layer where period spacing is known.
    GrowthRate => ("growth_rate", false),
    /// Exponentiation (`pow(base, exp)`), computed as `base^exp` with IEEE 754
    /// semantics: `pow(-1, 0.5)` is NaN, `pow(0, -1)` is +inf, and overflow
    /// saturates to ±inf.
    Pow => ("pow", true),
    /// Round to a number of decimal digits (`round(expr[, digits])`).
    ///
    /// Ties round half away from zero (Excel convention): `round(2.5)` is 3,
    /// `round(-2.5)` is -3. `digits` defaults to 0 and may be negative to
    /// round to tens/hundreds (`round(1234, -2)` is 1200). Non-finite inputs
    /// pass through unchanged.
    Round => ("round", true),
    /// Largest integer less than or equal to the value (`floor(expr)`).
    Floor => ("floor", true),
    /// Smallest integer greater than or equal to the value (`ceil(expr)`).
    Ceil => ("ceil", true),
    /// Natural logarithm (`ln(expr)`), IEEE semantics: `ln(0)` is -inf and
    /// `ln(x)` for `x < 0` is NaN.
    Ln => ("ln", true),
    /// Natural exponential (`exp(expr)`); overflow saturates to +inf.
    Exp => ("exp", true),
    /// Base-10 logarithm (`log10(expr)`), IEEE semantics: `log10(0)` is -inf
    /// and `log10(x)` for `x < 0` is NaN.
    Log10 => ("log10", true),
    /// Square root (`sqrt(expr)`), IEEE semantics: `sqrt(x)` for `x < 0` is
    /// NaN.
    Sqrt => ("sqrt", true),
    /// Clamp a value to an inclusive range (`clamp(expr, lo, hi)`),
    /// equivalent to `min(max(expr, lo), hi)`. Returns NaN when any argument
    /// is NaN or when `lo > hi`; it never panics on an inverted range.
    Clamp => ("clamp", true),
    /// Missing-value predicate (`is_missing(expr)`), returning 1 when the
    /// value is non-finite (NaN or ±inf) and 0 otherwise — the same
    /// "finite" convention used by `coalesce`.
    IsMissing => ("is_missing", true),
}

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
