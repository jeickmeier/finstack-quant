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
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
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
    ///
    /// # Example
    ///
    /// ```ignore
    /// let irs_pricer = GenericInstrumentPricer::<InterestRateSwap>::discounting(InstrumentType::IRS);
    /// ```
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
        let typed_instrument = instrument
            .as_any()
            .downcast_ref::<I>()
            .ok_or_else(|| PricingError::type_mismatch(self.instrument_type, instrument.key()))?;

        // Compute the base (unshocked) present value; scenario shocks are
        // applied by the registry lifecycle exactly once. The registry has
        // already resolved the effective valuation date passed as `as_of`.
        let pv = typed_instrument.base_value(market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
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
        let typed_instrument = instrument
            .as_any()
            .downcast_ref::<I>()
            .ok_or_else(|| PricingError::type_mismatch(self.instrument_type, instrument.key()))?;
        typed_instrument.base_value_raw(market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(typed_instrument).model(self.model_key),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use time::macros::date;

    #[test]
    fn test_generic_pricer_keys() {
        // Test the new discounting() convenience method
        let bond_pricer =
            GenericInstrumentPricer::<crate::instruments::Bond>::discounting(InstrumentType::Bond);
        assert_eq!(
            bond_pricer.key(),
            PricerKey::new(InstrumentType::Bond, ModelKey::Discounting)
        );

        let deposit_pricer = GenericInstrumentPricer::<crate::instruments::Deposit>::discounting(
            InstrumentType::Deposit,
        );
        assert_eq!(
            deposit_pricer.key(),
            PricerKey::new(InstrumentType::Deposit, ModelKey::Discounting)
        );
    }

    #[test]
    fn test_generic_instrument_pricer_with_model_key() {
        // Test that GenericInstrumentPricer works with any model key
        let pricer = GenericInstrumentPricer::<crate::instruments::Bond>::new(
            InstrumentType::Bond,
            ModelKey::HazardRate,
        );
        assert_eq!(
            pricer.key(),
            PricerKey::new(InstrumentType::Bond, ModelKey::HazardRate)
        );
    }

    #[test]
    fn generic_pricer_attaches_instrument_and_model_context_to_failures() {
        let bond = crate::instruments::Bond::fixed(
            "GENERIC-CTX",
            Money::new(100.0, Currency::USD),
            0.05,
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            "USD-OIS",
        )
        .expect("bond");
        let pricer =
            GenericInstrumentPricer::<crate::instruments::Bond>::discounting(InstrumentType::Bond);

        let err = pricer
            .price_dyn(&bond, &MarketContext::new(), date!(2025 - 01 - 01))
            .expect_err("missing discount curve must fail");

        match err {
            PricingError::ModelFailure { context, .. } => {
                assert_eq!(context.instrument_id.as_deref(), Some("GENERIC-CTX"));
                assert_eq!(context.instrument_type, Some(InstrumentType::Bond));
                assert_eq!(context.model, Some(ModelKey::Discounting));
            }
            other => panic!("expected model failure, got {other:?}"),
        }
    }

    #[test]
    fn generic_pricer_raw_path_attaches_context_to_failures() {
        let bond = crate::instruments::Bond::fixed(
            "GENERIC-RAW-CTX",
            Money::new(100.0, Currency::USD),
            0.05,
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            "USD-OIS",
        )
        .expect("bond");
        let pricer =
            GenericInstrumentPricer::<crate::instruments::Bond>::discounting(InstrumentType::Bond);

        let err = pricer
            .price_raw_dyn(&bond, &MarketContext::new(), date!(2025 - 01 - 01))
            .expect_err("missing discount curve must fail");

        match err {
            PricingError::ModelFailure { context, .. } => {
                assert_eq!(context.instrument_id.as_deref(), Some("GENERIC-RAW-CTX"));
                assert_eq!(context.instrument_type, Some(InstrumentType::Bond));
                assert_eq!(context.model, Some(ModelKey::Discounting));
            }
            other => panic!("expected model failure, got {other:?}"),
        }
    }
}
