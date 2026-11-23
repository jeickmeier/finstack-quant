//! Bond quote engine for mapping between price, yields, and spreads.
//!
//! This module provides a small, opinionated API that takes **one**
//! quote input (price, yield, or spread) and produces a consistent
//! set of derived bond quotes using the existing pricing and metric
//! infrastructure.
//!
//! All spread-style quantities exposed here use **decimal units**:
//! `0.01` corresponds to **100 basis points**.

mod annuity;
mod breakeven_ladder;
mod compute;
mod spread_price;
mod types;
mod workout_ladder;
mod yield_price;

pub use annuity::{
    asset_swap_forward_components, fixed_leg_annuity, par_rate_and_annuity_from_discount,
};
pub use breakeven_ladder::{BondBreakevenLadder, BondBreakevenRow, bond_breakeven_ladder};
pub use compute::compute_quotes;
pub use spread_price::{price_from_dm, price_from_oas, price_from_z_spread};
pub(crate) use types::ExitCandidate;
pub use types::{BondQuoteSet, YieldCompounding};
pub use workout_ladder::{BondWorkoutLadder, BondWorkoutRow, WorkoutExitKind, bond_workout_ladder};
pub use yield_price::discounting::{
    df_from_yield, price_from_ytm, price_from_ytm_compounded_params,
};
pub use yield_price::workout::price_from_ytw;

pub(crate) use annuity::{
    asw_index_conventions, asw_leg_schedule, curve_par_rate, floating_leg_pv_and_annuity,
    i_spread_horizon, settlement_asw_components,
};
pub(crate) use compute::{clear_price_driving_overrides, settlement_dirty_from_quote_overrides};
pub(crate) use spread_price::{
    BondZSpreadPricingKernel, WorkoutPath, quoted_workout_path, solve_kernel_spread,
};
pub(crate) use workout_ladder::workout_dirty_target;
pub(crate) use yield_price::{
    RedemptionBasis, act365l_flow_times, bond_flow_times, clean_price_from_japanese_simple_yield,
    enumerate_exit_paths, exercise_redemption_amount, flow_times, japanese_simple_yield,
    price_from_ytm_timed, solve_ytw_from_flows,
};

#[cfg(test)]
mod tests;
pub(crate) use yield_price::street_flow_times;
