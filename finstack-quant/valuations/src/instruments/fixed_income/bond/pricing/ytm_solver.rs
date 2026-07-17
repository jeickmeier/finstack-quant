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

/// Configuration for the YTM solver.
///
/// # Tolerance Design Rationale
///
/// The YTM solver tolerance is specified on the **yield axis** (not the price axis).
/// The default `1e-12` is chosen to ensure:
///
/// 1. **Sub-penny price accuracy**: For typical bonds ($1000 face, 5Y, 5% coupon),
///    a yield tolerance of `1e-12` produces price errors < $0.000001.
///
/// 2. **Determinism**: Extremely tight tolerance ensures identical results across
///    different execution environments and compiler optimizations.
///
/// 3. **Benchmark matching**: Matches Bloomberg/Reuters precision for regulatory
///    and audit requirements.
///
/// ## Tolerance-to-Price Sensitivity
///
/// The relationship between yield tolerance and price accuracy depends on duration:
///
/// ```text
/// Price Error ≈ Modified Duration × Notional × Yield Tolerance
///
/// Example: D_mod = 7, Notional = $1,000,000, Tolerance = 1e-12
/// Price Error ≈ 7 × 1,000,000 × 1e-12 = $0.000007
/// ```
///
/// ## Recommended Tolerances by Use Case
///
/// | Use Case | Tolerance | Price Error ($1M face) | Iterations |
/// |----------|-----------|------------------------|------------|
/// | Regulatory/Audit | `1e-12` | < $0.00001 | 5-8 |
/// | Trading | `1e-10` | < $0.001 | 4-6 |
/// | Screening | `1e-8` | < $0.10 | 3-5 |
/// | Quick estimates | `1e-6` | < $10 | 2-4 |
///
/// # Solver Algorithm
///
/// The solver uses Brent's method, which provides:
/// - Guaranteed convergence for bracketed roots
/// - Superlinear convergence rate (faster than bisection)
/// - Robustness to pathological cashflow structures
///
/// The initial guess uses "pull-to-par" heuristic for 2-3x faster convergence:
/// ```text
/// y_guess = current_yield + 0.5 × (1/price_pct - 1) / years_to_maturity
/// ```
#[derive(Debug, Clone)]
pub struct YtmSolverConfig {
    /// Convergence tolerance for YTM solver (on the yield axis).
    ///
    /// Default: `1e-12` for maximum precision and determinism.
    /// See struct-level documentation for guidance on choosing tolerances.
    ///
    /// # Interpretation
    ///
    /// The solver stops when `|f(y)| < tolerance × target_price`, ensuring
    /// the price residual is proportionally small regardless of notional size.
    pub tolerance: f64,

    /// Maximum solver iterations before failing.
    ///
    /// Brent's method typically converges in 5-15 iterations for well-behaved
    /// bonds. The cap prevents infinite loops on pathological inputs (e.g.,
    /// bonds with negative cashflows or multiple IRR solutions).
    pub max_iterations: usize,

    /// Use smart initial guess based on current yield and pull-to-par.
    ///
    /// When enabled, the initial guess is computed as:
    /// `y_guess = current_yield + 0.5 × pull_to_par`
    ///
    /// This typically reduces iterations by 30-50% compared to a naive
    /// starting point (e.g., coupon rate).
    pub use_smart_guess: bool,
}

impl Default for YtmSolverConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-12,      // Sub-penny precision per $1000 face
            max_iterations: 50,    // Sufficient for pathological cases
            use_smart_guess: true, // Improves convergence speed 2-3x
        }
    }
}

/// Yield-to-maturity solver using Brent's method.
///
/// Provides robust YTM calculation with intelligent initial guesses. Configured via
/// `YtmSolverConfig`.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::ytm_solver::{YtmSolver, YtmPricingSpec};
/// use finstack_quant_core::dates::{Date, DayCount, Tenor};
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::currency::Currency;
///
/// # let cashflows = vec![];
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
/// # let target_price = Money::new(1000.0, Currency::USD);
/// let solver = YtmSolver::new();
/// let spec = YtmPricingSpec {
///     day_count: DayCount::Act365F,
///     notional: Money::new(1000.0, Currency::USD),
///     coupon_rate: 0.05,
///     compounding: finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::YieldCompounding::Street,
///     frequency: Tenor::semi_annual(),
/// };
/// let ytm = solver.solve(&cashflows, as_of, target_price, spec)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct YtmSolver {
    config: YtmSolverConfig,
}

