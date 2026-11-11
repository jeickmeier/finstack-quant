//! Internal Rate of Return (IRR) and Extended IRR (XIRR), record
//! CORE-CONVENTIONS-017.
//!
//! - [`irr`] for regularly spaced periodic cashflows (`[f64]`), returning a
//!   per-period effective rate.
//! - [`xirr`] for dated cashflows (`[(Date, f64)]`), returning an effective
//!   annual rate on a fixed ACT/365F clock.
//!
//! # Mathematical Foundation
//!
//! The returned rate `r > -1` solves:
//! ```text
//! Σ C_i / (1 + r)^t_i = 0
//! ```
//! with `t_i` the integer period index ([`irr`]) or actual calendar days from
//! the first surviving nonzero net date divided by exactly 365 ([`xirr`]).
//! The rate is a decimal effective growth rate, not percent, basis points or a
//! continuously compounded rate.
//!
//! # Unique-Return Contract
//!
//! After removing zero amounts and, for XIRR, netting each date once, the
//! stream must have at least two nonzero amounts at distinct times and
//! exactly one sign change, in either investment or borrowing direction. That
//! stream has exactly one root `r > -1`. Streams with more sign changes are
//! rejected even when a particular one has a unique root; there is no guess,
//! root selection or multiple-root policy.
//!
//! # Numerical Contract
//!
//! The solver works in `x = log1p(r)` over the full finite rate domain
//! `next_up(-1) ..= f64::MAX`, evaluating the scaled residual
//! `rho(x) = Σ sign_i·exp(a_i - m) / Σ exp(a_i - m)` with
//! `a_i = ln|C_i| - t_i·x` and `m = max a_i`. The shared Brent solver runs at
//! most 100 iterations with log tolerance `1e-12`. Every success path returns
//! through one gate: a finite candidate inside the bracket, a finite rate
//! greater than -1 and a separately recomputed finite residual with
//! `|rho| <= 1e-8`. These are operational tolerances, not a guaranteed error
//! bound on `r`.
//!
//! # References
//!
//! - Brent, R. P. (1973), *Algorithms for Minimization Without Derivatives*. `docs/REFERENCES.md#brent-1973`
//! - Hull, J. C., *Options, Futures, and Other Derivatives*. `docs/REFERENCES.md#hull-options-futures`

use crate::InputError;
use crate::dates::Date;
use crate::math::solver::BrentSolver;
use crate::math::summation::NeumaierAccumulator;

/// Absolute root tolerance in log-growth coordinates, `log1p(r)`.
const LOG_RATE_TOLERANCE: f64 = 1e-12;

/// Largest accepted scaled residual `|rho|` at the returned rate.
const RESIDUAL_TOLERANCE: f64 = 1e-8;

/// Maximum Brent iterations.
const MAX_ITERATIONS: usize = 100;

/// Days in the fixed XIRR year (ACT/365F).
const XIRR_DAYS_PER_YEAR: f64 = 365.0;

/// Calculate the internal rate of return for periodic cashflows.
///
/// `cashflows[i]` occurs at integer period `i`. Zero amounts preserve elapsed
/// periods; the time origin is the first nonzero amount. The returned rate is
/// per input period and is annual only when the periods are annual.
///
/// # Arguments
///
/// * `cashflows` - Finite signed cash amounts on a regularly spaced period
///   grid owned by the caller; positive receipts, negative payments. After
///   dropping zeros there must be at least two amounts with exactly one sign
///   change.
///
/// # Returns
///
/// The per-period effective rate `r > -1` with `Σ cashflows[i]/(1+r)^i = 0`.
///
/// # Errors
///
/// Returns an error if an amount is non-finite, fewer than two nonzero amounts
/// remain, the sign does not change exactly once, a period offset is not
/// exactly representable as `f64`, or the solver cannot produce a finite rate
/// above -1 that passes the residual gate.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::cashflow::irr;
///
/// let rate = irr(&[-100.0, 60.0, 60.0])?;
/// assert!((rate - 0.1306623863).abs() < 1e-8);
/// // A zero amount keeps its period: [-100, 0, 121] is 10% per period.
/// assert!((irr(&[-100.0, 0.0, 121.0])? - 0.1).abs() < 1e-8);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn irr(cashflows: &[f64]) -> crate::Result<f64> {
    validate_amounts(cashflows.iter().copied())?;
    let nonzero: Vec<(usize, f64)> = cashflows
        .iter()
        .copied()
        .enumerate()
        .filter(|&(_, amount)| amount != 0.0)
        .collect();
    let Some(&(first, _)) = nonzero.first() else {
        return Err(InputError::TooFewPoints.into());
    };
    let mut stream = Vec::with_capacity(nonzero.len());
    for (index, amount) in nonzero {
        // Shift in integer space, then require an exact f64 offset.
        let offset = u64::try_from(index - first).map_err(|_| period_offset_error())?;
        stream.push((exact_time(offset)?, amount));
    }
    solve(&stream)
}

