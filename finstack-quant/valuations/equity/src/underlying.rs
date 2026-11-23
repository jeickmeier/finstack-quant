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

/// Explicit stochastic model policy for path-dependent equity structures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum EquityPathModel {
    /// Piecewise GBM calibrated to the ATM-forward total-variance term
    /// structure. This model does not represent strike skew or smile and must
    /// be selected explicitly as an approximation.
    AtmTermGbm,
}

/// Market dependencies of a single-name equity vol product: discount curve,
/// spot scalar, volatility surface keyed on spot (and `strike` when the
/// surface lookup is strike-specific), and the optional dividend-yield scalar.
///
/// # Arguments
///
/// * `discount_curve_id` - Discount curve for the payoff currency.
/// * `spot_id` - Spot price scalar.
/// * `vol_surface_id` - Implied-volatility surface.
/// * `strike` - Absolute strike for the volatility lookup, or `None` for ATM-forward products.
/// * `div_yield_id` - Continuous dividend-yield scalar, when the product uses one.
pub(crate) fn equity_vol_dependencies(
    discount_curve_id: &finstack_quant_core::types::CurveId,
    spot_id: &finstack_quant_core::types::PriceId,
    vol_surface_id: &finstack_quant_core::types::CurveId,
    strike: Option<f64>,
    div_yield_id: Option<&finstack_quant_core::types::PriceId>,
) -> finstack_quant_valuations_core::instruments::MarketDependencies {
    let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
    deps.add_discount_curve(discount_curve_id.clone());
    deps.add_market_scalar_id(spot_id.as_str());
    deps.add_volatility_dependency(
        finstack_quant_valuations_core::instruments::VolatilityDependency::new(
            vol_surface_id.clone(),
            Some(spot_id.clone()),
            strike,
        ),
    );
    if let Some(dividend_yield) = div_yield_id {
        deps.add_market_scalar_id(dividend_yield.as_str());
    }
    deps
}
