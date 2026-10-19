//! Generic pricer implementations for instrument pricing.
//!
//! This module provides generic pricer types that eliminate boilerplate by
//! delegating to instruments' `base_value()` methods. Use these when an instrument
//! implements the [`Instrument`] trait and doesn't need specialized pricing logic.
//!
//! The pricer returns the **unshocked** base PV; the registry lifecycle applies
//! the scenario shock exactly once before returning a result or computing
//! metrics.
//!
//! [`Pricer::price_dyn`] is the unchecked model kernel. The registry validates
//! the instrument and resolves its effective valuation date before invoking it;
//! direct callers are responsible for both preconditions.

use crate::instruments::common_impl::traits::Instrument;
use crate::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::market_data::context::MarketContext;
use std::marker::PhantomData;

/// Generic pricer for any instrument that implements the Instrument trait.
///
/// This eliminates the need for instrument-specific pricer implementations that just
/// forward to the instrument's unshocked `base_value()` kernel.
pub struct GenericInstrumentPricer<I> {
    instrument_type: InstrumentType,
    model_key: ModelKey,
    _phantom: PhantomData<I>,
}

impl<I> GenericInstrumentPricer<I>
where
    I: Instrument + 'static,
{
    /// Create a new generic pricer for the specified instrument and model type.
    pub fn new(instrument_type: InstrumentType, model_key: ModelKey) -> Self {
        Self {
            instrument_type,
            model_key,
            _phantom: PhantomData,
        }
    }

    /// Create a generic discounting pricer for the specified instrument type.
    ///
    /// This is a convenience method equivalent to `new(instrument_type, ModelKey::Discounting)`.
    /// Use this when the instrument uses simple cashflow discounting without specialized models.
    pub fn discounting(instrument_type: InstrumentType) -> Self {
        Self::new(instrument_type, ModelKey::Discounting)
    }
}

impl<I> Pricer for GenericInstrumentPricer<I>
where
    I: Instrument + 'static,
{
    fn key(&self) -> PricerKey {
        PricerKey::new(self.instrument_type, self.model_key)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        // Type-safe downcasting
        let typed_instrument = expect_inst::<I>(instrument, self.instrument_type)?;

        // Compute the base (unshocked) present value; scenario shocks are
        // applied by the registry lifecycle exactly once. The registry has
        // already resolved the effective valuation date passed as `as_of`.
        // Preserve the core error kind: a missing curve stays `MissingMarketData`
        // (host `KeyError`), a validation failure stays `InvalidInput` (host
        // `ValueError`); only calibration/internal failures become `ModelFailure`.
        let pv = typed_instrument.base_value(market, as_of).map_err(|e| {
            PricingError::from_core(
                e,
                PricingErrorContext::from_instrument(typed_instrument).model(self.model_key),
            )
        })?;

        let mut result = ValuationResult::stamped(typed_instrument.id(), as_of, pv);
        if let Some(details) = typed_instrument.valuation_details(market, as_of) {
            result = result.with_details(details);
        }
        Ok(result)
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<f64, PricingError> {
        let typed_instrument = expect_inst::<I>(instrument, self.instrument_type)?;
        typed_instrument.base_value_raw(market, as_of).map_err(|e| {
            PricingError::from_core(
                e,
                PricingErrorContext::from_instrument(typed_instrument).model(self.model_key),
            )
        })
    }
}
