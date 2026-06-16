//! Equity options with Black-Scholes-Merton pricing and Greeks.
//!
//! Vanilla equity options (calls and puts) on single stocks or indices with
//! analytical and numerical pricing methods. Supports European and American
//! exercise styles with full Greeks calculation.
//!
//! # Option Basics
//!
//! - **Call option**: Right to buy stock at strike K
//!   - Payoff: max(S_T - K, 0)
//!   - Benefits when stock rises
//!
//! - **Put option**: Right to sell stock at strike K
//!   - Payoff: max(K - S_T, 0)
//!   - Benefits when stock falls
//!
//! # Pricing Models
//!
//! ## European Options: Black-Scholes-Merton
//!
//! Closed-form solution for European options with continuous dividends:
//!
//! **Call:**
//! ```text
//! C = S·e^(-qT)·N(d₁) - K·e^(-rT)·N(d₂)
//! ```
//!
//! **Put:**
//! ```text
//! P = K·e^(-rT)·N(-d₂) - S·e^(-qT)·N(-d₁)
//! ```
//!
//! where d₁, d₂ are standard Black-Scholes parameters.
//!
//! ## American Options: Tree or LSM
//!
//! - **Binomial/Trinomial trees**: Cox-Ross-Rubinstein (1979)
//! - **Longstaff-Schwartz**: Least-squares Monte Carlo for early exercise
//!
//! # Dividend Treatment
//!
//! - **Continuous yield**: Best for indices (e.g., S&P 500 with ~2% yield)
//! - **Discrete dividends**: For single stocks with known ex-dates
//! - **Escrowed div adjustment**: Reduce spot by PV of dividends during option life
//!
//! # Greeks (Risk Sensitivities)
//!
//! Standard option Greeks computed analytically:
//! - **Delta (Δ)**: Sensitivity to spot price (hedge ratio)
//! - **Gamma (Γ)**: Delta sensitivity (curvature)
//! - **Vega (ν)**: Sensitivity to volatility
//! - **Theta (Θ)**: Time decay
//! - **Rho (ρ)**: Sensitivity to interest rates
//!
//! Higher-order Greeks for exotic positions:
//! - **Vanna**: Cross-gamma (∂²V/∂S∂σ)
//! - **Volga (Vomma)**: Vega convexity (∂²V/∂σ²)
//! - **Charm**: Delta decay (∂²V/∂S∂t)
//! - **Color**: Gamma decay
//! - **Speed**: Gamma of gamma
//!
//! # References
//!
//! - Black, F., & Scholes, M. (1973). "The Pricing of Options and Corporate
//!   Liabilities." *Journal of Political Economy*, 81(3), 637-654.
//!
//! - Merton, R. C. (1973). "Theory of Rational Option Pricing."
//!   *Bell Journal of Economics and Management Science*, 4(1), 141-183.
//!
//! - Cox, J. C., Ross, S. A., & Rubinstein, M. (1979). "Option Pricing: A
//!   Simplified Approach." *Journal of Financial Economics*, 7(3), 229-263.
//!   (Binomial tree method)
//!
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.).
//!   Pearson.
//!
//! # Examples
//!
//! See [`EquityOption`] for construction and usage examples.
//!
//! # See Also
//!
//! - [`EquityOption`] for equity option struct
//! - equity option metrics module for complete Greeks calculations
//! - equity option pricer module for pricing implementations

pub(crate) mod metrics;
pub(crate) mod parameters;
pub(crate) mod pde2d_pricer;
pub(crate) mod pde_pricer;
pub(crate) mod pricer;
mod types;

pub(crate) mod heston_mc_pricer;
pub(crate) mod rough_bergomi_mc_pricer;
pub(crate) mod rough_heston_fourier_pricer;
pub(crate) mod rough_heston_market;
pub(crate) mod rough_heston_mc_pricer;

pub use parameters::{EquityOptionMarketData, EquityOptionParams};
pub use types::EquityOption;
