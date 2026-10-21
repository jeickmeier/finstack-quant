//! Factor sensitivity engines and the portfolio-keyed sensitivity entry points.
//!
//! Hosts the engines that consume `&dyn Instrument` and bump-and-reprice
//! against a `MarketContext`:
//!
//! - `DeltaBasedEngine`: linear sensitivity via finite-difference bumps.
//! - `FullRepricingEngine`: P&L profile across a symmetric scenario grid.
//! - `FactorSensitivityEngine`: shared trait.
//!
//! The engines return [`finstack_quant_factor::SensitivityMatrix`]; its one
//! wire form is [`SensitivityMatrixJson`],
//! shared by both hosts in both directions.
//!
//! [`compute_factor_sensitivities`] and [`compute_pnl_profiles`] run those
//! engines over a [`crate::Portfolio`] (rows in position order, weighted by
//! `Position::scale_factor`, reported in `portfolio.base_currency`).

mod compute;
mod delta_engine;
mod json;
mod repricing_engine;
mod traits;

pub use compute::{compute_factor_sensitivities, compute_pnl_profiles};
pub(crate) use compute::{portfolio_rows, uses_assignment_driven_credit_shock};
pub(crate) use delta_engine::mapping_to_market_bumps;
pub use delta_engine::DeltaBasedEngine;
pub use json::{SensitivityMatrixJson, DEFAULT_PNL_SCENARIO_POINTS};
pub use repricing_engine::{FactorPnlProfile, FullRepricingEngine};
pub(crate) use traits::exact_factor_market_keys;
pub use traits::FactorSensitivityEngine;
