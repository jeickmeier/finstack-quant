//! Convertible bond pricing model using Tsiveriotis-Zhang tree.
//!
//! Implements a hybrid debt-equity pricing model that:
//! 1. Uses `CashFlowBuilder` to generate the bond's coupon schedule
//! 2. Applies Tsiveriotis-Zhang tree decomposition to capture the equity conversion option
//!    while accounting for credit risk on the cash-only component.
//! 3. Handles call/put provisions and various conversion policies
//!
//! Public API:
//! - `price_convertible_bond`: Present value using selected tree type
//! - `calculate_convertible_greeks`: Tree-based Greeks and price (central differences)
//! - `calculate_parity`: Equity parity ratio
//! - `calculate_conversion_premium`: Conversion premium versus equity value
//! - `calculate_accrued_interest`: Accrued coupon interest as of valuation date

mod engine;
mod tree_pricer;
mod tsiveriotis_zhang;
mod valuator;

pub(crate) use engine::{build_convertible_schedule, compute_conversion_value, price_bond_floor};
pub use engine::{
    calculate_accrued_interest, calculate_conversion_premium, calculate_convertible_greeks,
    calculate_parity, price_convertible_bond, settlement_date, ConvertibleTreeType,
};
pub(crate) use tree_pricer::ConvertibleTreePricer;

#[cfg(test)]
use engine::prepare_for_pricing;
#[cfg(test)]
use valuator::ConvertibleBondValuator;

#[cfg(test)]
mod tests;
