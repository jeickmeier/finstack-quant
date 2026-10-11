//! Convertible pricing with defaultable equity and recovery of principal.
//!
//! Implements a hybrid debt-equity pricing model that:
//! 1. Uses `CashFlowBuilder` for coupons, principal and PIK balances.
//! 2. Discounts surviving claims and pays recovery of live principal on default.
//! 3. Handles call/put provisions and various conversion policies
//!
//! Public API:
//! - `price_convertible_bond`: Present value using selected tree type
//! - `calculate_convertible_greeks`: Tree-based Greeks and price (central differences)
//! - `calculate_parity`: Equity parity ratio
//! - `calculate_accrued_interest`: Accrued coupon interest as of valuation date

mod credit_tree;
mod engine;
mod tree_pricer;
mod valuator;

pub(crate) use engine::{
    build_convertible_schedule, compute_conversion_value, price_bond_floor, settlement_dirty_price,
};
pub use engine::{
    calculate_accrued_interest, calculate_convertible_greeks, calculate_parity,
    price_convertible_bond, settlement_date, DEFAULT_CONVERTIBLE_TREE_STEPS,
};
pub(crate) use tree_pricer::ConvertibleTreePricer;

#[cfg(test)]
use engine::prepare_for_pricing;
#[cfg(test)]
use valuator::ConvertibleBondValuator;

#[cfg(test)]
mod tests;
