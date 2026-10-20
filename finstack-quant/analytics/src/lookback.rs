//! Lookback period selectors: MTD, QTD, YTD, FYTD.
//!
//! Crate-internal: callers use these through [`crate::Performance`].
//! [`select_range`] returns a `Range<usize>` into the sorted dates/returns
//! arrays for a `[period_start, ref_date]` window.

use crate::dates::{Date, DateExt, FiscalConfig};
use core::ops::Range;
use time::Month;

/// Index range `[period_start, ref_date]` inclusive in a sorted date slice.
pub(crate) fn select_range(dates: &[Date], period_start: Date, ref_date: Date) -> Range<usize> {
    let lo = dates.partition_point(|&d| d < period_start);
    let hi = dates.partition_point(|&date| date <= ref_date);
    lo..hi
}

/// First calendar day of `ref_date`'s month.
pub(crate) fn mtd_start_date(ref_date: Date) -> Date {
    ref_date.replace_day(1).unwrap_or(ref_date)
}

/// First calendar day of `ref_date`'s calendar quarter (Q1 = Jan, Q2 = Apr, Q3 = Jul, Q4 = Oct).
pub(crate) fn qtd_start_date(ref_date: Date) -> Date {
    let q = ref_date.quarter();
    let quarter_start_month = (q - 1) * 3 + 1;
    let (year, _month, _day) = ref_date.to_calendar_date();
    crate::dates::create_date(
        year,
        Month::try_from(quarter_start_month).unwrap_or(Month::January),
        1,
    )
    .unwrap_or(ref_date)
}

/// January 1 of `ref_date`'s calendar year.
pub(crate) fn ytd_start_date(ref_date: Date) -> Date {
    let (year, _month, _day) = ref_date.to_calendar_date();
    crate::dates::create_date(year, Month::January, 1).unwrap_or(ref_date)
}

/// Start date of the fiscal year containing `ref_date` under `fiscal_config`.
pub(crate) fn fytd_start_date(ref_date: Date, fiscal_config: FiscalConfig) -> Date {
    fiscal_config
        .year_start(ref_date.fiscal_year(fiscal_config))
        .unwrap_or(ref_date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;
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
        let ref_date = d(2025, 2, 15);
        let range = select_range(&dates, ytd_start_date(ref_date), ref_date);
        assert_eq!(range.start, 0);
        assert!(range.end > 30);
    }

    #[test]
    fn mtd_select_basic() {
        let dates = daily_dates(d(2025, 1, 1), 60);
        let ref_date = d(2025, 2, 15);
        let range = select_range(&dates, mtd_start_date(ref_date), ref_date);
        assert!(range.start > 0);
    }

    #[test]
    fn qtd_select_q1() {
        let dates = daily_dates(d(2025, 1, 1), 90);
        let ref_date = d(2025, 3, 15);
        let range = select_range(&dates, qtd_start_date(ref_date), ref_date);
        assert_eq!(range.start, 0);
    }

    #[test]
    fn fytd_select_us_federal() {
        let dates = daily_dates(d(2024, 10, 1), 120);
        let config = FiscalConfig::us_federal();
        let ref_date = d(2025, 1, 15);
        let range = select_range(&dates, fytd_start_date(ref_date, config), ref_date);
        assert_eq!(range.start, 0);
    }

    #[test]
    fn fiscal_start_day_exceeding_month_length_clamps_to_month_end() {
        let config = FiscalConfig::new(2, 30).expect("config accepts day 30");
        let start = fytd_start_date(d(2025, 6, 15), config);
        assert_eq!(start, d(2025, 2, 28));

        let start_leap = fytd_start_date(d(2024, 6, 15), config);
        assert_eq!(start_leap, d(2024, 2, 29));
    }

    #[test]
    fn fytd_select_includes_first_observation_on_or_after_fiscal_start() {
        let dates = daily_dates(d(2024, 12, 30), 10);
        let ref_date = d(2025, 1, 6);
        let range = select_range(
            &dates,
            fytd_start_date(ref_date, FiscalConfig::calendar_year()),
            ref_date,
        );
        assert_eq!(dates[range.start], d(2025, 1, 1));
        assert_ne!(dates[range.start], d(2025, 1, 2));
    }
}
