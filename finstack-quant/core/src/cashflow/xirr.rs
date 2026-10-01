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
//! # Unique-Return Contract
//!
//! After removing zero flows and, for XIRR, netting equal dates and year
//! fractions, cashflows must change sign exactly once. That pattern has at
//! most one root for `r > -1`. Nonconventional streams with more than one sign
//! change are rejected explicitly: they can have multiple or repeated roots,
//! and an initial guess does not establish which return is economically valid.
//! The rejection applies even when a particular nonconventional stream happens
//! to have only one root. This API does not select among ambiguous returns.
//!
//! # Rate Bounds
//!
//! The solver searches in `log(1 + r)` over the finite `f64` rate domain
//! `r > -1`, including distressed returns below -99.9%. It returns an error
//! when the mathematical return cannot be represented with sufficient NPV
//! accuracy as a finite `f64` rate greater than -1.
//!
//! # References
//!
//! - Brent, R. P. (1973), *Algorithms for Minimization Without Derivatives*. `docs/REFERENCES.md#brent-1973`
//! - Hull, J. C., *Options, Futures, and Other Derivatives*. `docs/REFERENCES.md#hull-options-futures`

use crate::dates::{Date, DayCount, DayCountContext};
use crate::error::InputError;
use crate::math::solver::BrentSolver;
use crate::math::NeumaierAccumulator;

/// Absolute root tolerance in log-growth coordinates, `log(1 + r)`.
const LOG_RATE_TOLERANCE: f64 = 1e-12;

/// Maximum residual relative to gross discounted cashflows after rate conversion.
const RELATIVE_NPV_TOLERANCE: f64 = 1e-8;

/// Default maximum iterations for IRR/XIRR solver.
const DEFAULT_MAX_ITERATIONS: usize = 100;

/// Default initial guess for IRR/XIRR.
const DEFAULT_GUESS: f64 = 0.1;

