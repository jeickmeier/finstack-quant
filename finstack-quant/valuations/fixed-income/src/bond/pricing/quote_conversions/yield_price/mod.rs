//! Yield-to-price, day-count timing conventions, and workout-path inversion.

mod conventions;
mod discounting;
#[cfg(test)]
mod tests;
mod workout;

pub use discounting::{df_from_yield, price_from_ytm, price_from_ytm_compounded_params};
pub use workout::price_from_ytw;

pub(crate) use conventions::{
    act365l_flow_times, bond_flow_times, clean_price_from_japanese_simple_yield, flow_times,
    japanese_simple_yield, street_flow_times,
};
pub(crate) use discounting::price_from_ytm_timed;
pub(crate) use workout::{
    RedemptionBasis, enumerate_exit_paths, exercise_redemption_amount, price_from_i_spread,
    solve_workout_exits, solve_ytw_from_flows,
};
