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
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::market::quotes::ids::QuoteId;
///
/// let id = QuoteId::new("USD-SOFR-DEP-1M");
/// assert_eq!(id.as_str(), "USD-SOFR-DEP-1M");
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord, schemars::JsonSchema,
)]
pub struct QuoteId(String);

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
    /// use finstack_quant_valuations::market::quotes::ids::QuoteId;
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
    /// use finstack_quant_valuations::market::quotes::ids::QuoteId;
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
/// # Examples
///
/// Using a tenor pillar:
/// ```rust
/// use finstack_quant_valuations::market::quotes::ids::Pillar;
///
/// # fn example() -> finstack_quant_core::Result<()> {
/// let pillar = Pillar::Tenor("5Y".parse()?);
/// # Ok(())
/// # }
/// ```
///
/// Using a date pillar:
/// ```rust
/// use finstack_quant_valuations::market::quotes::ids::Pillar;
/// use finstack_quant_core::dates::Date;
///
/// let pillar = Pillar::Date(Date::from_calendar_date(2029, time::Month::June, 20).unwrap());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
    Date(#[schemars(with = "String")] Date),
}

impl fmt::Display for Pillar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Pillar::Tenor(t) => write!(f, "{}", t),
            Pillar::Date(d) => write!(f, "{}", d),
        }
    }
}
