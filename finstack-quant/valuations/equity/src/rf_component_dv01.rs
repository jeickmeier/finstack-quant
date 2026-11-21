//! Per-basis-point sensitivities for instrument-owned appraisal rates.
//!
//! DCF equity and real-estate DCF valuations discount at WACC or a property
//! discount rate rather than a market curve. This calculator bumps that
//! instrument input directly and is therefore registered under `wacc01` or
//! `discount_rate01`, never under curve `dv01`.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::{
    MetricCalculator, MetricContext, sensitivity_central_diff,
};
use std::marker::PhantomData;

/// Instruments whose PV is discounted at an instrument-owned appraisal rate.
pub trait RfComponentPriced: Instrument {
    /// Present value with the appraisal discount rate shifted uniformly by
    /// `bump` (absolute, decimal). `bump = 0.0` must reproduce the unbumped PV.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context used for auxiliary financing or FX lookups.
    /// * `as_of` - Valuation date.
    /// * `bump` - Uniform absolute shift applied to the appraisal discount rate
    ///   as a decimal (for example `0.0001` for +1 bp).
    fn pv_with_rf_bump(
        &self,
        market: &MarketContext,
        as_of: Date,
        bump: f64,
    ) -> finstack_quant_core::Result<f64>;
}

/// Per-basis-point sensitivity to an instrument-owned appraisal discount rate.
pub struct RfComponentDv01Calculator<I> {
    _phantom: PhantomData<I>,
}

impl<I> Default for RfComponentDv01Calculator<I> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<I> RfComponentDv01Calculator<I> {
    /// A calculator for the instrument type `I`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl<I> MetricCalculator for RfComponentDv01Calculator<I>
where
    I: RfComponentPriced + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let bump_bp = sens_config::resolve(context)?.rate_bump_bp;
        let delta = bump_bp / 10_000.0;
        let market = context.curves.as_ref();
        let as_of = context.as_of;

        let pv_up = instrument.pv_with_rf_bump(market, as_of, delta)?;
        let pv_down = instrument.pv_with_rf_bump(market, as_of, -delta)?;
        sensitivity_central_diff(pv_up, pv_down, bump_bp)
    }
}
