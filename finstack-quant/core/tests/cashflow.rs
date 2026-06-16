//! Cashflow module integration tests.
//!
//! This test suite verifies market-standard correctness for:
//! - CashFlow struct construction and validation
//! - NPV/discounting calculations
//! - XIRR/IRR calculations with reference golden values
//! - Day count conventions including leap year handling
//!
//! # Test Organization
//!
//! - `test_helpers`: Shared tolerance constants and test curves
//! - `primitives`: CashFlow struct construction and validation
//! - `discounting`: NPV calculations and discount factor properties
//! - `irr`: IRR/XIRR golden values, edge cases, and input validation
//! - `daycount`: Day count conventions and year fraction calculations

#[path = "cashflow/test_helpers.rs"]
mod test_helpers;

#[path = "cashflow/daycount.rs"]
mod daycount;

#[path = "cashflow/discounting.rs"]
mod discounting;

#[path = "cashflow/irr.rs"]
mod irr;

#[path = "cashflow/primitives.rs"]
mod primitives;
