//! Inflation instrument quote types.
//!
//! Inflation instrument quotes for CPI and inflation curve calibration. Supports both
//! zero-coupon inflation swaps (ZCIS) and year-on-year (YoY) inflation swaps.

use super::ids::QuoteId;
use crate::market::conventions::ids::InflationSwapConventionId;
use finstack_quant_core::dates::{Date, Tenor};
#[cfg(feature = "ts_export")]
use ts_rs::TS;

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
/// use finstack_quant_valuations::market::quotes::inflation::InflationQuote;
/// use finstack_quant_valuations::market::quotes::ids::QuoteId;
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
/// use finstack_quant_valuations::market::quotes::inflation::InflationQuote;
/// use finstack_quant_valuations::market::quotes::ids::QuoteId;
/// use finstack_quant_valuations::market::conventions::ids::InflationSwapConventionId;
/// use finstack_quant_core::dates::{Date, Tenor};
///
/// # fn example() -> finstack_quant_core::Result<()> {
/// let quote = InflationQuote::YoYInflationSwap {
///     id: QuoteId::new("USA-CPI-U-YOY-5Y"),
///     maturity: Date::from_calendar_date(2029, time::Month::June, 20).unwrap(),
///     rate: 0.025,
///     index: "US-CPI-U".to_string(),
///     frequency: Tenor::new(1, finstack_quant_core::dates::TenorUnit::Years),
///     convention: InflationSwapConventionId::new("USD-CPI"),
/// };
/// # Ok(())
/// # }
/// ```
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[cfg_attr(feature = "ts_export", ts(rename_all = "snake_case"))]
// Keep this enum externally tagged. Market quote schemas, golden calibration
// payloads, and Python envelope payloads already depend on this shape.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum InflationQuote {
    /// Zero-coupon inflation swap (ZCIS) quote.
    InflationSwap {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// Swap maturity
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        #[schemars(with = "String")]
        maturity: Date,
        /// Fixed rate (decimal)
        rate: f64,
        /// Inflation index identifier
        index: String,
        /// Per-instrument conventions
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        convention: InflationSwapConventionId,
    },
    /// Year-on-year (YoY) inflation swap quote.
    YoYInflationSwap {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// Swap maturity
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        #[schemars(with = "String")]
        maturity: Date,
        /// Fixed rate (decimal)
        rate: f64,
        /// Inflation index identifier
        index: String,
        /// Payment frequency
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        frequency: Tenor,
        /// Instrument-wide conventions
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
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
    /// use finstack_quant_valuations::market::quotes::inflation::InflationQuote;
    /// use finstack_quant_valuations::market::quotes::ids::QuoteId;
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
    /// use finstack_quant_valuations::market::quotes::inflation::InflationQuote;
    /// use finstack_quant_valuations::market::quotes::ids::QuoteId;
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
        match self {
            InflationQuote::InflationSwap {
                id,
                maturity,
                rate,
                index,
                convention,
            } => InflationQuote::InflationSwap {
                id: id.clone(),
                maturity: *maturity,
                rate: rate + rate_bump,
                index: index.clone(),
                convention: convention.clone(),
            },
            InflationQuote::YoYInflationSwap {
                id,
                maturity,
                rate,
                index,
                frequency,
                convention,
            } => InflationQuote::YoYInflationSwap {
                id: id.clone(),
                maturity: *maturity,
                rate: rate + rate_bump,
                index: index.clone(),
                frequency: *frequency,
                convention: convention.clone(),
            },
        }
    }
}
