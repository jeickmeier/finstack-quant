//! Equity underlying asset parameters.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::types::PriceId;
use serde::{Deserialize, Serialize};

/// Equity underlying parameters for options and equity-linked swaps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EquityUnderlyingParams {
    /// Underlying ticker/identifier.
    pub ticker: String,
    /// Spot price identifier in market data.
    pub spot_id: PriceId,
    /// Optional dividend yield identifier.
    pub div_yield_id: Option<PriceId>,
    /// Base currency for pricing.
    pub currency: Currency,
}

impl EquityUnderlyingParams {
    /// Create equity underlying parameters.
    ///
    /// # Arguments
    ///
    /// * `ticker` - Equity symbol label (for reporting and diagnostics).
    /// * `spot_id` - Market scalar identifier for the spot equity price.
    /// * `currency` - Base currency in which the equity and options settle.
    pub fn new(ticker: impl Into<String>, spot_id: impl Into<PriceId>, currency: Currency) -> Self {
        Self {
            ticker: ticker.into(),
            spot_id: spot_id.into(),
            div_yield_id: None,
            currency,
        }
    }

    /// Set dividend yield identifier.
    ///
    /// # Arguments
    ///
    /// * `div_yield_id` - Market scalar identifier for the continuous dividend yield.
    pub fn with_div_yield_id(mut self, div_yield_id: impl Into<PriceId>) -> Self {
        self.div_yield_id = Some(div_yield_id.into());
        self
    }

    /// Validate identifiers and market-data routing fields.
    ///
    /// # Arguments
    ///
    /// * `context` - Instrument name included in validation diagnostics.
    pub fn validate(&self, context: &str) -> finstack_quant_core::Result<()> {
        if self.ticker.trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} requires a non-empty equity ticker"
            )));
        }
        if self.spot_id.as_str().trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} requires a non-empty equity spot_id"
            )));
        }
        if self
            .div_yield_id
            .as_ref()
            .is_some_and(|id| id.as_str().trim().is_empty())
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} dividend-yield identifier cannot be empty"
            )));
        }
        Ok(())
    }
}
