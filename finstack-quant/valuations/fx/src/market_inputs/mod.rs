//! Shared FX market-input collection, spot-basis conventions, and Greeks helpers.

pub(crate) mod forward;
mod greeks;
mod option;
#[cfg(test)]
mod tests;

pub(crate) use forward::FxSpotBasis;
pub(crate) use forward::{
    FAR_EXPIRY, FxForwardRateRequest, collect_fx_forward_inputs, covered_interest_parity_forward,
    event_has_occurred,
};
pub(crate) use greeks::{
    VolScenarios, active_vol_scenarios, discount_rho_greek, fx_matrix_bumped_market,
    fx_valuation_details, greeks_expired, settled_schedule, spot_date_from_lag, spot_delta,
    spot_gamma,
};
pub(crate) use option::{
    FxOptionInputRequest, FxOptionInputs, FxOptionInputsNoVol, collect_fx_option_inputs,
    collect_fx_option_inputs_at_spot, collect_fx_option_inputs_no_vol, resolve_fx_spot,
};
