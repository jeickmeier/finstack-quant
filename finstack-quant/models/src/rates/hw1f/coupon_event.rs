//! Coupon event state shared by TARN and snowball products.

use crate::rates::hw1f::PeriodForwardCoeffs;

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
