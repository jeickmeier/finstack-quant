//! Last-twelve-months (LTM) windows over a model's period grid.

use super::{Period, PeriodId, PeriodKind};
use crate::dates::date_extensions::DateExt;

/// The model periods a last-twelve-months sum reads, ending at one period.
///
/// Built by [`ltm_window`]. A window is complete only when no slot is
/// missing; callers never annualise or partially sum an incomplete window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LtmWindow {
    /// Window periods, oldest first, ending with the reporting period.
    /// Fixed-frequency slots are named whether or not the model holds them, so
    /// a caller still checks each slot has a value; daily slots are the
    /// model's own days.
    pub slots: Vec<PeriodId>,
    /// Number of window slots that cannot be named: for daily models the
    /// calendar days of the window before the model's first daily period, for
    /// other frequencies slots outside the supported date range.
    pub missing: u32,
}

impl LtmWindow {
    /// Number of slots a complete window holds: named plus missing.
    #[must_use]
    pub fn required(&self) -> usize {
        self.slots.len() + self.missing as usize
    }

    /// Whether every slot of the twelve-month window could be named.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.missing == 0
    }
}

/// Trailing-twelve-month window of model periods ending at (and including) `period`.
///
/// The window is the twelve calendar months ending at the period's exclusive
/// end date.
///
/// | Frequency | Window |
/// | --- | --- |
/// | Monthly, quarterly, semi-annual, annual | The 12, 4, 2 or 1 periods ending at `period`, Gregorian or fiscal |
/// | Weekly, Gregorian | The 52 weeks ending at `period` (364 days, the 52/53-week reporting convention) |
/// | Weekly, fiscal | The 53 blocks ending at `period`: 52 seven-day weeks and the one- or two-day final block of a fiscal year (365 or 366 days) |
/// | Daily | Every model daily period starting in `[end - 12 months, end)` |
///
/// Daily models may be a calendar-day or a business-day grid. A day absent from
/// the grid inside the window (a weekend or holiday) is a non-trading day, not
/// missing data, so a business-day model sums its roughly 252 rows and a
/// calendar-day model its 365 or 366. The daily window is incomplete only when
/// the model's first daily period starts after the window's first day; the
/// uncovered calendar days count as [`LtmWindow::missing`]. The window start is
/// `end` moved back twelve months, with a month-end such as February 29 clamped
/// to the last day of the target month.
///
/// # Arguments
///
/// * `period` - Reporting period that ends the window (inclusive).
/// * `periods` - The model's periods with their `[start, end)` dates. Only a
///   daily `period` reads them: its window holds the daily periods of the same
///   calendar (Gregorian or fiscal), and `period`'s own end date comes from
///   this list, falling back to its Gregorian bounds.
///
/// # Returns
///
/// The window's slots, oldest first, and the count of missing slots.
///
/// # Errors
///
/// Returns `Error::Validation` for a daily `period` that is fiscal and absent
/// from `periods` (its dates need the model's fiscal calendar), or whose
/// window leaves the supported date range.
///
/// # Example
/// ```
/// use finstack_quant_core::dates::{build_periods, ltm_window, PeriodId};
///
/// let plan = build_periods("2024Q1..2025Q2", None)?;
/// let window = ltm_window(PeriodId::quarter(2025, 1)?, &plan.periods)?;
/// assert!(window.is_complete());
/// assert_eq!(window.slots.first(), Some(&PeriodId::quarter(2024, 2)?));
///
/// assert_eq!(window.slots.len(), 4);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn ltm_window(period: PeriodId, periods: &[Period]) -> crate::Result<LtmWindow> {
    if period.kind() != PeriodKind::Daily {
        // A fiscal year of weeks is 52 seven-day blocks plus a one- or
        // two-day final block, so its twelve months are 53 blocks.
        let count = if period.kind() == PeriodKind::Weekly && period.is_fiscal() {
            i32::from(period.kind().capacity())
        } else {
            i32::from(period.kind().periods_per_year())
        };
        let mut window = LtmWindow {
            slots: Vec::with_capacity(count as usize),
            missing: 0,
        };
        for back in (0..count).rev() {
            match period.shift(-back) {
                Ok(slot) => window.slots.push(slot),
                Err(_) => window.missing += 1,
            }
        }
        return Ok(window);
    }

    let end = match periods.iter().find(|p| p.id == period) {
        Some(found) => found.end,
        None => period.bounds()?.1,
    };
    let start = end.add_months(-12)?;
    let mut grid: Vec<&Period> = periods
        .iter()
        .filter(|p| p.id.kind() == PeriodKind::Daily && p.id.is_fiscal() == period.is_fiscal())
        .collect();
    grid.sort_by_key(|p| p.start);
    let first = grid.first().map_or(end, |p| p.start);
    let uncovered = (first.min(end) - start).whole_days().max(0);
    Ok(LtmWindow {
        slots: grid
            .iter()
            .filter(|p| p.start >= start && p.start < end)
            .map(|p| p.id)
            .collect(),
        missing: u32::try_from(uncovered).map_err(|_| {
            crate::Error::Validation(format!("LTM window for {period} is out of range"))
        })?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::{Date, build_periods, create_date};
    use time::Month;

    fn day(date: Date, is_actual: bool) -> Period {
        let id = PeriodId::day(date.year(), date.ordinal()).expect("day id");
        Period {
            id,
            start: date,
            end: date.next_day().expect("next day"),
            is_actual,
        }
    }

    fn grid(from: Date, to: Date, business_only: bool) -> Vec<Period> {
        let mut out = Vec::new();
        let mut d = from;
        while d <= to {
            if !business_only || d.weekday().number_days_from_monday() < 5 {
                out.push(day(d, true));
            }
            d = d.next_day().expect("next day");
        }
        out
    }

    #[test]
    fn monthly_quarterly_and_annual_windows_use_twelve_months_of_slots() {
        let monthly = build_periods("2024M01..2025M06", None)
            .expect("plan")
            .periods;
        let window = ltm_window(PeriodId::month(2025, 3).unwrap(), &monthly).unwrap();
        assert!(window.is_complete());
        assert_eq!(window.slots.len(), 12);
        assert_eq!(window.slots[0], PeriodId::month(2024, 4).unwrap());

        let quarterly = build_periods("2024Q1..2025Q4", None).expect("plan").periods;
        let window = ltm_window(PeriodId::quarter(2024, 4).unwrap(), &quarterly).unwrap();
        assert_eq!(window.slots.len(), 4);
        assert!(window.is_complete());

        let annual = build_periods("2023..2025", None).expect("plan").periods;
        let window = ltm_window(
            PeriodId::annual(2024).expect("valid annual period"),
            &annual,
        )
        .unwrap();
        assert_eq!(
            window.slots,
            vec![PeriodId::annual(2024).expect("valid annual period")]
        );
        assert!(window.is_complete());
    }

    /// A fiscal year is 52 seven-day weeks plus a one- or two-day final
    /// block, so twelve months of fiscal weeks is 53 blocks: every window
    /// holds 52 full weeks and exactly one short block, wherever it ends.
    #[test]
    fn fiscal_weekly_window_covers_a_full_year_of_days() {
        use crate::dates::{FiscalConfig, build_fiscal_periods};
        let periods = build_fiscal_periods(
            "FY2024W1..FY2026W53",
            FiscalConfig::new(10, 1).expect("valid fiscal start"),
            None,
        )
        .expect("plan")
        .periods;
        let days = |id: &PeriodId| {
            let period = periods.iter().find(|p| p.id == *id).expect("model period");
            (period.end - period.start).whole_days()
        };
        for at in periods.iter().filter(|p| p.id.year() >= 2025) {
            let window = ltm_window(at.id, &periods).unwrap();
            assert!(window.is_complete());
            assert_eq!(window.slots.len(), 53, "{}", at.id);
            assert_eq!(window.slots.last(), Some(&at.id));
            // 52 full weeks and the one short block the window holds.
            let short_block = window
                .slots
                .iter()
                .find(|id| id.index() == 53)
                .expect("one final block per window");
            assert!((1..=2).contains(&days(short_block)));
            let covered: i64 = window.slots.iter().map(days).sum();
            assert_eq!(covered, 364 + days(short_block), "{}", at.id);
        }
    }

    /// Gregorian weeks have no short block: the window stays 52 weeks even
    /// across a 53-week ISO year.
    #[test]
    fn gregorian_weekly_window_is_fifty_two_full_weeks() {
        let periods = build_periods("2020W1..2021W20", None)
            .expect("plan")
            .periods;
        assert!(
            periods
                .iter()
                .any(|p| p.id.year() == 2020 && p.id.index() == 53)
        );
        for at in periods.iter().filter(|p| p.id.year() == 2021) {
            let window = ltm_window(at.id, &periods).unwrap();
            assert!(window.is_complete());
            assert_eq!(window.slots.len(), 52);
            let covered: i64 = window
                .slots
                .iter()
                .map(|id| {
                    let period = periods.iter().find(|p| p.id == *id).expect("model period");
                    (period.end - period.start).whole_days()
                })
                .sum();
            assert_eq!(covered, 364, "{}", at.id);
        }
    }

    #[test]
    fn calendar_day_window_spans_a_full_year_not_252_days() {
        let start = create_date(2024, Month::January, 1).unwrap();
        let end = create_date(2025, Month::December, 31).unwrap();
        let periods = grid(start, end, false);
        let at = PeriodId::day(2025, 365).unwrap();
        let window = ltm_window(at, &periods).unwrap();
        assert!(window.is_complete());
        assert_eq!(window.slots.len(), 365);
        assert_eq!(window.slots[0], PeriodId::day(2025, 1).unwrap());
        assert_eq!(window.slots.last(), Some(&at));
    }

    #[test]
    fn business_day_window_sums_the_grid_rows_in_twelve_months() {
        let start = create_date(2024, Month::January, 1).unwrap();
        let end = create_date(2025, Month::December, 31).unwrap();
        let periods = grid(start, end, true);
        let at = PeriodId::day(2025, 365).unwrap();
        let window = ltm_window(at, &periods).unwrap();
        assert!(window.is_complete());
        // 2025 has 261 weekdays.
        assert_eq!(window.slots.len(), 261);
    }

    #[test]
    fn daily_window_before_a_year_of_history_is_incomplete() {
        let start = create_date(2025, Month::March, 1).unwrap();
        let end = create_date(2025, Month::December, 31).unwrap();
        let periods = grid(start, end, false);
        let window = ltm_window(PeriodId::day(2025, 365).unwrap(), &periods).unwrap();
        assert!(!window.is_complete());
        // January and February 2025 are uncovered.
        assert_eq!(window.missing, 59);
        assert_eq!(window.required(), 365);
    }
}
