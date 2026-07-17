//! Stochastic-process definitions used by the Monte Carlo engine.
//!
//! Start with [`gbm`] for vanilla equity / FX-style simulations and
//! [`brownian`] for additive Gaussian dynamics. This module also exposes
//! Heston, CIR, Hull-White / Vasicek, jump-diffusion, Bates, and
//! Schwartz-Smith models.
//!
//! The Bates process must be paired with its dedicated
//! [`crate::discretization::qe_bates::QeBates`] scheme — the generic Euler
//! schemes silently drop both its jumps and the spot–vol correlation while
//! still subtracting the jump compensator from the drift (see the
//! [`bates`] module docs).
//!
//! Important assumptions such as time units, rate / volatility quoting, and
//! state-vector layout are documented in each process module. Use
//! [`metadata::ProcessMetadata`] when captured paths need a stable schema for
//! downstream consumers.

pub mod bates;
pub mod brownian;
pub mod cheyette_rough;
pub mod cir;
pub mod gbm;
pub mod gbm_dividends;
pub mod heston;
pub mod jump_diffusion;
pub mod lmm;
pub mod metadata;
pub mod multi_ou;
pub mod ou;
pub mod rough_bergomi;
pub mod rough_heston;
pub mod schwartz_smith;

pub use brownian::{BrownianParams, BrownianProcess, MultiBrownianProcess};
pub use gbm::{GbmParams, GbmProcess, MultiGbmProcess};
pub use gbm_dividends::{Dividend, GbmWithDividends};
pub use metadata::ProcessMetadata;
pub use multi_ou::{MultiOuParams, MultiOuProcess};
