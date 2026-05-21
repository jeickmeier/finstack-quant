//! Volatility models and Black-Scholes pricing helpers.
//!
//! This module provides stochastic volatility models, local volatility surfaces,
//! and fundamental Black-Scholes building blocks used throughout the pricing
//! framework.
//!
//! # Features
//!
//! - **Black-Scholes Helpers**: d₁, d₂, N(x) for option pricing
//! - **SABR Model**: Stochastic alpha-beta-rho for smile calibration
//! - **Heston Model**: Stochastic volatility with mean reversion
//! - **Local Volatility**: Dupire local vol surface construction
//! - **Normal Model**: Bachelier pricing for negative rates
//!
//! # Volatility Models
//!
//! | Model | Use Case | Calibration |
//! |-------|----------|-------------|
//! | Black-Scholes | Vanilla options | Single implied vol |
//! | SABR | Smile/skew fitting | α, β, ρ, ν to market quotes |
//! | Heston | Exotic path-dependent | κ, θ, σ, ρ, v₀ to surface |
//! | Local Vol | Barrier options | Dupire from call prices |
//! | Normal | Rate options | Bachelier vol |
//!
//! # SABR Model
//!
//! The SABR model captures volatility smile dynamics:
//!
//! ```text
//! dF = σ F^β dW₁
//! dσ = ν σ dW₂
//! ⟨dW₁, dW₂⟩ = ρ dt
//! ```
//!
//! where β controls backbone, ρ controls skew, ν controls smile wings.
//!
//! # Quick Example
//!
//! ```ignore
//! use finstack_valuations::instruments::models::volatility::{d1_d2, norm_cdf};
//!
//! let spot = 100.0;
//! let strike = 105.0;
//! let rate = 0.05;
//! let vol = 0.20;
//! let time = 0.5;
//! let div = 0.02;
//!
//! let (d1, d2) = d1_d2(spot, strike, rate, vol, time, div);
//! let call_delta = (-div * time).exp() * norm_cdf(d1);
//! ```
//!
//! # Academic References
//!
//! - Black, F., & Scholes, M. (1973). "The Pricing of Options and Corporate Liabilities."
//! - Hagan, P. S., et al. (2002). "Managing Smile Risk." *Wilmott Magazine*.
//! - Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic Volatility."
//! - Dupire, B. (1994). "Pricing with a Smile." *Risk Magazine*.
//!
//! # See Also
//!
//! - [`SABRModel`] for SABR smile interpolation
//! - [`HestonModel`] for stochastic volatility
//! - [`LocalVolSurface`] for Dupire local vol
//! - [`crate::instruments::models::closed_form`] for analytical formulas

pub mod black;
pub mod heston;
pub mod local_vol;
pub mod normal;
pub mod sabr;
pub mod sabr_derivatives;

pub use black::{d1, d1_black76, d1_d2, d1_d2_black76, d2, d2_black76};
pub use finstack_core::math::{norm_cdf, norm_pdf};
pub use heston::{HestonModel, HestonParameters};
pub use local_vol::{DupireParams, LocalVolBuilder, LocalVolSurface};
pub use normal::{bachelier_price, d_bachelier};
pub use sabr::{SABRCalibrator, SABRModel, SABRParameters, SABRSmile};
pub use sabr_derivatives::{SABRCalibrationDerivatives, SABRMarketData};
