//! Goal seek functionality for financial models.
//!
//! This module provides root-finding capabilities to solve for input drivers
//! that achieve specific target metric values. It wraps the core solver infrastructure
//! around the statement evaluator.
//!
//! # Examples
//!
//! ```rust
//! use finstack_statements::prelude::*;
//! use finstack_statements_analytics::analysis::goal_seek;
//! use finstack_core::dates::PeriodId;
//!
//! # fn main() -> Result<()> {
//! let mut model = ModelBuilder::new("goal_seek_test")
//!     .periods("2025Q1..Q4", None)?
//!     .value("revenue", &[
//!         (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100_000.0)),
//!     ])
//!     .forecast("revenue", ForecastSpec::growth(0.05))
//!     .compute("interest_expense", "10000.0")?
//!     .compute("ebitda", "revenue * 0.3")?
//!     .compute("interest_coverage", "ebitda / interest_expense")?
//!     .build()?;
//!
//! // Solve for Q4 revenue that achieves 2.0x interest coverage
//! let target_period = PeriodId::quarter(2025, 4);
//! let solved_revenue = goal_seek(
//!     &mut model,
//!     "interest_coverage",
//!     target_period,
//!     2.0,
//!     "revenue",
//!     target_period,
//!     true,  // Update model with solution
//!     None,
//! )?;
//!
//! println!("Revenue needed: ${:.2}", solved_revenue);
//! # Ok(())
//! # }
//! ```

use finstack_core::dates::PeriodId;
use finstack_core::math::solver::{BrentSolver, Solver};
use finstack_statements::error::{Error, Result};
use finstack_statements::evaluator::Evaluator;
use finstack_statements::types::{AmountOrScalar, FinancialModelSpec};
use std::cell::RefCell;

