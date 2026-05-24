//! Commodity option pricer engine.
//!
//! Provides deterministic PV for `CommodityOption` using Black-76 for
//! European exercise, binomial tree for American exercise, and Monte Carlo
//! with Schwartz-Smith two-factor dynamics.

use crate::instruments::commodity::commodity_option::CommodityOption;
use crate::instruments::common_impl::traits::Instrument;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;

// ---------------------------------------------------------------------------
// Monte Carlo Schwartz-Smith pricer (feature-gated)
// ---------------------------------------------------------------------------

/// Commodity option pricer using Monte Carlo with Schwartz-Smith dynamics.
///
/// This pricer is registered under `ModelKey::MonteCarloSchwartzSmith` and
/// delegates to `CommodityOption::npv_mc`. The `CommodityMcParams` must be
/// supplied via the instrument's `pricing_overrides.mc_config` (future work)
/// or by calling `npv_mc` directly.
pub struct CommodityOptionMcPricer {
    mc_params: super::types::CommodityMcParams,
}

impl CommodityOptionMcPricer {
    /// Create a new Schwartz-Smith MC pricer.
    pub fn new(mc_params: super::types::CommodityMcParams) -> Self {
        Self { mc_params }
    }
}

impl Pricer for CommodityOptionMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(
            InstrumentType::CommodityOption,
            ModelKey::MonteCarloSchwartzSmith,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let option = instrument
            .as_any()
            .downcast_ref::<CommodityOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::CommodityOption, instrument.key())
            })?;

        // Instrument-level mc_paths override takes precedence over the pricer
        // registration defaults (consistent with autocallable/asian/lookback/etc.).
        let mut mc_params = self.mc_params.clone();
        if let Some(n) = option.pricing_overrides.model_config.mc_paths {
            if n > 0 {
                mc_params.n_paths = n;
            }
        }

        let pv = option.npv_mc(&mc_params, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(option.id(), as_of, pv))
    }
}
