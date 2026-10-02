//! Inflation instrument quote types.
//!
//! Inflation instrument quotes for CPI and inflation curve calibration. Supports both
//! zero-coupon inflation swaps (ZCIS) and year-on-year (YoY) inflation swaps.

use super::ids::QuoteId;
use super::validate;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::Result;
use finstack_quant_valuations::market::conventions::ids::InflationSwapConventionId;

/// Inflation instrument quotes for CPI and inflation curve calibration.
///
/// Supports two types of inflation swaps:
/// 1. **Zero-coupon inflation swaps (ZCIS)**: Single payment at maturity based on cumulative inflation
/// 2. **Year-on-year (YoY) inflation swaps**: Periodic payments based on year-over-year inflation
///
/// # Examples
///
/// Zero-coupon inflation swap:
/// ```rust
/// use finstack_quant_calibration::quotes::inflation::InflationQuote;
/// use finstack_quant_calibration::quotes::ids::QuoteId;
/// use finstack_quant_valuations::market::conventions::ids::InflationSwapConventionId;
/// use finstack_quant_core::dates::Date;
///
/// let quote = InflationQuote::InflationSwap {
///     id: QuoteId::new("USA-CPI-U-ZCIS-5Y"),
///     maturity: Date::from_calendar_date(2029, time::Month::June, 20).unwrap(),
///     rate: 0.025, // 2.5% fixed rate
///     index: "US-CPI-U".to_string(),
///     convention: InflationSwapConventionId::new("USD-CPI"),
/// };
/// ```
///
/// Year-on-year inflation swap:
/// ```rust
/// use finstack_quant_calibration::quotes::inflation::InflationQuote;
/// use finstack_quant_calibration::quotes::ids::QuoteId;
/// use finstack_quant_valuations::market::conventions::ids::InflationSwapConventionId;
/// use finstack_quant_core::dates::{Date, Tenor};
///
/// # fn example() -> finstack_quant_core::Result<()> {
/// let quote = InflationQuote::YoYInflationSwap {
///     id: QuoteId::new("USA-CPI-U-YOY-5Y"),
///     maturity: Date::from_calendar_date(2029, time::Month::June, 20).unwrap(),
///     rate: 0.025,
///     index: "US-CPI-U".to_string(),
///     frequency: Tenor::new(1, finstack_quant_core::dates::TenorUnit::Years).expect("valid tenor fixture"),
///     convention: InflationSwapConventionId::new("USD-CPI"),
/// };
/// # Ok(())
/// # }
/// ```
// Keep this enum externally tagged. Market quote schemas, golden calibration
// payloads, and Python envelope payloads already depend on this shape.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum InflationQuote {
    /// Zero-coupon inflation swap (ZCIS) quote.
    InflationSwap {
        /// Unique identifier for the quote.
        id: QuoteId,
        /// Swap maturity
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        maturity: Date,
        /// Fixed rate (decimal)
        rate: f64,
        /// Inflation index identifier
        index: String,
        /// Per-instrument conventions
        convention: InflationSwapConventionId,
    },
    /// Year-on-year (YoY) inflation swap quote.
    #[serde(rename = "yoy_inflation_swap")]
    YoYInflationSwap {
        /// Unique identifier for the quote.
        id: QuoteId,
        /// Swap maturity
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        maturity: Date,
        /// Fixed rate (decimal)
        rate: f64,
        /// Inflation index identifier
        index: String,
        /// Payment frequency
        frequency: Tenor,
        /// Instrument-wide conventions
        convention: InflationSwapConventionId,
    },
}

impl InflationQuote {
    /// Get the unique identifier of the quote.
    pub fn id(&self) -> &QuoteId {
        match self {
            InflationQuote::InflationSwap { id, .. }
            | InflationQuote::YoYInflationSwap { id, .. } => id,
        }
    }

