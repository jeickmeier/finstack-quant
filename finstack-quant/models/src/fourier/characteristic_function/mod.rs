//! Characteristic function trait and model implementations for Fourier pricing.
//!
//! Provides a generic [`CharacteristicFunction`] trait representing the characteristic
//! function of a log-price process phi(u, t) = E[exp(i * u * ln(S_T / S_0))].
//! All Fourier pricing methods (COS, Lewis, FFT) consume this trait.
//!
//! # Model Implementations
//!
//! - [`BlackScholesCf`]: Geometric Brownian motion
//! - [`VarianceGammaCf`]: Time-changed Brownian motion with Gamma subordinator
//! - [`MertonJumpCf`]: GBM plus compound Poisson jumps with log-normal sizes
//!
//! # References
//!
//! - Cont, R. & Tankov, P. (2004). *Financial Modelling with Jump Processes*.
//!   Chapman & Hall/CRC. Chapter 11. `docs/REFERENCES.md#cont-tankov-2004`

mod black_scholes;
mod merton;
#[cfg(test)]
mod tests;
mod traits;
mod variance_gamma;

pub use black_scholes::BlackScholesCf;
pub use merton::MertonJumpCf;
#[cfg(test)]
pub(crate) use tests::cumulants_from_cf;
pub use traits::{CharacteristicFunction, Cumulants};
pub use variance_gamma::VarianceGammaCf;
