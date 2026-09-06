//! Period grammar, range parsing, and stepping.

use super::*;

pub(super) fn parse_range_with_calendar<C: PeriodCalendar>(
    s: &str,
    calendar: &C,
) -> crate::Result<(PeriodId, PeriodId)> {
    let s = s.trim();
    let (lhs, rhs_raw) = s
        .split_once("..")
        .ok_or_else(|| invalid_period(s, "range is missing the '..' separator"))?;
    let start = parse_id_with_calendar(lhs, calendar)?;
    let rhs_raw = rhs_raw.trim();
    let rhs_upper = rhs_raw.to_ascii_uppercase();
    let rhs = rhs_upper.as_str();
    // Relative if RHS is a bare designator (Q/M/W/H/A). Absolute forms start
    // with a Gregorian year or the explicit fiscal-year `FY` marker.
    let end = if rhs.starts_with("FY")
        || rhs
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
    {
        parse_id_with_calendar(rhs, calendar)?
    } else {
        // relative form like "..D100" / "..Q4" / "..M12" / "..W52" / "..H2" / "..A"
        let designator = start.kind.designator().ok_or_else(|| {
            invalid_period(
                s,
                "annual ranges need an absolute end year (for example 2024..2026)",
            )
        })?;
        let index = start.kind.parse_index_with_limit(
            rhs.trim_start_matches(designator),
            calendar.max_index(start.year, start.kind)?,
        )?;
        if start.fiscal {
            start.kind.build_fiscal_id(start.year, index)
        } else {
            start.kind.build_id(start.year, index)
        }
    };
    if start.kind != end.kind {
        return Err(invalid_period(
            s,
            &format!(
                "range start ({}) and end ({}) must share one period kind",
                start.kind, end.kind
            ),
        ));
    }
    if start > end {
        return Err(invalid_period(
            s,
            &format!("range start {start} is after end {end}"),
        ));
    }
    Ok((start, end))
}

fn parse_designated_id<C: PeriodCalendar>(
    s: &str,
    split_index: usize,
    kind: PeriodKind,
    calendar: &C,
) -> crate::Result<PeriodId> {
    let explicit_fiscal = s.starts_with("FY");
    let fiscal = explicit_fiscal || calendar.is_fiscal();
    let year_raw = s[..split_index]
        .strip_prefix("FY")
        .unwrap_or(&s[..split_index]);
    let year: i32 = year_raw
        .parse()
        .map_err(|_| invalid_period(s, "year is not a valid integer"))?;
    let max_index = if explicit_fiscal {
        kind.relative_max_index()
    } else {
        calendar.max_index(year, kind)?
    };
    let index = kind.parse_index_with_limit(&s[split_index + 1..], max_index)?;
    Ok(if fiscal {
        kind.build_fiscal_id(year, index)
    } else {
        kind.build_id(year, index)
    })
}

pub(super) fn parse_id(s: &str) -> crate::Result<PeriodId> {
    parse_id_with_calendar(s, &Gregorian)
}

pub(super) fn parse_id_with_calendar<C: PeriodCalendar>(
    s: &str,
    calendar: &C,
) -> crate::Result<PeriodId> {
    let s = s.trim();
    // Normalize to uppercase to accept lowercase inputs.
    let s = s.to_ascii_uppercase();
    let s = s.as_str();
    if let Some(i) = s.find('D') {
        return parse_designated_id(s, i, PeriodKind::Daily, calendar);
    }
    if let Some(i) = s.find('Q') {
        return parse_designated_id(s, i, PeriodKind::Quarterly, calendar);
    }
    if let Some(i) = s.find('M') {
        return parse_designated_id(s, i, PeriodKind::Monthly, calendar);
    }
    if let Some(i) = s.find('W') {
        return parse_designated_id(s, i, PeriodKind::Weekly, calendar);
    }
    if let Some(i) = s.find('H') {
        return parse_designated_id(s, i, PeriodKind::SemiAnnual, calendar);
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        // annual
        let year: i32 = s
            .parse()
            .map_err(|_| invalid_period(s, "year is not a valid integer"))?;
        return Ok(if calendar.is_fiscal() {
            PeriodKind::Annual.build_fiscal_id(year, 1)
        } else {
            PeriodId::annual(year)
        });
    }
    if let Some(year) = s.strip_prefix("FY") {
        let year: i32 = year
            .parse()
            .map_err(|_| invalid_period(s, "fiscal year is not a valid integer"))?;
        return Ok(PeriodKind::Annual.build_fiscal_id(year, 1));
    }
    Err(invalid_period(s, "unrecognized period id"))
}

/// Accepted period-id and range grammar, appended to every parse error.
const PERIOD_GRAMMAR: &str = "accepted forms are YYYY, YYYYQn, YYYYMmm, YYYYHn, YYYYWww, \
     YYYYDddd (e.g. 2024Q1, 2024M01), fiscal FY2024Q1, and ranges such as 2024Q1..Q4, \
     2024M01..2025M12 or FY2024Q1..Q4";

/// Build the validation error for a malformed period id or range.
///
/// # Arguments
///
/// * `value` - The offending input exactly as the caller supplied it.
/// * `reason` - Short explanation of what was wrong with `value`.
pub(super) fn invalid_period(value: &str, reason: &str) -> crate::Error {
    crate::Error::Validation(format!(
        "invalid period '{value}': {reason}; {PERIOD_GRAMMAR}"
    ))
}

pub(super) fn enumerate_ids<C: PeriodCalendar>(
    mut cur: PeriodId,
    end: PeriodId,
    calendar: &C,
) -> crate::Result<Vec<PeriodId>> {
    let mut out = Vec::new();
    while cur <= end {
        out.push(cur);
        let max = calendar.max_index(cur.year, cur.kind)?;
        if cur.index >= max {
            cur.year += 1;
            cur.index = 1;
        } else {
            cur.index += 1;
        }
    }
    Ok(out)
}

pub(super) fn days_in_year(year: i32) -> u16 {
    if time::util::is_leap_year(year) {
        366
    } else {
        365
    }
}

pub(super) fn step(mut id: PeriodId) -> crate::Result<PeriodId> {
    (id.year, id.index) = id.kind.step_forward(id.year, id.index);
    Ok(id)
}

/// Step backward by one period (inverse of step).
pub(super) fn step_backward(mut id: PeriodId) -> crate::Result<PeriodId> {
    (id.year, id.index) = id.kind.step_backward(id.year, id.index);
    Ok(id)
}

pub(super) fn step_with_calendar<C: PeriodCalendar>(
    mut id: PeriodId,
    calendar: &C,
    forward: bool,
) -> crate::Result<PeriodId> {
    if forward {
        let max = calendar.max_index(id.year, id.kind)?;
        if id.index >= max {
            id.year += 1;
            id.index = 1;
        } else {
            id.index += 1;
        }
    } else if id.index == 1 {
        id.year -= 1;
        id.index = calendar.max_index(id.year, id.kind)?;
    } else {
        id.index -= 1;
    }
    Ok(id)
}