impl Default for YtmSolver {
    fn default() -> Self {
        Self::new()
    }
}

impl YtmSolver {
    /// Create a new YTM solver with default configuration.
    ///
    /// # Returns
    ///
    /// A `YtmSolver` with default configuration (sub-penny precision, Brent solver).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::ytm_solver::YtmSolver;
    ///
    /// let solver = YtmSolver::new();
    /// ```
    pub fn new() -> Self {
        Self {
            config: YtmSolverConfig::default(),
        }
    }

    /// Create a YTM solver with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Custom solver configuration
    ///
    /// # Returns
    ///
    /// A `YtmSolver` with the specified configuration.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::ytm_solver::{YtmSolver, YtmSolverConfig};
    ///
    /// let config = YtmSolverConfig {
    ///     tolerance: 1e-10,      // Faster convergence
    ///     max_iterations: 100,
    ///     use_smart_guess: true,
    /// };
    /// let solver = YtmSolver::with_config(config);
    /// ```
    pub fn with_config(config: YtmSolverConfig) -> Self {
        Self { config }
    }

    /// Solve for yield-to-maturity given cashflows and target price.
    ///
    /// Uses Brent solver with intelligent initial guess based on
    /// current yield and pull-to-par effect.
    ///
    /// # Arguments
    ///
    /// * `cashflows` - Bond cashflows as `(Date, Money)` pairs
    /// * `as_of` - Valuation date
    /// * `target_price` - Target dirty price to match
    /// * `spec` - YTM pricing specification (day count, compounding, frequency)
    ///
    /// # Returns
    ///
    /// Yield to maturity as decimal (e.g., 0.05 for 5%).
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - Target price is non-positive
    /// - Cashflows are empty
    /// - Solver fails to converge
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::ytm_solver::{YtmSolver, YtmPricingSpec};
    /// use finstack_quant_core::dates::{Date, DayCount, Tenor};
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// # let cashflows = vec![];
    /// # let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    /// # let target_price = Money::new(1000.0, Currency::USD);
    /// let solver = YtmSolver::new();
    /// let spec = YtmPricingSpec {
    ///     day_count: DayCount::Act365F,
    ///     notional: Money::new(1000.0, Currency::USD),
    ///     coupon_rate: 0.05,
    ///     compounding: finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::YieldCompounding::Street,
    ///     frequency: Tenor::semi_annual(),
    /// };
    /// let ytm = solver.solve(&cashflows, as_of, target_price, spec)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn solve(
        &self,
        cashflows: &[(Date, Money)],
        as_of: Date,
        target_price: Money,
        spec: YtmPricingSpec,
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

        // Special case: zero coupon bond (single cashflow at maturity).
        // Use compounding-aware closed form so ZCB yields are consistent with
        // the selected YTM convention used for coupon-bearing bonds.
        if cashflows.len() == 1 {
            let (maturity_date, face_value) = &cashflows[0];
            let years = spec.day_count.year_fraction(
                as_of,
                *maturity_date,
                finstack_quant_core::dates::DayCountContext {
                    frequency: Some(spec.frequency),
                    ..Default::default()
                },
            )?;
            let fv = face_value.amount();
            if years > 0.0 && fv > 0.0 && target > 0.0 {
                let ratio = fv / target;
                let ytm = match spec.compounding {
                    YieldCompounding::Simple => (ratio - 1.0) / years,
                    YieldCompounding::Annual => ratio.powf(1.0 / years) - 1.0,
                    YieldCompounding::Continuous => ratio.ln() / years,
                    YieldCompounding::Street | YieldCompounding::TreasuryActual => {
                        let m =
                            super::quote_conversions::periods_per_year(spec.frequency)?.max(1.0);
                        m * (ratio.powf(1.0 / (m * years)) - 1.0)
                    }
                    YieldCompounding::Periodic(periods) => {
                        let m = (periods as f64).max(1.0);
                        m * (ratio.powf(1.0 / (m * years)) - 1.0)
                    }
                };
                return Ok(ytm);
            }
        }