/// Calculate XIRR for dated cashflows on the fixed ACT/365F clock.
///
/// Each date's amounts are netted once (ascending absolute value, then signed
/// value, with compensated summation); dates whose net is exactly zero are
/// removed before the first surviving date becomes the time origin. Time is
/// actual calendar days from that origin divided by exactly 365, with no
/// holiday, coupon or fiscal adjustment.
///
/// # Arguments
///
/// * `flows` - `(date, amount)` pairs of finite signed cash amounts in one
///   owner-selected currency; positive receipts, negative payments. The input
///   may be unsorted and repeat dates. After netting there must be at least
///   two nonzero dates with exactly one sign change.
///
/// # Returns
///
/// The effective annual rate on a 365-day basis, `r > -1`.
///
/// # Errors
///
/// Returns an error if an amount or a date's net is non-finite, fewer than two
/// nonzero net dates remain, the net stream does not change sign exactly once,
/// or the solver cannot produce a finite rate above -1 that passes the
/// residual gate.
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
/// assert!((xirr(&flows)? - 0.10).abs() < 1e-8);
///
/// // 2024 has a leap day: the return is (1.1)^(365/366) - 1, not 10%.
/// let leap = [
///     (create_date(2024, Month::January, 1)?, -100.0),
///     (create_date(2025, Month::January, 1)?, 110.0),
/// ];
/// assert!((xirr(&leap)? - (1.1_f64.powf(365.0 / 366.0) - 1.0)).abs() < 1e-10);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn xirr(flows: &[(Date, f64)]) -> crate::Result<f64> {
    validate_amounts(flows.iter().map(|&(_, amount)| amount))?;
    let mut dated = flows.to_vec();
    dated.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.abs().total_cmp(&b.1.abs()))
            .then_with(|| a.1.total_cmp(&b.1))
    });

    let mut nets: Vec<(Date, f64)> = Vec::new();
    let mut start = 0;
    while start < dated.len() {
        let date = dated[start].0;
        let mut sum = NeumaierAccumulator::new();
        let mut end = start;
        while end < dated.len() && dated[end].0 == date {
            sum.add(dated[end].1);
            end += 1;
        }
        let net = sum.total();
        if !net.is_finite() {
            return Err(crate::Error::Validation(
                "XIRR net amount for a date must be finite".into(),
            ));
        }
        if net != 0.0 {
            nets.push((date, net));
        }
        start = end;
    }
    let Some(&(origin, _)) = nets.first() else {
        return Err(InputError::TooFewPoints.into());
    };

    let mut stream = Vec::with_capacity(nets.len());
    for (date, amount) in nets {
        let days =
            u64::try_from((date - origin).whole_days()).map_err(|_| period_offset_error())?;
        stream.push((exact_time(days)? / XIRR_DAYS_PER_YEAR, amount));
    }
    solve(&stream)
}

/// Reject non-finite amounts before any shortcut.
fn validate_amounts(amounts: impl IntoIterator<Item = f64>) -> crate::Result<()> {
    if amounts.into_iter().any(|amount| !amount.is_finite()) {
        return Err(crate::Error::Validation(
            "IRR cashflow amounts must be finite".into(),
        ));
    }
    Ok(())
}

/// An integer time offset as an exactly representable `f64`.
fn exact_time(offset: u64) -> crate::Result<f64> {
    let time = offset as f64;
    if time >= 2.0_f64.powi(64) || time as u64 != offset {
        return Err(period_offset_error());
    }
    Ok(time)
}

fn period_offset_error() -> crate::Error {
    crate::Error::Validation("IRR time offset is not exactly representable as f64".into())
}

