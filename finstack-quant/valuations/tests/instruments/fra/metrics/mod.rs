//! FRA metrics test suite.
//!
//! Comprehensive tests for all FRA metric calculators including:
//! - PV (present value passthrough)
//! - DV01 (dollar value of a basis point)
//! - Par Rate (forward rate that zeroes PV)
//! - Theta (time decay)
//! - Bucketed DV01 (risk by tenor bucket)

mod bucketed_dv01;
mod dv01;
mod par_rate;
mod pv;
mod theta;
