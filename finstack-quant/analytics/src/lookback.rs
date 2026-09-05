//! Lookback period selectors: MTD, QTD, YTD, FYTD.
//!
//! Crate-internal: callers use these through [`crate::Performance`].
//! Each function returns a `Range<usize>` into the dates/returns arrays rather
//! than sliced data, so callers slice their own arrays.

use crate::dates::{Date, DateExt, Duration, FiscalConfig, Month};
use core::ops::Range;

/// Index of the first date on or after `target` via binary search.
fn lower_bound(dates: &[Date], target: Date) -> usize {
    dates.partition_point(|&d| d < target)
}

/// Shared range builder: `[period_start, ref_date]` inclusive.
fn select_range(dates: &[Date], period_start: Date, ref_date: Date) -> Range<usize> {
    let lo = lower_bound(dates, period_start);
    let hi = lower_bound(dates, ref_date + Duration::days(1));
    lo..hi
}

/// Month-to-date index range: from the first calendar day of `ref_date`'s
/// month through `ref_date` (inclusive).
///
/// # Arguments
///
/// * `dates`    - Sorted slice of observation dates.
/// * `ref_date` - Reference date (typically "today").
///
/// # Returns
///
/// A `Range<usize>` into `dates` covering the MTD window.
/// The range may be empty if no dates fall within the window.
pub(crate) fn mtd_select(dates: &[Date], ref_date: Date) -> Range<usize> {
    let month_start = ref_date.replace_day(1).unwrap_or(ref_date);
    select_range(dates, month_start, ref_date)
}

/// Quarter-to-date index range: from the first calendar day of `ref_date`'s
/// quarter through `ref_date` (inclusive).
///
/// Quarter boundaries follow calendar convention: Q1 = Jan–Mar,
/// Q2 = Apr–Jun, Q3 = Jul–Sep, Q4 = Oct–Dec.
///
/// # Arguments
///
/// * `dates`    - Sorted slice of observation dates.
/// * `ref_date` - Reference date (typically "today").
///
/// # Returns
///
/// A `Range<usize>` into `dates` covering the QTD window.
pub(crate) fn qtd_select(dates: &[Date], ref_date: Date) -> Range<usize> {
    let q = ref_date.quarter();
    let quarter_start_month = (q - 1) * 3 + 1;
    let (year, _month, _day) = ref_date.to_calendar_date();
    let qtr_start = crate::dates::create_date(
        year,
        Month::try_from(quarter_start_month).unwrap_or(Month::January),
        1,
    )
    .unwrap_or(ref_date);
    select_range(dates, qtr_start, ref_date)
}

/// Year-to-date index range: from January 1 of `ref_date`'s calendar year
/// through `ref_date` (inclusive).
///
/// # Arguments
///
/// * `dates`    - Sorted slice of observation dates.
/// * `ref_date` - Reference date (typically "today").
///
/// # Returns
///
/// A `Range<usize>` into `dates` covering the YTD window.
pub(crate) fn ytd_select(dates: &[Date], ref_date: Date) -> Range<usize> {
    let (year, _month, _day) = ref_date.to_calendar_date();
    let year_start = crate::dates::create_date(year, Month::January, 1).unwrap_or(ref_date);
    select_range(dates, year_start, ref_date)
}

/// Fiscal-year-to-date index range: first observation on or after the
/// fiscal calendar start through `ref_date` (inclusive).
///
/// The fiscal year start is determined by [`FiscalConfig`] (start month and
/// day) with no holiday skip. A January 1 holiday still begins the window
/// at the first observation `>=` January 1. The first included simple
/// return still spans the prior close (the return on date `T` is the
/// move from `T-1` to `T`).
///
/// # Arguments
///
/// * `dates`         - Sorted slice of observation dates.
/// * `ref_date`      - Reference date (typically "today").
/// * `fiscal_config` - Fiscal year configuration (start month, start day).
///
/// # Returns
///
/// A `Range<usize>` into `dates` covering the FYTD window.
pub(crate) fn fytd_select(
    dates: &[Date],
    ref_date: Date,
    fiscal_config: FiscalConfig,
) -> Range<usize> {
    let fy_start = fiscal_year_start_date(ref_date, fiscal_config);
    select_range(dates, fy_start, ref_date)
}

