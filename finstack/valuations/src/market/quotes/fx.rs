//! FX market quote schema.

use super::ids::{Pillar, QuoteId};
use crate::instruments::OptionType;
use crate::market::conventions::ids::{FxConventionId, FxOptionConventionId};
use finstack_core::dates::Date;
use finstack_core::types::CurveId;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts_export")]
use ts_rs::TS;

/// Market quote for FX instruments.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[cfg_attr(feature = "ts_export", ts(rename_all = "snake_case"))]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FxQuote {
    /// Outright FX forward quote quoted as quote-currency per base-currency.
    ForwardOutright {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// FX pair convention identifier (e.g., `EUR/USD`).
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        convention: FxConventionId,
        /// Maturity pillar for the forward settlement date.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        pillar: Pillar,
        /// Forward outright quoted as quote-currency per base-currency.
        forward_rate: f64,
    },
    /// Spot-start FX swap quote with explicit near and far outright rates.
    SwapOutright {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// FX pair convention identifier (e.g., `EUR/USD`).
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        convention: FxConventionId,
        /// Far-leg maturity pillar; near leg is the convention spot date.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        far_pillar: Pillar,
        /// Near-leg outright rate quoted as quote-currency per base-currency.
        near_rate: f64,
        /// Far-leg outright rate quoted as quote-currency per base-currency.
        far_rate: f64,
    },
    /// European vanilla FX option quote with explicit strike and volatility surface.
    OptionVanilla {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// FX option convention identifier (e.g., `EUR/USD-VANILLA`).
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        convention: FxOptionConventionId,
        /// Option expiry date.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        #[schemars(with = "String")]
        expiry: Date,
        /// Strike exchange rate quoted as quote-currency per base-currency.
        strike: f64,
        /// Call or put on the base currency.
        #[cfg_attr(feature = "ts_export", ts(type = "\"call\" | \"put\""))]
        option_type: OptionType,
        /// Volatility surface identifier used for pricing.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        vol_surface_id: CurveId,
    },
}

impl FxQuote {
    /// Get the unique identifier of the quote.
    pub fn id(&self) -> &QuoteId {
        match self {
            FxQuote::ForwardOutright { id, .. }
            | FxQuote::SwapOutright { id, .. }
            | FxQuote::OptionVanilla { id, .. } => id,
        }
    }

    /// Get the resolved primary rate of the quote.
    pub fn value(&self) -> f64 {
        match self {
            FxQuote::ForwardOutright { forward_rate, .. } => *forward_rate,
            FxQuote::SwapOutright { far_rate, .. } => *far_rate,
            FxQuote::OptionVanilla { strike, .. } => *strike,
        }
    }

    /// Create a new quote with its outright rate(s) bumped by a decimal amount.
    pub fn bump_rate_decimal(&self, rate_bump: f64) -> Self {
        match self {
            FxQuote::ForwardOutright {
                id,
                convention,
                pillar,
                forward_rate,
            } => FxQuote::ForwardOutright {
                id: id.clone(),
                convention: convention.clone(),
                pillar: pillar.clone(),
                forward_rate: forward_rate + rate_bump,
            },
            FxQuote::SwapOutright {
                id,
                convention,
                far_pillar,
                near_rate,
                far_rate,
            } => FxQuote::SwapOutright {
                id: id.clone(),
                convention: convention.clone(),
                far_pillar: far_pillar.clone(),
                near_rate: near_rate + rate_bump,
                far_rate: far_rate + rate_bump,
            },
            FxQuote::OptionVanilla {
                id,
                convention,
                expiry,
                strike,
                option_type,
                vol_surface_id,
            } => FxQuote::OptionVanilla {
                id: id.clone(),
                convention: convention.clone(),
                expiry: *expiry,
                strike: strike + rate_bump,
                option_type: *option_type,
                vol_surface_id: vol_surface_id.clone(),
            },
        }
    }
}
