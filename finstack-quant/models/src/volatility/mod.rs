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
//! - **FX delta quotes**: ATM DNS / risk-reversal / butterfly conversion and
//!   smile evaluation (`FxVolSurfaceBuilder`, `get_fx_delta_vol`)
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
//! ```
//! use finstack_quant_models::volatility::{d1_d2, norm_cdf};
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
//! - Black, F., & Scholes, M. (1973). "The Pricing of Options and Corporate Liabilities." `docs/REFERENCES.md#black-scholes-1973`
//! - Hagan, P. S., et al. (2002). "Managing Smile Risk." *Wilmott Magazine*. `docs/REFERENCES.md#hagan-2002-sabr`
//! - Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic Volatility." `docs/REFERENCES.md#heston-1993`
//! - Dupire, B. (1994). "Pricing with a Smile." *Risk Magazine*. `docs/REFERENCES.md#dupire-1994`
//!
//! # See Also
//!
//! - [`SabrModel`] for SABR smile interpolation
//! - [`FxVolSurfaceBuilder`] for FX delta-quoted smile materialization
//! - [`crate::closed_form`] for analytical formulas

pub mod arbitrage;
pub mod black;
mod conventions;
mod convert;
mod fx;
pub mod heston;
mod implied;
pub mod local_vol;
pub mod normal;
pub mod rough_heston;
pub mod sabr;
pub mod sabr_derivatives;
mod source;
pub mod svi;

pub use black::{d1, d1_black76, d1_d2, d1_d2_black76, d2, d2_black76};
pub use conventions::VolatilityConvention;
pub use convert::convert_atm_volatility;
pub use finstack_quant_core::math::{norm_cdf, norm_pdf};
pub use implied::{implied_vol_bachelier, implied_vol_black};
pub use normal::{bachelier_price, d_bachelier};
pub use sabr::{
    vega_weight, SabrCalibrationOutcome, SabrCalibrator, SabrModel, SabrParameters, SabrSmile,
};
pub use sabr_derivatives::{SabrCalibrationDerivatives, SabrMarketData};
pub use source::{
    get_cube_normal_vol, get_cube_normal_vol_clamped, get_cube_vol, get_cube_vol_clamped,
    get_surface_vol, get_surface_vol_clamped, get_surface_vol_extrapolated,
    materialize_cube_expiry_slice, materialize_cube_expiry_slice_normal, materialize_cube_grid,
    materialize_cube_tenor_slice, materialize_cube_tenor_slice_normal, measure_vol_surface_shift,
    VolSource,
};

pub use fx::{
    delta_to_strike, get_fx_delta_pillar_vols, get_fx_delta_vol, materialize_fx_delta_surface,
    strike_to_delta, FxVolSurfaceBuilder,
};