/// Solve the prepared stream: strictly increasing times from zero, nonzero
/// finite amounts.
fn solve(stream: &[(f64, f64)]) -> crate::Result<f64> {
    if stream.len() < 2 {
        return Err(InputError::TooFewPoints.into());
    }
    let sign_changes = count_sign_changes(stream.iter().map(|&(_, amount)| amount));
    if sign_changes != 1 {
        return Err(crate::Error::Validation(format!(
            "IRR requires exactly one sign change after netting; found {sign_changes}"
        )));
    }
    let terms: Vec<(f64, f64, f64)> = stream
        .iter()
        .map(|&(time, amount)| (time, amount.abs().ln(), amount.signum()))
        .collect();
    let rho = |log_growth: f64| scaled_residual(&terms, log_growth);

    let min_rate = (-1.0_f64).next_up();
    let max_rate = f64::MAX;
    let lower = min_rate.ln_1p();
    let upper = max_rate.ln_1p();
    let at_lower = rho(lower)?;
    let at_upper = rho(upper)?;
    if at_lower != 0.0 && at_upper != 0.0 && at_lower.signum() == at_upper.signum() {
        return Err(crate::Error::Validation(
            "IRR root is not bracketed in the finite rate domain greater than -1".into(),
        ));
    }

    let candidate = BrentSolver::new()
        .with_tolerance(LOG_RATE_TOLERANCE)
        .with_max_iterations(MAX_ITERATIONS)
        .solve_in_bracket(rho, lower, upper)?;
    accept(&terms, candidate, (lower, min_rate), (upper, max_rate))
}

/// The single acceptance gate for every success path (record 017 #9/#10).
fn accept(
    terms: &[(f64, f64, f64)],
    candidate: f64,
    (lower, min_rate): (f64, f64),
    (upper, max_rate): (f64, f64),
) -> crate::Result<f64> {
    let unrepresentable = || {
        crate::Error::Validation(
            "IRR root is not representable as a sufficiently accurate finite rate greater than -1"
                .into(),
        )
    };
    if !candidate.is_finite() || candidate < lower || candidate > upper {
        return Err(unrepresentable());
    }
    // Endpoints use their finite rates directly; `expm1(log1p(MAX))` can
    // overflow without proving MAX unrepresentable.
    let rate = if candidate.to_bits() == lower.to_bits() {
        min_rate
    } else if candidate.to_bits() == upper.to_bits() {
        max_rate
    } else {
        candidate.exp_m1()
    };
    if !rate.is_finite() || rate <= -1.0 {
        return Err(unrepresentable());
    }
    let residual = scaled_residual(terms, rate.ln_1p())?;
    if !(residual.is_finite() && residual.abs() <= RESIDUAL_TOLERANCE) {
        return Err(unrepresentable());
    }
    // Normalize a signed zero.
    Ok(if rate == 0.0 { 0.0 } else { rate })
}

/// Scaled residual `Σ sign·exp(a - m) / Σ exp(a - m)` at `log_growth`.
///
/// Entries are `(time, ln|amount|, sign)`. Non-finite intermediates and a
/// non-positive or non-finite gross sum are errors, never a fabricated sign.
fn scaled_residual(terms: &[(f64, f64, f64)], log_growth: f64) -> crate::Result<f64> {
    let invalid =
        || crate::Error::Validation("IRR residual evaluation is not finite at this rate".into());
    let mut max_log = f64::NEG_INFINITY;
    for &(time, log_amount, _) in terms {
        let a = log_amount - time * log_growth;
        if !a.is_finite() {
            return Err(invalid());
        }
        max_log = max_log.max(a);
    }
    let mut net = NeumaierAccumulator::new();
    let mut gross = NeumaierAccumulator::new();
    for &(time, log_amount, sign) in terms {
        let scaled = (log_amount - time * log_growth - max_log).exp();
        net.add(sign * scaled);
        gross.add(scaled);
    }
    let (net, gross) = (net.total(), gross.total());
    if !(gross.is_finite() && gross > 0.0) {
        return Err(invalid());
    }
    let residual = net / gross;
    if !residual.is_finite() {
        return Err(invalid());
    }
    Ok(residual)
}

