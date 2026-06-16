//! Pathwise money-market account (bank-account numeraire) helpers.
//!
//! Under the risk-neutral measure a cashflow `X` paid at `T` is valued
//! `E[X / B(T)]` with `B(t) = exp(∫₀ᵗ r(s) ds)`. Discounting simulated
//! cashflows with the deterministic time-0 curve DF instead replaces the
//! stochastic discount factor with its expectation `E[1/B(T)] = P(0,T)`,
//! dropping the payoff/numeraire correlation that a short-rate model exists
//! to capture. Every HW1F Monte Carlo pricer must therefore accumulate the
//! pathwise `B(t)` and discount with it (or with ratios of it).

use finstack_quant_monte_carlo::time_grid::TimeGrid;

/// One-step trapezoidal bank-account growth factor over `[t, t + dt]`.
///
/// Approximates `exp(∫ r ds)` with the trapezoidal rule using both endpoint
/// short rates, which the exact-HW1F transition supplies:
///
/// ```text
/// B(t + dt) = B(t) · exp(½·(r(t) + r(t + dt))·dt)
/// ```
///
/// The cheap left-endpoint Riemann sum `exp(r(t)·dt)` leaves an avoidable
/// O(Δt) bias on coarse event-aligned grids; the trapezoidal rule is O(Δt²)
/// and uses only path values already simulated.
#[inline]
pub fn bank_step_factor(r_start: f64, r_end: f64, dt: f64) -> f64 {
    (0.5 * (r_start + r_end) * dt).exp()
}

/// Accumulate the pathwise money-market numeraire `B(t)` along a simulated
/// short-rate path, one entry per grid point (`num_steps + 1`, `B(t_0) = 1`).
///
/// `rate_path` carries one short rate per grid point. Each step applies
/// [`bank_step_factor`] over the corresponding grid interval.
pub fn accumulate_bank_factors(rate_path: &[f64], time_grid: &TimeGrid) -> Vec<f64> {
    let num_steps = time_grid.num_steps();
    let mut bank = Vec::with_capacity(num_steps + 1);
    bank.push(1.0); // B(t_0) = 1
    let mut acc = 1.0;
    for (step, pair) in rate_path.windows(2).enumerate() {
        acc *= bank_step_factor(pair[0], pair[1], time_grid.dt(step));
        bank.push(acc);
    }
    bank
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_rate_bank_matches_exponential() {
        let grid = TimeGrid::from_times(vec![0.0, 0.5, 1.0, 2.0]).expect("grid");
        let rates = vec![0.03, 0.03, 0.03, 0.03];
        let bank = accumulate_bank_factors(&rates, &grid);
        assert_eq!(bank.len(), 4);
        for (i, &b) in bank.iter().enumerate() {
            let t = grid.time(i);
            assert!((b - (0.03 * t).exp()).abs() < 1e-12, "B({t}) = {b}");
        }
    }

    #[test]
    fn step_factor_is_trapezoidal() {
        let f = bank_step_factor(0.02, 0.04, 0.5);
        assert!((f - (0.5_f64 * 0.06 * 0.5).exp()).abs() < 1e-15);
    }
}
