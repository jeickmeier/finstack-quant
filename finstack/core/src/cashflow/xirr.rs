//! Internal Rate of Return (IRR) and Extended IRR (XIRR).
//!
//! This module provides:
//! - `irr` for periodic cashflows (`[f64]`): Standard IRR
//! - `xirr` / `xirr_with_daycount` / `xirr_with_daycount_ctx` for irregular
//!   cashflows (`[(Date, f64)]`): XIRR (Extended internal rate of return)
//!
//! # Mathematical Foundation
//!
//! IRR is the rate r such that NPV(r) = 0:
//! ```text
//! Σ CF_i / (1 + r)^i = 0  (periodic)
//! Σ CF_i / (1 + r)^t_i = 0  (irregular, XIRR)
//! ```
//!
//! XIRR uses a day count convention (defaulting to Act/365F) to calculate year fractions.
//!
//! # Multiple Roots
//!
//! **Important**: For certain cashflow patterns, the NPV equation can have multiple roots
//! (i.e., multiple rates that produce NPV = 0). This typically occurs when:
//! - Cashflows change sign more than once (e.g., outflow, inflow, outflow)
//! - There are large negative cashflows late in the sequence
//!
//! When multiple roots exist, this implementation collects **all** valid roots from
//! every solver phase (Brent direct, Newton, Brent fallback), deduplicates them
//! within `1e-9` tolerance, and returns the root **closest to 0.0**. This
//! deterministic selection avoids dependence on solver iteration order.
//!
//! # Rate Bounds
//!
//! The solver rejects rates below `MIN_VALID_RATE` (-99.9%) as these represent
//! near-total loss scenarios that are economically implausible for most applications.
//!
//! # References
//!
//! - Brent, R. P. (1973), *Algorithms for Minimization Without Derivatives*.
//! - Hull, J. C., *Options, Futures, and Other Derivatives*.

use crate::dates::{Date, DayCount, DayCountContext};
use crate::error::InputError;
use crate::math::solver::{BrentSolver, NewtonSolver, Solver};
use crate::math::NeumaierAccumulator;

/// Default tolerance for IRR/XIRR solver.
///
/// Set to 1e-8 to match QuantLib's `CashFlows::irr()` standard.
/// Previous value (1e-6) was Excel-grade; 1e-8 provides professional-grade
/// precision suitable for long-duration cashflows where tolerance matters.
pub(crate) const DEFAULT_TOLERANCE: f64 = 1e-8;

/// Default maximum iterations for IRR/XIRR solver.
pub(crate) const DEFAULT_MAX_ITERATIONS: usize = 100;

/// Default initial guess for IRR/XIRR.
pub(crate) const DEFAULT_GUESS: f64 = 0.1;

/// Minimum valid rate threshold.
///
/// Rates at or below this threshold (-99.9%) represent near-total loss scenarios
/// where (1 + r) approaches zero. Such extreme rates:
/// - Cause numerical instability in discounting calculations
/// - Are economically implausible for most applications
/// - Often indicate solver convergence to an invalid root
///
/// The solver rejects any root at or below this threshold.
pub(crate) const MIN_VALID_RATE: f64 = -0.999;

/// Calculate the internal rate of return for periodic cashflows.
///
/// Periodic IRR treats `cashflows[i]` as occurring at integer time `i`, so the
/// returned rate is per period rather than annualized unless the input periods
/// are annual.
///
/// # Arguments
///
/// - `cashflows`: Cashflow amounts ordered by period. At least one positive and
///   one negative amount are required.
/// - `guess`: Optional initial solver guess as a decimal rate. `None` uses the
///   crate default of 10%.
///
/// # Returns
///
/// Returns the rate `r` that makes `sum(cashflows[i] / (1 + r)^i)` approximately
/// zero. When the cashflow sequence admits multiple roots, the deterministic
/// selection rule described in the module docs is used.
///
/// # Errors
///
/// Returns an error if there are fewer than two cashflows, no sign change, or no
/// valid root above `MIN_VALID_RATE` can be found within the configured solver
/// bounds.
///
/// # Examples
///
/// ```rust
/// use finstack_core::cashflow::irr;
///
/// let rate = irr(&[-100.0, 60.0, 60.0], None)?;
/// assert!((rate - 0.1306623863).abs() < 1e-8);
/// # Ok::<(), finstack_core::Error>(())
/// ```
#[inline]
pub fn irr(cashflows: &[f64], guess: Option<f64>) -> crate::Result<f64> {
    solve_rate_of_return(
        cashflows
            .iter()
            .enumerate()
            .map(|(i, &amt)| (i as f64, amt)),
        guess,
    )
}

