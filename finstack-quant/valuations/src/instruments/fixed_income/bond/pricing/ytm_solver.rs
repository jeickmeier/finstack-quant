//! Enhanced YTM solver.
//!
//! Provides a robust yield-to-maturity solver using Brent's method with
//! intelligent initial guesses.

use finstack_quant_core::dates::Tenor;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use std::cell::RefCell;

use super::quote_conversions::{price_from_ytm_compounded_params, YieldCompounding};

/// Specification for yield-to-maturity calculations
#[derive(Debug, Clone, Copy)]
pub struct YtmPricingSpec {
    /// Day count convention for accrual calculations
    pub day_count: DayCount,
    /// Bond notional amount
    pub notional: Money,
    /// Annual coupon rate (as decimal, e.g., 0.05 for 5%)
    pub coupon_rate: f64,
    /// Yield compounding convention
    pub compounding: YieldCompounding,
    /// Coupon payment frequency
    pub frequency: Tenor,
}

/// YTM solver tolerance on the yield axis.
///
/// `1e-12` keeps the price residual near `D_mod × notional × 1e-12` (about
/// $0.00001 per $1M face at duration 7), tight enough for deterministic,
/// benchmark-grade yields.
const YTM_TOLERANCE: f64 = 1e-12;

/// Maximum Brent iterations before the YTM solve reports non-convergence.
///
/// Brent typically converges in 5-15 iterations; the cap stops pathological
/// inputs (negative flows, multiple IRRs) from looping.
const YTM_MAX_ITERATIONS: usize = 50;

/// Solve for yield-to-maturity with an explicit Brent tolerance and
/// iteration cap.
///
/// Seeds Brent with a current-yield plus pull-to-par guess; one-cashflow
/// conventions with a closed-form inverse skip the root solve.
///
/// # Errors
///
/// Returns `Err` when the target price is non-positive, the cashflows are
/// empty, a pricing evaluation fails, or Brent does not converge.
fn solve_ytm_with(
    cashflows: &[(Date, Money)],
    as_of: Date,
    target_price: Money,
    spec: YtmPricingSpec,
    tolerance: f64,
    max_iterations: usize,
) -> Result<f64> {
    let target = target_price.amount();
    if target <= 0.0 {
        return Err(finstack_quant_core::Error::from(
            finstack_quant_core::InputError::Invalid,
        ));
    }
    if cashflows.is_empty() {
        return Err(finstack_quant_core::Error::from(
            finstack_quant_core::InputError::TooFewPoints,
        ));
    }

    // Most one-cashflow conventions have a closed-form inverse.
    // TreasuryActual can combine a simple fractional period with periodic
    // compounding, so it must use the shared root solve below.
    if cashflows.len() == 1 && !matches!(spec.compounding, YieldCompounding::TreasuryActual) {
        let (maturity_date, face_value) = &cashflows[0];
        let years = spec.day_count.year_fraction(
            as_of,
            *maturity_date,
            finstack_quant_core::dates::DayCountContext {
                frequency: Some(spec.frequency),
                coupon_period: super::quote_conversions::icma_reference_period(
                    spec.day_count,
                    spec.frequency,
                    cashflows.iter().map(|(d, _)| *d),
                    as_of,
                ),
                ..Default::default()
            },
        )?;
        let fv = face_value.amount();
        if years > 0.0 && fv > 0.0 && target > 0.0 {
            let ratio = fv / target;
            let ytm = match spec.compounding {
                YieldCompounding::Simple | YieldCompounding::Moosmuller => (ratio - 1.0) / years,
                YieldCompounding::Annual => ratio.powf(1.0 / years) - 1.0,
                YieldCompounding::Continuous => ratio.ln() / years,
                YieldCompounding::Street => {
                    let m = super::quote_conversions::periods_per_year(spec.frequency)?.max(1.0);
                    m * (ratio.powf(1.0 / (m * years)) - 1.0)
                }
                YieldCompounding::TreasuryActual => {
                    return Err(finstack_quant_core::Error::Validation(
                        "TreasuryActual one-cashflow yield requires numerical inversion"
                            .to_string(),
                    ));
                }
                YieldCompounding::Periodic(periods) => {
                    let m = (periods as f64).max(1.0);
                    m * (ratio.powf(1.0 / (m * years)) - 1.0)
                }
            };
            return Ok(ytm);
        }
    }

    let initial_guess = initial_guess(cashflows, as_of, target_price, &spec)?;

    // Capture first pricing error to avoid masking errors with 0.0.
    // This pattern ensures the solver doesn't converge to fake roots when
    // underlying pricing calculations fail (e.g., invalid dates, overflow).
    let pricing_error: RefCell<Option<finstack_quant_core::Error>> = RefCell::new(None);

    let price_fn = |y: f64| -> f64 {
        match price_from_ytm_compounded_params(
            spec.day_count,
            spec.frequency,
            cashflows,
            as_of,
            y,
            spec.compounding,
        ) {
            Ok(price) => price - target,
            Err(e) => {
                // Capture the first error for later reporting
                let mut slot = pricing_error.borrow_mut();
                if slot.is_none() {
                    *slot = Some(e);
                }
                drop(slot);
                // Return a large residual whose sign reflects price-vs-target,
                // NOT the sign of `y`.
                //
                // The bond price is monotonically decreasing in yield, and
                // `price_from_ytm_compounded_params` can only fail for a
                // *yield-dependent* reason in the deep-negative-yield regime
                // (a non-positive compounding base `1 + y/m <= 0`). There the
                // true price diverges to `+infinity`, so `price - target` is
                // unambiguously large and positive. Returning `+1e12` keeps
                // the objective consistent with the monotone price/yield
                // curve; the previous `sign(y)`-based residual could flip
                // sign and manufacture a spurious bracket for Brent.
                1e12
            }
        }
    };

    // Always use BrentSolver for robustness. `BrentSolver::solve` returns
    // an explicit `SolverConvergenceFailed` error (rather than a silent
    // last iterate) when the iteration cap is hit, no sign-changing
    // bracket is found, or the objective is non-finite — so non-convergent
    // YTM solves surface as `Err`.
    let solver = BrentSolver::new()
        .tolerance(tolerance)
        .max_iterations(max_iterations);
    let ytm = solver.solve(price_fn, initial_guess)?;

    // If any pricing error occurred during objective evaluation, surface it
    // instead of returning a potentially meaningless yield.
    if let Some(err) = pricing_error.into_inner() {
        return Err(err);
    }

    Ok(ytm)
}

