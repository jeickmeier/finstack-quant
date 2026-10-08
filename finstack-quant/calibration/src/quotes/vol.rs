//! Volatility quote types for surface calibration.
//!
//! Defines the volatility quote types used for surface calibration of options and swaptions.
//! Volatility quotes include strike, expiry, and implied volatility values for building
//! volatility surfaces.

use super::ids::QuoteId;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::surfaces::VolQuoteType;
use finstack_quant_core::types::UnderlyingId;
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations::instruments::OptionType;
use finstack_quant_valuations::market::conventions::ids::SwaptionConventionId;

/// Volatility quotes for option and swaption surface calibration.
///
/// Supports two types of volatility quotes:
/// 1. **Option volatility**: For equity, commodity, or FX options with strike and expiry
/// 2. **Swaption volatility**: For interest rate swaptions with strike, expiry, and underlying swap maturity date
///
/// # Examples
///
/// Option volatility quote:
/// ```rust
/// use finstack_quant_calibration::quotes::vol::VolQuote;
/// use finstack_quant_calibration::quotes::ids::QuoteId;
/// use finstack_quant_valuations::instruments::OptionType;
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::types::UnderlyingId;
///
/// let quote = VolQuote::OptionVol {
///     id: QuoteId::new("SPX-VOL-20241220-4500"),
///     underlying: UnderlyingId::new("SPX"),
///     expiry: Date::from_calendar_date(2024, time::Month::December, 20).unwrap(),
///     strike: 4500.0,
///     vol: 0.20, // 20% implied volatility
///     option_type: OptionType::Call,
/// };
/// ```
///
/// Swaption volatility quote:
/// ```rust
/// use finstack_quant_calibration::quotes::vol::VolQuote;
/// use finstack_quant_calibration::quotes::ids::QuoteId;
/// use finstack_quant_valuations::market::conventions::ids::SwaptionConventionId;
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::market_data::surfaces::VolQuoteType;
///
/// let quote = VolQuote::SwaptionVol {
///     id: QuoteId::new("USD-SWPTN-VOL-1Yx5Y-ATM"),
///     expiry: Date::from_calendar_date(2025, time::Month::June, 20).unwrap(),
///     maturity: Date::from_calendar_date(2030, time::Month::June, 20).unwrap(),
///     strike: 0.045, // 4.5% strike rate
///     vol: 0.15, // 15% implied volatility
///     quote_type: VolQuoteType::Normal,
///     convention: SwaptionConventionId::new("USD"),
/// };
/// ```
// The externally tagged shape makes each quote payload's concrete kind
// explicit while keeping every nested field fully typed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum VolQuote {
    /// Equity or commodity option implied volatility quote.
    OptionVol {
        /// Unique identifier for the quote.
        id: QuoteId,
        /// Underlying identifier
        underlying: UnderlyingId,
        /// Option expiry
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        expiry: Date,
        /// Strike
        strike: f64,
        /// Implied volatility in decimal units (for example, `0.20` for 20%).
        vol: f64,
        /// Option type (Call or Put).
        option_type: OptionType,
    },
    /// Interest rate swaption implied volatility quote.
    SwaptionVol {
        /// Unique identifier for the quote.
        id: QuoteId,
        /// Option expiry
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        expiry: Date,
        /// Underlying swap maturity date
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        maturity: Date,
        /// Strike rate
        strike: f64,
        /// Implied volatility in canonical decimal units: absolute rate
        /// volatility for normal quotes and Black volatility for lognormal quotes.
        vol: f64,
        /// Volatility quoting convention. Shifted Black quotes require a
        /// `ShiftedLognormal` calibration plan supplying their displacement;
        /// Hull-White calibration rejects them because its quote contract
        /// carries no displacement.
        quote_type: VolQuoteType,
        /// Option exercise conventions
        convention: SwaptionConventionId,
    },
    /// Interest rate cap/floor implied volatility quote.
    CapFloorVol {
        /// Unique identifier for the quote.
        id: QuoteId,
        /// Cap/floor maturity or caplet expiry.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        expiry: Date,
        /// Strike rate.
        strike: f64,
        /// Implied volatility in canonical decimal units: absolute rate
        /// volatility for normal quotes and Black volatility for lognormal quotes.
        vol: f64,
        /// Volatility quoting convention. Hull-White calibration accepts
        /// normal and unshifted Black quotes; it rejects shifted Black
        /// because this quote does not carry a displacement.
        quote_type: VolQuoteType,
        /// `true` for cap, `false` for floor.
        is_cap: bool,
    },
}

impl VolQuote {
    /// Get the unique identifier of the quote.
    pub fn id(&self) -> &QuoteId {
        match self {
            VolQuote::OptionVol { id, .. }
            | VolQuote::SwaptionVol { id, .. }
            | VolQuote::CapFloorVol { id, .. } => id,
        }
    }

    /// Return the quoted volatility in decimal units.
    #[must_use]
    pub fn volatility(&self) -> f64 {
        match self {
            Self::OptionVol { vol, .. }
            | Self::SwaptionVol { vol, .. }
            | Self::CapFloorVol { vol, .. } => *vol,
        }
    }

    /// Validate the quote before calibration or bumping.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::OptionVol { strike, vol, .. } => {
                if !strike.is_finite() || *strike <= 0.0 {
                    return Err(Error::Validation(format!(
                        "option volatility strike must be finite and positive, got {strike}"
                    )));
                }
                validate_volatility(*vol)
            }
            Self::SwaptionVol {
                expiry,
                maturity,
                strike,
                vol,
                ..
            } => {
                if maturity <= expiry {
                    return Err(Error::Validation(
                        "swaption volatility maturity must be after expiry".to_string(),
                    ));
                }
                if !strike.is_finite() {
                    return Err(Error::Validation(
                        "swaption volatility strike must be finite".to_string(),
                    ));
                }
                validate_volatility(*vol)
            }
            Self::CapFloorVol { strike, vol, .. } => {
                if !strike.is_finite() {
                    return Err(Error::Validation(
                        "cap/floor volatility strike must be finite".to_string(),
                    ));
                }
                validate_volatility(*vol)
            }
        }
    }
}

fn validate_volatility(volatility: f64) -> Result<()> {
    if !volatility.is_finite() || volatility < 0.0 {
        return Err(Error::Validation(format!(
            "implied volatility must be finite and non-negative, got {volatility}"
        )));
    }
    Ok(())
}
