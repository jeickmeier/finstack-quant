//! Registry adapter for convertible tree pricing.

use crate::pricer::expect_inst;
use finstack_quant_core::market_data::context::MarketContext;

use crate::instruments::common_impl::traits::Instrument;

use super::engine::price_convertible_bond;

/// Registry pricer for Convertible Bond using Tsiveriotis-Zhang tree-based pricing.
pub(crate) struct ConvertibleTreePricer;

impl ConvertibleTreePricer {
    /// Create a new convertible bond tree pricer.
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for ConvertibleTreePricer {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::pricer::Pricer for ConvertibleTreePricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(
            crate::pricer::InstrumentType::Convertible,
            crate::pricer::ModelKey::Tree,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<crate::results::ValuationResult, crate::pricer::PricingError> {
        let convertible = expect_inst::<
            crate::instruments::fixed_income::convertible::ConvertibleBond,
        >(instrument, crate::pricer::InstrumentType::Convertible)?;

        let pv = price_convertible_bond(convertible, market, as_of).map_err(|e| {
            crate::pricer::PricingError::model_failure_with_context(
                e.to_string(),
                crate::pricer::PricingErrorContext::default(),
            )
        })?;

        Ok(crate::results::ValuationResult::stamped(
            convertible.id(),
            as_of,
            pv,
        ))
    }
}
