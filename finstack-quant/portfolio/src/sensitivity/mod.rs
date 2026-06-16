//! Factor sensitivity engines, JSON façade, and position parsing.
//!
//! Hosts the engines that consume `&dyn Instrument` and bump-and-reprice
//! against a `MarketContext`:
//!
//! - `DeltaBasedEngine`: linear sensitivity via finite-difference bumps.
//! - `FullRepricingEngine` + `ScenarioGrid`: P&L profile across a
//!   scenario grid.
//! - `FactorSensitivityEngine`: shared trait.
//!
//! `SensitivityMatrix` is re-exported from
//! [`finstack_quant_factor_model::sensitivity_matrix`] for binding stability.
//!
//! The `json` submodule holds the JSON-facing helpers used by Python and WASM
//! bindings; the `positions` submodule parses tagged position JSON into boxed
//! `Instrument` trait objects via the shared instrument JSON pipeline.

mod delta_engine;
pub mod json;
pub mod positions;
mod repricing_engine;
mod traits;

pub use delta_engine::{mapping_to_market_bumps, DeltaBasedEngine};
pub use finstack_quant_factor_model::sensitivity_matrix::SensitivityMatrix;
pub use json::{
    compute_factor_sensitivities_from_json, compute_factor_sensitivities_json,
    compute_pnl_profiles_from_json, compute_pnl_profiles_json, parse_bump_config_json,
    parse_factor_definitions_json, FactorPnlProfileJson, SensitivityMatrixJson,
    DEFAULT_PNL_SCENARIO_POINTS,
};
pub use positions::{parse_positions_json, pricing_positions, ParsedPosition};
pub use repricing_engine::{FactorPnlProfile, FullRepricingEngine, ScenarioGrid};
pub use traits::FactorSensitivityEngine;
