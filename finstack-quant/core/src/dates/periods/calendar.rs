//! Calendar and fiscal period bounds.

use super::*;

pub(super) trait PeriodCalendar {
    fn bounds(&self, year: i32, kind: PeriodKind, index: u16) -> crate::Result<(Date, Date)>;
    fn max_index(&self, year: i32, kind: PeriodKind) -> crate::Result<u16>;
    fn is_fiscal(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Gregorian;

impl PeriodCalendar for Gregorian {
    fn bounds(&self, year: i32, kind: PeriodKind, index: u16) -> crate::Result<(Date, Date)> {
        kind.gregorian_bounds(year, index)
    }

    fn max_index(&self, year: i32, kind: PeriodKind) -> crate::Result<u16> {
        Ok(kind.max_index_for_year(year))
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct FiscalCalendar {
    pub(super) config: FiscalConfig,
}

impl PeriodCalendar for FiscalCalendar {
    fn bounds(&self, year: i32, kind: PeriodKind, index: u16) -> crate::Result<(Date, Date)> {
        kind.fiscal_bounds(year, index, self.config)
    }

    fn max_index(&self, year: i32, kind: PeriodKind) -> crate::Result<u16> {
        let days = (fiscal_year_start(year + 1, self.config)?
            - fiscal_year_start(year, self.config)?)
        .whole_days() as u16;
        Ok(match kind {
            PeriodKind::Daily => days,
            PeriodKind::Weekly => days.div_ceil(7),
            _ => kind.relative_max_index(),
        })
    }

    fn is_fiscal(&self) -> bool {
        true
    }
}

/// Generic builder using a provided calendar policy.
pub(super) fn build_periods_with_calendar<C: PeriodCalendar>(
    range: &str,
    calendar: C,
    actuals_until: Option<&str>,
) -> crate::Result<PeriodPlan> {
    let (start, end) = parse_range_with_calendar(range, &calendar)?;
    let mut ids = enumerate_ids(start, end, &calendar)?;

    let actual_cut = actuals_until
        .map(|value| parse_id_with_calendar(value, &calendar))
        .transpose()?;
    let periods = ids
        .drain(..)
        .map(|pid| make_period_with_calendar(pid, &calendar, actual_cut.as_ref()))
        .collect::<crate::Result<Vec<_>>>()?;
    Ok(PeriodPlan { periods })
}

fn make_period_with_calendar<C: PeriodCalendar>(
    pid: PeriodId,
    calendar: &C,
    cut: Option<&PeriodId>,
) -> crate::Result<Period> {
    let (start, end) = calendar.bounds(pid.year, pid.kind, pid.index)?;
    let is_actual = cut.map(|c| pid <= *c).unwrap_or(false);
    Ok(Period {
        id: pid,
        start,
        end,
        is_actual,
    })
}

// Period bounds helpers are fallible to avoid sentinel dates and silent corruption.

pub(super) fn daily_bounds(year: i32, ordinal: u16) -> crate::Result<(Date, Date)> {
    use time::Duration;
    let start = Date::from_ordinal_date(year, ordinal).map_err(|_| {
        invalid_period(
            &format!("{year}D{ordinal}"),
            &format!("ordinal must be in 1..={}", days_in_year(year)),
        )
    })?;
    let end = start + Duration::days(1);
    Ok((start, end))
}

pub(super) fn quarter_bounds(year: i32, q: u8) -> crate::Result<(Date, Date)> {
    let (sm, em) = match q {
        1 => (Month::January, Month::April),
        2 => (Month::April, Month::July),
        3 => (Month::July, Month::October),
        _ => (Month::October, Month::January),
    };
    let start = crate::dates::create_date(year, sm, 1)?;
    let end_year = if q == 4 { year + 1 } else { year };
    let end = crate::dates::create_date(end_year, em, 1)?;
    Ok((start, end))
}

pub(super) fn month_bounds(year: i32, m: u8) -> crate::Result<(Date, Date)> {
    let sm = Month::try_from(m)
        .map_err(|_| invalid_period(&format!("{year}M{m:02}"), "month must be in 1..=12"))?;
    let start = crate::dates::create_date(year, sm, 1)?;
    let (ey, em) = if m == 12 {
        (year + 1, Month::January)
    } else {
        (
            year,
            Month::try_from(m + 1).map_err(|_| {
                invalid_period(&format!("{year}M{m:02}"), "month must be in 1..=12")
            })?,
        )
    };
    let end = crate::dates::create_date(ey, em, 1)?;
    Ok((start, end))
}

pub(super) fn iso_weeks_in_year(year: i32) -> u8 {
    use time::Weekday;

    if Date::from_iso_week_date(year, 53, Weekday::Monday).is_ok() {
        53
    } else {
        52
    }
}

/// Calculate ISO 8601 week bounds for a given ISO week-year and week number.
pub(super) fn week_bounds(year: i32, w: u8) -> crate::Result<(Date, Date)> {
    use time::Duration;
    use time::Weekday;

    let weeks = iso_weeks_in_year(year);
    if w == 0 || w > weeks {
        return Err(invalid_period(
            &format!("{year}W{w:02}"),
            &format!("ISO week must be in 1..={weeks} for {year}"),
        ));
    }
    let start = Date::from_iso_week_date(year, w, Weekday::Monday).map_err(|_| {
        invalid_period(
            &format!("{year}W{w:02}"),
            "ISO week-year out of the supported date range",
        )
    })?;
    let end = start + Duration::days(7);
    Ok((start, end))
}

pub(super) fn half_bounds(year: i32, h: u8) -> crate::Result<(Date, Date)> {
    let jan1 = crate::dates::create_date(year, Month::January, 1)?;
    let jul1 = crate::dates::create_date(year, Month::July, 1)?;
    let jan1_next = crate::dates::create_date(year + 1, Month::January, 1)?;
    match h {
        1 => Ok((jan1, jul1)),
        _ => Ok((jul1, jan1_next)),
    }
}

pub(super) fn annual_bounds(year: i32) -> crate::Result<(Date, Date)> {
    let start = crate::dates::create_date(year, Month::January, 1)?;
    let end = crate::dates::create_date(year + 1, Month::January, 1)?;
    Ok((start, end))
}

pub(super) fn fiscal_daily_bounds(
    fiscal_year: i32,
    ordinal: u16,
    config: FiscalConfig,
) -> crate::Result<(Date, Date)> {
    use time::Duration;

    if ordinal == 0 {
        return Err(invalid_period(
            &format!("FY{fiscal_year}D{ordinal}"),
            "fiscal day ordinal must be at least 1",
        ));
    }
    let fy_start = fiscal_year_start(fiscal_year, config)?;
    let fy_end = fiscal_year_start(fiscal_year + 1, config)?;
    let start = fy_start + Duration::days(i64::from(ordinal - 1));
    if start >= fy_end {
        return Err(invalid_period(
            &format!("FY{fiscal_year}D{ordinal}"),
            &format!(
                "fiscal day ordinal exceeds the {} days in FY{fiscal_year}",
                (fy_end - fy_start).whole_days()
            ),
        ));
    }
    Ok((start, (start + Duration::days(1)).min(fy_end)))
}

pub(super) fn fiscal_quarter_bounds(
    fiscal_year: i32,
    q: u8,
    config: FiscalConfig,
) -> crate::Result<(Date, Date)> {
    let fy_start = fiscal_year_start(fiscal_year, config)?;

    let quarter_start_month_offset = (q - 1) * 3;
    let quarter_end_month_offset = q * 3;

    let start = fy_start.add_months(quarter_start_month_offset as i32);
    let end = fy_start.add_months(quarter_end_month_offset as i32);

    Ok((start, end))
}

pub(super) fn fiscal_month_bounds(
    fiscal_year: i32,
    m: u8,
    config: FiscalConfig,
) -> crate::Result<(Date, Date)> {
    let fy_start = fiscal_year_start(fiscal_year, config)?;

    let start = fy_start.add_months((m - 1) as i32);
    let end = fy_start.add_months(m as i32);

    Ok((start, end))
}

/// Calculate fiscal week bounds using simple fiscal year start anchoring.
///
/// Like regular week_bounds, this uses simple 7-day blocks starting from the
/// fiscal year start date, not ISO 8601 week numbering.
pub(super) fn fiscal_week_bounds(
    fiscal_year: i32,
    w: u8,
    config: FiscalConfig,
) -> crate::Result<(Date, Date)> {
    use time::Duration;

    let fy_start = fiscal_year_start(fiscal_year, config)?;
    let fy_end = fiscal_year_start(fiscal_year + 1, config)?;

    let start = fy_start + Duration::days(((w - 1) as i64) * 7);
    if start >= fy_end {
        return Err(invalid_period(
            &format!("FY{fiscal_year}W{w:02}"),
            &format!("fiscal week starts after the end of FY{fiscal_year}"),
        ));
    }
    let end = (start + Duration::days(7)).min(fy_end);

    Ok((start, end))
}

pub(super) fn fiscal_half_bounds(
    fiscal_year: i32,
    h: u8,
    config: FiscalConfig,
) -> crate::Result<(Date, Date)> {
    let fy_start = fiscal_year_start(fiscal_year, config)?;

    let half_start_month_offset = (h - 1) * 6;
    let half_end_month_offset = h * 6;

    let start = fy_start.add_months(half_start_month_offset as i32);
    let end = fy_start.add_months(half_end_month_offset as i32);

    Ok((start, end))
}

pub(super) fn fiscal_annual_bounds(
    fiscal_year: i32,
    config: FiscalConfig,
) -> crate::Result<(Date, Date)> {
    let start = fiscal_year_start(fiscal_year, config)?;
    let end = fiscal_year_start(fiscal_year + 1, config)?;
    Ok((start, end))
}

/// Calculate the start date of a fiscal year
fn fiscal_year_start(fiscal_year: i32, config: FiscalConfig) -> crate::Result<Date> {
    let calendar_year = if config.start_month == 1 {
        fiscal_year
    } else {
        // Non-January fiscal years start in the previous calendar year
        // (FY2025 starting Oct 1 begins 2024-10-01).
        fiscal_year - 1
    };

    let month = Month::try_from(config.start_month).map_err(|_| {
        crate::Error::Validation(format!(
            "fiscal start_month must be in 1..=12, got {}",
            config.start_month
        ))
    })?;
    match crate::dates::create_date(calendar_year, month, config.start_day) {
        Ok(d) => Ok(d),
        Err(_) => {
            // If the day doesn't exist (e.g., Feb 30), use the last day of the month.
            let last_day = month.length(calendar_year);
            crate::dates::create_date(calendar_year, month, last_day)
        }
    }
}
