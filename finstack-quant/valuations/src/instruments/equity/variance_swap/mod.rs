//! Variance swap instruments for pure volatility exposure.
//!
//! Variance swaps are forward contracts on realized variance, providing
//! pure volatility exposure without delta risk. They pay the difference
//! between realized variance and a strike variance level.
//!
//! # Structure
//!
//! - **Variance notional**: Notional in variance units (not vega notional)
//! - **Strike variance**: Agreed variance level (σ²)
//! - **Observation period**: Period over which variance is measured
//! - **Observation frequency**: Daily, weekly, or business days
//!
//! # Payoff at Maturity
//!
//! ```text
//! Payoff = Notional_var × (σ²_realized - σ²_strike)
//! ```
//!
//! For **vega notional** (market convention):
//! ```text
//! Notional_var = Notional_vega / (2 × σ_strike)
//! ```
//!
//! # Realized Variance Calculation
//!
//! For n observations with returns r_i:
//!
//! ```text
//! σ²_realized = (252/n) × Σᵢ r²ᵢ  (annualized, assuming 252 trading days)
//! ```
//!
//! Returns typically calculated as:
//! - **Close-to-close**: ln(S_i / S_{i-1})
//! - **OHLC**: Incorporating open, high, low, close (Garman-Klass)
//!
//! # Pricing
//!
//! Before expiration, variance swap value has two components:
//!
//! ```text
//! PV = N_var × [σ²_partial_realized + σ²_forward_implied - σ²_strike] × DF(T)
//! ```
//!
//! where σ²_partial_realized is variance from historical observations.
//!
//! # Relationship to Volatility Swaps
//!
//! Volatility swaps pay on volatility (σ) not variance (σ²):
//! ```text
//! Var swap payoff = Notional × (σ²_realized - K²)
//! Vol swap payoff = Notional × (σ_realized - K)
//! ```
//!
//! Variance swaps are more liquid and easier to hedge via options.
//!
//! # References
//!
//! - Demeterfi, K., Derman, E., Kamal, M., & Zou, J. (1999). "More Than You
//!   Ever Wanted to Know About Volatility Swaps." *Goldman Sachs Quantitative
//!   Strategies Research Notes*.
//!
//! - Carr, P., & Madan, D. (1998). "Towards a Theory of Volatility Trading."
//!   *Volatility: New Estimation Techniques for Pricing Derivatives*, 417-427.
//!
//! # See Also
//!
//! - [`VarianceSwap`] for instrument struct
//! - `RealizedVarMethod` for variance calculation methods
//! - Variance calculation functions in [`finstack_quant_core::math::stats`]

pub(crate) mod metrics;
/// Variance swap pricer implementation
pub(crate) mod pricer;
pub(crate) mod types;

pub use types::{PayReceive, VarianceSwap};

// Re-export from core
pub use finstack_quant_core::math::stats::{
    realized_variance, realized_variance_ohlc, RealizedVarMethod,
};
