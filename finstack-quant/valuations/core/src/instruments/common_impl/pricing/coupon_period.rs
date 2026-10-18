//! Quasi-coupon period resolution for ACT/ACT (ICMA) year fractions.

use finstack_quant_core::dates::Date;

/// Infer an ACT/ACT (ICMA) reference coupon period from the cashflow schedule.
///
/// Returns the quasi-coupon period `(first_flow − frequency, first_flow)`
/// surrounding the settlement date, where `first_flow` is the first cashflow
/// date strictly after `as_of`. Supplying this period through
/// [`DayCountContext::coupon_period`](finstack_quant_core::dates::DayCountContext::coupon_period) lets ISMA year fractions resolve for
/// spans that are not whole multiples of the coupon frequency (mid-coupon
/// settlement, stubs), which the frequency-only path rejects; the core
/// reference-period traversal then walks the quasi-coupon grid anchored on
/// this period for every flow date.
///
/// # Arguments
///
/// * `day_count` - Bond coupon day-count convention; only
///   [`finstack_quant_core::dates::DayCount::ActActIsma`] yields a period.
/// * `frequency` - Contractual coupon frequency defining the quasi-coupon
///   grid; week/day frequencies are unsupported by ICMA and yield `None`.
/// * `dates` - Coupon payment dates in ascending order; the first date
///   strictly after `as_of` anchors the grid.
/// * `as_of` - Settlement/valuation date the reference period must surround.
///
/// Returns `None` for non-ICMA day counts, week/day frequencies, or when no
/// flow falls strictly after `as_of` (callers then keep the frequency-only
/// context and its existing error behavior).
pub fn icma_reference_period(
    day_count: finstack_quant_core::dates::DayCount,
    frequency: finstack_quant_core::dates::Tenor,
    dates: impl IntoIterator<Item = Date>,
    as_of: Date,
) -> Option<(Date, Date)> {
    use finstack_quant_core::dates::{DateExt, DayCount, TenorUnit};
    if day_count != DayCount::ActActIsma {
        return None;
    }
    let months = match frequency.unit() {
        TenorUnit::Months => frequency.count() as i32,
        TenorUnit::Years => (frequency.count() as i32).checked_mul(12)?,
        TenorUnit::Weeks | TenorUnit::Days => return None,
    };
    if months <= 0 {
        return None;
    }
    let next = dates.into_iter().find(|d| *d > as_of)?;
    let mut prev = next.add_months(-months).ok()?;
    // Preserve the end-of-month roll so the quasi-coupon grid matches an
    // EOM schedule (mirrors the traversal's own EOM handling).
    if next == next.end_of_month() {
        prev = prev.end_of_month();
    }
    (prev < next).then_some((prev, next))
}
