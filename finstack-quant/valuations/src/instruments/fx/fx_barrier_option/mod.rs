//! FX barrier options with Garman-Kohlhagen and barrier adjustments.
//!
//! FX barrier options combine currency option features with knock-in/out
//! barriers. Popular for structured FX products and cost reduction vs
//! vanilla FX options.
//!
//! # Structure
//!
//! Combines FX option (Garman-Kohlhagen) with barrier feature:
//! - **Underlying**: Currency pair (e.g., EUR/USD)
//! - **Barrier**: Level that activates or deactivates the option
//! - **Option type**: Call or put on foreign currency
//! - **Barrier type**: Up/down and in/out
//!
//! # Pricing
//!
//! - **Analytical**: Reiner-Rubinstein formulas adapted for FX
//! - **Discrete barriers**: Monte Carlo with adjustment
//!
//! # References
//!
//! - Reiner, E., & Rubinstein, M. (1991). "Breaking Down the Barriers."
//!   *Risk Magazine*, 4(8), 28-35.
//!
//! - Wystup, U. (2006). *FX Options and Structured Products*. Wiley.
//!
//! # See Also
//!
//! - [`FxBarrierOption`] for instrument struct
//! - [`crate::instruments::fx::fx_option`] for vanilla FX options

pub(crate) mod metrics;
pub mod monte_carlo;
pub(crate) mod pricer;
pub(crate) mod types;

pub use types::FxBarrierOption;