/// Calculate XIRR for dated cashflows using the default `Act365F` convention.
///
/// XIRR sorts flows by date, measures each date from the earliest flow, and
/// solves for an annualized decimal rate. Prefer [`xirr_with_daycount`] when
/// the valuation convention should be explicit at the call site.
///
/// # Arguments
///
/// - `cashflows`: `(date, amount)` pairs. The input may be unsorted; same-date
///   flows are aggregated before solving.
/// - `guess`: Optional initial solver guess as a decimal annual rate. `None`
///   uses the crate default of 10%.
///
/// # Returns
///
/// Returns the annualized rate `r` that makes the dated NPV approximately zero
/// under `Act365F` year fractions.
///
/// # Errors
///
/// Returns an error if there are fewer than two flows, no sign change, a
/// day-count calculation fails, or no valid root above `MIN_VALID_RATE` can be
/// found.
///
/// # Examples
///
/// ```rust
/// use finstack_core::cashflow::xirr;
/// use finstack_core::dates::create_date;
/// use time::Month;
///
/// let flows = [
///     (create_date(2025, Month::January, 1)?, -100.0),
///     (create_date(2026, Month::January, 1)?, 110.0),
/// ];
/// let rate = xirr(&flows, None)?;
/// assert!((rate - 0.10).abs() < 1e-8);
/// # Ok::<(), finstack_core::Error>(())
/// ```
#[inline]
pub fn xirr(cashflows: &[(Date, f64)], guess: Option<f64>) -> crate::Result<f64> {
    xirr_with_daycount(cashflows, DayCount::Act365F, guess)
}

/// Calculate XIRR for dated cashflows with an explicit day-count convention.
///
/// Use this overload when the annualization basis matters for pricing,
/// reporting, or reconciliation. For conventions that require coupon or
/// calendar context, use [`xirr_with_daycount_ctx`].
///
/// # Arguments
///
/// - `cashflows`: `(date, amount)` pairs. The input may be unsorted; same-date
///   flows are aggregated before solving.
/// - `day_count`: Convention used to convert dates into year fractions from the
///   earliest flow date.
/// - `guess`: Optional initial solver guess as a decimal annual rate. `None`
///   uses the crate default of 10%.
///
/// # Returns
///
/// Returns the annualized rate `r` that makes the dated NPV approximately zero
/// under the supplied day-count convention.
///
/// # Errors
///
/// Returns an error if there are fewer than two flows, no sign change, the
/// day-count convention cannot evaluate a year fraction for the inputs, or no
/// valid root above `MIN_VALID_RATE` can be found.
///
/// # Examples
///
/// ```rust
/// use finstack_core::cashflow::xirr_with_daycount;
/// use finstack_core::dates::{create_date, DayCount};
/// use time::Month;
///
/// let flows = [
///     (create_date(2025, Month::January, 1)?, -100.0),
///     (create_date(2026, Month::January, 1)?, 110.0),
/// ];
/// let rate = xirr_with_daycount(&flows, DayCount::Act365F, None)?;
/// assert!((rate - 0.10).abs() < 1e-8);
/// # Ok::<(), finstack_core::Error>(())
/// ```
#[inline]
pub fn xirr_with_daycount(
    cashflows: &[(Date, f64)],
    day_count: DayCount,
    guess: Option<f64>,
) -> crate::Result<f64> {
    xirr_with_daycount_ctx(cashflows, day_count, DayCountContext::default(), guess)
}

