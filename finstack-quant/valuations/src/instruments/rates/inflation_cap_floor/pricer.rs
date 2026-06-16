use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::inflation_cap_floor::InflationCapFloor;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::market_data::context::MarketContext;

/// Simplified inflation cap/floor pricer supporting Black-76 and Normal models.
pub(crate) struct SimpleInflationCapFloorPricer {
    model: ModelKey,
}

impl SimpleInflationCapFloorPricer {
    /// Create a new pricer with default Black-76 model.
    pub(crate) fn new() -> Self {
        Self {
            model: ModelKey::Black76,
        }
    }

    /// Create a pricer with specified model key.
    pub(crate) fn with_model(model: ModelKey) -> Self {
        Self { model }
    }
}

impl Default for SimpleInflationCapFloorPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for SimpleInflationCapFloorPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::InflationCapFloor, self.model)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let option = instrument
            .as_any()
            .downcast_ref::<InflationCapFloor>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::InflationCapFloor, instrument.key())
            })?;

        let pv = option
            .npv_with_model(market, as_of, self.model)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        Ok(ValuationResult::stamped(option.id(), as_of, pv))
    }
}