/// Current yield plus half the annualised pull-to-par, clamped to
/// `[-1, 10]`; only Brent's starting point, not a bound.
fn initial_guess(
    cashflows: &[(Date, Money)],
    as_of: Date,
    target_price: Money,
    spec: &YtmPricingSpec,
) -> Result<f64> {
    let current_yield = spec.coupon_rate * spec.notional.amount() / target_price.amount();
    let maturity = cashflows
        .last()
        .map(|(date, _)| *date)
        .ok_or(finstack_quant_core::InputError::TooFewPoints)?;
    let years_to_maturity = spec.day_count.year_fraction(
        as_of,
        maturity,
        finstack_quant_core::dates::DayCountContext {
            frequency: Some(spec.frequency),
            coupon_period: super::quote_conversions::icma_reference_period(
                spec.day_count,
                spec.frequency,
                cashflows.iter().map(|(d, _)| *d),
                as_of,
            ),
            ..Default::default()
        },
    )?;
    if years_to_maturity <= 0.0 {
        return Ok(current_yield);
    }
    let price_pct = target_price.amount() / spec.notional.amount();
    let pull_to_par = (1.0 / price_pct - 1.0) / years_to_maturity;
    let initial_guess = current_yield + 0.5 * pull_to_par;
    // Clamp to [-1.0, 10.0] to seed Brent for distressed debt with YTMs up to
    // ~1000% while still providing reasonable bounds. The clamp only affects
    // the initial guess; Brent will continue searching outside this band.
    Ok(initial_guess.clamp(-1.0, 10.0))
}

