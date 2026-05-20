//! Cumulative coupon tracker shared by TARN and Snowball products.

use crate::instruments::rates::exotics_shared::PeriodForwardCoeffs;

/// One coupon's worth of pre-computed state for a path-dependent rate exotic
/// that fixes **in advance** (e.g. TARN, Snowball / inverse floater).
///
/// The floating rate `L` is set at the period start and applies over
/// `[start, end]`. The index is the `[start, end]`-tenor simple forward,
/// reconstructed via the affine HW1F bond formula from the short rate
/// sampled **at the period start** — which is when the event fires.
#[derive(Debug, Clone, Copy)]
pub struct CouponEvent {
    /// Year fraction of the accrual period under the contract day-count.
    pub accrual_fraction: f64,
    /// Risk-free discount factor from `as_of` to the coupon payment date.
    pub discount_factor: f64,
    /// HW1F bond-reconstruction coefficients for the coupon's floating index.
    pub forward_coeffs: PeriodForwardCoeffs,
    /// `true` for a forward-starting coupon (period start strictly after
    /// `as_of`): the simulation fires one `on_event` at the period start and
    /// the payoff reads the short rate there. `false` for an already-seasoned
    /// first coupon (period start at or before `as_of`): its fixing is the
    /// deterministic `r(0) = f(0,0)` reconstruction, so `forward_coeffs`
    /// degenerates to a flat rate and the event consumes no path sample.
    pub needs_path_sample: bool,
}

/// Tracks cumulative coupon for path-dependent products (TARN, Snowball).
///
/// Handles the running sum of coupons paid so far and determines whether
/// a target-based knockout condition is met. Used during Monte Carlo
/// simulation to manage path-dependent coupon accumulation.
///
/// # TARN Usage
///
/// For TARNs, the tracker is created with a target level. When the
/// cumulative coupon reaches or exceeds the target, the final coupon
/// is capped so that the cumulative equals the target exactly.
///
/// # Snowball Usage
///
/// For snowball notes, the tracker has no target (just accumulates
/// coupons for reporting).
#[derive(Debug, Clone)]
pub struct CumulativeCouponTracker {
    /// Running sum of coupons paid (in absolute terms, not rate).
    cumulative_coupon: f64,
    /// Target level for knockout (TARN); None = no knockout.
    target: Option<f64>,
    /// Whether the target has been breached.
    knocked_out: bool,
    /// Period at which knockout occurred (None if still alive).
    knockout_period: Option<usize>,
    /// Current period index (incremented on each add_coupon call).
    current_period: usize,
}

impl CumulativeCouponTracker {
    /// Create a new tracker for a TARN with target knockout.
    ///
    /// When the cumulative coupon reaches `target`, the instrument
    /// redeems at par and no further coupons are paid.
    pub fn with_target(target: f64) -> Self {
        Self {
            cumulative_coupon: 0.0,
            target: Some(target),
            knocked_out: false,
            knockout_period: None,
            current_period: 0,
        }
    }

    /// Create a new tracker without knockout (for snowball coupon accumulation).
    pub fn no_target() -> Self {
        Self {
            cumulative_coupon: 0.0,
            target: None,
            knocked_out: false,
            knockout_period: None,
            current_period: 0,
        }
    }

    /// Add a coupon payment. Returns the actual coupon paid (may be reduced
    /// if cumulative hits the target mid-period).
    ///
    /// For TARN: if cumulative + coupon > target, the paid coupon is
    /// capped at (target - cumulative) and knocked_out is set to true.
    pub fn add_coupon(&mut self, coupon: f64) -> f64 {
        if self.knocked_out {
            return 0.0;
        }

        let actual = match self.target {
            Some(target) => {
                let remaining = target - self.cumulative_coupon;
                if coupon >= remaining {
                    // Knockout: cap the coupon to hit the target exactly
                    self.knocked_out = true;
                    self.knockout_period = Some(self.current_period);
                    remaining.max(0.0)
                } else {
                    coupon
                }
            }
            None => coupon,
        };

        self.cumulative_coupon += actual;
        self.current_period += 1;
        actual
    }

    /// Whether the instrument has been knocked out (cumulative hit target).
    pub fn is_knocked_out(&self) -> bool {
        self.knocked_out
    }

    /// The period index at which knockout occurred.
    pub fn knockout_period(&self) -> Option<usize> {
        self.knockout_period
    }

    /// Current cumulative coupon level.
    pub fn cumulative(&self) -> f64 {
        self.cumulative_coupon
    }

    /// Reset for next MC path.
    pub fn reset(&mut self) {
        self.cumulative_coupon = 0.0;
        self.knocked_out = false;
        self.knockout_period = None;
        self.current_period = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tarn_knockout_exact_target() {
        let mut tracker = CumulativeCouponTracker::with_target(0.10);
        let c1 = tracker.add_coupon(0.05);
        assert!((c1 - 0.05).abs() < 1e-12);
        assert!(!tracker.is_knocked_out());

        let c2 = tracker.add_coupon(0.05);
        assert!((c2 - 0.05).abs() < 1e-12);
        assert!(tracker.is_knocked_out());
        assert_eq!(tracker.knockout_period(), Some(1));
        assert!((tracker.cumulative() - 0.10).abs() < 1e-12);
    }

    #[test]
    fn tarn_knockout_mid_period() {
        let mut tracker = CumulativeCouponTracker::with_target(0.10);
        let c1 = tracker.add_coupon(0.06);
        assert!((c1 - 0.06).abs() < 1e-12);
        assert!(!tracker.is_knocked_out());

        // Next coupon would overshoot: 0.06 + 0.07 = 0.13 > 0.10
        let c2 = tracker.add_coupon(0.07);
        assert!((c2 - 0.04).abs() < 1e-12); // capped to remaining
        assert!(tracker.is_knocked_out());
        assert!((tracker.cumulative() - 0.10).abs() < 1e-12);
    }

    #[test]
    fn tarn_no_coupon_after_knockout() {
        let mut tracker = CumulativeCouponTracker::with_target(0.05);
        tracker.add_coupon(0.05);
        assert!(tracker.is_knocked_out());

        let c = tracker.add_coupon(0.03);
        assert!((c).abs() < 1e-12);
    }

    #[test]
    fn snowball_no_target() {
        let mut tracker = CumulativeCouponTracker::no_target();
        tracker.add_coupon(0.05);
        tracker.add_coupon(0.08);
        tracker.add_coupon(0.03);
        assert!(!tracker.is_knocked_out());
        assert!((tracker.cumulative() - 0.16).abs() < 1e-12);
    }

    #[test]
    fn reset_clears_state() {
        let mut tracker = CumulativeCouponTracker::with_target(0.10);
        tracker.add_coupon(0.10);
        assert!(tracker.is_knocked_out());

        tracker.reset();
        assert!(!tracker.is_knocked_out());
        assert!(tracker.knockout_period().is_none());
        assert!((tracker.cumulative()).abs() < 1e-12);
    }

    #[test]
    fn zero_target_knocks_out_immediately() {
        let mut tracker = CumulativeCouponTracker::with_target(0.0);
        let c = tracker.add_coupon(0.01);
        assert!((c).abs() < 1e-12);
        assert!(tracker.is_knocked_out());
    }
}
