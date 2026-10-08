//! Cross-currency swap market quote schema.

use super::ids::{Pillar, QuoteId};
use super::validate;
use finstack_quant_core::Result;
use finstack_quant_valuations::market::conventions::ids::XccyConventionId;
use serde::{Deserialize, Serialize};

/// Market quote for a cross-currency basis swap.
///
/// The quote is a spread on the base-currency floating leg. Optional `spot_fx` is
/// quote-currency per 1 unit of base currency and is used to size the FX-equivalent
/// notionals when the build context supplies only one standard notional.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct XccyQuote {
    /// Unique identifier for the quote.
    pub id: QuoteId,
    /// XCCY pair convention identifier (e.g., `EUR/USD-XCCY`).
    pub convention: XccyConventionId,
    /// Far-leg maturity pillar; near leg is the convention spot date.
    pub far_pillar: Pillar,
    /// Basis spread in basis points on the base-currency leg.
    pub basis_spread_bp: f64,
    /// Optional spot FX quote (quote currency per 1 unit of base currency).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot_fx: Option<f64>,
}

impl XccyQuote {
    /// Get the unique identifier of the quote.
    pub fn id(&self) -> &QuoteId {
        &self.id
    }

    /// Get the primary value of the quote (basis spread in bp).
    pub fn value(&self) -> f64 {
        self.basis_spread_bp
    }

    /// Validate that the basis spread is finite and any spot FX is positive.
    pub fn validate(&self) -> Result<()> {
        validate::finite(self.basis_spread_bp, "basis_spread_bp")?;
        if let Some(value) = self.spot_fx {
            validate::positive(value, "spot_fx")?;
        }
        Ok(())
    }
}
