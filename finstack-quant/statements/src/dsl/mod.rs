//! Domain-Specific Language (DSL) for financial statement formulas.
//!
//! The DSL engine provides:
//! - **Parser**: Convert formula text to AST
//! - **AST**: Structured representation of formulas
//! - **Compiler**: Transform AST to core's `Expr` for evaluation
//!
//! ## Supported Operations
//!
//! ### Arithmetic
//! - `+`, `-`, `*`, `/`, `%`
//! - `abs(expr)`, `sign(expr)` - Math helpers
//!
//! ### Comparison
//! - `==`, `!=`, `<`, `<=`, `>`, `>=`
//!
//! ### Logical
//! - `and`, `or`
//! - `not expr` / `!expr`
//!
//! ### Function Reference
//!
//! | Function | Arity | Behavior |
//! | --- | --- | --- |
//! | `if(condition, then_expr, else_expr)` | 3 | Conditional expression; non-zero values are truthy. |
//! | `min(a, b, ...)`, `max(a, b, ...)` | 2+ | Pairwise min/max lowered to nested conditionals. NaN comparison behavior follows IEEE 754. |
//! | `abs(expr)`, `sign(expr)` | 1 | Absolute value and sign indicator (`-1`, `0`, `1`, or NaN). |
//! | `sum(...)`, `mean(...)` | 1+ | Aggregate finite argument values; non-finite values are skipped. |
//! | `coalesce(expr, default, ...)` | 2+ | First finite argument, or NaN when every argument is non-finite. |
//! | `lag(expr, n)`, `shift(expr, n)` | 2 | Historical offset lookup. `lag` requires a non-negative offset; `shift` accepts signed offsets. |
//! | `diff(expr[, n])`, `pct_change(expr[, n])` | 1-2 | Difference or percentage change versus `n` periods ago, defaulting to 1. Missing or near-zero denominators return NaN. |
//! | `growth_rate(expr[, periods])` | 1-2 | Compound annual growth rate between the current value and `periods` periods ago. Defaults to the current period frequency. |
//! | `cumsum(expr)`, `cumprod(expr)`, `cummin(expr)`, `cummax(expr)` | 1 | Cumulative aggregate through the current period, skipping non-finite values. |
//! | `rolling_mean(expr, window)`, `rolling_sum(expr, window)`, `rolling_std(expr, window)`, `rolling_var(expr, window)`, `rolling_median(expr, window)`, `rolling_min(expr, window)`, `rolling_max(expr, window)`, `rolling_count(expr, window)` | 2 | Rolling-window aggregate over finite observations. Empty finite windows return NaN except `rolling_count`, which returns a count. |
//! | `std(expr)`, `var(expr)`, `median(expr)` | 1 | Historical distribution statistic over finite observations available through the current period. |
//! | `rank(expr[, ascending])`, `quantile(expr, q)` | 1-2 | Historical rank and linear quantile over finite observations. |
//! | `ewm_mean(column, alpha)` | 2 | Exponentially weighted moving mean for a column reference. |
//! | `ewm_std(column, alpha[, adjust])`, `ewm_var(column, alpha[, adjust])` | 2-3 | Exponentially weighted variance or standard deviation. The optional `adjust` flag controls pandas-compatible bias correction. |
//! | `ttm(expr)`, `ltm(expr)` | 1 | Trailing-twelve-month sum. Quarterly models require 4 quarters; monthly models require 12 months. |
//! | `ytd(expr)` | 1 | Calendar year-to-date finite sum. |
//! | `qtd(expr)` | 1 | Quarter-to-date finite sum for monthly models. |
//! | `fiscal_ytd(expr, start_month)` | 2 | Fiscal year-to-date finite sum using a 1-12 fiscal start month. |
//! | `annualize(expr[, periods])` | 1-2 | Scale a period value to an annual amount. Defaults to the current period frequency. |
//! | `annualize_rate(expr[, periods])` | 1-2 | Compound a period rate to an annual rate. Defaults to the current period frequency. |
//!
//! `lead(...)` is intentionally not available because forward-looking formulas
//! can leak future values into historical periods.
//!
//! ## Example
//!
//! ```rust
//! use finstack_quant_statements::dsl::{parse_formula, compile};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Parse a formula
//! let ast = parse_formula("(revenue - cogs) / revenue")?;
//!
//! // Compile to core Expr
//! let expr = compile(&ast)?;
//! # Ok(())
//! # }
//! ```

pub mod ast;
pub mod compiler;
pub mod parser;

pub use ast::{BinOp, StmtExpr, UnaryOp};
pub use compiler::compile;
pub use parser::parse_formula;

/// Parse and compile a formula in one step.
///
/// This is a convenience function that combines parsing and compilation.
///
/// # Arguments
/// * `formula` - DSL expression to parse then compile
///
/// # Returns
/// Core [`Expr`](finstack_quant_core::expr::Expr) ready for evaluation by the engine.
///
/// # Example
///
/// ```rust
/// use finstack_quant_statements::dsl::parse_and_compile;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let expr = parse_and_compile("revenue - cogs")?;
/// # let _ = expr;
/// # Ok(())
/// # }
/// ```
pub fn parse_and_compile(formula: &str) -> crate::error::Result<finstack_quant_core::expr::Expr> {
    let ast = parse_formula(formula)?;
    compile(&ast)
}

#[cfg(test)]
mod stack_safety {
    /// A formula at exactly the term budget must compile without overflowing a
    /// small stack.
    ///
    /// This pins the budget to the property that motivates it. Formulas are
    /// compiled on rayon workers during Monte Carlo, which get Rust's 2 MiB
    /// default stack rather than the main thread's 8 MiB, and a stack overflow
    /// aborts the process (SIGABRT) rather than unwinding — the bindings cannot
    /// catch it. 512 KiB is deliberately 4x smaller than that default, so this
    /// test fails long before a real deployment would.
    ///
    /// If this test starts aborting, `MAX_FORMULA_TERMS` is too high for the
    /// per-term stack cost of the AST walkers (`compile`, `validate_dimensions`,
    /// `Drop`) — lower the budget rather than raising the stack here.
    #[test]
    fn formula_at_term_budget_compiles_on_a_small_stack() {
        // One term per operand, so this sits exactly at the budget.
        let formula = std::iter::repeat_n("1", 256).collect::<Vec<_>>().join("+");
        let compiled = std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(move || crate::dsl::parse_and_compile(&formula).is_ok())
            .expect("probe thread should spawn")
            .join()
            .expect("compiling a budget-sized formula must not overflow a 512 KiB stack");
        assert!(compiled, "a budget-sized formula must compile successfully");
    }
}
