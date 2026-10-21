//! ISO-8601 date helpers shared by WASM bindings.
//!
//! Thin shims over the core grammar (`finstack_quant_core::dates::parse_iso_date`,
//! strict `YYYY-MM-DD`) so every binding reports the same error for the same
//! input; formatting is `finstack_quant_core::dates::Date`'s ISO `Display`.

use finstack_quant_core::dates::Date;
use wasm_bindgen::JsValue;

use super::to_js_err;

/// Parse a strict ISO-8601 calendar date (`"YYYY-MM-DD"`) into a [`finstack_quant_core::dates::Date`].
pub fn parse_iso_date(s: &str) -> Result<Date, JsValue> {
    finstack_quant_core::dates::parse_iso_date(s).map_err(to_js_err)
}

/// Format a [`finstack_quant_core::dates::Date`] as `"YYYY-MM-DD"`.
pub fn date_to_iso(d: Date) -> String {
    d.to_string()
}

/// Parse a slice of ISO date strings.
pub fn parse_iso_dates(date_strs: &[String]) -> Result<Vec<Date>, JsValue> {
    date_strs.iter().map(|s| parse_iso_date(s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    #[test]
    fn parse_and_format_round_trip() {
        let d = Date::from_calendar_date(2024, Month::March, 15).unwrap();
        assert_eq!(date_to_iso(d), "2024-03-15");
        assert_eq!(parse_iso_date("2024-03-15").unwrap(), d);
    }

    #[test]
    fn rejects_non_iso_input() {
        assert!(parse_iso_date("15/03/2024").is_err());
        assert!(parse_iso_date("2024-3-15").is_err());
    }
}
