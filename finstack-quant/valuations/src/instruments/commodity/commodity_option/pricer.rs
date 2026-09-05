//! Commodity option pricer engine.
//!
//! Provides deterministic PV for `CommodityOption` using Black-76 for
//! European exercise, binomial tree for American exercise, and Monte Carlo
//! with Schwartz-Smith two-factor dynamics.

use crate::instruments::commodity::commodity_option::CommodityOption;
use crate::instruments::common_impl::traits::Instrument;
use crate::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;

// Monte Carlo Schwartz-Smith pricer (feature-gated)

/// Commodity option pricer using Monte Carlo with Schwartz-Smith dynamics.
///
/// This pricer is registered under `ModelKey::MonteCarloSchwartzSmith` and
/// reports the estimator standard error, path counts, seed and time grid in
/// `ValuationDetails::MonteCarlo`. Parameters come from registration, with
/// the instrument path-count override taking precedence.
pub struct CommodityOptionMcPricer {
    mc_params: super::types::CommodityMcParams,
}

impl CommodityOptionMcPricer {
    /// Create a new Schwartz-Smith MC pricer.
    ///
    /// # Arguments
    /// * `mc_params` - Risk-neutral model parameters, path count, grid steps and RNG seed.
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
        let option = expect_inst::<CommodityOption>(instrument, InstrumentType::CommodityOption)?;

        // Instrument-level mc_paths override takes precedence over the pricer
        // registration defaults (consistent with autocallable/asian/lookback/etc.).
        let mut mc_params = self.mc_params.clone();
        if let Some(n) = option.instrument_pricing_overrides.model_config.mc_paths {
            if n > 0 {
                mc_params.n_paths = n;
            }
        }

        let (pv, diagnostics) = option
            .price_mc(&mc_params, market, as_of)
            .map_err(|e| PricingError::from_core(e, PricingErrorContext::default()))?;

        let result = ValuationResult::stamped(option.id(), as_of, pv);
        Ok(match diagnostics {
            Some(details) => {
                result.with_details(crate::results::ValuationDetails::MonteCarlo(details))
            }
            None => result,
        })
    }
}
