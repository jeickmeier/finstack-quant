//! Metrics tests for interest rate options.
//!
//! Comprehensive tests for all Greeks and risk measures:
//! - Delta: First derivative w.r.t. forward rate
//! - Gamma: Second derivative w.r.t. forward rate
//! - Vega: Sensitivity to volatility
//! - Theta: Time decay
//! - Rho: Sensitivity to discount rate
//! - DV01: Dollar value of 01bp
//! - Forward PV01: Forward curve sensitivity
//! - Bucketed DV01: Term structure sensitivities
//! - Implied Vol: Reverse-solving for volatility

mod delta;
mod dv01;
mod forward_pv01;
mod gamma;
mod implied_vol;
mod rho;
mod theta;
mod vega;
