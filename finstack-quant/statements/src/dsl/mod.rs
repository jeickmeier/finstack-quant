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
//! - `!expr` binds like unary minus: `!a > b` is `(!a) > b`
//! - `not expr` binds outside a comparison: `not a > b` is `not (a > b)`
//! - A comparison does not chain; write `(a > b) == (c > d)` with parentheses
//!
//! ### Values and Errors
//!
//! A formula evaluates to a finite scalar, an exact amount of money, a
//! Boolean or an unavailable value. Arithmetic is checked: division or
//! remainder by zero, an undefined domain, an overflow and mixed units or
//! currencies are evaluation errors naming the node and period. An
//! unavailable operand makes the result unavailable. `==` and `!=` compare
//! the represented values with no tolerance; same-currency money compares as
//! exact decimals. `and`, `or` and `not` need Boolean operands. `and` and
//! `or` evaluate their right operand only when the left does not decide the
//! result, and use strong three-valued logic: a false operand decides `and`
//! and a true operand decides `or` even when the other is unavailable. A node whose formula is Boolean is published as `1` or `0`.
//!
//! ### Function Reference
//!
//! | Function | Arity | Behavior |
//! | --- | --- | --- |
//! | `if(condition, then_expr, else_expr)` | 3 | Conditional expression. `condition` must be Boolean (a comparison, a logical operator or `is_missing`); only the selected branch is evaluated, and an unavailable condition makes the result unavailable. |
//! | `min(a, b, ...)`, `max(a, b, ...)` | 1+ | Smallest or largest argument. Every argument must be available and in the same units; an unavailable argument makes the result unavailable. |
//! | `abs(expr)`, `sign(expr)` | 1 | Absolute value (exact for money) and sign indicator (`-1`, `0` or `1`). |
//! | `pow(base, exp)` | 2 | Exponentiation of unitless numbers. An undefined or overflowing result (`pow(-1, 0.5)`, `pow(0, -1)`) is an error. |
//! | `round(expr[, digits])` | 1-2 | Round to `digits` decimal places (default 0 for a scalar), midpoint ties away from zero. Negative `digits` rounds a scalar to tens/hundreds. Money needs `digits` stated and non-negative; one-argument `round` of money is rejected at build. `digits` must be an exact integer. |
//! | `floor(expr)`, `ceil(expr)` | 1 | Largest/smallest integer at or below/above a unitless value. Money is rejected at build: use `round(value, digits)`. |
//! | `ln(expr)`, `exp(expr)`, `log10(expr)`, `sqrt(expr)` | 1 | Natural log, exponential, base-10 log and square root of a unitless number. A value outside the domain (`ln(0)`, `sqrt(-1)`) or an overflow is an error. |
//! | `clamp(expr, lo, hi)` | 3 | Clamp to the inclusive range `[lo, hi]` in the same units. `lo > hi` is an error; an unavailable argument makes the result unavailable. |
//! | `is_missing(expr)` | 1 | Boolean: true when the value is missing, masked or unavailable. An error in `expr` still propagates. |
//! | `sum(...)`, `mean(...)` | 1+ | Sum or mean of the arguments, exact for money. Every argument must be available and in the same units; an unavailable argument makes the result unavailable. |
//! | `coalesce(expr, default, ...)` | 2+ | First available argument, including zero; later arguments are not evaluated. Unavailable when no argument is available. An error is not recovered. |
//! | `lag(expr, n)`, `shift(expr, n)` | 2 | The expression's typed value `n` periods back in the model plan's calendar; `n` is an exact non-negative integer and `0` is the current value. A slot the run has not evaluated is unavailable. A fractional offset or a future read is an error. Fiscal daily and weekly model timelines are supported, including leap days and shortened fiscal weeks. |
//! | `diff(expr[, n])`, `pct_change(expr[, n])` | 1-2 | `diff` is current minus the value `n` periods back (default 1), in the input's units and exact for money. `pct_change` is the signed decimal `(current - base) / base`, also for a negative base; an exactly zero base is unavailable (`undefined_change`). Missing history is unavailable. |
//! | `growth_rate(expr, periods)` | 2 | Compound annual growth rate in decimal units, `(ending / base)^(1 / years) - 1`, over a required exact positive integer offset (no default lookback). The base must be strictly positive and the ending value non-negative, otherwise the result is unavailable (`undefined_change`); a zero ending value is -100%. Monthly/quarterly/semiannual/annual years equal the offset divided by the reporting slots in a year (12, 4, 2, 1); daily/weekly years use Actual/365 Fixed between the periods' final included dates. |
//! | `cumsum(expr)`, `cumprod(expr)`, `cummin(expr)`, `cummax(expr)` | 1 | Cumulative aggregate over every slot from the model start through the current period. A slot without a value makes the result unavailable. `cumsum` is exact for money; `cumprod` takes unitless numbers and an overflow is an error. |
//! | `rolling_mean(expr, window[, min_periods])`, `rolling_sum(expr, window[, min_periods])`, `rolling_std(expr, window[, min_periods])`, `rolling_var(expr, window[, min_periods])`, `rolling_median(expr, window[, min_periods])`, `rolling_min(expr, window[, min_periods])`, `rolling_max(expr, window[, min_periods])`, `rolling_count(expr, window[, min_periods])` | 2-3 | Statistic over the last `window` calendar slots through the current period (slots, not observations). `window` is an exact positive integer; `min_periods` is an exact integer from 1 through the window and defaults to the window. The result is unavailable with fewer available observations. `std` and `var` are sample statistics (`n - 1`) and need two observations. Sums and means are exact for money, and `std` keeps the currency. |
//! | `std(expr)`, `var(expr)`, `median(expr)` | 1 | Sample statistic over the observations available from the model start through the current period; a `gapped_sample` warning reports the sample count when slots are missing. `std` and `var` use `n - 1` and need two observations. |
//! | `rank(node[, ascending])` | 1-2 | Rank of the current value among the available observations of a statement-node or `cs.*` reference, with exact competition ties (tied values share the lowest rank). Ascending by default; a direction flag of exactly `0` selects descending. Unavailable when the current value is. |
//! | `quantile(node, q)` | 2 | R-7 (linear interpolation) quantile of the available observations of a statement-node or `cs.*` reference, in the series' units and currency. `q` is a finite level in `[0, 1]`. |
//! | `ewm_mean(expr, alpha)` | 2 | Exponentially weighted mean with weights decayed once per calendar slot from the model start (pandas `adjust=False, ignore_na=False`). A slot without a value ages the weights and carries the previous estimate. Requires `0 < alpha <= 1`. |
//! | `ewm_std(expr, alpha[, unbiased])`, `ewm_var(expr, alpha[, unbiased])` | 2-3 | Exponentially weighted standard deviation or variance on the same slot decay. The squared-weight correction `1 / (1 - sum(w^2))` is on by default; the optional toggle is a Boolean, or exactly `0` or `1`. The corrected statistic is unavailable with fewer than two observations. |
//! | `ttm(expr)` | 1 | Trailing-twelve-month sum over twelve calendar months (`ltm_window`). Quarterly models require 4 quarters, monthly models 12 months; daily models sum every model day (calendar or business) in the year and need a daily grid reaching back a full year. |
//! | `ytd(expr)` | 1 | Year-to-date sum over the plan's reporting year, including a fiscal plan's own year. Every slot of the window must hold a value; otherwise the result is unavailable. Exact for money. |
//! | `qtd(expr)` | 1 | Quarter-to-date sum for monthly models, over the Gregorian or fiscal quarter the month belongs to. Every slot must hold a value. |
//! | `fiscal_ytd(expr, start_month)` | 2 | Fiscal year-to-date sum on a Gregorian monthly plan, using an integer fiscal start month from 1 to 12. Every slot must hold a value. A fiscal plan uses `ytd`. |
//! | `annualize(expr, annualization_factor)` | 2 | Scale a period value to an annual amount by a required positive annualization factor; none is inferred from the period frequency. |
//! | `annualize_rate(rate, periods_per_year, compounding)` | 3 | Annualize a periodic decimal rate using the supplied positive finite periods per year. `compounding = 0` computes `rate × periods_per_year`; `compounding = 1` computes `(1 + rate)^periods_per_year - 1` and rejects a rate below -100%. Any other value is an error. |
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

pub(crate) mod ast;
pub(crate) mod compiler;
mod parser;
pub(crate) mod signatures;
mod validate;

pub use crate::utils::formula::extract_all_identifiers;
pub use ast::StmtExpr;
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
    /// per-term stack cost of the AST walkers (`compile`, `infer_value_type`,
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

    /// The deepest tree the preflight accepts must lower, render and drop
    /// on the same small stack.
    #[test]
    fn tree_at_depth_limit_compiles_on_a_small_stack() {
        use crate::dsl::StmtExpr;
        use finstack_quant_core::expr::UnaryOp;

        let compiled = std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(|| {
                let tree = (1..crate::dsl::validate::MAX_AST_DEPTH)
                    .fold(StmtExpr::node_ref("a"), |acc, _| {
                        StmtExpr::unary_op(UnaryOp::Neg, acc)
                    });
                let _ = tree.to_string();
                crate::dsl::compile(&tree).is_ok()
            })
            .expect("probe thread should spawn")
            .join()
            .expect("a depth-limit tree must not overflow a 512 KiB stack");
        assert!(compiled, "a depth-limit tree must compile successfully");
    }
}
