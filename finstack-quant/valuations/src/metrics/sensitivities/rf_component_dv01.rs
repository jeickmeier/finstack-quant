//! Per-basis-point sensitivities for instrument-owned appraisal rates.
//!
//! DCF equity and real-estate DCF valuations discount at WACC or a property
//! discount rate rather than a market curve. This calculator bumps that
//! instrument input directly and is therefore registered under `wacc01` or
//! `discount_rate01`, never under curve `dv01`.

use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use std::marker::PhantomData;

/// Instruments whose PV is discounted at an instrument-owned appraisal rate.
pub(crate) trait RfComponentPriced: Instrument {
    /// Present value with the discount rate bumped by `bump_at(t)` (absolute,
    /// decimal) at each cashflow tenor `t` (in years). `bump_at = |_| 0.0`
    /// must reproduce the unbumped PV.
    fn pv_with_rf_bump(
        &self,
        market: &MarketContext,
        as_of: Date,
        bump_at: &dyn Fn(f64) -> f64,
    ) -> finstack_quant_core::Result<f64>;
}

/// Per-basis-point sensitivity to an instrument-owned appraisal discount rate.
pub(crate) struct RfComponentDv01Calculator<I> {
    _phantom: PhantomData<I>,
}

impl<I> RfComponentDv01Calculator<I> {
    pub(crate) fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for RfComponentDv01Calculator<I>
where
    I: RfComponentPriced + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let defaults = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_pricing_overrides(),
        )?;
        let bump_bp = defaults.rate_bump_bp;
        let delta = bump_bp / 10_000.0;
        let market = context.curves.as_ref();
        let as_of = context.as_of;

        let pv_up = instrument.pv_with_rf_bump(market, as_of, &|_| delta)?;
        let pv_down = instrument.pv_with_rf_bump(market, as_of, &|_| -delta)?;
        Ok((pv_up - pv_down) / (2.0 * bump_bp))
    }
}