/// Calculate the internal rate of return for periodic cashflows.
///
/// Periodic IRR treats `cashflows[i]` as occurring at integer time `i`, so the
/// returned rate is per period rather than annualized unless the input periods
/// are annual.
///
/// # Arguments
///
/// - `cashflows`: Cashflow amounts ordered by period. At least one positive and
///   one negative amount are required. Zero amounts preserve elapsed period
///   spacing but do not affect the solver's time origin or residual scale.
///   Nonzero amounts must have exactly one sign change.
/// - `guess`: Optional finite decimal rate greater than -1 used to split the
///   search bracket. `None` uses 10%; it never selects among multiple roots.
///
/// # Returns
///
/// Returns the rate `r` that makes `sum(cashflows[i] / (1 + r)^i)` approximately
/// zero, when the cashflow sign pattern guarantees at most one root.
///
/// # Errors
///
/// Returns an error if an amount is non-finite, there are fewer than two nonzero
/// cashflows, there is not exactly one sign change, the guess is non-finite or
/// at most -1, or no sufficiently accurate finite rate greater than -1 exists.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::cashflow::irr;
///
/// let rate = irr(&[-100.0, 60.0, 60.0], None)?;
/// assert!((rate - 0.1306623863).abs() < 1e-8);
/// # Ok::<(), finstack_quant_core::Error>(())
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
/// XIRR nets flows by date, measures each remaining date from the earliest
/// nonzero net dated flow, and solves for an annualized decimal rate. Prefer
/// [`xirr_with_daycount`] when the valuation convention should be explicit at
/// the call site.
///
/// # Arguments
///
/// - `cashflows`: `(date, amount)` pairs with finite amounts. The input may be
///   unsorted; same-date flows are netted and zero net dates are ignored before
///   selecting the time origin. Net flows must have exactly one sign change.
/// - `guess`: Optional finite decimal annual rate greater than -1 used to
///   split the search bracket. `None` uses 10%; it does not select a root.
///
/// # Returns
///
/// Returns the annualized rate `r` that makes the dated NPV approximately zero
/// under `Act365F` year fractions.
///
/// # Errors
///
/// Returns an error if an amount or net amount is non-finite, there are fewer
/// than two nonzero net flows, there is not exactly one net sign change, the
/// guess is non-finite or at most -1, a day-count calculation fails, or no
/// sufficiently accurate finite rate greater than -1 can be found.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::cashflow::xirr;
/// use finstack_quant_core::dates::create_date;
/// use time::Month;
///
/// let flows = [
///     (create_date(2025, Month::January, 1)?, -100.0),
///     (create_date(2026, Month::January, 1)?, 110.0),
/// ];
/// let rate = xirr(&flows, None)?;
/// assert!((rate - 0.10).abs() < 1e-8);
/// # Ok::<(), finstack_quant_core::Error>(())
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
/// - `cashflows`: `(date, amount)` pairs with finite amounts. The input may be
///   unsorted; same-date flows are netted and zero net dates are ignored before
///   selecting the time origin. Amounts at identical year fractions are netted
///   again before solving; the resulting flows must change sign exactly once.
/// - `day_count`: Convention used to convert dates into year fractions from the
///   earliest nonzero net dated flow date.
/// - `guess`: Optional finite decimal annual rate greater than -1 used to
///   split the search bracket. `None` uses 10%; it does not select a root.
///
/// # Returns
///
/// Returns the annualized rate `r` that makes the dated NPV approximately zero
/// under the supplied day-count convention.
///
/// # Errors
///
/// Returns an error if an amount or net amount is non-finite, there are fewer
/// than two nonzero net flows, there is not exactly one net sign change, the
/// guess is non-finite or at most -1, the day-count convention cannot evaluate
/// a finite year fraction, or no sufficiently accurate finite rate greater
/// than -1 can be found.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::cashflow::xirr_with_daycount;
/// use finstack_quant_core::dates::{create_date, DayCount};
/// use time::Month;
///
/// let flows = [
///     (create_date(2025, Month::January, 1)?, -100.0),
///     (create_date(2026, Month::January, 1)?, 110.0),
/// ];
/// let rate = xirr_with_daycount(&flows, DayCount::Act365F, None)?;
/// assert!((rate - 0.10).abs() < 1e-8);
/// # Ok::<(), finstack_quant_core::Error>(())
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
/// - `flows`: `(date, amount)` pairs with finite amounts. The input may be
///   unsorted; same-date flows are netted before selecting the time origin.
///   Remaining dates are converted to year fractions from the earliest nonzero
///   net dated flow, with amounts at identical year fractions netted again.
///   The resulting nonzero flows must change sign exactly once.
/// - `day_count`: Convention used to convert dates into year fractions.
/// - `ctx`: Supplemental day-count context, such as coupon schedule information
///   or business-day calendar data. `Act365L` requires frequency and one full
///   enclosing coupon period containing the earliest nonzero net flow and all
///   remaining flow dates.
/// - `guess`: Optional finite decimal annual rate greater than -1 used to
///   split the search bracket. `None` uses 10%; it does not select a root.
///
/// # Returns
///
/// Returns the annualized rate `r` that makes the dated NPV approximately zero
/// under the supplied day-count convention and context.
///
/// # Errors
///
/// Returns an error if an amount or net amount is non-finite, there are fewer
/// than two nonzero net flows, there is not exactly one net sign change, the
/// guess is non-finite or at most -1, the day-count convention rejects the
/// context or produces a non-finite year fraction, or no sufficiently accurate
/// finite rate greater than -1 can be found.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::cashflow::xirr_with_daycount_ctx;
/// use finstack_quant_core::dates::{create_date, DayCount, DayCountContext};
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
/// # Ok::<(), finstack_quant_core::Error>(())
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

    if flows.iter().any(|(_, amount)| !amount.is_finite()) {
        return Err(crate::Error::Validation(
            "IRR cashflow amounts must be finite".into(),
        ));
    }

    let mut dated_flows = flows.to_vec();
    dated_flows.sort_by_key(|&(date, _)| date);

    // Net dates before selecting the origin: offsetting flows are equivalent
    // to a zero amount even under non-additive day counts such as 30/360 US.
    // Keep each surviving group's original components until the final time
    // aggregation, so rounding a large date total cannot erase a small residue
    // when different dates map to the same year fraction.
    let mut net_dates = Vec::new();
    let mut start = 0;
    while start < dated_flows.len() {
        let date = dated_flows[start].0;
        let mut end = start;
        let mut sum = NeumaierAccumulator::new();
        while end < dated_flows.len() && dated_flows[end].0 == date {
            sum.add(dated_flows[end].1);
            end += 1;
        }
        let amount = sum.total();
        if !amount.is_finite() {
            return Err(crate::Error::Validation(
                "IRR net dated cashflow amounts must be finite".into(),
            ));
        }
        if amount != 0.0 {
            net_dates.push((date, start..end));
        }
        start = end;
    }
    let first_date = net_dates.first().ok_or(InputError::TooFewPoints)?.0;

    let mut years_and_amounts = Vec::with_capacity(dated_flows.len());
    for (date, components) in net_dates {
        let years = day_count.signed_year_fraction(first_date, date, ctx)?;
        if !years.is_finite() {
            return Err(crate::Error::Validation(
                "IRR cashflow times must be finite".into(),
            ));
        }
        years_and_amounts.extend(
            dated_flows[components]
                .iter()
                .map(|&(_, amount)| (years, amount)),
        );
    }
    years_and_amounts.sort_by(|a, b| a.0.total_cmp(&b.0));

    // Distinct dates can have identical times under conventions such as
    // 30/360. Net those amounts too, so gross-PV residual normalization does
    // not count large offsetting components as economic cashflows.
    let mut aggregated = Vec::with_capacity(years_and_amounts.len());
    start = 0;
    while start < years_and_amounts.len() {
        let time = years_and_amounts[start].0;
        let mut end = start;
        let mut sum = NeumaierAccumulator::new();
        while end < years_and_amounts.len() && years_and_amounts[end].0.total_cmp(&time).is_eq() {
            sum.add(years_and_amounts[end].1);
            end += 1;
        }
        let amount = sum.total();
        if !amount.is_finite() {
            return Err(crate::Error::Validation(
                "IRR net cashflow amounts must be finite".into(),
            ));
        }
        if amount != 0.0 {
            aggregated.push((time, amount));
        }
        start = end;
    }

    solve_rate_of_return(aggregated, guess)
}

