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
//! | `if(condition, then_expr, else_expr)` | 3 | Conditional expression; finite non-zero values are truthy. |
//! | `min(a, b, ...)`, `max(a, b, ...)` | 1+ | Min/max over arguments from left to right. A leading NaN yields to a finite peer; a trailing NaN propagates. |
//! | `abs(expr)`, `sign(expr)` | 1 | Absolute value and sign indicator (`-1`, `0`, `1`, or NaN). |
//! | `pow(base, exp)` | 2 | Exponentiation with IEEE 754 semantics (`pow(-1, 0.5)` is NaN, `pow(0, -1)` is +inf). |
//! | `round(expr[, digits])` | 1-2 | Round to `digits` decimal places (default 0), ties half away from zero (Excel convention). Negative `digits` rounds to tens/hundreds. Non-finite values pass through. |
//! | `floor(expr)`, `ceil(expr)` | 1 | Largest/smallest integer at or below/above the value. |
//! | `ln(expr)`, `exp(expr)`, `log10(expr)`, `sqrt(expr)` | 1 | Natural log, exponential, base-10 log, and square root with IEEE 754 semantics (`ln(0)` is -inf; `ln`/`sqrt` of negatives are NaN). |
//! | `clamp(expr, lo, hi)` | 3 | Clamp to the inclusive range `[lo, hi]`. NaN in any argument or `lo > hi` returns NaN. |
//! | `is_missing(expr)` | 1 | `1` when the value is non-finite (NaN or ±inf), `0` otherwise — the same "finite" convention as `coalesce`. |
//! | `sum(...)`, `mean(...)` | 1+ | Aggregate finite argument values; non-finite values are skipped. |
//! | `coalesce(expr, default, ...)` | 2+ | First finite argument, or NaN when every argument is non-finite. |
//! | `lag(expr, n)`, `shift(expr, n)` | 2 | Historical expression lookup by calendar-period offset, preserving missing slots. `lag` requires a non-negative offset; negative `shift` returns NaN to prohibit forward-looking reads. Fiscal daily and weekly model timelines are supported, including leap days and shortened fiscal weeks. |
//! | `diff(expr[, n])`, `pct_change(expr[, n])` | 1-2 | Difference or percentage change versus `n` periods ago, defaulting to 1. Missing or near-zero denominators return NaN. |
//! | `growth_rate(expr[, periods])` | 1-2 | Compound annual growth rate in decimal units, `(current / prior)^(1 / years) - 1`, over a positive integer calendar-period offset. Missing observations or a non-positive prior value return NaN. The default lookback is the frequency's periods per year (252 daily, 52 weekly, 12 monthly, 4 quarterly, 2 semiannual, 1 annual). Monthly/quarterly/semiannual/annual years equal offset divided by frequency; daily/weekly years use Actual/Actual ISDA between the final included dates of the two periods. |
//! | `cumsum(expr)`, `cumprod(expr)`, `cummin(expr)`, `cummax(expr)` | 1 | Cumulative aggregate through the current period, skipping non-finite values. |
//! | `rolling_mean(expr, window[, min_periods])`, `rolling_sum(expr, window[, min_periods])`, `rolling_std(expr, window[, min_periods])`, `rolling_var(expr, window[, min_periods])`, `rolling_median(expr, window[, min_periods])`, `rolling_min(expr, window[, min_periods])`, `rolling_max(expr, window[, min_periods])`, `rolling_count(expr, window[, min_periods])` | 2-3 | Rolling-window aggregate over finite observations. `min_periods` defaults to `window` (pandas parity): a window with fewer finite observations returns NaN. Pass a smaller `min_periods` for expanding-until-full behavior. |
//! | `std(expr)`, `var(expr)`, `median(expr)` | 1 | Historical distribution statistic over finite observations available through the current period. |
//! | `rank(node[, ascending])` | 1-2 | Historical rank of a statement-node or `cs.*` reference over finite observations; ties share the minimum rank. The finite scalar direction flag defaults to `1` (ascending); `0` selects descending and any non-zero value selects ascending. |
//! | `quantile(node, q)` | 2 | Historical linear quantile of a statement-node or `cs.*` reference over finite observations, retaining the series' units and currency. `q` is a scalar level in `[0, 1]`. |
//! | `ewm_mean(expr, alpha)` | 2 | Exponentially weighted moving mean over the expression's historical series (pandas `adjust=False, ignore_na=False`: decay advances across NaN gaps). Requires `0 < alpha <= 1`. |
//! | `ewm_std(expr, alpha[, unbiased])`, `ewm_var(expr, alpha[, unbiased])` | 2-3 | Exponentially weighted variance or standard deviation (pandas `adjust=False, ignore_na=False`). The optional `unbiased` flag is pandas' `bias` toggle: nonzero (default) applies the `bias=False` correction, `0` returns the biased variance. Requires `0 < alpha <= 1`. |
//! | `ttm(expr)`, `ltm(expr)` | 1 | Trailing-twelve-month sum. Quarterly models require 4 quarters; monthly models require 12 months. |
//! | `ytd(expr)` | 1 | Calendar year-to-date finite sum. |
//! | `qtd(expr)` | 1 | Quarter-to-date finite sum for monthly models. |
//! | `fiscal_ytd(expr, start_month)` | 2 | Fiscal year-to-date finite sum using a 1-12 fiscal start month. |
//! | `annualize(expr[, periods])` | 1-2 | Scale a period value to an annual amount. Defaults to the current period frequency. |
//! | `annualize_rate(rate, periods_per_year, compounding)` | 3 | Annualize a periodic decimal rate using the supplied positive periods per year. `compounding = 0` multiplies the rate by periods per year; any non-zero flag computes `(1 + rate)^periods_per_year - 1`. |
//!
//! `lead(...)` is intentionally not available because forward-looking formulas
//! can leak future values into historical periods.
//!
//! Historical expression functions, including `lag`, `shift`, `growth_rate`,
//! `ytd`, `qtd`, and `fiscal_ytd`, accept direct capital-structure references such
//! as `cs.interest_expense.total`. They read completed historical snapshots;
//! references to a missing instrument or unavailable currency aggregate remain
//! errors instead of being silently omitted from a sum.
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
///
/// # Errors
///
/// Returns a formula-parse error if `formula` is not one complete valid DSL
/// expression, or a compilation error if the parsed AST contains an unsupported
/// capital-structure component or invalid function/operator form. It performs
/// no model-specific reference or dimension validation; those require the
/// model's node set and value types.
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
