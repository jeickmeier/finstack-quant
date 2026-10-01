//! Pricing and metric helpers for interest-rate instruments.
//!
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::inflation_cap_floor::InflationCapFloor;
use crate::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
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
        let option =
            expect_inst::<InflationCapFloor>(instrument, InstrumentType::InflationCapFloor)?;

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

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<f64, PricingError> {
        let option =
            expect_inst::<InflationCapFloor>(instrument, InstrumentType::InflationCapFloor)?;
        option
            .npv_raw_with_model(market, as_of, self.model)
            .map_err(|error| {
                PricingError::model_failure_with_context(
                    error.to_string(),
                    PricingErrorContext::default(),
                )
            })
    }
}
