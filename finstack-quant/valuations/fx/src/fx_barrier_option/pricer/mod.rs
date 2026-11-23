//! FX barrier option pricers (Monte Carlo and analytical).

mod analytical;
mod mc;
mod state;
#[cfg(test)]
mod tests;

pub(crate) use analytical::FxBarrierOptionAnalyticalPricer;
pub(crate) use mc::FxBarrierOptionMcPricer;
pub(crate) use mc::compute_pv;
pub(crate) use state::resolve_fx_spot;
use state::{
    BarrierState, barrier_pricing_context, barrier_state, notional_value, validate_monitoring_state,
};
#[cfg(test)]
use state::{collect_fx_barrier_inputs, expired_barrier_value_per_unit};
