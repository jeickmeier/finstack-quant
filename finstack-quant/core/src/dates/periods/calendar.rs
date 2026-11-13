//! Calendar and fiscal period bounds.

use super::*;

/// Calendar context that bounds and steps period identifiers.
///
/// An unprefixed identifier belongs to `Gregorian`, an `FY` identifier to
/// `Fiscal(config)`. A [`PeriodPlan`] retains its calendar once; operations
/// that resolve or step an identifier take this context explicitly (record
/// CORE-CONVENTIONS-014 #4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PeriodCalendar {
    /// Gregorian years with ISO 8601 weeks.
    Gregorian,
    /// Fiscal years opening on the configured month and day.
    Fiscal(FiscalConfig),
}

impl PeriodCalendar {
    /// Whether this is a fiscal calendar.
    #[must_use]
    pub fn is_fiscal(self) -> bool {
        matches!(self, Self::Fiscal(_))
    }

    /// The fiscal configuration, or `None` for the Gregorian calendar.
    #[must_use]
    pub fn fiscal_config(self) -> Option<FiscalConfig> {
        match self {
            Self::Gregorian => None,
            Self::Fiscal(config) => Some(config),
        }
    }

    fn year_start(self, year: i32) -> crate::Result<Date> {
        match self {
            Self::Gregorian => crate::dates::create_date(year, Month::January, 1),
            Self::Fiscal(config) => config.year_start(year),
        }
    }

    /// Highest valid index of `kind` in `year`.
    pub(super) fn max_index(self, year: i32, kind: PeriodKind) -> crate::Result<u16> {
        Ok(match (self, kind) {
            (Self::Gregorian, PeriodKind::Daily) => days_in_year(year),
            (Self::Gregorian, PeriodKind::Weekly) => u16::from(iso_weeks_in_year(year)),
            (Self::Fiscal(_), PeriodKind::Daily) => {
                (self.year_start(checked_year_offset(year, 1)?)? - self.year_start(year)?)
                    .whole_days() as u16
            }
            _ => kind.capacity(),
        })
    }

    /// Inclusive start and exclusive end of the `index`-th `kind` period of `year`.
    pub(super) fn bounds(
        self,
        year: i32,
        kind: PeriodKind,
        index: u16,
    ) -> crate::Result<(Date, Date)> {
        match (self, kind) {
            (Self::Gregorian, PeriodKind::Daily) => daily_bounds(year, index),
            (Self::Gregorian, PeriodKind::Weekly) => week_bounds(year, index),
            (Self::Fiscal(config), PeriodKind::Daily) => fiscal_daily_bounds(year, index, config),
            (Self::Fiscal(config), PeriodKind::Weekly) => fiscal_week_bounds(year, index, config),
            _ => self.month_block_bounds(year, kind, index),
        }
    }

    /// Bounds of the `index`-th equal block of 1, 3, 6 or 12 months in `year`.
    fn month_block_bounds(
        self,
        year: i32,
        kind: PeriodKind,
        index: u16,
    ) -> crate::Result<(Date, Date)> {
        let count = kind.periods_per_year();
        if !(1..=count).contains(&index) {
            return Err(invalid_period(
                &format!("{kind} {year} index {index}"),
                &format!("{kind} index must be in 1..={count}"),
            ));
        }
        let months = i32::from(12 / count);
        let year_start = self.year_start(year)?;
        let start = year_start.add_months((i32::from(index) - 1) * months)?;
        let end = if index == count {
            self.year_start(checked_year_offset(year, 1)?)?
        } else {
            year_start.add_months(i32::from(index) * months)?
        };
        Ok((start, end))
    }
}