/// Calculate XIRR with an explicit day-count context.
///
/// Use this helper for day-count conventions that require additional context,
/// such as `ActActIsma` coupon metadata or `Bus252` holiday calendars.
///
/// # Arguments
///
/// - `flows`: `(date, amount)` pairs. The input may be unsorted; same-date flows
///   are aggregated after converting dates to year fractions.
/// - `day_count`: Convention used to convert dates into year fractions.
/// - `ctx`: Supplemental day-count context, such as coupon schedule information
///   or business-day calendar data.
/// - `guess`: Optional initial solver guess as a decimal annual rate. `None`
///   uses the crate default of 10%.
///
/// # Returns
///
/// Returns the annualized rate `r` that makes the dated NPV approximately zero
/// under the supplied day-count convention and context.
///
/// # Errors
///
/// Returns an error if there are fewer than two flows, no sign change, the
/// day-count convention rejects the supplied context, or no valid root above
/// `MIN_VALID_RATE` can be found.
///
/// # Examples
///
/// ```rust
/// use finstack_core::cashflow::xirr_with_daycount_ctx;
/// use finstack_core::dates::{create_date, DayCount, DayCountContext};
/// use time::Month;
///
/// let flows = [
///     (create_date(2025, Month::January, 1)?, -100.0),
///     (create_date(2026, Month::January, 1)?, 110.0),
/// ];
/// let rate = xirr_with_daycount_ctx(
///     &flows,
///     DayCount::Act365F,
///     DayCountContext::default(),
///     None,
/// )?;
/// assert!((rate - 0.10).abs() < 1e-8);
/// # Ok::<(), finstack_core::Error>(())
/// ```
pub fn xirr_with_daycount_ctx(
    flows: &[(Date, f64)],
    day_count: DayCount,
    ctx: DayCountContext<'_>,
    guess: Option<f64>,
) -> crate::Result<f64> {
    if flows.len() < 2 {
        return Err(crate::Error::Validation(
            "Cashflows must contain at least two cashflows".to_string(),
        ));
    }

    // Find the earliest date (anchor) to compute year fractions from.
    // This avoids sorting by date first; we sort once by year fraction.
    let first_date = flows.iter().map(|(d, _)| *d).min().ok_or_else(|| {
        crate::Error::Validation("Cashflows must contain at least one flow".to_string())
    })?;

    // Compute (year_fraction, amount) for all flows in a single pass.
    // Sorting by year fraction happens below after collection.
    let mut years_and_amounts: Vec<(f64, f64)> = Vec::with_capacity(flows.len());
    for (date, amount) in flows.iter().copied() {
        let years = day_count.signed_year_fraction(first_date, date, ctx)?;
        years_and_amounts.push((years, amount));
    }

    // Sort once by year fraction (f64 total ordering).
    years_and_amounts.sort_by(|a, b| a.0.total_cmp(&b.0));

    // Aggregate entries with identical year-fractions by summing amounts.
    // This deduplicates same-date flows, reduces iteration cost, and avoids
    // f64 summation noise from carrying redundant entries.
    let mut aggregated: Vec<(f64, f64)> = Vec::with_capacity(years_and_amounts.len());
    for (t, amount) in years_and_amounts {
        if let Some(last) = aggregated.last_mut() {
            if (last.0 - t).abs() < 1e-12 {
                last.1 += amount;
                continue;
            }
        }
        aggregated.push((t, amount));
    }

    solve_rate_of_return(aggregated, guess)
}

// -----------------------------------------------------------------------------
// Solver logic
// -----------------------------------------------------------------------------

