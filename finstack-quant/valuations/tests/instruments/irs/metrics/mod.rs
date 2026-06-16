//! IRS metrics calculator tests.
//!
//! Individual tests for each metric calculator:
//! - annuity: Fixed leg annuity calculation
//! - dv01: Dollar value of a basis point
//! - par_rate: Par swap rate calculation
//! - pv_fixed: Fixed leg present value
//! - pv_float: Floating leg present value
//! - theta: Time decay
//! - bucketed_dv01: Risk by tenor bucket

mod annuity;
mod bucketed_dv01;
mod dv01;
mod ir_convexity;
mod par_rate;
mod pv_fixed;
mod pv_float;
mod theta;