/// Perform goal seek on a financial model.
///
/// Solves for the driver node value that achieves a target metric value in a specific period.
/// This uses Brent's method for robust root-finding.
///
/// # Arguments
///
/// * `model` - Mutable reference to the financial model
/// * `target_node` - Node identifier for the target metric
/// * `target_period` - Period in which to evaluate the target
/// * `target_value` - Desired value for the target metric
/// * `driver_node` - Node identifier for the driver input to vary
/// * `driver_period` - Period in which to vary the driver
/// * `update_model` - If true, update the model with the solved driver value
/// * `bounds` - Optional `(lower, upper)` bracket to constrain the search
///
/// # Returns
///
/// Returns the solved driver value that achieves the target, or an error if no solution exists.
///
/// # Errors
///
/// Returns an error if:
/// - The target or driver node doesn't exist
/// - The specified periods are not in the model
/// - No solution exists within reasonable bounds
/// - The model evaluation fails
///
/// # Examples
///
/// ```ignore
/// use finstack_statements::prelude::*;
/// use finstack_statements_analytics::analysis::goal_seek;
/// use finstack_core::dates::PeriodId;
///
/// # fn main() -> Result<()> {
/// let mut model = ModelBuilder::new("example")
///     .periods("2025Q1", None)?
///     .value("revenue", &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100_000.0))])
///     .compute("profit_margin", "0.15")?
///     .compute("net_income", "revenue * profit_margin")?
///     .build()?;
///
/// // Verify model evaluates correctly first
/// let mut evaluator = Evaluator::new();
/// let results = evaluator.evaluate(&model)?;
/// let initial_net_income = results.get("net_income", &PeriodId::quarter(2025, 1)).unwrap();
/// assert!((initial_net_income - 15_000.0).abs() < 0.01);
///
/// // Solve for revenue that achieves $18,000 net income (closer to initial value for better convergence)
/// let period = PeriodId::quarter(2025, 1);
/// let solved = goal_seek(&mut model, "net_income", period, 18_000.0, "revenue", period, false, None)?;
/// // Expected: 18_000 / 0.15 = 120_000
/// assert!((solved - 120_000.0).abs() < 10.0);
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - Root-finding background: `docs/REFERENCES.md#press-numerical-recipes`
#[allow(clippy::too_many_arguments)]
pub fn goal_seek(
    model: &mut FinancialModelSpec,
    target_node: &str,
    target_period: PeriodId,
    target_value: f64,
    driver_node: &str,
    driver_period: PeriodId,
    update_model: bool,
    bounds: Option<(f64, f64)>,
) -> Result<f64> {
    // Validate that nodes exist
    if !model.has_node(target_node) {
        return Err(Error::invalid_input(format!(
            "Target node '{}' not found in model",
            target_node
        )));
    }

    if !model.has_node(driver_node) {
        return Err(Error::invalid_input(format!(
            "Driver node '{}' not found in model",
            driver_node
        )));
    }

    // Validate that periods exist
    if !model.periods.iter().any(|p| p.id == target_period) {
        return Err(Error::invalid_input(format!(
            "Target period '{}' not found in model",
            target_period
        )));
    }

    if !model.periods.iter().any(|p| p.id == driver_period) {
        return Err(Error::invalid_input(format!(
            "Driver period '{}' not found in model",
            driver_period
        )));
    }

    // Get initial guess from current driver value if available.
    //
    // # Initial Guess Strategy
    //
    // When the driver node has an existing value for the target period, we use that
    // as the initial guess. This provides a reasonable starting point that is likely
    // close to the solution.
    //
    // When no existing value is available, we use a fallback of 1.0 rather than an
    // arbitrary large number. This is because:
    // - For ratio/percentage drivers (e.g., margins), 1.0 is a reasonable neutral value
    // - For absolute drivers (e.g., revenue), the adaptive bounds logic below will
    //   expand the search range based on abs(initial_guess), so starting at 1.0
    //   gives a ±10.0 initial bracket which is then refined
    // - Using 0.0 would cause division-based bounds to collapse
    //
    // For better convergence on specific use cases, callers should either:
    // 1. Ensure the driver node has a value for the period, or
    // 2. Provide explicit bounds via the `bounds` parameter
    let initial_guess = model
        .get_node(driver_node)
        .and_then(|node| node.values.as_ref())
        .and_then(|values| values.get(&driver_period))
        .map(|v| match v {
            AmountOrScalar::Scalar(s) => *s,
            AmountOrScalar::Amount(a) => a.amount(),
        })
        .unwrap_or(1.0); // Default initial guess (see rationale above)

    let mut evaluator = Evaluator::new();
    let Ok(prepared) = evaluator.prepare(model) else {
        return Err(Error::eval("Goal seek: failed to prepare evaluator"));
    };
    let eval_cell = RefCell::new((evaluator, model.clone()));

    // Records the most recent objective-evaluation failure. The solver only
    // sees `NaN` for a failed probe; capturing the cause here lets a
    // non-convergence error explain *why* probes failed (DSL error, missing
    // target node, etc.) instead of reporting an opaque "no solution".
    let last_error: RefCell<Option<String>> = RefCell::new(None);

    let objective = |driver_value: f64| -> f64 {
        let mut borrow = eval_cell.borrow_mut();
        let (ref mut eval, ref mut temp_model) = *borrow;

        if let Some(node) = temp_model.nodes.get_mut(driver_node) {
            // Update only the driver period's entry in place. Cloning the whole
            // `values` map here would copy every period on every solver
            // iteration; Brent's method probes the objective many times, so a
            // per-iteration map clone is pure overhead.
            node.values
                .get_or_insert_with(Default::default)
                .insert(driver_period, AmountOrScalar::scalar(driver_value));
        }

        match eval.evaluate_prepared(temp_model, &prepared) {
            Ok(results) => match results.get(target_node, &target_period) {
                Some(actual_value) => actual_value - target_value,
                None => {
                    *last_error.borrow_mut() = Some(format!(
                        "target node '{target_node}' produced no value for period \
                         {target_period} at driver value {driver_value}"
                    ));
                    f64::NAN
                }
            },
            Err(e) => {
                *last_error.borrow_mut() = Some(format!(
                    "evaluation failed at driver value {driver_value}: {e}"
                ));
                f64::NAN
            }
        }
    };

    // Enrich a solver failure with the last captured objective error.
    let describe_failure = |base: String| -> Error {
        match last_error.borrow().as_ref() {
            Some(cause) => Error::eval(format!("{base} Last objective failure: {cause}")),
            None => Error::eval(base),
        }
    };

    if let Some((lower, upper)) = bounds {
        let solution = solve_with_bounds(&objective, lower, upper).map_err(|e| {
            describe_failure(format!(
                "Goal seek failed to find solution within bounds [{lower}, {upper}]: \
                 target_node='{target_node}', target_value={target_value}, \
                 driver_node='{driver_node}'. {e}"
            ))
        })?;
        return apply_solution(model, driver_node, driver_period, update_model, solution);
    }

    // Pick an initial bracket size scaled to the guess magnitude, then let
    // Brent's own bracket-expansion search widen as needed. The solver's
    // default `bracket_min`/`bracket_max` (±1e6) define the outer limits,
    // which we tighten on one side to honour the sign of the initial guess:
    // if the guess is strictly positive, the root is almost certainly also
    // positive (revenue, leverage ratio, hazard-rate intensity, etc.) and
    // the evaluator may produce NaN for non-positive inputs — same logic
    // applies symmetrically for strictly negative guesses. A zero guess
    // carries no sign information so we leave both sides at the default.
    let mut solver = BrentSolver::new();
    let abs_guess = initial_guess.abs().max(1.0);
    solver.initial_bracket_size = Some(abs_guess);
    if initial_guess > 0.0 {
        solver.bracket_min = 0.0;
    } else if initial_guess < 0.0 {
        solver.bracket_max = 0.0;
    }

    let solution = match solver.solve(&objective, initial_guess) {
        Ok(solution) => solution,
        Err(first_error) if initial_guess != 0.0 => {
            let mut widened = BrentSolver::new();
            widened.initial_bracket_size = Some(abs_guess);
            widened
                .solve(&objective, initial_guess)
                .map_err(|second_error| {
                    describe_failure(format!(
                        "Goal seek failed to find solution: target_node='{target_node}', \
                     target_value={target_value}, driver_node='{driver_node}'. \
                     Sign-constrained attempt failed with: {first_error}. \
                     Widened attempt failed with: {second_error}"
                    ))
                })?
        }
        Err(e) => {
            return Err(describe_failure(format!(
                "Goal seek failed to find solution: target_node='{target_node}', \
                 target_value={target_value}, driver_node='{driver_node}'. {e}"
            )));
        }
    };

    apply_solution(model, driver_node, driver_period, update_model, solution)
}