/// Solves for the rate of return (r) that sets the Net Present Value (NPV) to zero.
///
/// # Determinism contract
///
/// All solver phases (Brent direct, Newton, Brent fallback) are run exhaustively.
/// Valid roots are collected, deduplicated within `1e-9` tolerance, and the root
/// closest to `0.0` that satisfies `rate > MIN_VALID_RATE` is returned. This
/// guarantees a deterministic result regardless of solver iteration order.
///
/// # Arguments
/// * `flows` - Iterator of (time, amount) pairs
/// * `guess` - Optional initial guess
fn solve_rate_of_return<I>(flows: I, guess: Option<f64>) -> crate::Result<f64>
where
    I: IntoIterator<Item = (f64, f64)> + Clone,
{
    // We need to iterate multiple times:
    // 1. Validation (sign change)
    // 2. Solving (NPV / dNPV evaluation)
    // So we collect into a vector.
    let data: Vec<(f64, f64)> = flows.into_iter().collect();

    // Validate inputs
    if data.len() < 2 {
        return Err(InputError::TooFewPoints.into());
    }

    // Check for sign change
    if !has_sign_change(data.iter().map(|&(_, amt)| amt)) {
        return Err(InputError::Invalid.into());
    }
    if has_multiple_sign_changes(data.iter().map(|&(_, amt)| amt)) {
        tracing::warn!(
            num_flows = data.len(),
            algorithm = "newton_then_brent",
            "xirr: cashflows contain multiple sign changes; IRR may be non-unique, returning the first converged solution"
        );
    }

    // Define NPV function: Σ C_t / (1+r)^t using Neumaier compensated summation.
    // Uses exp(−t·ln(1+r)) instead of powf(t) to hoist the transcendental out of
    // the inner loop (one ln vs N powf calls).
    let npv = |rate: f64| -> f64 {
        let df_base = 1.0 + rate;
        if df_base <= 0.0 {
            return f64::INFINITY;
        }
        let ln_df = df_base.ln();
        let mut acc = NeumaierAccumulator::new();
        for &(t, amount) in &data {
            acc.add(amount * (-t * ln_df).exp());
        }
        acc.total()
    };

    // Define derivative d(NPV)/dr: Σ -t * C_t / (1+r)^(t+1) using Neumaier.
    // Same ln-exp hoist as npv.
    let npv_derivative = |rate: f64| -> f64 {
        let df_base = 1.0 + rate;
        if df_base <= 0.0 {
            return f64::INFINITY;
        }
        let ln_df = df_base.ln();
        let mut acc = NeumaierAccumulator::new();
        for &(t, amount) in &data {
            acc.add(-t * amount * (-(t + 1.0) * ln_df).exp());
        }
        acc.total()
    };

    // Newton solver with default configuration
    let newton = NewtonSolver::new()
        .tolerance(DEFAULT_TOLERANCE)
        .max_iterations(DEFAULT_MAX_ITERATIONS);

    // Initial guess strategy: user-provided guess or default
    let initial_guess = guess.unwrap_or(DEFAULT_GUESS);

    // Candidates: User guess + Combined list from legacy irr_periodic and xirr
    let seeds: &[f64] = &[
        initial_guess,
        0.1,   // 10% (common default)
        0.05,  // 5%
        0.2,   // 20%
        0.01,  // 1%
        0.5,   // 50%
        -0.05, // -5%
        -0.2,  // -20%
        -0.5,  // -50%
        -0.9,  // -90% (Distressed)
        -0.99, // -99% (Near total loss)
        -0.75, // -75%
        -0.25, // -25%
        0.0,   // 0%
        1.0,   // 100%
        2.0,   // 200% (VC/Startup)
        5.0,   // 500%
    ];

    // Collect all valid roots across solver phases, then pick the one closest to 0.
    let mut all_roots: Vec<f64> = Vec::new();

    let is_valid = |root: f64| root > MIN_VALID_RATE && root.is_finite();

    // Phase 0: Attempt direct Brent bracketing from NPV sign at r=0.
    let npv_at_zero = npv(0.0);
    if npv_at_zero.is_finite() && npv_at_zero.abs() > DEFAULT_TOLERANCE {
        let brent_direct = BrentSolver::new()
            .tolerance(DEFAULT_TOLERANCE)
            .max_iterations(DEFAULT_MAX_ITERATIONS)
            .bracket_hint(crate::math::solver::BracketHint::Xirr)
            .bracket_bounds(-0.99, 10.0);
        let direct_seed = if npv_at_zero > 0.0 { 0.1 } else { -0.1 };
        if let Ok(root) = brent_direct.solve(npv, direct_seed) {
            if is_valid(root) {
                all_roots.push(root);
            }
        }
    }

    // Phase 1: Try Newton-Raphson (fast, quadratic convergence) with all seeds
    for &g in seeds {
        if let Ok(root) = newton.solve_with_derivative(npv, npv_derivative, g) {
            if is_valid(root) && npv(root).abs() < DEFAULT_TOLERANCE * 100.0 {
                all_roots.push(root);
            }
        }
    }

    // Phase 2: fall back to Brent's method when a valid bracket is available.
    let brent = BrentSolver::new()
        .tolerance(DEFAULT_TOLERANCE)
        .max_iterations(DEFAULT_MAX_ITERATIONS)
        .bracket_hint(crate::math::solver::BracketHint::Xirr)
        .bracket_bounds(-0.99, 10.0);

    let brent_seeds: &[f64] = &[0.1, 0.0, -0.5, 0.5, -0.9, 1.0, 2.0, 5.0];

    for &g in brent_seeds {
        if let Ok(root) = brent.solve(npv, g) {
            if is_valid(root) {
                all_roots.push(root);
            }
        }
    }

    // Deduplicate roots within 1e-9 tolerance, then pick closest to 0.
    all_roots.sort_by(|a, b| a.total_cmp(b));
    all_roots.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    all_roots
        .into_iter()
        .min_by(|a, b| a.abs().total_cmp(&b.abs()))
        .ok_or_else(|| crate::Error::Validation("IRR calculation failed: no convergence".into()))
}