    /// Get maturity date for this quote if applicable.
    ///
    /// # Returns
    ///
    /// `Some(maturity_date)` for all inflation quote variants, or `None` if not applicable.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_calibration::quotes::inflation::InflationQuote;
    /// use finstack_quant_calibration::quotes::ids::QuoteId;
    /// use finstack_quant_valuations::market::conventions::ids::InflationSwapConventionId;
    /// use finstack_quant_core::dates::Date;
    ///
    /// let quote = InflationQuote::InflationSwap {
    ///     id: QuoteId::new("USA-CPI-U-ZCIS-5Y"),
    ///     maturity: Date::from_calendar_date(2029, time::Month::June, 20).unwrap(),
    ///     rate: 0.025,
    ///     index: "US-CPI-U".to_string(),
    ///     convention: InflationSwapConventionId::new("USD-CPI"),
    /// };
    ///
    /// assert_eq!(quote.maturity_date(), Some(Date::from_calendar_date(2029, time::Month::June, 20).unwrap()));
    /// ```
    pub fn maturity_date(&self) -> Option<Date> {
        match self {
            InflationQuote::InflationSwap { maturity, .. }
            | InflationQuote::YoYInflationSwap { maturity, .. } => Some(*maturity),
        }
    }

    /// Validate that the quoted inflation rate is finite.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::InflationSwap { rate, .. } | Self::YoYInflationSwap { rate, .. } => {
                validate::finite(*rate, "rate")
            }
        }
    }

    /// Create a new quote with the inflation rate bumped by a decimal amount.
    ///
    /// # Arguments
    ///
    /// * `rate_bump` - The bump amount in decimal terms (e.g., `0.0001` for 1 basis point)
    ///
    /// # Returns
    ///
    /// A new `InflationQuote` with the bumped rate.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_calibration::quotes::inflation::InflationQuote;
    /// use finstack_quant_calibration::quotes::ids::QuoteId;
    /// use finstack_quant_valuations::market::conventions::ids::InflationSwapConventionId;
    /// use finstack_quant_core::dates::Date;
    ///
    /// let quote = InflationQuote::InflationSwap {
    ///     id: QuoteId::new("USA-CPI-U-ZCIS-5Y"),
    ///     maturity: Date::from_calendar_date(2029, time::Month::June, 20).unwrap(),
    ///     rate: 0.025,
    ///     index: "US-CPI-U".to_string(),
    ///     convention: InflationSwapConventionId::new("USD-CPI"),
    /// };
    ///
    /// // Bump by 1 basis point
    /// let bumped = quote.bump_rate_decimal(0.0001);
    /// ```
    pub fn bump_rate_decimal(&self, rate_bump: f64) -> Self {
        let mut quote = self.clone();
        match &mut quote {
            Self::InflationSwap { rate, .. } | Self::YoYInflationSwap { rate, .. } => {
                *rate += rate_bump;
            }
        }
        quote
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn yoy_quote_uses_canonical_acronym_spelling() {
        let quote = InflationQuote::YoYInflationSwap {
            id: QuoteId::new("USA-CPI-U-YOY-5Y"),
            maturity: date!(2029 - 06 - 20),
            rate: 0.025,
            index: "US-CPI-U".to_string(),
            frequency: Tenor::new(1, finstack_quant_core::dates::TenorUnit::Years)
                .expect("valid tenor fixture"),
            convention: InflationSwapConventionId::new("USD-CPI"),
        };
        let value = serde_json::to_value(quote).expect("serialize YoY quote");
        assert!(value.get("yoy_inflation_swap").is_some());

        // schema-rejection-test
        assert!(serde_json::from_value::<InflationQuote>(serde_json::json!({
            "yo_y_inflation_swap": {
                "id": "USA-CPI-U-YOY-5Y",
                "maturity": "2029-06-20",
                "rate": 0.025,
                "index": "US-CPI-U",
                "frequency": "1Y",
                "convention": "USD-CPI"
            }
        }))
        .is_err());
    }
}
