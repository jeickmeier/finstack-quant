//! Convertible pricing on a binomial equity lattice, with two credit models.
//!
//! Both models:
//! 1. Use `CashFlowBuilder` for coupons, principal and PIK balances.
//! 2. Handle call/put provisions and various conversion policies.
//!
//! They differ in how issuer credit enters:
//! - `tree` (the default): the stock jumps to zero on default,
//!   surviving claims are discounted and recovery of live principal is paid.
//! - `tsiveriotis_fernandes`: no default event; the cash-only part
//!   of the bond is discounted at the risk-free rate plus the credit spread.
//!
//! Public API:
//! - `price_convertible_bond`: Present value using selected tree type
//! - `calculate_convertible_greeks`: Tree-based Greeks and price (central differences)
//! - `calculate_parity`: Equity parity ratio
//! - `calculate_accrued_interest`: Accrued coupon interest as of valuation date
//! - `convertible_spot_ladder`: Price, delta, parity, premium and regime across share prices

mod credit_tree;
pub(crate) mod engine;
mod exercise_level;
pub(crate) mod spot_ladder;
mod tree_pricer;
mod valuator;

pub(crate) use engine::{
    ConvertibleModel, build_convertible_schedule, calculate_accrued_interest,
    calculate_convertible_greeks, calculate_convertible_greeks_with_model, calculate_parity,
    compute_conversion_value, price_bond_floor, price_convertible_bond,
    price_convertible_bond_with_model, settlement_dirty_price,
};
#[cfg(test)]
pub(crate) use engine::{DEFAULT_CONVERTIBLE_TREE_STEPS, settlement_date};
pub(crate) use tree_pricer::{ConvertibleTreePricer, ConvertibleTsiveriotisFernandesPricer};

#[cfg(test)]
use engine::prepare_for_pricing;
#[cfg(test)]
use valuator::ConvertibleBondValuator;

#[cfg(test)]
mod tests;
