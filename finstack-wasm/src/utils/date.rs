//! ISO-8601 date parsing/formatting helpers shared by WASM bindings.
//!
//! Centralizing these avoids duplicate `YYYY-MM-DD` parsers across the
//! `core/market_data`, `analytics`, and other domain modules.
//!
//! [`parse_iso_date`] and [`date_to_iso`] are a **matched round-trip pair**:
//! `parse_iso_date(date_to_iso(d)) == d` holds for every [`time::Date`],
//! including negative ("BC") and year-zero dates. `time::Date` years span
//! `-9999..=9999` (no `large-dates` feature), so the year component carries an
//! optional leading sign; the parser peels that sign before splitting the
//! `YYYY-MM-DD` body on `-`, which a naïve `split('-')` would otherwise
//! mis-tokenize into four parts for a negative year.

use time::Date;
use wasm_bindgen::JsValue;

use super::to_js_err;

/// Parse an ISO date string (`"YYYY-MM-DD"`, optionally signed) into a
/// [`time::Date`].
///
/// Accepts an optional leading `+`/`-` sign on the year so it round-trips the
/// full [`date_to_iso`] output domain — including negative and year-zero
/// dates. The year, month, and day fields are each zero-padded but the field
/// widths are not otherwise constrained.
pub fn parse_iso_date(s: &str) -> Result<Date, JsValue> {
    // Peel an optional ISO-8601 sign from the year before tokenizing, so a
    // negative year (e.g. "-0044-03-15") is not mis-split into four parts.
    let (year_sign, body) = match s.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, s.strip_prefix('+').unwrap_or(s)),
    };
    let parts: Vec<&str> = body.split('-').collect();
    if parts.len() != 3 {
        return Err(to_js_err(format!("expected YYYY-MM-DD, got {s:?}")));
    }
    let year_magnitude: i32 = parts[0].parse().map_err(to_js_err)?;
    let year = year_sign * year_magnitude;
    let month_num: u8 = parts[1].parse().map_err(to_js_err)?;
    let day: u8 = parts[2].parse().map_err(to_js_err)?;
    let month = time::Month::try_from(month_num).map_err(to_js_err)?;
    Date::from_calendar_date(year, month, day).map_err(to_js_err)
}

/// Format a [`time::Date`] as `"YYYY-MM-DD"` (year signed when negative).
///
/// The year is rendered with [`i32`]'s `Display`, zero-padded to a minimum of
/// four digits; for a negative year this yields a leading `-`, e.g.
/// `-0044-03-15`. [`parse_iso_date`] understands that signed form, so the two
/// functions round-trip for every [`time::Date`].
pub fn date_to_iso(d: Date) -> String {
    let year = d.year();
    // `{:04}` is a *minimum* width: it pads the magnitude and keeps the sign,
    // producing the ISO-8601-style signed year `parse_iso_date` accepts.
    format!("{:04}-{:02}-{:02}", year, d.month() as u8, d.day())
}

/// Parse a slice of ISO date strings.
pub fn parse_iso_dates(date_strs: &[String]) -> Result<Vec<Date>, JsValue> {
    date_strs
        .iter()
        .map(|s| parse_iso_date(s))
        .collect::<Result<_, _>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    /// Parse a date via the unsigned form, asserting the calendar components.
    fn parsed(s: &str) -> Date {
        parse_iso_date(s).expect("valid ISO date")
    }

    #[test]
    fn round_trips_ordinary_date() {
        let d = parsed("2024-06-30");
        assert_eq!((d.year(), d.month(), d.day()), (2024, Month::June, 30));
        assert_eq!(date_to_iso(d), "2024-06-30");
    }

    #[test]
    fn round_trips_negative_year() {
        // A negative ("BC") year must survive `date_to_iso` -> `parse_iso_date`.
        // Naïve `split('-')` mis-tokenizes the leading sign into a 4th part.
        let d = Date::from_calendar_date(-44, Month::March, 15).expect("valid date");
        let iso = date_to_iso(d);
        assert!(iso.starts_with('-'), "negative year must be signed: {iso}");
        let back = parse_iso_date(&iso).expect("signed year must round-trip");
        assert_eq!(back, d, "round-trip failed for {iso}");
    }

    #[test]
    fn round_trips_year_zero_and_boundaries() {
        // `time::Date` (no `large-dates`) spans -9999..=9999; every endpoint
        // and year zero must round-trip through the matched pair.
        for year in [-9999_i32, -1, 0, 1, 9999] {
            let d = Date::from_calendar_date(year, Month::January, 1)
                .unwrap_or_else(|e| panic!("year {year} should be a valid Date: {e}"));
            let iso = date_to_iso(d);
            let back = parse_iso_date(&iso)
                .unwrap_or_else(|_| panic!("date_to_iso output {iso:?} must re-parse"));
            assert_eq!(back, d, "round-trip failed for year {year} (iso={iso})");
        }
    }

    #[test]
    fn accepts_explicit_plus_sign() {
        // An explicit `+` sign on the year is tolerated (ISO-8601 expanded
        // form) and parses identically to the unsigned spelling.
        assert_eq!(parse_iso_date("+2024-01-15").expect("signed"), parsed("2024-01-15"));
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(parse_iso_date("2024-01").is_err(), "too few fields must error");
        assert!(
            parse_iso_date("2024-01-15-00").is_err(),
            "too many fields must error"
        );
        assert!(parse_iso_date("not-a-date").is_err(), "non-numeric must error");
    }
}