fn apply_solution(
    model: &mut FinancialModelSpec,
    driver_node: &str,
    driver_period: PeriodId,
    update_model: bool,
    value: f64,
) -> Result<f64> {
    if update_model {
        if let Some(node) = model.nodes.get_mut(driver_node) {
            let mut values = node.values.clone().unwrap_or_default();
            values.insert(driver_period, AmountOrScalar::scalar(value));
            node.values = Some(values);
        }
    }
    Ok(value)
}

fn solve_with_bounds<F>(f: &F, lower: f64, upper: f64) -> Result<f64>
where
    F: Fn(f64) -> f64,
{
    const TOLERANCE: f64 = 1e-9;

    if !lower.is_finite() || !upper.is_finite() {
        return Err(Error::invalid_input(
            "Goal seek bounds must be finite values",
        ));
    }

    if lower >= upper {
        return Err(Error::invalid_input(
            "Goal seek lower bound must be less than upper bound",
        ));
    }

    let flo = f(lower);
    let fhi = f(upper);

    if !flo.is_finite() || !fhi.is_finite() {
        return Err(Error::eval(
            "Goal seek bounds produced non-finite objective values",
        ));
    }

    if flo.abs() < TOLERANCE {
        return Ok(lower);
    }
    if fhi.abs() < TOLERANCE {
        return Ok(upper);
    }

    if flo * fhi > 0.0 {
        return Err(Error::eval(format!(
            "Goal seek bounds [{:.4}, {:.4}] do not bracket a root",
            lower, upper
        )));
    }

    // Use Brent's method inside the user-supplied bracket; inverse quadratic
    // interpolation with safeguarded bisection converges super-linearly on
    // smooth objectives while retaining the robustness of bisection.
    let solver = BrentSolver::new().tolerance(TOLERANCE);
    solver
        .solve_in_bracket(f, lower, upper)
        .map_err(|e| Error::eval(format!("Goal seek failed within bounds: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::solve_with_bounds;

    #[test]
    fn solve_with_bounds_accepts_endpoint_within_tolerance() {
        let solution = solve_with_bounds(
            &|x| {
                if (x - 1.0).abs() < f64::EPSILON {
                    5e-10
                } else {
                    1.0
                }
            },
            1.0,
            2.0,
        )
        .expect("near-zero endpoint should be accepted as converged");

        assert!((solution - 1.0).abs() < f64::EPSILON);
    }
}