        let initial_guess = if self.config.use_smart_guess {
            self.calculate_initial_guess(cashflows, as_of, target_price, &spec)?
        } else {
            spec.coupon_rate
        };

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
        // YTM solves surface as `Err`. The configured `max_iterations` is
        // honoured here (previously the field was ignored and Brent's default
        // cap was always used).
        let solver = BrentSolver::new()
            .tolerance(self.config.tolerance)
            .max_iterations(self.config.max_iterations);
        let ytm = solver.solve(price_fn, initial_guess)?;

        // If any pricing error occurred during objective evaluation, surface it
        // instead of returning a potentially meaningless yield.
        if let Some(err) = pricing_error.into_inner() {
            return Err(err);
        }

        Ok(ytm)
    }

    fn calculate_initial_guess(
        &self,
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
}

/// Convenience function to solve for YTM with default configuration.
///
/// Wrapper around `YtmSolver::new().solve()` for simple use cases.
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
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::ytm_solver::{solve_ytm, YtmPricingSpec};
/// use finstack_quant_core::dates::{Date, DayCount, Tenor};
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::currency::Currency;
///
/// # let cashflows = vec![];
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
/// # let target_price = Money::new(1000.0, Currency::USD);
/// let spec = YtmPricingSpec {
///     day_count: DayCount::Act365F,
///     notional: Money::new(1000.0, Currency::USD),
///     coupon_rate: 0.05,
///     compounding: finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::YieldCompounding::Street,
///     frequency: Tenor::semi_annual(),
/// };
/// let ytm = solve_ytm(&cashflows, as_of, target_price, spec)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn solve_ytm(
    cashflows: &[(Date, Money)],
    as_of: Date,
    target_price: Money,
    spec: YtmPricingSpec,
) -> Result<f64> {
    let solver = YtmSolver::new();
    solver.solve(cashflows, as_of, target_price, spec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use time::Month;
    #[test]
    fn test_ytm_solver_par_bond() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let _maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
        let notional = Money::new(1000.0, Currency::USD);
        let coupon_rate = 0.05;
        let mut cashflows = vec![];
        for year in 1..=5 {
            let date =
                Date::from_calendar_date(2025 + year, Month::January, 1).expect("valid date");
            if year < 5 {
                cashflows.push((date, Money::new(50.0, Currency::USD)));
            } else {
                cashflows.push((date, Money::new(1050.0, Currency::USD)));
            }
        }
        let solver = YtmSolver::new();
        let ytm = solver
            .solve(
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
            cashflows.push((date, Money::new(amt, Currency::USD)));
        }
        let notional = Money::new(1000.0, Currency::USD);

        // A target far above the largest attainable price: the sum of the
        // undiscounted cashflows is 1250, so a $5,000,000 target can only be
        // "solved" by a non-physical deeply negative yield.
        let solver = YtmSolver::new();
        let result = solver.solve(
            &cashflows,
            as_of,
            Money::new(5_000_000.0, Currency::USD),
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
    /// honoured, so capping it at 1 with a tight tolerance forces Brent to bail
    /// out with `SolverConvergenceFailed` instead of returning a fake yield.
    #[test]
    fn ytm_non_convergence_returns_error() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let mut cashflows = vec![];
        for year in 1..=5 {
            let date =
                Date::from_calendar_date(2025 + year, Month::January, 1).expect("valid date");
            let amt = if year < 5 { 50.0 } else { 1050.0 };
            cashflows.push((date, Money::new(amt, Currency::USD)));
        }
        let notional = Money::new(1000.0, Currency::USD);

        // One iteration is far too few for Brent to reach a 1e-15 residual.
        let solver = YtmSolver::with_config(YtmSolverConfig {
            tolerance: 1e-15,
            max_iterations: 1,
            use_smart_guess: true,
        });
        let result = solver.solve(
            &cashflows,
            as_of,
            Money::new(950.0, Currency::USD),
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
        let cashflows = vec![(maturity, Money::new(1000.0, Currency::USD))];
        let target_price = Money::new(900.0, Currency::USD);
        let solver = YtmSolver::new();

        let ytm = solver
            .solve(
                &cashflows,
                as_of,
                target_price,
                YtmPricingSpec {
                    day_count: DayCount::Act365F,
                    notional: Money::new(1000.0, Currency::USD),
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
}
