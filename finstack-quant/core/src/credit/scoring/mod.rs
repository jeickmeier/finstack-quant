//! Academic credit scoring models: Altman Z-Score family, Ohlson O-Score,
//! and Zmijewski probit model.
//!
//! Each model takes an explicit input struct of financial ratios and returns
//! a [`ScoringResult`] with raw score, zone classification, and implied PD.
//!
//! # Models
//!
//! - [`altman_z_score`]: Original Altman Z-Score (1968) for public manufacturing firms.
//! - [`altman_z_prime`]: Z'-Score variant for private firms.
//! - [`altman_z_double_prime`]: Z''-Score variant for emerging markets / non-manufacturing.
//! - [`ohlson_o_score`]: Ohlson O-Score (1980) nine-predictor logistic model.
//! - [`zmijewski_score`]: Zmijewski (1984) three-predictor probit model.
//!
//! # Examples
//!
//! ```
//! use finstack_quant_core::credit::scoring::{AltmanZScoreInput, altman_z_score, ScoringZone};
//!
//! let input = AltmanZScoreInput {
//!     working_capital_to_total_assets: 0.10,
//!     retained_earnings_to_total_assets: 0.20,
//!     ebit_to_total_assets: 0.15,
//!     market_equity_to_total_liabilities: 1.50,
//!     sales_to_total_assets: 1.80,
//! };
//! let result = altman_z_score(&input).unwrap();
//! assert!(result.score > 2.99); // Safe zone
//! assert_eq!(result.zone, ScoringZone::Safe);
//! ```

pub mod altman;
pub mod ohlson;
#[cfg(test)]
mod tests;
pub mod types;
pub mod zmijewski;

// Re-exports
pub use altman::{
    altman_z_double_prime, altman_z_prime, altman_z_score, AltmanZDoublePrimeInput,
    AltmanZPrimeInput, AltmanZScoreInput,
};
pub use ohlson::{ohlson_o_score, OhlsonOScoreInput};
pub use types::{CreditScoringError, ScoringResult, ScoringZone};
pub use zmijewski::{zmijewski_score, ZmijewskiInput};
