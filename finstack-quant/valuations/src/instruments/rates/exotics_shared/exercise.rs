//! Exercise boundary protocol for LSMC-priced callable rate exotics.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_monte_carlo::traits::Payoff;

/// Additional contract a `Payoff` must implement to be priced via LSMC
/// in [`crate::instruments::rates::exotics_shared::hw1f_lsmc::RateExoticHw1fLsmcPricer`].
///
/// The harness handles path simulation, discounting, and backward regression;
/// each product implements the three product-specific hooks below.
///
/// Implementors must also satisfy [`Payoff`]'s `Send + Sync + Clone` bounds
/// (required because the harness clones payoffs per-path and may simulate
/// paths across threads).
pub trait ExerciseBoundaryPayoff: Payoff {
    /// The intrinsic value (i.e., "what the issuer pays on call") at the
    /// specified exercise-date index, evaluated along a single path whose
    /// state at that date is `short_rate`.
    ///
    /// The returned amount is the **undiscounted value at the exercise date**
    /// (e.g. `notional * call_price` for a note callable at par). The LSMC
    /// harness discounts it to time 0 with the pathwise money-market
    /// numeraire `B(t_exercise)` — implementations must NOT pre-discount with
    /// the deterministic curve DF.
    fn intrinsic_at(&self, exercise_idx: usize, short_rate: f64, currency: Currency) -> Money;

    /// Regression basis used for continuation-value estimation at the
    /// specified exercise date. A canonical implementation returns
    /// [`standard_basis`]`(t_years, short_rate)` (`[1, r, r², t·r]`).
    /// Longer basis improves accuracy but adds variance.
    fn continuation_basis(&self, exercise_idx: usize, t_years: f64, short_rate: f64) -> Vec<f64>;

    /// Whether the path has reached a state where exercise is not allowed
    /// (e.g., knocked out). When `true`, the path is excluded from the
    /// continuation-value regression.
    ///
    /// The harness calls this at each exercise date after `Payoff::on_event`
    /// has processed any events on that date. Products that track knockout
    /// state internally (e.g., via path-dependent flags updated inside
    /// `Payoff::on_event`) should return the current path's status from here.
    fn is_path_inactive(&self) -> bool {
        false
    }

    /// Time-0 pathwise PV of the cashflows occurring strictly **after** the
    /// specified exercise date (same pathwise-numeraire discounting as
    /// [`Payoff::value`]).
    ///
    /// The LSMC harness uses this to decompose the pathwise value into a
    /// pre-exercise component (coupons already paid, kept regardless of the
    /// exercise decision) and a post-exercise component (regressed against
    /// the continuation basis and replaced by the call amount on exercise).
    ///
    /// The harness calls this once per exercise-date index after the full
    /// forward pass, so implementations may rely on complete path state.
    ///
    /// The default returns the full [`Payoff::value`], which is exact for
    /// bullet payoffs (a single cashflow at maturity, after every exercise
    /// date). Payoffs with intermediate coupons MUST override this so that
    /// coupons paid before an exercise date are neither fed into the
    /// continuation regression nor dropped when the issuer calls.
    fn value_after(&self, exercise_idx: usize, currency: Currency) -> Money {
        let _ = exercise_idx;
        self.value(currency)
    }
}

/// Standard degree-2 regression basis `[1, r, r², t·r]`.
pub fn standard_basis(t_years: f64, short_rate: f64) -> Vec<f64> {
    vec![
        1.0,
        short_rate,
        short_rate * short_rate,
        t_years * short_rate,
    ]
}

/// Degree-3 regression basis `[1, r, r², r³, t·r, t·r²]`.
pub fn extended_basis(t_years: f64, short_rate: f64) -> Vec<f64> {
    let r = short_rate;
    vec![1.0, r, r * r, r * r * r, t_years * r, t_years * r * r]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_basis_values() {
        let b = standard_basis(0.5, 0.03);
        assert_eq!(b, vec![1.0, 0.03, 0.03 * 0.03, 0.5 * 0.03]);
    }

    #[test]
    fn extended_basis_values() {
        let b = extended_basis(0.5, 0.03);
        let r = 0.03_f64;
        let t = 0.5_f64;
        assert_eq!(b, vec![1.0, r, r * r, r * r * r, t * r, t * r * r]);
    }

    #[test]
    fn basis_values_are_finite() {
        for v in standard_basis(2.0, 0.04) {
            assert!(v.is_finite());
        }
    }
}
