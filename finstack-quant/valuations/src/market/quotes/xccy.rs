//! Cross-currency swap market quote schema.

use super::ids::{Pillar, QuoteId};
use crate::market::conventions::ids::XccyConventionId;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts_export")]
use ts_rs::TS;

/// Market quote for cross-currency basis swap instruments.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[cfg_attr(feature = "ts_export", ts(rename_all = "snake_case"))]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum XccyQuote {
    /// Cross-currency basis swap quoted as a spread over the quote-currency leg.
    BasisSwap {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// XCCY pair convention identifier (e.g., `EUR/USD-XCCY`).
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        convention: XccyConventionId,
        /// Far-leg maturity pillar; near leg is the convention spot date.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        far_pillar: Pillar,
        /// Basis spread in basis points applied to the quote-currency leg.
        basis_spread_bp: f64,
        /// Optional spot FX quote (quote currency per 1 unit of base currency).
        ///
        /// Used to derive FX-equivalent leg notionals when the build context
        /// provides only a single standard notional.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spot_fx: Option<f64>,
    },
}

impl XccyQuote {
    /// Get the unique identifier of the quote.
    pub fn id(&self) -> &QuoteId {
        match self {
            XccyQuote::BasisSwap { id, .. } => id,
        }
    }

    /// Get the primary value of the quote (basis spread in bp).
    pub fn value(&self) -> f64 {
        match self {
            XccyQuote::BasisSwap {
                basis_spread_bp, ..
            } => *basis_spread_bp,
        }
    }

    /// Create a new quote with its spread bumped by basis-point units.
    pub fn bump_spread_bp(&self, bump_bp: f64) -> Self {
        match self {
            XccyQuote::BasisSwap {
                id,
                convention,
                far_pillar,
                basis_spread_bp,
                spot_fx,
            } => XccyQuote::BasisSwap {
                id: id.clone(),
                convention: convention.clone(),
                far_pillar: far_pillar.clone(),
                basis_spread_bp: basis_spread_bp + bump_bp,
                spot_fx: *spot_fx,
            },
        }
    }

    /// Create a new quote with its spread bumped by decimal units (e.g., `0.0001` = 1bp).
    pub fn bump_spread_decimal(&self, bump_decimal: f64) -> Self {
        self.bump_spread_bp(bump_decimal * 10_000.0)
    }
}
