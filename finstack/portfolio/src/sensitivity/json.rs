//! JSON-facing helpers for factor-model bindings.

use super::{
    pricing_positions, DeltaBasedEngine, FactorPnlProfile, FactorSensitivityEngine,
    FullRepricingEngine, SensitivityMatrix,
};
use finstack_core::dates::Date;
use finstack_core::factor_model::{BumpSizeConfig, FactorDefinition};
use finstack_core::market_data::context::MarketContext;
use finstack_core::{Error, Result};
use serde::Serialize;

/// Default scenario count for symmetric P&L profile grids.
///
/// `5` produces `[-2, -1, 0, 1, 2]`.
pub const DEFAULT_PNL_SCENARIO_POINTS: usize = 5;

/// JSON shape returned by WASM factor sensitivity helpers.
#[derive(Debug, Clone, Serialize)]
pub struct SensitivityMatrixJson {
    /// Ordered position identifiers.
    pub position_ids: Vec<String>,
    /// Ordered factor identifiers.
    pub factor_ids: Vec<String>,
    /// Row-major matrix as nested rows.
    pub data: Vec<Vec<f64>>,
}

/// JSON shape returned by WASM P&L profile helpers.
#[derive(Debug, Clone, Serialize)]
pub struct FactorPnlProfileJson {
    /// Shocked factor identifier.
    pub factor_id: String,
    /// Scenario shift coordinates.
    pub shifts: Vec<f64>,
    /// P&L rows indexed as `[shift_idx][position_idx]`.
    pub position_pnls: Vec<Vec<f64>>,
}

/// Parse factor definitions from their JSON binding representation.
pub fn parse_factor_definitions_json(json: &str) -> Result<Vec<FactorDefinition>> {
    serde_json::from_str(json)
        .map_err(|e| Error::Validation(format!("invalid factor definitions JSON: {e}")))
}

/// Parse optional bump configuration JSON, defaulting to the canonical config.
pub fn parse_bump_config_json(json: Option<&str>) -> Result<BumpSizeConfig> {
    match json {
        Some(json) => serde_json::from_str(json)
            .map_err(|e| Error::Validation(format!("invalid bump config JSON: {e}"))),
        None => Ok(BumpSizeConfig::default()),
    }
}

/// Compute factor sensitivities from JSON binding inputs.
pub fn compute_factor_sensitivities_from_json(
    positions_json: &str,
    factors_json: &str,
    market: &MarketContext,
    as_of: Date,
    bump_config_json: Option<&str>,
) -> Result<SensitivityMatrix> {
    let parsed_positions = super::parse_positions_json(positions_json)?;
    let positions = pricing_positions(&parsed_positions);
    let factors = parse_factor_definitions_json(factors_json)?;
    let bump_config = parse_bump_config_json(bump_config_json)?;
    let engine = DeltaBasedEngine::new(bump_config);
    engine.compute_sensitivities(&positions, &factors, market, as_of)
}

/// Compute factor sensitivities from all-JSON binding inputs.
pub fn compute_factor_sensitivities_json(
    positions_json: &str,
    factors_json: &str,
    market_json: &str,
    as_of: &str,
    bump_config_json: Option<&str>,
) -> Result<String> {
    let market: MarketContext = serde_json::from_str(market_json)
        .map_err(|e| Error::Validation(format!("invalid market JSON: {e}")))?;
    let as_of = finstack_valuations::pricer::parse_as_of_date(as_of)?;
    let matrix = compute_factor_sensitivities_from_json(
        positions_json,
        factors_json,
        &market,
        as_of,
        bump_config_json,
    )?;
    let output = SensitivityMatrixJson::from(&matrix);
    serde_json::to_string(&output)
        .map_err(|e| Error::Validation(format!("failed to serialize sensitivity matrix: {e}")))
}

/// Compute P&L profiles from JSON binding inputs.
pub fn compute_pnl_profiles_from_json(
    positions_json: &str,
    factors_json: &str,
    market: &MarketContext,
    as_of: Date,
    bump_config_json: Option<&str>,
    n_scenario_points: usize,
) -> Result<Vec<FactorPnlProfile>> {
    let parsed_positions = super::parse_positions_json(positions_json)?;
    let positions = pricing_positions(&parsed_positions);
    let factors = parse_factor_definitions_json(factors_json)?;
    let bump_config = parse_bump_config_json(bump_config_json)?;
    let engine = FullRepricingEngine::try_new(bump_config, n_scenario_points)?;
    engine.compute_pnl_profiles(&positions, &factors, market, as_of)
}

/// Compute P&L profiles from all-JSON binding inputs.
pub fn compute_pnl_profiles_json(
    positions_json: &str,
    factors_json: &str,
    market_json: &str,
    as_of: &str,
    bump_config_json: Option<&str>,
    n_scenario_points: Option<usize>,
) -> Result<String> {
    let market: MarketContext = serde_json::from_str(market_json)
        .map_err(|e| Error::Validation(format!("invalid market JSON: {e}")))?;
    let as_of = finstack_valuations::pricer::parse_as_of_date(as_of)?;
    let profiles = compute_pnl_profiles_from_json(
        positions_json,
        factors_json,
        &market,
        as_of,
        bump_config_json,
        n_scenario_points.unwrap_or(DEFAULT_PNL_SCENARIO_POINTS),
    )?;
    let output: Vec<FactorPnlProfileJson> =
        profiles.iter().map(FactorPnlProfileJson::from).collect();
    serde_json::to_string(&output)
        .map_err(|e| Error::Validation(format!("failed to serialize P&L profiles: {e}")))
}

impl From<&SensitivityMatrix> for SensitivityMatrixJson {
    fn from(matrix: &SensitivityMatrix) -> Self {
        Self {
            position_ids: matrix.position_ids().to_vec(),
            factor_ids: matrix
                .factor_ids()
                .iter()
                .map(ToString::to_string)
                .collect(),
            data: (0..matrix.n_positions())
                .map(|idx| matrix.position_deltas(idx).to_vec())
                .collect(),
        }
    }
}

impl From<&FactorPnlProfile> for FactorPnlProfileJson {
    fn from(profile: &FactorPnlProfile) -> Self {
        Self {
            factor_id: profile.factor_id.to_string(),
            shifts: profile.shifts.clone(),
            position_pnls: profile.position_pnls.clone(),
        }
    }
}
