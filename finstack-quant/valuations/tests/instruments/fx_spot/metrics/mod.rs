//! FX Spot metrics comprehensive test suite.
//!
//! Tests for all metric calculators:
//! - `base_amount`: Base currency notional
//! - `quote_amount`: Quote currency PV
//! - `spot_rate`: Realized spot rate
//! - `inverse_rate`: Inverse of spot rate
//! - `dv01`: FX sensitivity to rates
//! - `theta`: Time decay

mod base_amount;
mod dv01;
mod inverse_rate;
mod quote_amount;
mod spot_rate;
mod theta;
