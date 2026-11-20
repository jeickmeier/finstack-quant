//! WASM bindings for the `finstack-quant-models` crate.
//!
//! Split by model family:
//! - [`closed_form`] — closed-form option primitives.
//! - [`fourier`] — COS-method Fourier pricers.
//! - [`volatility`] — volatility models, evaluators, and convention conversion.
//! - [`credit`] — structural-credit, LGD, PD, migration, scoring, recovery-waterfall
//!   and liability-management models.
//! - [`factor`] — credit factor hierarchy and factor-risk decomposition.
//! - [`correlation`] — copula, recovery, and joint-probability utilities.
//! - [`monte_carlo`] — stochastic option-pricing convenience functions.
//! - [`liquidity`] — liquidity estimation, risk, and market-impact models.
//! - [`rates`] — interest-rate models and dynamic term-structure engines.

pub mod closed_form;
pub mod correlation;
pub mod credit;
pub mod factor;
pub mod fourier;
pub mod liquidity;
pub mod monte_carlo;
pub mod rates;
pub mod volatility;