fn fiscal_year_start_date(ref_date: Date, fiscal_config: FiscalConfig) -> Date {
    let fy = ref_date.fiscal_year(fiscal_config);
    let fy_start_month = Month::try_from(fiscal_config.start_month).unwrap_or(Month::January);
    let calendar_year = if fiscal_config.start_month == 1 && fiscal_config.start_day <= 1 {
        fy
    } else {
        fy - 1
    };
    // `FiscalConfig` accepts start_day 1..=31 regardless of month length;
    // clamp to the last valid day of the start month (matching the overflow
    // semantics of `DateExt::fiscal_year`, where "Feb 30" means the last day
    // of February) so the construction below cannot fail and silently fall
    // back to `ref_date`.
    let start_day = fiscal_config
        .start_day
        .min(fy_start_month.length(calendar_year));
    crate::dates::create_date(calendar_year, fy_start_month, start_day).unwrap_or(ref_date)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(y: i32, m: u8, day: u8) -> Date {
        crate::dates::create_date(y, Month::try_from(m).expect("valid month"), day)
            .expect("valid date")
    }

    fn daily_dates(start: Date, n: usize) -> Vec<Date> {
        (0..n).map(|i| start + Duration::days(i as i64)).collect()
    }

    #[test]
    fn ytd_select_basic() {
        let dates = daily_dates(d(2025, 1, 1), 60);
        let range = ytd_select(&dates, d(2025, 2, 15));
        assert_eq!(range.start, 0);
        assert!(range.end > 30);
    }

    #[test]
    fn mtd_select_basic() {
        let dates = daily_dates(d(2025, 1, 1), 60);
        let range = mtd_select(&dates, d(2025, 2, 15));
        assert!(range.start > 0);
    }

    #[test]
    fn qtd_select_q1() {
        let dates = daily_dates(d(2025, 1, 1), 90);
        let range = qtd_select(&dates, d(2025, 3, 15));
        assert_eq!(range.start, 0);
    }

    #[test]
    fn fytd_select_us_federal() {
        let dates = daily_dates(d(2024, 10, 1), 120);
        let config = FiscalConfig::us_federal();
        let range = fytd_select(&dates, d(2025, 1, 15), config);
        assert_eq!(range.start, 0);
    }

    #[test]
    fn fiscal_start_day_exceeding_month_length_clamps_to_month_end() {
        // "Feb 30" fiscal start means "last day of February" (matching
        // `DateExt::fiscal_year` overflow semantics). Before the clamp this
        // silently degenerated to a single-observation FYTD window anchored
        // at `ref_date`.
        let config = FiscalConfig::new(2, 30).expect("config accepts day 30");
        let start = fiscal_year_start_date(d(2025, 6, 15), config);
        assert_eq!(start, d(2025, 2, 28));

        // Leap year: clamps to Feb 29.
        let start_leap = fiscal_year_start_date(d(2024, 6, 15), config);
        assert_eq!(start_leap, d(2024, 2, 29));
    }

    #[test]
    fn fytd_select_includes_first_observation_on_or_after_fiscal_start() {
        let dates = daily_dates(d(2024, 12, 30), 10);
        let range = fytd_select(&dates, d(2025, 1, 6), FiscalConfig::calendar_year());
        // Jan 1 2025 is a holiday; the window still starts at the first
        // observation on/after Jan 1, not the Following business day.
        assert_eq!(dates[range.start], d(2025, 1, 1));
        assert_ne!(dates[range.start], d(2025, 1, 2));
    }
}
