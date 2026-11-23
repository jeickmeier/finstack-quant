//! Cheyette + rough stochastic volatility model for interest rates.
//!
//! The Cheyette framework (1-factor Markovian HJM) tracks the short rate via a
//! two-dimensional Markov state (x, y) with the instantaneous forward rate
//! f(0, t) as a deterministic shift.  This module augments that framework with
//! rough stochastic volatility, where the rate diffusion coefficient σ(t) is
//! driven by a Volterra fractional Brownian motion as in the rBergomi paradigm.
//!
//! # Stochastic Differential Equations
//!
//! ```text
//! r(t) = x(t) + φ(t)
//! dx(t) = [y(t) − κ·x(t)] dt + σ(t)·dW(t)
//! dy(t) = [σ(t)² − 2κ·y(t)] dt
//!
//! σ(t)  = σ₀(t) · exp(½·η·W̃_H(t) − ¼·η²·t^{2H})
//! ```
//!
//! The vol-of-vol convention follows rBergomi **variance-lognormal**
//! semantics: the instantaneous variance `V(t) = σ(t)²
//! = σ₀(t)²·exp(η·W̃_H(t) − ½·η²·t^{2H})` is lognormal with full
//! compensation, so `E[σ²(t)] = σ₀²(t)` for every `t` and `η` is directly
//! comparable to the rBergomi vol-of-vol. (The previous σ-lognormal
//! convention `σ = σ₀·exp(η·W̃ − ½η²t^{2H})` compensated the *vol* level
//! instead, inflating `E[σ²]` by `exp(η²·t^{2H})` — ≈22× at η=1.5, t=5y,
//! H=0.1.)
//!
//! where:
//! - **x(t)**: Rate state variable (deviation from initial forward curve)
//! - **y(t)**: Accumulated variance state variable
//! - **φ(t) = f(0, t)**: Instantaneous forward rate from the initial curve
//! - **σ₀(t)**: Base (deterministic) volatility term structure
//! - **W̃_H(t)**: Volterra fBM with Hurst exponent H ∈ (0, 0.5)
//! - **η**: Vol-of-vol scaling
//! - **ρ**: Correlation between rate BM W and vol fBM W̃_H
//! - **κ**: Mean reversion speed for the short rate
//!
//! # Design Notes
//!
//! Because σ(t) depends on the entire fBM path history, the standard
//! `drift`/`diffusion` interface cannot express the non-Markovian volatility
//! dynamics.  Accordingly, `drift` and `diffusion` are implemented as formal
//! no-ops — the actual dynamics are handled entirely by the dedicated
//! [`CheyetteRoughEuler`](super::super::discretization::cheyette_rough::CheyetteRoughEuler)
//! discretization, which tracks the accumulated Volterra integral internally.
//!
//! # References
//!
//! - Cheyette, O. (1994). "Markov Representation of the Heath-Jarrow-Morton
//!   Model." Working paper. `docs/REFERENCES.md#jarrow-lando-turnbull-1997`
//! - Andersen, L. & Piterbarg, V. (2010). *Interest Rate Modeling*, Volume II,
//!   Chapter 12 (Cheyette / quasi-Gaussian models). `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
//! - Bayer, C., Friz, P. & Gatheral, J. (2016). "Pricing under rough
//!   volatility." *Quantitative Finance*, 16(6), 887–904. `docs/REFERENCES.md#bayer-friz-gatheral-2016`
//! - Gatheral, J., Jaisson, T. & Rosenbaum, M. (2018). "Volatility is rough."
//!   *Quantitative Finance*, 18(6), 933–949. `docs/REFERENCES.md#gatheral-jaisson-rosenbaum-2018`
//!
//! # Examples
//!
//! ```
//! use finstack_quant_models::monte_carlo::process::cheyette_rough::{
//!     BaseVolatility, CheyetteRoughVolProcess, CheyetteRoughVolParams,
//! };
//! use finstack_quant_models::monte_carlo::traits::StochasticProcess;
//! use finstack_quant_core::math::fractional::HurstExponent;
//!
//! let params = CheyetteRoughVolParams::new(
//!     0.03,                                       // κ = 3% mean reversion
//!     BaseVolatility::flat(0.005).unwrap(),  // σ₀ = 50bps base vol
//!     HurstExponent::new(0.1).unwrap(),            // H = 0.1 (rough)
//!     1.5,                                         // η = vol-of-vol
//!     -0.3,                                        // ρ = rate-vol correlation
//!     &[(0.0, 0.02), (5.0, 0.025), (30.0, 0.03)], // φ(t) = f(0, t) curve
//! )
//! .unwrap();
//!
//! let process = CheyetteRoughVolProcess::new(params);
//! assert_eq!(process.dim(), 2);
//! assert_eq!(process.num_factors(), 3);
//! ```

mod base_volatility;
mod process;
#[cfg(test)]
mod tests;

pub use base_volatility::BaseVolatility;
pub use process::{CheyetteRoughVolParams, CheyetteRoughVolProcess};
