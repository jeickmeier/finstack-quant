//! Shared FX market-input collection, spot-basis conventions, and Greeks helpers.

pub(crate) mod forward;
mod greeks;
mod option;
#[cfg(test)]
mod tests;

pub(crate) use forward::FxSpotBasis;
pub(crate) use forward::{
    FAR_EXPIRY, FxForwardRateRequest, cip_forward_from_dfs, collect_fx_forward_inputs,
    covered_interest_parity_forward, event_has_occurred, joint_calendar, resolve_fx_matrix_spot,
    roll_fx_spot_and_maturity, spot_date_from_lag,
};
pub(crate) use greeks::{
    FxSpotGreekSpec, FxVolGreekSpec, discount_rho_greek, fx_matrix_bumped_market,
    fx_valuation_details, greeks_expired, option_spot_delta, option_spot_gamma, option_vanna,
    option_vega, option_volga, settled_schedule, spot_delta,
};
pub(crate) use option::{
    FxOptionInputRequest, FxOptionInputs, FxOptionInputsNoVol, collect_fx_option_inputs,
    collect_fx_option_inputs_at_spot, collect_fx_option_inputs_no_vol, discount_rates_for_curves,
    discount_rates_to, resolve_fx_spot,
};
