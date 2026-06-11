//! Shared business-day averaging for commodity floating legs.
//!
//! Both the [`commodity_swap`](super::commodity_swap) floating leg and the
//! [`commodity_swaption`](super::commodity_swaption) forward swap rate
//! average daily business-day prices over each settlement period. The
//! averaging windows are **half-open** `[start, end)` so a payment date is
//! never observed by two adjacent periods; the final period sets
//! `include_end = true` so the swap maturity date is observed exactly once.

use finstack_core::dates::Date;
use finstack_core::Result;

/// Average `get_price` over the business days of the half-open window
/// `[obs_start, obs_end)`, optionally including `obs_end` itself (used by
/// the final period of a schedule so the maturity date is observed exactly
/// once across the whole swap).
///
/// `is_business_day` filters observation dates (weekends/holidays).
/// `get_price` failures (missing fixings, curve coverage) are propagated —
/// never silently substituted (W-11 policy).
///
/// Degenerate windows with no business-day observations fall back to a
/// single mid-window observation.
pub(crate) fn business_day_average_price(
    get_price: impl Fn(Date) -> Result<f64>,
    is_business_day: impl Fn(Date) -> bool,
    obs_start: Date,
    obs_end: Date,
    include_end: bool,
) -> Result<f64> {
    // Compensated (Neumaier) summation keeps the average accurate over a
    // long observation window — a multi-month period can sum many hundreds
    // of daily observations.
    let mut sum = finstack_core::math::NeumaierAccumulator::new();
    let mut count = 0u64;
    let mut current = obs_start;

    while current < obs_end {
        if is_business_day(current) {
            sum.add(get_price(current)?);
            count += 1;
        }
        current += time::Duration::days(1);
    }
    if include_end && obs_end >= obs_start && is_business_day(obs_end) {
        sum.add(get_price(obs_end)?);
        count += 1;
    }

    if count == 0 {
        // Fallback: use the midpoint if no business days were found
        // (shouldn't happen for realistic windows).
        let mid = obs_start + (obs_end - obs_start) / 2;
        return get_price(mid);
    }

    Ok(sum.total() / count as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    fn d(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid date")
    }

    fn weekdays_only(date: Date) -> bool {
        let wd = date.weekday();
        wd != time::Weekday::Saturday && wd != time::Weekday::Sunday
    }

    /// Adjacent half-open windows must partition the observation dates: the
    /// shared boundary date is observed exactly once (by the later window's
    /// start, not the earlier window's end).
    #[test]
    fn adjacent_windows_do_not_double_count_boundary() {
        // 2025-07-01 (Tue) .. 2025-07-31 (Thu) — boundary 2025-07-16 (Wed).
        let start = d(2025, Month::July, 1);
        let boundary = d(2025, Month::July, 16);
        let end = d(2025, Month::July, 31);

        // Price = 1.0 only on the boundary date; 0 elsewhere. The total
        // count of boundary observations across both windows must be 1.
        let price = |date: Date| -> Result<f64> { Ok(if date == boundary { 1.0 } else { 0.0 }) };

        let count_obs = |s: Date, e: Date, inc: bool| -> (f64, u64) {
            let mut n = 0u64;
            let mut cur = s;
            while cur < e {
                if weekdays_only(cur) {
                    n += 1;
                }
                cur += time::Duration::days(1);
            }
            if inc && weekdays_only(e) {
                n += 1;
            }
            let avg = business_day_average_price(price, weekdays_only, s, e, inc).expect("avg");
            (avg, n)
        };

        let (avg1, n1) = count_obs(start, boundary, false);
        let (avg2, n2) = count_obs(boundary, end, true);

        let boundary_hits = avg1 * n1 as f64 + avg2 * n2 as f64;
        assert!(
            (boundary_hits - 1.0).abs() < 1e-12,
            "boundary date must be observed exactly once across adjacent \
             windows, got {boundary_hits}"
        );
    }

    /// The final period includes the maturity date exactly once.
    #[test]
    fn include_end_observes_final_date() {
        let start = d(2025, Month::July, 28); // Mon
        let end = d(2025, Month::July, 31); // Thu

        let price = |date: Date| -> Result<f64> { Ok(if date == end { 1.0 } else { 0.0 }) };

        let without = business_day_average_price(price, weekdays_only, start, end, false)
            .expect("avg without end");
        let with = business_day_average_price(price, weekdays_only, start, end, true).expect("avg");

        assert!(without.abs() < 1e-12, "half-open window must exclude end");
        // With include_end: Mon, Tue, Wed, Thu = 4 observations, one hit.
        assert!(
            (with - 0.25).abs() < 1e-12,
            "final window must observe the end date once: got {with}"
        );
    }

    /// Errors from the price source propagate (no silent substitution).
    #[test]
    fn price_errors_propagate() {
        let start = d(2025, Month::July, 1);
        let end = d(2025, Month::July, 10);
        let price = |_d: Date| -> Result<f64> {
            Err(finstack_core::Error::Validation("no coverage".into()))
        };
        assert!(business_day_average_price(price, weekdays_only, start, end, false).is_err());
    }
}