/// Solve for the yield that reprices `cashflows` to `target_price`.
///
/// Uses Brent's method with tolerance `1e-12` on the yield axis and at most
/// 50 iterations, seeded by a current-yield plus pull-to-par guess.
///
/// # Arguments
///
/// * `cashflows` - Dated contractual coupon and principal payments in the
///   same currency as `spec.notional`; payments on or before `as_of` do not
///   contribute to the solved price.
/// * `as_of` - Valuation date from which remaining cashflow times are measured
///   with `spec.day_count`.
/// * `target_price` - Dirty present value in the bond currency that the yield
///   solve must reproduce.
/// * `spec` - Coupon, notional, day-count, payment-frequency, and compounding
///   conventions used to discount the supplied cashflows.
///
/// # Returns
///
/// Yield to maturity as decimal.
pub fn solve_ytm(
    cashflows: &[(Date, Money)],
    as_of: Date,
    target_price: Money,
    spec: YtmPricingSpec,
) -> Result<f64> {
    solve_ytm_with(
        cashflows,
        as_of,
        target_price,
        spec,
        YTM_TOLERANCE,
        YTM_MAX_ITERATIONS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use time::Month;

    /// ACT/ACT ICMA needs a reference coupon period for any span that is not a
    /// whole number of coupons. `initial_guess` measures settlement to
    /// maturity, which is irregular unless settlement lands exactly on a coupon
    /// date — so the seed calculation used to hard-error and take the whole YTM
    /// solve down with it, even though the seed only picks Brent's starting point.
    ///
    /// Fixed by inferring the reference coupon period from the cashflow
    /// schedule (`icma_reference_period`) and threading it through the
    /// day-count context in both `price_from_ytm_compounded_params` and the
    /// solver's seed/zero-coupon paths, so ISMA spans resolve on the
    /// quasi-coupon grid.
    #[test]
    fn ytm_solves_for_an_act_act_icma_bond_settling_off_a_coupon_date() {
        // Semi-annual Jan/Jul coupons; settlement deliberately mid-period.
        let as_of = Date::from_calendar_date(2025, Month::March, 17).expect("valid date");
        let notional = Money::from((1000_i64, Currency::USD));
        let coupon_rate = 0.0425;
        let mut cashflows = vec![];
        for (year, month) in [
            (2025, Month::July),
            (2026, Month::January),
            (2026, Month::July),
            (2027, Month::January),
        ] {
            cashflows.push((
                Date::from_calendar_date(year, month, 15).expect("valid date"),
                Money::new(21.25, Currency::USD).expect("valid money fixture"),
            ));
        }
        cashflows.push((
            Date::from_calendar_date(2027, Month::July, 15).expect("valid date"),
            Money::new(1021.25, Currency::USD).expect("valid money fixture"),
        ));

        let ytm = solve_ytm(
            &cashflows,
            as_of,
            notional,
            YtmPricingSpec {
                day_count: DayCount::ActActIsma,
                notional,
                coupon_rate,
                compounding: YieldCompounding::Street,
                frequency: Tenor::semi_annual(),
            },
        )
        .expect("ACT/ACT ICMA must not fail merely because settlement is mid-coupon");

        // Priced at par, so the solved yield sits near the coupon.
        assert!(
            (ytm - coupon_rate).abs() < 0.02,
            "expected a yield near the {coupon_rate} coupon, got {ytm}"
        );
    }
    #[test]
    fn test_ytm_solver_par_bond() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let _maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
        let notional = Money::from((1000_i64, Currency::USD));
        let coupon_rate = 0.05;
        let mut cashflows = vec![];
        for year in 1..=5 {
            let date =
                Date::from_calendar_date(2025 + year, Month::January, 1).expect("valid date");
            if year < 5 {
                cashflows.push((date, Money::from((50_i64, Currency::USD))));
            } else {
                cashflows.push((date, Money::from((1050_i64, Currency::USD))));
            }
        }
        let ytm = solve_ytm(
            &cashflows,
            as_of,
            notional,
            YtmPricingSpec {
                day_count: DayCount::Act365F,
                notional,
                coupon_rate,
                compounding: YieldCompounding::Street,
                frequency: Tenor::annual(),
            },
        )
        .expect("should succeed");
        assert!((ytm - coupon_rate).abs() < 1e-4);
    }

    /// Item 6 regression: when `price_from_ytm_compounded_params` fails (only
    /// possible in the deep-negative-yield regime, where the price diverges to
    /// `+infinity`), the solver must not "converge" to a fake yield off the
    /// back of a `sign(y)`-flipped residual. A target price unreachable by any
    /// valid yield must surface as an explicit `Err`.
    #[test]
    fn ytm_unreachable_target_does_not_return_fake_yield() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let mut cashflows = vec![];
        for year in 1..=5 {
            let date =
                Date::from_calendar_date(2025 + year, Month::January, 1).expect("valid date");
            let amt = if year < 5 { 50.0 } else { 1050.0 };
            cashflows.push((
                date,
                Money::new(amt, Currency::USD).expect("valid money fixture"),
            ));
        }
        let notional = Money::from((1000_i64, Currency::USD));

        // A target far above the largest attainable price: the sum of the
        // undiscounted cashflows is 1250, so a $5,000,000 target can only be
        // "solved" by a non-physical deeply negative yield.
        let result = solve_ytm(
            &cashflows,
            as_of,
            Money::from((5_000_000_i64, Currency::USD)),
            YtmPricingSpec {
                day_count: DayCount::Act365F,
                notional,
                coupon_rate: 0.05,
                compounding: YieldCompounding::Street,
                frequency: Tenor::annual(),
            },
        );

        assert!(
            result.is_err(),
            "an unreachable target price must surface as Err, not a fake yield \
             from a sign-flipped objective residual"
        );
    }

    /// Item 12 regression: a non-convergent YTM solve must return an explicit
    /// error, never a silent last iterate. The configured `max_iterations` is
    /// passed to Brent, so capping it at 1 with a tight tolerance forces Brent to bail
    /// out with `SolverConvergenceFailed` instead of returning a fake yield.
    #[test]
    fn ytm_non_convergence_returns_error() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let mut cashflows = vec![];
        for year in 1..=5 {
            let date =
                Date::from_calendar_date(2025 + year, Month::January, 1).expect("valid date");
            let amt = if year < 5 { 50.0 } else { 1050.0 };
            cashflows.push((
                date,
                Money::new(amt, Currency::USD).expect("valid money fixture"),
            ));
        }
        let notional = Money::from((1000_i64, Currency::USD));

        // One iteration is far too few for Brent to reach a 1e-15 residual.
        let result = solve_ytm_with(
            &cashflows,
            as_of,
            Money::from((950_i64, Currency::USD)),
            YtmPricingSpec {
                day_count: DayCount::Act365F,
                notional,
                coupon_rate: 0.05,
                compounding: YieldCompounding::Street,
                frequency: Tenor::annual(),
            },
            1e-15,
            1,
        );

        assert!(
            result.is_err(),
            "non-convergent YTM solve must return Err, not a silent last iterate"
        );
        match result {
            Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::SolverConvergenceFailed { .. },
            )) => {}
            other => panic!("expected SolverConvergenceFailed, got {other:?}"),
        }
    }

    #[test]
    fn test_zcb_ytm_honors_street_compounding() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2027, Month::January, 1).expect("valid date");
        let cashflows = vec![(maturity, Money::from((1000_i64, Currency::USD)))];
        let target_price = Money::from((900_i64, Currency::USD));

        let ytm = solve_ytm(
            &cashflows,
            as_of,
            target_price,
            YtmPricingSpec {
                day_count: DayCount::Act365F,
                notional: Money::from((1000_i64, Currency::USD)),
                coupon_rate: 0.0,
                compounding: YieldCompounding::Street,
                frequency: Tenor::semi_annual(),
            },
        )
        .expect("should solve");

        let m = 2.0_f64;
        let years = 2.0_f64;
        let expected = m * ((1000.0_f64 / 900.0_f64).powf(1.0 / (m * years)) - 1.0);
        assert!((ytm - expected).abs() < 1e-12);
    }

    #[test]
    fn test_zcb_ytm_honors_moosmuller_simple_first_period() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2027, Month::January, 1).expect("valid date");
        let cashflows = vec![(maturity, Money::from((1000_i64, Currency::USD)))];
        let target_price = Money::from((900_i64, Currency::USD));

        let ytm = solve_ytm(
            &cashflows,
            as_of,
            target_price,
            YtmPricingSpec {
                day_count: DayCount::Act365F,
                notional: Money::from((1000_i64, Currency::USD)),
                coupon_rate: 0.0,
                compounding: YieldCompounding::Moosmuller,
                frequency: Tenor::annual(),
            },
        )
        .expect("should solve");

        let years = 2.0_f64;
        let expected = (1000.0_f64 / 900.0_f64 - 1.0) / years;
        assert!((ytm - expected).abs() < 1e-12);
    }
}
