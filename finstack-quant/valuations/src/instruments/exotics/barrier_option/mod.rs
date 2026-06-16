//! Barrier option instruments with Reiner-Rubinstein formulas.
//!
//! Barrier options activate (knock-in) or deactivate (knock-out) when the
//! underlying price crosses a predetermined barrier level. Popular for
//! structured products and hedging with reduced premium cost.
//!
//! # Barrier Types
//!
//! - **Up-and-Out**: Deactivated when S > Barrier
//! - **Up-and-In**: Activated when S > Barrier
//! - **Down-and-Out**: Deactivated when S < Barrier
//! - **Down-and-In**: Activated when S < Barrier
//!
//! # Pricing Methods
//!
//! - **Continuous monitoring**: Analytical formulas (Reiner & Rubinstein 1991)
//! - **Discrete monitoring**: Monte Carlo with barrier adjustment
//! - See [`models::closed_form::barrier`](crate::models::closed_form::barrier) for formulas
//!
//! # Discrete Barrier Correction
//!
//! Real-world barriers are checked discretely (e.g., daily close). Broadie-Glasserman-Kou
//! correction adjusts barrier: H_adj = H · exp(±0.5826σ√Δt)
//!
//! # References
//!
//! - Reiner & Rubinstein (1991) - "Breaking Down the Barriers"
//! - Broadie, Glasserman & Kou (1997) - Discrete barrier correction
//!
//! # See Also
//!
//! - [`BarrierOption`] for instrument struct
//! - [`BarrierType`] for up/down and in/out classification
//! - [`models::closed_form::barrier`](crate::models::closed_form::barrier) for pricing

pub(crate) mod metrics;
pub(crate) mod pde_pricer;
pub(crate) mod pricer;
pub(crate) mod traits;
pub(crate) mod types;

pub(crate) mod heston_mc_pricer;

pub use types::{BarrierOption, BarrierOptionBuilder, BarrierType};
