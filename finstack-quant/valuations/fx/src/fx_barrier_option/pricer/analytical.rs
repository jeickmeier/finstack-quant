//! Analytical Reiner-Rubinstein pricer for continuously monitored FX barrier options.

use super::{
    BarrierState, barrier_pricing_context, barrier_state, notional_value, validate_monitoring_state,
};
use crate::fx_barrier_option::types::FxBarrierOption;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::BarrierType;
use finstack_quant_models::closed_form::{BarrierParams, barrier_price, barrier_rebate};
use finstack_quant_valuations_core::instruments::{Instrument, Monitoring};
use finstack_quant_valuations_core::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, expect_inst,
};
use finstack_quant_valuations_core::results::ValuationResult;

/// FX Barrier option analytical pricer (continuous monitoring).
pub(crate) struct FxBarrierOptionAnalyticalPricer;

/// Compute the BS barrier price + optional rebate (without notional scaling).
fn bs_barrier_price_per_unit(
    fx_barrier: &FxBarrierOption,
    fx_spot: f64,
    r_dom: f64,
    r_for: f64,
    sigma: f64,
    t: f64,
    analytical_barrier_type: BarrierType,
) -> f64 {
    let params = BarrierParams::new(
        fx_spot,
        fx_barrier.strike,
        fx_barrier.barrier,
        t,
        r_dom,
        r_for,
        sigma,
    );
    let price = barrier_price(&params, analytical_barrier_type, fx_barrier.option_type);

    let rebate_val = if let Some((rebate, timing)) = fx_barrier.rebate_terms_per_unit() {
        barrier_rebate(&params, rebate, analytical_barrier_type, timing)
    } else {
        0.0
    };

    price + rebate_val
}

impl Pricer for FxBarrierOptionAnalyticalPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(
            InstrumentType::FxBarrierOption,
            ModelKey::FxBarrierBSContinuous,
        )
    }

    #[tracing::instrument(
        name = "fx_barrier_option.analytical.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let fx_barrier =
            expect_inst::<FxBarrierOption>(instrument, InstrumentType::FxBarrierOption)?;

        let context = barrier_pricing_context(fx_barrier, ModelKey::FxBarrierBSContinuous);
        validate_monitoring_state(fx_barrier, as_of)
            .map_err(|error| PricingError::from_core(error, context.clone()))?;

        if as_of > fx_barrier.expiry {
            return Ok(ValuationResult::stamped(
                fx_barrier.id(),
                as_of,
                Money::from((0_i64, fx_barrier.quote_currency)),
            ));
        }

        if matches!(fx_barrier.monitoring, Monitoring::Discrete { .. }) {
            return Err(PricingError::invalid_input_with_context(
                "Discrete FX barrier monitoring requires the Monte Carlo pricer; the analytical \
                 Reiner-Rubinstein pricer assumes continuous monitoring.",
                context,
            ));
        }

        if fx_barrier
            .monitoring_start_date
            .is_some_and(|start| start > as_of)
        {
            return Err(PricingError::invalid_input_with_context(
                "Future FX barrier monitoring requires the Monte Carlo pricer; the analytical \
                 Reiner-Rubinstein pricer requires monitoring_start_date on or before as_of.",
                context,
            ));
        }

        let pv = barrier_state(fx_barrier, market, as_of)
            .and_then(|state| {
                let per_unit = match state {
                    BarrierState::Fixed(per_unit) => per_unit,
                    BarrierState::Live(inputs) => bs_barrier_price_per_unit(
                        fx_barrier,
                        inputs.spot,
                        inputs.r_domestic,
                        inputs.r_foreign,
                        inputs.sigma,
                        inputs.t,
                        fx_barrier.barrier_type,
                    ),
                };
                notional_value(fx_barrier, per_unit)
            })
            .map_err(|error| PricingError::from_core(error, context))?;
        Ok(ValuationResult::stamped(fx_barrier.id(), as_of, pv))
    }
}
