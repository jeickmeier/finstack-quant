//! Stochastic-process definitions used by the Monte Carlo engine.
//!
//! Start with [`gbm`] for vanilla equity / FX-style simulations and
//! [`brownian`] for additive Gaussian dynamics. This module also exposes
//! Heston, Dupire local volatility, CIR, Hull-White / Vasicek, and
//! Schwartz-Smith models.
//!
//! Important assumptions such as time units, rate / volatility quoting, and
//! state-vector layout are documented in each process module. Use
//! [`metadata::ProcessMetadata`] when captured paths need a stable schema for
//! downstream consumers.

pub mod brownian;
pub mod cheyette_rough;
pub mod cir;
pub mod gbm;
pub mod gbm_dividends;
pub mod heston;
pub mod lmm;
pub mod local_vol;
pub mod metadata;
pub mod multi_ou;
pub mod ou;
pub mod rough_bergomi;
pub mod rough_heston;
pub mod schwartz_smith;

pub use brownian::BrownianParams;
pub use gbm::{GbmParams, GbmProcess};
pub use metadata::ProcessMetadata;
pub use multi_ou::MultiOuParams;
