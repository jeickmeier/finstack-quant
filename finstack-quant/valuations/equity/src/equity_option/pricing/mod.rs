//! Equity-option pricing engines and market-input collection.
//!
//! Owns every pricing engine registered for `EquityOption`: Black–Scholes /
//! Leisen–Reimer trees, Heston Fourier, 1D/2D PDE, Heston Monte Carlo, and
//! rough-volatility (rough Heston and rough Bergomi) adapters.

mod black;
mod heston_fourier;
mod heston_mc;
pub(crate) mod inputs;
mod pde;
mod pde2d;
mod rough_bergomi_mc;
mod rough_heston_fourier;
mod rough_heston_market;
mod rough_heston_mc;

#[cfg(test)]
mod tests;

pub(crate) use black::{SimpleEquityOptionBlackPricer, compute_greeks, compute_pv};
pub(crate) use heston_fourier::EquityOptionHestonFourierPricer;
pub(crate) use heston_mc::EquityOptionHestonMcPricer;
pub(crate) use inputs::{
    ModelInputs, collect_inputs, has_future_discrete_dividends, model_error, model_inputs,
    stochastic_vol_inputs,
};
pub(crate) use pde::EquityOptionPdePricer;
pub(crate) use pde2d::EquityOptionHestonPdePricer;
pub(crate) use rough_bergomi_mc::EquityOptionRoughBergomiMcPricer;
pub(crate) use rough_heston_fourier::EquityOptionRoughHestonFourierPricer;
pub(crate) use rough_heston_mc::EquityOptionRoughHestonMcPricer;