/// Count the number of sign changes in a numeric sequence.
///
/// Zero values are skipped.
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

    fn date(y: i32, m: Month, d: u8) -> Date {
        create_date(y, m, d).expect("valid date")
    }

    fn periodic_npv(amounts: &[f64], rate: f64) -> f64 {
        amounts
            .iter()
            .enumerate()
            .map(|(i, &a)| a / (1.0 + rate).powi(i as i32))
            .sum()
    }

    #[test]
    fn count_sign_changes_skips_zeros() {
        assert_eq!(count_sign_changes([1.0, 0.0, -1.0]), 1);
        assert_eq!(count_sign_changes([1.0, -1.0, 0.0, 1.0]), 2);
        assert_eq!(count_sign_changes([0.0, 0.0, 1.0]), 0);
    }

    // Record 017 boundary rows.

    #[test]
    fn periodic_investment_and_borrowing_directions() {
        assert!((irr(&[-100.0, 110.0]).expect("investment") - 0.1).abs() < 1e-10);
        assert!((irr(&[100.0, -110.0]).expect("borrowing") - 0.1).abs() < 1e-10);
    }

    #[test]
    fn periodic_zero_keeps_its_period_and_leading_zero_shifts_origin() {
        assert!((irr(&[-100.0, 0.0, 121.0]).expect("spaced") - 0.1).abs() < 1e-10);
        let shifted = irr(&[0.0, -100.0, 110.0]).expect("shifted");
        let plain = irr(&[-100.0, 110.0]).expect("plain");
        assert!((shifted - plain).abs() < 1e-14);
        for lag in [1, 5, 20] {
            let mut delayed = vec![0.0; lag];
            delayed.extend_from_slice(&[-100.0, -30.0, 150.0]);
            let expected = irr(&[-100.0, -30.0, 150.0]).expect("root");
            assert!((irr(&delayed).expect("delayed") - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn periodic_rates_satisfy_the_npv_equation() {
        for amounts in [
            &[-1000.0, 300.0, 300.0, 300.0, 300.0][..],
            &[-100.0, 60.0, 60.0][..],
        ] {
            let rate = irr(amounts).expect("unique return");
            assert!(periodic_npv(amounts, rate).abs() / 1000.0 < 1e-9);
        }
    }

    #[test]
    fn xirr_annual_and_leap_year_clock() {
        let r = xirr(&[
            (date(2025, Month::January, 1), -100.0),
            (date(2026, Month::January, 1), 110.0),
        ])
        .expect("annual");
        assert!((r - 0.1).abs() < 1e-10);
        let leap = xirr(&[
            (date(2024, Month::January, 1), -100.0),
            (date(2025, Month::January, 1), 110.0),
        ])
        .expect("leap year");
        assert!((leap - (1.1_f64.powf(365.0 / 366.0) - 1.0)).abs() < 1e-12);
    }

    #[test]
    fn xirr_is_order_independent_and_nets_each_date_once() {
        let d0 = date(2025, Month::January, 1);
        let d1 = date(2026, Month::January, 1);
        let amounts = [-1e16, -1.0, 1e16];
        let mut results = Vec::new();
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
            results.push(xirr(&flows).expect("compensated net outflow"));
        }
        assert!((results[0] - 0.1).abs() < 1e-8);
        assert!(results.iter().all(|r| r.to_bits() == results[0].to_bits()));
    }

    #[test]
    fn xirr_zero_and_cancelled_dates_do_not_set_the_origin() {
        let early = date(2025, Month::January, 1);
        let d0 = date(2030, Month::January, 1);
        let d1 = date(2031, Month::January, 1);
        let expected = xirr(&[(d0, -100.0), (d1, 110.0)]).expect("root");
        for prefix in [vec![(early, 0.0)], vec![(early, -1.0), (early, 1.0)]] {
            let mut flows = prefix;
            flows.extend([(d0, -100.0), (d1, 110.0)]);
            assert_eq!(xirr(&flows).expect("root").to_bits(), expected.to_bits());
        }
    }

    #[test]
    fn underdetermined_and_one_sign_streams_are_errors() {
        let d = date(2025, Month::January, 1);
        let later = date(2026, Month::January, 1);
        assert!(irr(&[]).is_err());
        assert!(irr(&[0.0, 0.0]).is_err());
        assert!(irr(&[-100.0]).is_err());
        assert!(irr(&[100.0, 200.0, 300.0]).is_err());
        assert!(xirr(&[]).is_err());
        assert!(
            xirr(&[(d, -100.0), (d, 110.0)]).is_err(),
            "one surviving date"
        );
        assert!(
            xirr(&[(d, -100.0), (later, 100.0), (later, -100.0)]).is_err(),
            "cancelled"
        );
        // An exact wipeout is not a finite rate above -1.
        assert!(irr(&[-100.0, 0.0]).is_err());
    }

    #[test]
    fn more_than_one_sign_change_is_rejected() {
        // Roots -0.52, -0.43, 0.48, 0.63 and 4.25: no root selection.
        let amounts = [-1.0, 9.41, -27.7915, 34.629291, -18.42559164, 3.46517136];
        let error = irr(&amounts).expect_err("several sign changes");
        assert!(error.to_string().contains("exactly one sign change"));
        // A repeated root also stays outside the supported contract.
        assert!(irr(&[-1.0, 2.2, -1.21]).is_err());
        assert!(irr(&[-100.0, 100.0, -100.0]).is_err());
    }

    #[test]
    fn non_finite_amounts_are_rejected_before_shortcuts() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(irr(&[-100.0, bad, 110.0]).is_err());
            assert!(irr(&[0.0, bad]).is_err());
            let d = date(2025, Month::January, 1);
            assert!(xirr(&[(d, bad)]).is_err());
        }
        // A finite exact net can still overflow under summation: rejected.
        let d0 = date(2025, Month::January, 1);
        let d1 = date(2026, Month::January, 1);
        assert!(xirr(&[(d0, f64::MAX), (d0, f64::MAX), (d1, -1.0)]).is_err());
    }

    #[test]
    fn distressed_and_large_finite_returns_converge_without_a_guess() {
        let start = date(2025, Month::January, 1);
        let end = date(2026, Month::January, 1);
        for payoff in [0.2, 0.01, 0.0001, 1e10, 1e100] {
            let growth = payoff / 100.0;
            let rate = irr(&[-100.0, payoff]).expect("unique finite return");
            assert!(((1.0 + rate) / growth - 1.0).abs() < 1e-8);
            let dated = xirr(&[(start, -100.0), (end, payoff)]).expect("dated return");
            assert!(((1.0 + dated) / growth - 1.0).abs() < 1e-8);
        }
        // Growth factors whose rates round to -1 or overflow f64 are errors.
        assert!(irr(&[-1.0, 1e-100]).is_err());
        assert!(irr(&[-1e-100, 1e300]).is_err());
    }

    #[test]
    fn scale_invariant_for_large_notionals() {
        let d0 = date(2024, Month::January, 1);
        let d1 = date(2024, Month::July, 1);
        let d2 = date(2025, Month::January, 1);
        let unit = xirr(&[(d0, -1.0), (d1, 0.03), (d2, 1.05)]).expect("unit");
        let large = xirr(&[(d0, -1.0e9), (d1, 0.03e9), (d2, 1.05e9)]).expect("large");
        assert!((unit - large).abs() < 1e-10);
        let high = xirr(&[(d0, -1.0), (d2, 12.0)]).expect("high return");
        assert!(high > 10.0);
    }

    #[test]
    fn residual_matches_the_one_period_closed_form_and_fails_explicitly() {
        let terms = [(0.0, 100.0_f64.ln(), -1.0), (1.0, 110.0_f64.ln(), 1.0)];
        // For [-100, 110], rho = (0.1 - r) / (2.1 + r).
        for rate in [-0.99_f64, -0.5, 0.0, 0.1, 1.0, 10.0] {
            let residual = scaled_residual(&terms, rate.ln_1p()).expect("finite");
            assert!((residual - (0.1 - rate) / (2.1 + rate)).abs() < 1e-14);
        }
        for rate in [(-1.0_f64).next_up(), f64::MAX] {
            assert!(scaled_residual(&terms, rate.ln_1p()).is_ok());
        }
        assert!(scaled_residual(&terms, f64::NAN).is_err());
        assert!(scaled_residual(&terms, f64::INFINITY).is_err());
    }

    #[test]
    fn returned_zero_rate_is_unsigned() {
        let rate = irr(&[-100.0, 100.0]).expect("zero return");
        assert!(rate.abs() < 1e-12);
        // The gate normalizes a signed-zero candidate.
        let terms = [(0.0, 100.0_f64.ln(), -1.0), (1.0, 100.0_f64.ln(), 1.0)];
        let lower = (-1.0_f64).next_up();
        let accepted = accept(
            &terms,
            -0.0,
            (lower.ln_1p(), lower),
            (f64::MAX.ln_1p(), f64::MAX),
        )
        .expect("exact zero return");
        assert_eq!(accepted.to_bits(), 0.0_f64.to_bits());
    }
}