/// Solve for a unique rate of return whose NPV is zero.
///
/// Time-ordered, net cashflows must change sign exactly once. The generalized
/// Descartes rule then gives at most one positive discount-factor root, so the
/// initial guess only narrows a bracket and cannot select another return.
///
/// # Arguments
///
/// * `flows` - Strictly time-ordered, net (year fraction, amount) pairs.
/// * `guess` - Optional finite decimal rate greater than -1 for bracketing.
fn solve_rate_of_return<I>(flows: I, guess: Option<f64>) -> crate::Result<f64>
where
    I: IntoIterator<Item = (f64, f64)>,
{
    let mut data: Vec<(f64, f64)> = flows.into_iter().collect();
    if data
        .iter()
        .any(|&(time, amount)| !time.is_finite() || !amount.is_finite())
    {
        return Err(crate::Error::Validation(
            "IRR cashflow times and amounts must be finite".into(),
        ));
    }
    data.retain(|&(_, amount)| amount != 0.0);
    if data.len() < 2 {
        return Err(InputError::TooFewPoints.into());
    }

    // A common time shift cannot change a root. Removing it also prevents
    // delayed cashflows from shrinking the residual at large positive rates.
    let first_time = data[0].0;
    for (time, _) in &mut data {
        *time -= first_time;
    }
    let sign_changes = count_sign_changes(data.iter().map(|&(_, amount)| amount));
    if sign_changes != 1 {
        return Err(crate::Error::Validation(format!(
            "IRR requires exactly one sign change after netting; found {sign_changes}. Nonconventional cashflows can have multiple or repeated roots"
        )));
    }

    let initial_guess = guess.unwrap_or(DEFAULT_GUESS);
    if !initial_guess.is_finite() || initial_guess <= -1.0 {
        return Err(crate::Error::Validation(
            "IRR guess must be finite and greater than -1".into(),
        ));
    }

    // Log magnitudes and gross-PV normalization preserve small finite flows
    // and prevent discount-factor overflow across the entire search domain.
    let data: Vec<_> = data
        .into_iter()
        .map(|(time, amount)| (time, amount.abs().ln(), amount.signum()))
        .collect();
    let npv = |log_growth| relative_npv(&data, log_growth);

    // Search every representable rate above -1, without an economic loss
    // floor or arbitrary positive cap. Log coordinates avoid singular steps.
    let min_rate = -1.0 + f64::EPSILON / 2.0;
    let mut lower = min_rate.ln_1p();
    let mut upper = f64::MAX.ln();
    let seed = initial_guess.ln_1p();
    let at_seed = npv(seed);
    if at_seed != 0.0 {
        // At the infinite log-growth endpoints, the last and first cashflows
        // respectively dominate. Uniqueness identifies the correct half.
        if at_seed.is_sign_positive() == data[0].2.is_sign_positive() {
            upper = seed;
        } else {
            lower = seed;
        }
    } else {
        return Ok(initial_guess);
    }

    let at_lower = npv(lower);
    let at_upper = npv(upper);
    if at_lower != 0.0 && at_upper != 0.0 && at_lower.signum() == at_upper.signum() {
        return Err(crate::Error::Validation(
            "IRR root lies outside the representable finite rate domain greater than -1".into(),
        ));
    }

    let log_growth = BrentSolver::new()
        .tolerance(LOG_RATE_TOLERANCE)
        .max_iterations(DEFAULT_MAX_ITERATIONS)
        .solve_in_bracket(npv, lower, upper)?;
    let rate = log_growth.exp_m1();
    // Rounding expm1 back into the rate domain can lose meaningful accuracy
    // near -1. Validate the returned rate, not just its log-space root.
    if !rate.is_finite() || rate <= -1.0 || npv(rate.ln_1p()).abs() > RELATIVE_NPV_TOLERANCE {
        return Err(crate::Error::Validation(
            "IRR root is not representable as a sufficiently accurate finite rate greater than -1"
                .into(),
        ));
    }
    Ok(rate)
}

