//! FX spot position instruments for currency pair exposure.
//!
//! Represents spot foreign exchange positions (currency pairs) with
//! mark-to-market valuation and FX sensitivity (delta) calculations.
//!
//! # Structure
//!
//! An FX spot position consists of:
//! - **Base currency**: Amount held (e.g., EUR)
//! - **Quote currency**: Conversion currency (e.g., USD)
//! - **Spot rate**: Exchange rate (e.g., 1.10 USD per EUR)
//!
//! # Pricing
//!
//! FX spot position value in quote currency:
//!
//! ```text
//! Value_quote = Amount_base × Spot_rate
//! ```
//!
//! For conversion to reporting currency, apply additional FX rate.
//!
//! # Market Conventions
//!
//! Currency pair quoting:
//! - **EUR/USD**: 1 EUR = X USD (EUR is base, USD is quote)
//! - **USD/JPY**: 1 USD = Y JPY (USD is base, JPY is quote)
//! - **GBP/USD**: 1 GBP = Z USD (GBP is base, "cable")
//!
//! Settlement: Typically T+2 for major pairs, T+1 for USD/CAD
//!
//! # Key Metrics
//!
//! - **Spot rate**: Current exchange rate
//! - **Base amount**: Amount in base currency
//! - **Quote amount**: Converted amount in quote currency
//! - **FX delta**: Sensitivity to spot rate changes
//!
//! # See Also
//!
//! - [`FxSpot`] for instrument struct
//! - [`crate::instruments::fx::fx_option`] for FX option pricing
//! - [`crate::instruments::fx::fx_swap`] for FX forwards

mod types;

pub(crate) mod metrics;
pub use types::FxSpot;

// Re-export metric calculators for test access.
#[doc(hidden)]
pub use metrics::base_amount::BaseAmountCalculator;
#[doc(hidden)]
pub use metrics::inverse_rate::InverseRateCalculator;
#[doc(hidden)]
pub use metrics::spot_rate::SpotRateCalculator;
