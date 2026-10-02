//! Heston model semi-analytical pricing via Fourier inversion.
//!
//! Implements the Heston (1993) characteristic function approach for
//! European option pricing under stochastic volatility.
//!
//! # Algorithm
//!
//! Uses the Gil-Pelaez / P1-P2 formulation:
//! ```text
//! C = S * exp(-qT) * P1 - K * exp(-rT) * P2
//! ```
//!
//! where P1 and P2 are risk-neutral probabilities computed via Fourier inversion
//! of the probability characteristic functions ψ_j(φ).
//!
//! # Numerical Stability
//!
//! Implements the "Little Heston Trap" formulation from Albrecher et al. (2007)
//! to avoid branch-cut discontinuities in the complex logarithm.
//!
//! # Conventions
//!
//! | Parameter | Convention | Units |
//! |-----------|-----------|-------|
//! | Rates (r, q) | Continuously compounded | Decimal (0.05 = 5%) |
//! | Variance (v0, theta) | Annualized variance | Decimal (0.04 = 20% vol) |
//! | Vol-of-vol (sigma_v) | Annualized | Decimal |
//! | Time (T) | ACT/365-style | Years |
//! | Prices | Per unit of underlying | Currency units |
//!
//! # Reference
//!
//! - Heston (1993) - "A Closed-Form Solution for Options with Stochastic Volatility" `docs/REFERENCES.md#heston-1993`
//! - Carr & Madan (1999) - "Option valuation using the fast Fourier transform" `docs/REFERENCES.md#carr-madan-1999-fft`
//! - Albrecher et al. (2007) - "The Little Heston Trap" `docs/REFERENCES.md#albrecher-2007-little-heston-trap`

mod characteristic_fn;
mod fourier_prices;
mod params;
mod quadrature;
mod strip_pricer;

pub use fourier_prices::{
    heston_call_price_fourier, heston_call_prices_fourier, heston_price, heston_put_price_fourier,
    heston_put_prices_fourier,
};
pub use params::{heston_defaults, HestonFourierSettings, HestonPricingParams};
pub use strip_pricer::HestonStripPricer;

#[cfg(test)]
use characteristic_fn::heston_pj_characteristic_function;
#[cfg(test)]
use fourier_prices::black_scholes_call;
#[cfg(test)]
use params::{HESTON_TAIL_DIAGNOSTIC_THRESHOLD, SUPPORTED_GL_ORDERS};
#[cfg(test)]
use quadrature::heston_pj_with_diagnostics;

#[cfg(test)]
mod tests;