/// Return `true` if the iterator contains at least one positive and one negative value.
pub(crate) fn has_sign_change<I>(iter: I) -> bool
where
    I: IntoIterator<Item = f64>,
{
    let mut has_positive = false;
    let mut has_negative = false;

    for v in iter {
        if v > 0.0 {
            has_positive = true;
        } else if v < 0.0 {
            has_negative = true;
        }
        if has_positive && has_negative {
            return true;
        }
    }
    false
}

pub(crate) fn has_multiple_sign_changes<I>(iter: I) -> bool
where
    I: IntoIterator<Item = f64>,
{
    count_sign_changes(iter) > 1
}

/// Count the number of sign changes in a numeric sequence.
///
/// Zero values are skipped. This count is used by Descartes' rule of signs
/// to bound the number of positive real roots.
pub(crate) fn count_sign_changes<I>(iter: I) -> usize
where
    I: IntoIterator<Item = f64>,
{
    let mut prev_sign = 0i8;
    let mut changes = 0usize;
    for value in iter {
        let sign = if value > 0.0 {
            1
        } else if value < 0.0 {
            -1
        } else {
            0
        };
        if sign == 0 {
            continue;
        }
        if prev_sign != 0 && sign != prev_sign {
            changes += 1;
        }
        prev_sign = sign;
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::create_date;
    use time::Month;

    #[test]
    fn count_sign_changes_skips_zeros() {
        assert_eq!(count_sign_changes([1.0, 0.0, -1.0].iter().copied()), 1);
        assert_eq!(count_sign_changes([1.0, -1.0, 0.0, 1.0].iter().copied()), 2);
        assert_eq!(count_sign_changes([0.0, 0.0, 1.0].iter().copied()), 0);
    }

    /// Helper to compute NPV for periodic cashflows at a given rate
    fn compute_periodic_npv(amounts: &[f64], rate: f64) -> f64 {
        amounts
            .iter()
            .enumerate()
            .map(|(i, &a)| a / (1.0 + rate).powi(i as i32))
            .sum()
    }

    #[test]
    fn test_irr_periodic() {
        let amounts = [-100.0, 110.0];
        let rate = irr(&amounts, None).expect("IRR calculation should succeed in test");
        assert!((rate - 0.1).abs() < 1e-6); // 10% return

        let npv_at_irr = compute_periodic_npv(&amounts, rate);
        assert!(npv_at_irr.abs() < 1e-6);
    }

    #[test]
    fn test_irr_periodic_multiple_periods() {
        let amounts = [-1000.0, 300.0, 300.0, 300.0, 300.0];
        let rate = irr(&amounts, None).expect("IRR calculation should succeed in test");
        assert!(rate > 0.07 && rate < 0.08);

        let npv_at_irr = compute_periodic_npv(&amounts, rate);
        assert!(npv_at_irr.abs() < 1e-6);
    }

    #[test]
    fn test_irr_periodic_near_minus_100() {
        let amounts = [-100.0, 1.0];
        let rate = irr(&amounts, Some(-0.5)).expect("IRR calculation should succeed in test");
        assert!(rate < -0.9);
    }

    #[test]
    fn test_irr_periodic_high_positive() {
        let amounts = [-100.0, 300.0];
        let rate = irr(&amounts, Some(0.5)).expect("IRR calculation should succeed in test");
        assert!(rate > 1.0);
    }

    #[test]
    fn test_irr_periodic_no_sign_change() {
        let amounts = [100.0, 200.0, 300.0];
        assert!(irr(&amounts, None).is_err());
    }

    #[test]
    fn test_unified_irr_api() {
        let periodic_flows = [-100.0, 110.0];
        let periodic_irr = irr(&periodic_flows, None).expect("Periodic IRR failed");
        assert!((periodic_irr - 0.1).abs() < 1e-6);

        let dated_flows = [
            (
                create_date(2024, Month::January, 1).expect("Date"),
                -100_000.0,
            ),
            (
                create_date(2025, Month::January, 1).expect("Date"),
                110_000.0,
            ),
        ];
        let xirr_res = xirr(&dated_flows, None).expect("XIRR failed");
        let expected = (1.1_f64).powf(365.0 / 366.0) - 1.0;
        assert!((xirr_res - expected).abs() < 1e-6);
    }

    #[test]
    fn test_xirr_basic() {
        let flows = [
            (
                create_date(2024, Month::January, 1).expect("Valid test date"),
                -100_000.0,
            ),
            (
                create_date(2025, Month::January, 1).expect("Valid test date"),
                110_000.0,
            ),
        ];

        let result = xirr(&flows, None).expect("XIRR calculation should succeed in test");
        let expected = (1.1_f64).powf(365.0 / 366.0) - 1.0;
        assert!((result - expected).abs() < 1e-6);
    }

    #[test]
    fn test_xirr_multiple_flows() {
        let flows = [
            (
                create_date(2024, Month::January, 1).expect("Valid test date"),
                -100_000.0,
            ),
            (
                create_date(2024, Month::July, 1).expect("Valid test date"),
                5_000.0,
            ),
            (
                create_date(2025, Month::January, 1).expect("Valid test date"),
                110_000.0,
            ),
        ];

        let result = xirr(&flows, None).expect("XIRR calculation should succeed in test");
        assert!(result > 0.1 && result < 0.2);
    }

    #[test]
    fn test_xirr_unsorted_inputs_equivalence() {
        let sorted = [
            (
                create_date(2024, Month::January, 1).expect("Valid test date"),
                -100_000.0,
            ),
            (
                create_date(2025, Month::January, 1).expect("Valid test date"),
                110_000.0,
            ),
        ];
        let mut unsorted = sorted.to_vec();
        unsorted.reverse();

        let r1 = xirr(&sorted, None).expect("XIRR calculation should succeed in test");
        let r2 = xirr(&unsorted, None).expect("XIRR calculation should succeed in test");
        assert!((r1 - r2).abs() < 1e-8);
    }

    #[test]
    fn test_xirr_negative_return() {
        let flows = [
            (
                create_date(2024, Month::January, 1).expect("Valid test date"),
                -100_000.0,
            ),
            (
                create_date(2025, Month::January, 1).expect("Valid test date"),
                90_000.0,
            ),
        ];

        let result = xirr(&flows, None).expect("XIRR calculation should succeed in test");
        let expected = (0.9_f64).powf(365.0 / 366.0) - 1.0;
        assert!((result - expected).abs() < 1e-6);
    }

    #[test]
    fn test_xirr_no_sign_change() {
        let flows = [
            (
                create_date(2024, Month::January, 1).expect("Valid test date"),
                100_000.0,
            ),
            (
                create_date(2025, Month::January, 1).expect("Valid test date"),
                110_000.0,
            ),
        ];

        let result = xirr(&flows, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_xirr_too_few_flows() {
        let flows = [(
            create_date(2024, Month::January, 1).expect("Valid test date"),
            -100_000.0,
        )];

        let result = xirr(&flows, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_xirr_with_daycount_act365f() {
        let flows = [
            (
                create_date(2024, Month::January, 1).expect("Valid test date"),
                -100_000.0,
            ),
            (
                create_date(2025, Month::January, 1).expect("Valid test date"),
                110_000.0,
            ),
        ];

        let result1 = xirr(&flows, None).expect("XIRR calculation should succeed in test");
        let result2 = xirr_with_daycount(&flows, DayCount::Act365F, None)
            .expect("XIRR with Act/365F should succeed in test");

        assert!((result1 - result2).abs() < 1e-12);
    }

    #[test]
    fn test_xirr_with_daycount_act360() {
        let flows = [
            (
                create_date(2024, Month::January, 1).expect("Valid test date"),
                -100_000.0,
            ),
            (
                create_date(2024, Month::July, 1).expect("Valid test date"),
                102_500.0,
            ),
        ];

        let result_365 = xirr_with_daycount(&flows, DayCount::Act365F, None)
            .expect("XIRR with Act/365F should succeed");
        let result_360 = xirr_with_daycount(&flows, DayCount::Act360, None)
            .expect("XIRR with Act/360 should succeed");

        assert!(result_365 > 0.0);
        assert!(result_360 > 0.0);
        assert!((result_360 - result_365).abs() < 0.015);
    }

    #[test]
    fn test_xirr_allows_multiple_sign_changes_when_solver_can_converge() {
        let flows = [
            (
                create_date(2024, Month::January, 1).expect("Valid test date"),
                -100.0,
            ),
            (
                create_date(2025, Month::January, 1).expect("Valid test date"),
                230.0,
            ),
            (
                create_date(2026, Month::January, 1).expect("Valid test date"),
                -132.0,
            ),
        ];

        let rate = xirr(&flows, Some(0.10))
            .expect("multiple sign changes should warn but still attempt a solve");

        assert!(rate.is_finite(), "expected a finite IRR, got {rate}");
    }
}
