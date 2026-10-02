//! Quote identifiers and pillar types.

use finstack_quant_core::dates::Tenor;
use serde::{Deserialize, Serialize};
use std::fmt;
use time::Date;

/// A stable identifier for a market quote (e.g., "USD-OIS-SWAP-5Y").
///
/// This ID is used for human readability, logging, and potentially matching against external
/// data sources. Quote IDs should be unique within a calibration set and follow a consistent
/// naming convention (e.g., "{currency}-{index}-{type}-{pillar}").
///
/// [`QuoteId::new`] is infallible, including for empty strings. Empty or
/// whitespace-only IDs are rejected when this type is deserialized from the
/// wire, and again by [`MarketQuote::validate`](super::market_quote::MarketQuote::validate).
///
/// # Examples
///
/// ```rust
/// use finstack_quant_calibration::quotes::ids::QuoteId;
///
/// let id = QuoteId::new("USD-SOFR-DEP-1M");
/// assert_eq!(id.as_str(), "USD-SOFR-DEP-1M");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, PartialOrd, Ord)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(transparent))]
pub struct QuoteId(
    #[cfg_attr(
        feature = "json-schema",
        schemars(length(min = 1), regex(pattern = r".*\S.*"))
    )]
    String,
);

/// Shared deserialize/validate message for empty or whitespace-only quote ids.
pub(crate) const EMPTY_QUOTE_ID: &str = "quote id must not be empty or whitespace";

impl<'de> Deserialize<'de> for QuoteId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value.trim().is_empty() {
            return Err(serde::de::Error::custom(EMPTY_QUOTE_ID));
        }
        Ok(Self(value))
    }
}

impl QuoteId {
    /// Create a new `QuoteId` from a string.
    ///
    /// # Arguments
    ///
    /// * `s` - The identifier string (e.g., "USD-OIS-SWAP-5Y")
    ///
    /// # Returns
    ///
    /// A new `QuoteId` instance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_calibration::quotes::ids::QuoteId;
    ///
    /// let id = QuoteId::new("USD-SOFR-DEP-1M");
    /// ```
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// View the inner string representation.
    ///
    /// # Returns
    ///
    /// A string slice containing the identifier.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_calibration::quotes::ids::QuoteId;
    ///
    /// let id = QuoteId::new("USD-SOFR-DEP-1M");
    /// assert_eq!(id.as_str(), "USD-SOFR-DEP-1M");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for QuoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for QuoteId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for QuoteId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// The maturity pillar of a quote.
///
/// The pillar represents the maturity of the instrument referenced by the quote. OTC instruments
/// (swaps, deposits) typically use `Tenor` (e.g., "5Y") to allow rolling headers that automatically
/// adjust as the valuation date changes. Futures or bespoke runs may use `Date` to pin a specific
/// maturity date.
///
/// The JSON wire form is externally tagged: `{"tenor": {"count": 5, "unit": "years"}}`
/// or `{"date": "2029-06-20"}`. A bare string such as `"5Y"` is rejected.
///
/// # Examples
///
/// Using a tenor pillar:
/// ```rust
/// use finstack_quant_calibration::quotes::ids::Pillar;
///
/// # fn example() -> finstack_quant_core::Result<()> {
/// let pillar = Pillar::Tenor("5Y".parse()?);
/// # Ok(())
/// # }
/// ```
///
/// Using a date pillar:
/// ```rust
/// use finstack_quant_calibration::quotes::ids::Pillar;
/// use finstack_quant_core::dates::Date;
///
/// let pillar = Pillar::Date(Date::from_calendar_date(2029, time::Month::June, 20).unwrap());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Pillar {
    /// A relative tenor (e.g., 5Y, 3M).
    ///
    /// The maturity is calculated relative to the valuation date, allowing quotes to "roll"
    /// forward automatically. This is the standard approach for OTC instruments.
    Tenor(Tenor),
    /// An absolute date.
    ///
    /// The maturity is fixed to a specific date, regardless of the valuation date. This is
    /// typically used for futures contracts or bespoke instruments with fixed maturities.
    Date(
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        Date,
    ),
}

impl std::str::FromStr for Pillar {
    type Err = finstack_quant_core::Error;

    /// Parse a pillar from a tenor string (`"3M"`, `"5Y"`) or an ISO-8601
    /// calendar date (`"2030-06-20"`).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if let Ok(tenor) = trimmed.parse::<Tenor>() {
            return Ok(Pillar::Tenor(tenor));
        }
        let format = time::macros::format_description!("[year]-[month]-[day]");
        Date::parse(trimmed, &format)
            .map(Pillar::Date)
            .map_err(|_| {
                finstack_quant_core::Error::Validation(format!(
                    "invalid pillar '{s}': expected a tenor such as '3M' or '5Y', or an ISO-8601 date"
                ))
            })
    }
}

impl fmt::Display for Pillar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Pillar::Tenor(t) => write!(f, "{}", t),
            Pillar::Date(d) => write!(f, "{}", d),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::QuoteId;

    #[test]
    fn new_accepts_empty_id() {
        let id = QuoteId::new("");
        assert_eq!(id.as_str(), "");
        assert_eq!(QuoteId::new("   ").as_str(), "   ");
    }

    #[test]
    fn deserialize_accepts_nonempty_id() {
        let id: QuoteId = serde_json::from_str("\"USD-OIS-SWAP-5Y\"").expect("valid quote id");
        assert_eq!(id.as_str(), "USD-OIS-SWAP-5Y");
    }

    #[test]
    fn deserialize_rejects_empty_id() {
        let err = serde_json::from_str::<QuoteId>("\"\"").expect_err("empty quote id");
        assert!(
            err.to_string().contains("quote id must not be empty"),
            "unexpected deserialize error: {err}"
        );
    }

    #[test]
    fn deserialize_rejects_whitespace_id() {
        let err = serde_json::from_str::<QuoteId>("\"  \\t\\n\"").expect_err("whitespace quote id");
        assert!(
            err.to_string().contains("quote id must not be empty"),
            "unexpected deserialize error: {err}"
        );
    }
}
