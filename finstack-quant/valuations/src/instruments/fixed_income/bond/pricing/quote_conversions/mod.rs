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
mod compute;
mod spread_price;
mod types;
mod yield_price;

pub use annuity::{
    asset_swap_forward_components, fixed_leg_annuity, par_rate_and_annuity_from_discount,
    periods_per_year,
};
pub use compute::compute_quotes;
pub use spread_price::{price_from_dm, price_from_oas, price_from_z_spread};
pub use types::{BondQuoteInput, BondQuoteSet, YieldCompounding};
pub use yield_price::{
    df_from_yield, price_from_ytm, price_from_ytm_compounded_params, price_from_ytw,
};

pub(crate) use annuity::floating_leg_pv_and_annuity;
pub(crate) use compute::{clear_price_driving_overrides, settlement_dirty_from_quote_overrides};
pub(crate) use yield_price::{
    clean_price_from_japanese_simple_yield, enumerate_exit_paths, exercise_redemption_amount,
    icma_reference_period, japanese_simple_yield, solve_ytw_from_flows, RedemptionBasis,
};

/// One candidate early-exit for yield-to-worst enumeration.
///
/// Represents a single admissible exercise date and the corresponding clean
/// redemption price expressed as a percentage of par (e.g. `103.0` for 103%).
#[derive(Debug, Clone)]
pub(crate) struct ExitCandidate {
    /// The exact admissible exercise date, clipped to
    /// `[as_of, bond.maturity]`.
    pub(crate) date: finstack_quant_core::dates::Date,
    /// Clean redemption price as percent of par (e.g. `103.0` for 103%).
    pub(crate) price_pct_of_par: f64,
    /// Deterministic make-whole term retained for issuer-call candidates.
    pub(crate) make_whole: Option<crate::instruments::fixed_income::bond::MakeWholeSpec>,
}

#[cfg(test)]
mod tests;
