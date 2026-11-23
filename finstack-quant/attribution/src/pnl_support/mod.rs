//! Shared P&L endpoint, execution-policy, sensitivity-step, and carry primitives.

mod carry;
mod endpoints;
#[cfg(test)]
mod tests;

pub(crate) use carry::{
    TotalReturnCarryInputs, apply_total_return_carry, total_return_carry_inputs,
};
pub(crate) use endpoints::{
    FactorWorking, InstrumentFactorUse, compute_pnl, compute_pnl_with_fx, endpoint_values,
    factor_money_or_invalid, finalize_attribution, instrument_at_t0, map_policy,
    measured_unless_missing, money_or_zero, seed_attribution, sensitivity_step, stamp_endpoints,
    stamp_fx_policy, try_map_policy, validate_attribution_period, volatility_expiry_times,
};
