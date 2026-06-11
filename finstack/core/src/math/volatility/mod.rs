//! Volatility conventions, pricing formulas, and conversion utilities.
//!
//! This module provides:
//! - **Pricing formulas**: Bachelier (normal) and Black-76 (lognormal) option pricing
//! - **Greeks**: Vega, delta, gamma for calibration and risk management
//! - **Convention handling**: Volatility type conversions (normal ↔ lognormal)
//!
//! # Market Conventions
//!
//! | Currency | Model | Vol Quote |
//! |----------|-------|-----------|
//! | USD | Black-76 | Lognormal |
//! | EUR | Bachelier | Normal (post-2015) |
//! | GBP | Black-76 | Lognormal |
//!
//! # Example
//!
//! ```rust
//! use finstack_core::math::volatility::{black_call, black_vega, bachelier_call};
//!
//! // Price a USD swaption (Black-76)
//! let fwd = 0.05;
//! let strike = 0.05;
//! let sigma = 0.20;  // 20% lognormal vol
//! let t = 1.0;
//!
//! let call_price = black_call(fwd, strike, sigma, t);
//! let vega = black_vega(fwd, strike, sigma, t);
//! ```

mod conventions;
mod convert;
pub mod heston;
mod implied;
pub mod local_vol;
mod pricing;
pub mod rough_heston;
pub mod sabr;
pub mod svi;

pub use conventions::VolatilityConvention;
pub use convert::convert_atm_volatility;

// Bachelier (normal) model - EUR swaptions, negative rates
pub use pricing::{
    bachelier_call, bachelier_delta_call, bachelier_delta_put, bachelier_gamma, bachelier_put,
    bachelier_vega,
};

// Black-76 (lognormal) model - USD/GBP swaptions, caps/floors
pub use pricing::{
    black_call, black_delta_call, black_delta_put, black_gamma, black_put, black_vega, d1_black76,
};
pub use pricing::{black_scholes_spot_call, black_scholes_spot_put, geometric_asian_call};

// Shifted Black model - low/negative rate environments
pub use pricing::{black_shifted_call, black_shifted_put, black_shifted_vega};

// Implied volatility initial guess approximations
pub use pricing::{
    brenner_subrahmanyam_approx, implied_vol_initial_guess, manaster_koehler_approx,
};

// Implied volatility solvers (bracketed bisection + Halley refinement)
pub use implied::{implied_vol_bachelier, implied_vol_black};
