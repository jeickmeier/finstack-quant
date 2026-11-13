//! Period grammar, range parsing, and stepping.

use super::*;

/// How an identifier's calendar context is decided while parsing.
#[derive(Clone, Copy, Debug)]
pub(super) enum IdContext {
    /// A standalone identifier: `FY` text is fiscal and checked against
    /// symbolic capacities only; unprefixed text is Gregorian and checked
    /// against its actual bounds.
    Standalone,
    /// An identifier read by a builder for `calendar`: a Gregorian builder
    /// rejects `FY` text, a fiscal builder reads prefixed and unprefixed text
    /// as fiscal (record CORE-CONVENTIONS-014 #4).
    Builder(PeriodCalendar),
}

pub(super) fn parse_range(s: &str, context: IdContext) -> crate::Result<(PeriodId, PeriodId)> {
    let s = s.trim();
    let (lhs, rhs_raw) = s
        .split_once("..")
        .ok_or_else(|| invalid_period(s, "range is missing the '..' separator"))?;
    if rhs_raw.contains("..") {
        return Err(invalid_period(s, "range has more than one '..' separator"));
    }
    let start = parse_id(lhs, context)?;
    let rhs = rhs_raw.trim().to_ascii_uppercase();
    // A relative end is exactly one designator of the start's kind followed by
    // an unsigned index; anything else is an absolute identifier.
    let relative = rhs
        .chars()
        .next()
        .and_then(designator_kind)
        .filter(|_| rhs.len() > 1 && rhs[1..].bytes().all(|b| b.is_ascii_digit()));
    let end = match relative {
        Some(kind) => {
            if kind != start.kind {
                return Err(invalid_period(
                    s,
                    &format!(
                        "relative range end '{rhs}' must use the start's {} designator",
                        start.kind
                    ),
                ));
            }
            let index = parse_index(&rhs[1..], s)?;
            let end = PeriodId { index, ..start };
            validate_in_context(end, context, s)?;
            end
        }
        None if start.kind == PeriodKind::Annual
            && rhs.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && !rhs.starts_with("FY") =>
        {
            return Err(invalid_period(
                s,
                "annual ranges need an absolute end year (for example 2024..2026)",
            ));
        }
        None => parse_id(&rhs, context)?,
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
    if start.fiscal != end.fiscal {
        return Err(invalid_period(
            s,
            "range start and end must use the same fiscal or Gregorian identifiers",
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

/// Kind named by a period designator letter.
fn designator_kind(c: char) -> Option<PeriodKind> {
    match c {
        'D' => Some(PeriodKind::Daily),
        'W' => Some(PeriodKind::Weekly),
        'M' => Some(PeriodKind::Monthly),
        'Q' => Some(PeriodKind::Quarterly),
        'H' => Some(PeriodKind::SemiAnnual),
        _ => None,
    }
}

/// Parse an unsigned decimal index of at least 1.
fn parse_index(raw: &str, whole: &str) -> crate::Result<u16> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid_period(
            whole,
            "index must be an unsigned decimal number",
        ));
    }
    let index: u16 = raw
        .parse()
        .map_err(|_| invalid_period(whole, "index is out of range"))?;
    if index == 0 {
        return Err(invalid_period(whole, "index must be at least 1"));
    }
    Ok(index)
}

/// Check an identifier against its context: actual Gregorian bounds, or the
/// symbolic capacity of a fiscal kind (actual fiscal validity is checked when
/// the identifier is resolved under its configuration).
fn validate_in_context(id: PeriodId, context: IdContext, whole: &str) -> crate::Result<()> {
    if id.fiscal {
        if id.index > id.kind.capacity() {
            return Err(invalid_period(
                whole,
                &format!(
                    "fiscal {} index must be in 1..={}",
                    id.kind,
                    id.kind.capacity()
                ),
            ));
        }
        if let IdContext::Builder(calendar @ PeriodCalendar::Fiscal(_)) = context {
            calendar.bounds(id.year, id.kind, id.index)?;
        }
        return Ok(());
    }
    PeriodCalendar::Gregorian.bounds(id.year, id.kind, id.index)?;
    Ok(())
}

/// Parse one period identifier under `context`.
///
/// Grammar (after outer trimming, ASCII case-insensitive): an optional `FY`,
/// an ASCII signed decimal year that fits `i32`, then either nothing (annual)
/// or exactly one of `D`/`W`/`M`/`Q`/`H` followed by an unsigned decimal index
/// of at least 1 (record CORE-CONVENTIONS-014 #5).
pub(super) fn parse_id(s: &str, context: IdContext) -> crate::Result<PeriodId> {
    let whole = s.trim();
    let upper = whole.to_ascii_uppercase();
    let (prefixed, rest) = match upper.strip_prefix("FY") {
        Some(rest) => (true, rest),
        None => (false, upper.as_str()),
    };
    let sign = usize::from(rest.starts_with(['+', '-']));
    let digits = rest[sign..].bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return Err(invalid_period(
            whole,
            "year must be an ASCII signed decimal number",
        ));
    }
    let year: i32 = rest[..sign + digits]
        .parse()
        .map_err(|_| invalid_period(whole, "year does not fit a 32-bit integer"))?;
    let tail = &rest[sign + digits..];
    let (kind, index) = match tail.chars().next() {
        None => (PeriodKind::Annual, 1),
        Some(c) => {
            let kind = designator_kind(c)
                .ok_or_else(|| invalid_period(whole, "unrecognized period designator"))?;
            (kind, parse_index(&tail[1..], whole)?)
        }
    };
    let fiscal = match context {
        IdContext::Standalone => prefixed,
        IdContext::Builder(PeriodCalendar::Gregorian) => {
            if prefixed {
                return Err(invalid_period(
                    whole,
                    "fiscal (FY) identifiers need a fiscal builder with an explicit FiscalConfig",
                ));
            }
            false
        }
        IdContext::Builder(PeriodCalendar::Fiscal(_)) => true,
    };
    let id = PeriodId {
        year,
        index,
        kind,
        fiscal,
    };
    validate_in_context(id, context, whole)?;
    Ok(id)
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

pub(super) fn checked_year_offset(year: i32, offset: i32) -> crate::Result<i32> {
    year.checked_add(offset).ok_or_else(|| {
        invalid_period(
            &year.to_string(),
            "year stepping exceeds the supported integer range",
        )
    })
}

/// Step one period forward or backward using the year capacity of `calendar`.
pub(super) fn step(
    mut id: PeriodId,
    calendar: PeriodCalendar,
    forward: bool,
) -> crate::Result<PeriodId> {
    let max = calendar.max_index(id.year, id.kind)?;
    if id.index == 0 || id.index > max {
        return Err(invalid_period(
            &id.to_string(),
            "period index is outside this year's range",
        ));
    }
    if forward {
        if id.index >= max {
            id.year = checked_year_offset(id.year, 1)?;
            id.index = 1;
        } else {
            id.index += 1;
        }
    } else if id.index == 1 {
        id.year = checked_year_offset(id.year, -1)?;
        id.index = calendar.max_index(id.year, id.kind)?;
    } else {
        id.index -= 1;
    }
    Ok(id)
}
