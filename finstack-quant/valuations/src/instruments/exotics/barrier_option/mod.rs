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
//! - **Discrete monitoring**: Monte Carlo or PDE on explicit observation dates
//! - See [`models::closed_form::barrier`](finstack_quant_models::closed_form::barrier) for formulas
//!
//! # Monitoring and rebates
//!
//! [`crate::instruments::Monitoring`] distinguishes continuous monitoring from
//! exact discrete observation dates. GBM continuous monitoring uses a log-Brownian
//! bridge; the Heston bridge freezes the path variance over a step and remains
//! an approximation requiring time-step convergence. Discrete paths do not
//! interpolate barrier crossings. Rebate Money is the total trade payment.
//!
//! # References
//!
//! - Reiner & Rubinstein (1991) - "Breaking Down the Barriers" `docs/REFERENCES.md#reiner-rubinstein-1991`
//! - Broadie, Glasserman & Kou (1997) - Discrete barrier correction `docs/REFERENCES.md#glasserman-2004-monte-carlo` `docs/REFERENCES.md#broadie-glasserman-kou-1997`
//!
//! # See Also
//!
//! - [`BarrierOption`] for instrument struct
//! - [`finstack_quant_core::types::BarrierType`] for up/down and in/out classification
//! - [`models::closed_form::barrier`](finstack_quant_models::closed_form::barrier) for pricing

pub(crate) mod metrics;
pub(crate) mod pde_pricer;
pub(crate) mod pricer;
pub(crate) mod types;

pub(crate) mod heston_mc_pricer;

pub use types::{BarrierOption, BarrierOptionBuilder};

crate::impl_equity_exotic_traits!(BarrierOption);
