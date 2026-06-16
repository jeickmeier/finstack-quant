//! Term-structure tests (curves/surfaces) that are independent of calibration APIs.
//!
//! ## Test Organization
//!
//! - `curve_monotonicity` - Property-based tests for discount factor monotonicity
//! - `forward_parity` - Property-based tests for forward rate parity relationships

mod curve_monotonicity;
mod forward_parity;