/// NPV divided by gross discounted cashflows at a log growth factor.
///
/// Entries are `(time, log_absolute_amount, sign)` with finite, nonnegative
/// times. Rescaling bounds each term without erasing finite input amounts.
fn relative_npv(data: &[(f64, f64, f64)], log_growth: f64) -> f64 {
    let max_log_pv = data
        .iter()
        .map(|&(time, log_amount, _)| log_amount - time * log_growth)
        .fold(f64::NEG_INFINITY, f64::max);
    if !max_log_pv.is_finite() {
        return f64::INFINITY;
    }
    let mut npv = NeumaierAccumulator::new();
    let mut gross_pv = NeumaierAccumulator::new();
    for &(time, log_amount, sign) in data {
        let pv = sign * (log_amount - time * log_growth - max_log_pv).exp();
        npv.add(pv);
        gross_pv.add(pv.abs());
    }
    npv.total() / gross_pv.total()
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

        // The solver measures NPV relative to gross discounted cashflows,
        // so the residual bound is scale-free rather than in currency units.
        let npv_at_irr = compute_periodic_npv(&amounts, rate);
        assert!(npv_at_irr.abs() / 1000.0 < 1e-6);
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
    fn irr_leading_zero_periods_preserve_roots_and_missing_roots() {
        for amounts in [&[-100.0, 110.0][..], &[-100.0, -30.0, 150.0][..]] {
            let expected = irr(amounts, None).expect("cashflows have a valid root");
            for lag in [1, 5, 20] {
                let mut delayed = vec![0.0; lag];
                delayed.extend_from_slice(amounts);
                let actual = irr(&delayed, None).expect("time shift preserves roots");
                assert!((actual - expected).abs() < 1e-12);
            }
        }

        // NPV = -100*v^lag*(1-v+v^2), v = 1/(1+r), is strictly
        // negative for every r > -1. A small discounted residual is not a root.
        for lag in [0, 1, 5, 20] {
            let mut rootless = vec![0.0; lag];
            rootless.extend_from_slice(&[-100.0, 100.0, -100.0]);
            assert!(irr(&rootless, None).is_err(), "lag={lag} has no IRR");
        }
    }

    #[test]
    fn xirr_ignores_zero_dates_when_selecting_the_time_origin() {
        let early = create_date(2025, Month::January, 1).expect("date");
        let d0 = create_date(2030, Month::January, 1).expect("date");
        let d1 = create_date(2031, Month::January, 1).expect("date");
        let d2 = create_date(2032, Month::January, 1).expect("date");

        let expected = xirr(&[(d0, -100.0), (d1, 110.0)], None).expect("valid root");
        let actual = xirr(&[(early, 0.0), (d0, -100.0), (d1, 110.0)], None)
            .expect("zero amount does not change the root");
        assert!((actual - expected).abs() < 1e-12);

        assert!(xirr(
            &[(early, 0.0), (d0, -100.0), (d1, 100.0), (d2, -100.0)],
            None
        )
        .is_err());
    }

    #[test]
    fn xirr_nets_dates_before_selecting_a_thirty360_origin() {
        let early = create_date(2025, Month::January, 29).expect("date");
        let d0 = create_date(2025, Month::February, 28).expect("date");
        let d1 = create_date(2025, Month::March, 30).expect("date");
        let expected = 1.1_f64.powi(12) - 1.0;
        for prefix in [
            vec![],
            vec![(early, 0.0)],
            vec![(early, -1.0), (early, 1.0)],
        ] {
            let mut flows = prefix;
            flows.extend([(d0, -100.0), (d1, 110.0)]);
            let rate = xirr_with_daycount(&flows, DayCount::Thirty360, None)
                .expect("net-zero date does not affect the origin");
            assert!((rate - expected).abs() < 1e-8);
        }
    }

    #[test]
    fn xirr_compensates_same_date_cancellation_in_every_order() {
        let d0 = create_date(2025, Month::January, 1).expect("date");
        let d1 = create_date(2026, Month::January, 1).expect("date");
        let amounts = [-1e16, -1.0, 1e16];
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let mut flows: Vec<_> = order.into_iter().map(|i| (d0, amounts[i])).collect();
            flows.push((d1, 1.1));
            assert!((xirr(&flows, None).expect("compensated net outflow") - 0.1).abs() < 1e-8);
        }
    }

    #[test]
    fn xirr_compensates_distinct_dates_with_identical_year_fractions() {
        let d0 = create_date(2025, Month::January, 30).expect("date");
        let same_time = create_date(2025, Month::January, 31).expect("date");
        let d1 = create_date(2026, Month::January, 30).expect("date");
        let flows = [(d0, -1e16), (same_time, -1.0), (same_time, 1e16), (d1, 1.1)];
        let rate = xirr_with_daycount(&flows, DayCount::Thirty360, None)
            .expect("compensated net time-zero outflow");
        assert!((rate - 0.1).abs() < 1e-8);
    }

    #[test]
    fn irr_rejects_non_finite_cashflows() {
        for amount in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(irr(&[-100.0, amount, 110.0], None).is_err());
        }
    }

    #[test]
    fn irr_does_not_confuse_large_later_amounts_with_a_root() {
        for scale in [1e2, 1e4, 1e8] {
            // -1 + scale*v - scale^2*v^2 has negative discriminant, so
            // none of these streams has a root, regardless of amount scale.
            let rootless = [-1.0, scale, -scale * scale];
            assert!(irr(&rootless, None).is_err(), "scale={scale} has no IRR");
        }
    }

    #[test]
    fn relative_npv_matches_one_period_closed_form() {
        let flows = [(0.0, 100.0_f64.ln(), -1.0), (1.0, 110.0_f64.ln(), 1.0)];
        // For [-100,110], relative NPV = (0.1-r)/(2.1+r).
        for rate in [-0.99_f64, -0.5, 0.0, 0.1, 1.0, 10.0] {
            let residual = relative_npv(&flows, rate.ln_1p());
            let expected = (0.1 - rate) / (2.1 + rate);
            assert!((residual - expected).abs() < 1e-14);
        }

        for rate in [-1.0 + f64::EPSILON, f64::MAX] {
            assert!(relative_npv(&flows, rate.ln_1p()).is_finite());
        }
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

    /// IRR is scale-invariant: a 1e9-notional stream must converge to the
    /// same rate as the identical stream scaled to 1.0. Residual tolerances
    /// must therefore be relative to the discounted cashflows.
    #[test]
    fn test_xirr_scale_invariant_large_notional() {
        let d0 = create_date(2024, Month::January, 1).expect("Valid test date");
        let d1 = create_date(2024, Month::July, 1).expect("Valid test date");
        let d2 = create_date(2025, Month::January, 1).expect("Valid test date");

        let unit = [(d0, -1.0), (d1, 0.03), (d2, 1.05)];
        let large = [(d0, -1.0e9), (d1, 0.03e9), (d2, 1.05e9)];

        let r_unit = xirr(&unit, None).expect("unit-scale XIRR should converge");
        let r_large = xirr(&large, None).expect("1e9-notional XIRR should converge");

        assert!(
            (r_unit - r_large).abs() < 1e-10,
            "XIRR must be scale-invariant: unit={r_unit}, large={r_large}"
        );
    }

    /// High-rate streams must also be scale-invariant (the review noted
    /// >1000% IRRs being lost to the bounded Brent fallback at large scale).
    #[test]
    fn test_xirr_scale_invariant_high_rate() {
        let d0 = create_date(2024, Month::January, 1).expect("Valid test date");
        let d1 = create_date(2025, Month::January, 1).expect("Valid test date");

        let unit = [(d0, -1.0), (d1, 12.0)]; // ~1100% annual return
        let large = [(d0, -1.0e9), (d1, 12.0e9)];

        let r_unit = xirr(&unit, None).expect("unit-scale XIRR should converge");
        let r_large = xirr(&large, None).expect("large-scale XIRR should converge");

        assert!(r_unit > 10.0, "expected ~1100% IRR, got {r_unit}");
        assert!(
            (r_unit - r_large).abs() < 1e-8,
            "high-rate XIRR must be scale-invariant: unit={r_unit}, large={r_large}"
        );
    }

    #[test]
    fn xirr_rejects_multiple_sign_changes_even_with_a_root_guess() {
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

        let error = xirr(&flows, Some(0.10)).expect_err("ambiguous return is rejected");
        assert!(error.to_string().contains("exactly one sign change"));
    }

    #[test]
    fn irr_rejects_ambiguous_polynomial_and_repeated_roots() {
        // Multiplying NPV by (1+r)^5 yields a polynomial with exact roots
        // -0.52, -0.43, 0.48, 0.63, and 4.25. Finite seed searches previously
        // returned +48% while claiming to select the closest root, -43%.
        let amounts = [-1.0, 9.41, -27.7915, 34.629291, -18.42559164, 3.46517136];
        for root in [-0.52, -0.43, 0.48, 0.63, 4.25] {
            assert!(compute_periodic_npv(&amounts, root).abs() < 1e-10);
            let error = irr(&amounts, Some(root)).expect_err("guess cannot select a branch");
            assert!(error.to_string().contains("exactly one sign change"));
        }
        assert!(irr(&amounts, None).is_err());

        // NPV*(1+r)^2 = -(r-0.1)^2: a repeated root also requires an
        // explicit nonconventional-return policy, not residual-only guesses.
        let repeated = [-1.0, 2.2, -1.21];
        assert!(compute_periodic_npv(&repeated, 0.1).abs() < 1e-14);
        assert!(irr(&repeated, Some(0.1)).is_err());

        let start = create_date(2025, Month::January, 1).expect("date");
        let dated: Vec<_> = amounts
            .iter()
            .enumerate()
            .map(|(i, &amount)| (start + time::Duration::days(365 * i as i64), amount))
            .collect();
        assert!(xirr(&dated, None).is_err());
    }

    #[test]
    fn irr_covers_distressed_and_large_finite_returns() {
        let start = create_date(2025, Month::January, 1).expect("date");
        let end = create_date(2026, Month::January, 1).expect("date");
        for payoff in [0.2, 0.01, 0.0001, 1e10, 1e100] {
            let expected_growth = payoff / 100.0;
            let amounts = [-100.0, payoff];
            for guess in [None, Some(-0.99), Some(100.0)] {
                let rate = irr(&amounts, guess).expect("unique finite return");
                assert!(((1.0 + rate) / expected_growth - 1.0).abs() < 1e-8);
                let dated = xirr(&[(start, -100.0), (end, payoff)], guess)
                    .expect("dated unique finite return");
                assert!(((1.0 + dated) / expected_growth - 1.0).abs() < 1e-8);
            }
        }
    }

    #[test]
    fn irr_rejects_invalid_guesses_and_unrepresentable_returns() {
        for guess in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, -2.0] {
            assert!(irr(&[-100.0, 110.0], Some(guess)).is_err());
        }
        // The mathematical growth factors are positive, but their rates
        // round to -1 or exceed the largest finite f64.
        assert!(irr(&[-1.0, 1e-100], None).is_err());
        assert!(irr(&[-1e-100, 1e300], None).is_err());
    }
}