/// Build the contiguous period plan for `range` under `calendar`.
pub(super) fn build_periods_with_calendar(
    range: &str,
    calendar: PeriodCalendar,
    actuals_until: Option<&str>,
) -> crate::Result<PeriodPlan> {
    let context = IdContext::Builder(calendar);
    let (start, end) = parse_range(range, context)?;
    // Reject unsupported dates and excessive work before allocating any periods.
    calendar.bounds(start.year, start.kind, start.index)?;
    calendar.bounds(end.year, end.kind, end.index)?;
    let actual_cut = actuals_until
        .map(|value| parse_id(value, context))
        .transpose()?;
    if let Some(cut) = actual_cut {
        if cut.kind != start.kind || cut.fiscal != start.fiscal {
            return Err(invalid_period(
                actuals_until.unwrap_or_default(),
                "actuals cutoff must use the same period kind and calendar as the range",
            ));
        }
        calendar.bounds(cut.year, cut.kind, cut.index)?;
    }
    let mut count = 0usize;
    for year in start.year..=end.year {
        let first = if year == start.year { start.index } else { 1 };
        let last = if year == end.year {
            end.index
        } else {
            calendar.max_index(year, start.kind)?
        };
        count += usize::from(last - first + 1);
        if count > MAX_PERIODS {
            return Err(invalid_period(
                range,
                "range exceeds the 100000-period limit",
            ));
        }
    }
    let mut periods = Vec::with_capacity(count);
    let mut current = start;
    for index in 0..count {
        let (period_start, period_end) =
            calendar.bounds(current.year, current.kind, current.index)?;
        periods.push(Period {
            id: current,
            start: period_start,
            end: period_end,
            is_actual: actual_cut.is_some_and(|cut| current <= cut),
        });
        if index + 1 < count {
            current = current.shift(1, calendar)?;
        }
    }
    Ok(PeriodPlan { calendar, periods })
}

const MAX_PERIODS: usize = 100_000;

pub(super) use time::util::{days_in_year, weeks_in_year as iso_weeks_in_year};

fn daily_bounds(year: i32, ordinal: u16) -> crate::Result<(Date, Date)> {
    let start = Date::from_ordinal_date(year, ordinal).map_err(|_| {
        invalid_period(
            &format!("{year}D{ordinal}"),
            &format!("ordinal must be in 1..={}", days_in_year(year)),
        )
    })?;
    Ok((start, start.add_days(1)?))
}

/// ISO 8601 week bounds (Monday to Monday) for an ISO week-year.
fn week_bounds(year: i32, w: u16) -> crate::Result<(Date, Date)> {
    let weeks = iso_weeks_in_year(year);
    if w == 0 || w > u16::from(weeks) {
        return Err(invalid_period(
            &format!("{year}W{w:02}"),
            &format!("ISO week must be in 1..={weeks} for {year}"),
        ));
    }
    let start = Date::from_iso_week_date(year, w as u8, time::Weekday::Monday).map_err(|_| {
        invalid_period(
            &format!("{year}W{w:02}"),
            "ISO week-year out of the supported date range",
        )
    })?;
    Ok((start, start.add_days(7)?))
}

fn fiscal_daily_bounds(
    fiscal_year: i32,
    ordinal: u16,
    config: FiscalConfig,
) -> crate::Result<(Date, Date)> {
    if ordinal == 0 {
        return Err(invalid_period(
            &format!("FY{fiscal_year}D{ordinal}"),
            "fiscal day ordinal must be at least 1",
        ));
    }
    let fy_start = config.year_start(fiscal_year)?;
    let fy_end = config.year_start(checked_year_offset(fiscal_year, 1)?)?;
    let start = fy_start.add_days(i64::from(ordinal - 1))?;
    if start >= fy_end {
        return Err(invalid_period(
            &format!("FY{fiscal_year}D{ordinal}"),
            &format!(
                "fiscal day ordinal exceeds the {} days in FY{fiscal_year}",
                (fy_end - fy_start).whole_days()
            ),
        ));
    }
    Ok((start, start.add_days(1)?))
}

/// Fiscal week bounds as 7-day blocks from the fiscal-year start.
///
/// Unlike Gregorian [`week_bounds`] (ISO 8601 weeks), the final block is
/// truncated at the next fiscal-year start, so week 53 is partial.
fn fiscal_week_bounds(
    fiscal_year: i32,
    w: u16,
    config: FiscalConfig,
) -> crate::Result<(Date, Date)> {
    if w == 0 {
        return Err(invalid_period(
            &format!("FY{fiscal_year}W{w:02}"),
            "fiscal week must be at least 1",
        ));
    }
    let fy_start = config.year_start(fiscal_year)?;
    let fy_end = config.year_start(checked_year_offset(fiscal_year, 1)?)?;
    let start = fy_start.add_days(i64::from(w - 1) * 7)?;
    if start >= fy_end {
        return Err(invalid_period(
            &format!("FY{fiscal_year}W{w:02}"),
            &format!("fiscal week starts after the end of FY{fiscal_year}"),
        ));
    }
    let end = start.add_days(7.min((fy_end - start).whole_days()))?;
    Ok((start, end))
}
