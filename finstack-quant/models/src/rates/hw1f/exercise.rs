//! Exercise boundary protocol for LSMC-priced callable rate exotics.

use crate::monte_carlo::pricer::basis::BasisFunctions;
use crate::monte_carlo::traits::Payoff;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;

/// Additional contract a `Payoff` must implement to be priced via LSMC
/// in [`crate::rates::hw1f::lsmc::Hw1fLsmcPricer`].
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
    /// (e.g. `notional * price_pct_of_par / 100` for a note callable at par). The LSMC
    /// harness discounts it to time 0 with the pathwise money-market
    /// numeraire `B(t_exercise)` — implementations must NOT pre-discount with
    /// the deterministic curve DF.
    fn intrinsic_at(
        &self,
        exercise_idx: usize,
        short_rate: f64,
        currency: Currency,
    ) -> finstack_quant_core::Result<Money>;

    /// Regression basis used for continuation-value estimation at the
    /// specified exercise date. A canonical implementation returns
    /// the degree-2 [`ShortRateBasis::row`] (`[1, r, r², t·r]`). A longer basis
    /// improves accuracy but adds variance.
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
    fn value_after(
        &self,
        exercise_idx: usize,
        currency: Currency,
    ) -> finstack_quant_core::Result<Money> {
        let _ = exercise_idx;
        self.value(currency)
    }
}

/// LSMC regression basis over `(t, r)` for short-rate exercise boundaries.
///
/// Degrees below 3 produce the degree-2 feature row
/// `[1, r, r², t·r]`; degrees of 3 or greater produce the degree-3 row
/// `[1, r, r², r³, t·r, t·r²]`. The degree is
/// [`crate::rates::hw1f::Hw1fMcConfig::basis_degree`],
/// which keeps that setting a live configuration surface for every payoff built
/// on the shared HW1F LSMC harness.
///
/// The type implements [`BasisFunctions`] with the short rate as the primary
/// state and the exercise time as the auxiliary state, so it composes with the
/// regression utilities in [`crate::monte_carlo::pricer`]; [`Self::row`] is the
/// allocating convenience used by payoffs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortRateBasis {
    degree: usize,
}

impl ShortRateBasis {
    /// Create the basis for a configured polynomial degree.
    ///
    /// # Arguments
    ///
    /// * `degree` - Configured polynomial degree
    ///   ([`crate::rates::hw1f::Hw1fMcConfig::basis_degree`]).
    ///   Values below `3` select the four-term degree-2 row; `3` or greater
    ///   select the six-term degree-3 row.
    #[must_use]
    pub const fn new(degree: usize) -> Self {
        Self { degree }
    }

    /// Whether this basis uses the six-term degree-3 row.
    #[must_use]
    pub const fn is_extended(&self) -> bool {
        self.degree >= 3
    }

    /// Evaluate the basis row at an exercise time and short rate.
    ///
    /// # Arguments
    ///
    /// * `t_years` - Time from valuation to the exercise date in year
    ///   fractions, on the same time basis as the HW1F LSMC harness.
    /// * `short_rate` - Simulated instantaneous short rate at that exercise
    ///   time, expressed as a decimal annual rate.
    #[must_use]
    pub fn row(&self, t_years: f64, short_rate: f64) -> Vec<f64> {
        let mut out = vec![0.0; self.num_basis()];
        self.evaluate_with_aux(short_rate, Some(t_years), &mut out);
        out
    }
}

impl BasisFunctions for ShortRateBasis {
    fn num_basis(&self) -> usize {
        if self.is_extended() {
            6
        } else {
            4
        }
    }

    fn evaluate(&self, state: f64, out: &mut [f64]) {
        self.evaluate_with_aux(state, None, out);
    }

    fn evaluate_with_aux(&self, state: f64, aux: Option<f64>, out: &mut [f64]) {
        let r = state;
        let t = aux.unwrap_or(0.0);
        if self.is_extended() {
            out[..6].copy_from_slice(&[1.0, r, r * r, r * r * r, t * r, t * r * r]);
        } else {
            out[..4].copy_from_slice(&[1.0, r, r * r, t * r]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degree_two_row_matches_the_historical_standard_basis() {
        let (t, r) = (0.5_f64, 0.03_f64);
        assert_eq!(ShortRateBasis::new(2).row(t, r), vec![1.0, r, r * r, t * r]);
        assert_eq!(ShortRateBasis::new(0).row(t, r), vec![1.0, r, r * r, t * r]);
    }

    #[test]
    fn degree_three_row_matches_the_historical_extended_basis() {
        let (t, r) = (0.5_f64, 0.03_f64);
        let expected = vec![1.0, r, r * r, r * r * r, t * r, t * r * r];
        assert_eq!(ShortRateBasis::new(3).row(t, r), expected);
        assert_eq!(ShortRateBasis::new(7).row(t, r), expected);
    }

    #[test]
    fn evaluate_without_aux_zeroes_the_time_terms() {
        let basis = ShortRateBasis::new(2);
        let mut out = vec![0.0; basis.num_basis()];
        basis.evaluate(0.04, &mut out);
        assert_eq!(out, vec![1.0, 0.04, 0.04 * 0.04, 0.0]);
    }

    #[test]
    fn rows_are_finite() {
        for v in ShortRateBasis::new(3).row(2.0, 0.04) {
            assert!(v.is_finite());
        }
    }
}
