//! JSON-facing helpers for factor-model bindings.

use super::{
    pricing_positions, DeltaBasedEngine, FactorPnlProfile, FactorSensitivityEngine,
    FullRepricingEngine, SensitivityMatrix,
};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{Error, Result};
use finstack_quant_factor_model::{BumpSizeConfig, FactorDefinition};
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

/// Parse factor definitions from the binding JSON representation.
///
/// The input must encode the complete [`FactorDefinition`] array accepted by
/// the sensitivity engines; no market data is resolved at this stage.
///
/// # Arguments
///
/// * `json` - UTF-8 JSON array of complete canonical factor definitions.
///
/// # Errors
///
/// Returns [`Error::Validation`] when `json` is not a valid factor-definition
/// array or does not match its serialized field and enum shapes.
pub fn parse_factor_definitions_json(json: &str) -> Result<Vec<FactorDefinition>> {
    serde_json::from_str(json)
        .map_err(|e| Error::Validation(format!("invalid factor definitions JSON: {e}")))
}

/// Parse optional sensitivity-bump configuration, defaulting when omitted.
///
/// `None` selects [`BumpSizeConfig::default`]; `Some` must hold the complete
/// serialized configuration rather than a partial override.
///
/// # Arguments
///
/// * `json` - Optional UTF-8 JSON bump configuration; `None` selects the
///   canonical default and `Some` replaces it entirely.
///
/// # Errors
///
/// Returns [`Error::Validation`] when a supplied string cannot be deserialized
/// as [`BumpSizeConfig`].
pub fn parse_bump_config_json(json: Option<&str>) -> Result<BumpSizeConfig> {
    match json {
        Some(json) => serde_json::from_str(json)
            .map_err(|e| Error::Validation(format!("invalid bump config JSON: {e}"))),
        None => Ok(BumpSizeConfig::default()),
    }
}

/// Compute position-by-factor sensitivities from JSON binding inputs.
///
/// Positions and factor definitions are parsed from JSON, while `market` and
/// `as_of` are already-typed Rust values. The result preserves engine ordering:
/// rows correspond to priced positions and columns to the supplied factors.
///
/// # Arguments
///
/// * `positions_json` - UTF-8 JSON array of supported position definitions;
///   its position IDs determine result-row order.
/// * `factors_json` - UTF-8 JSON factor-definition array; its factor IDs
///   determine result-column order and bump semantics.
/// * `market` - Typed market snapshot used to price the positions before and
///   after each factor bump.
/// * `as_of` - Valuation date applied to instrument pricing and market-data
///   lookups.
/// * `bump_config_json` - Optional UTF-8 JSON [`BumpSizeConfig`]; `None`
///   selects canonical bump sizes and `Some` replaces that configuration.
///
/// # Errors
///
/// Propagates invalid position, factor, or bump JSON; unsupported factor
/// definitions; and failures while bumping or repricing against `market`.
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

/// Compute factor sensitivities from fully serialized binding inputs.
///
/// `market_json` is decoded into a [`MarketContext`] and `as_of` must be an ISO
/// calendar date. The returned JSON is the `SensitivityMatrixJson` view, with
/// position IDs, factor IDs, and rows of sensitivity values.
///
/// # Arguments
///
/// * `positions_json` - UTF-8 JSON array of positions; its IDs determine
///   serialized sensitivity-row order.
/// * `factors_json` - UTF-8 JSON factor-definition array that determines
///   factor IDs, order, and applied bump semantics.
/// * `market_json` - UTF-8 JSON representation of the pricing
///   [`MarketContext`] for the valuation snapshot.
/// * `as_of` - ISO-8601 calendar date parsed as the valuation date.
/// * `bump_config_json` - Optional UTF-8 JSON [`BumpSizeConfig`]; `None`
///   uses the canonical defaults.
///
/// # Errors
///
/// Propagates JSON/date parsing, sensitivity-engine, and result-serialization
/// failures.
pub fn compute_factor_sensitivities_json(
    positions_json: &str,
    factors_json: &str,
    market_json: &str,
    as_of: &str,
    bump_config_json: Option<&str>,
) -> Result<String> {
    let market: MarketContext = serde_json::from_str(market_json)
        .map_err(|e| Error::Validation(format!("invalid market JSON: {e}")))?;
    let as_of = finstack_quant_core::dates::parse_iso_date(as_of)?;
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

/// Compute repriced P&L profiles from JSON binding inputs.
///
/// Each factor is shifted across `n_scenario_points` around its configured
/// bump. The resulting profiles hold per-position P&L rows indexed by shift;
/// their units are the positions' reporting/base-currency valuation amounts.
///
/// # Arguments
///
/// * `positions_json` - UTF-8 JSON array of positions; its IDs order the P&L
///   rows in each factor profile.
/// * `factors_json` - UTF-8 JSON factor-definition array to shock and
///   profile, in the returned profile order.
/// * `market` - Typed market snapshot used to fully reprice every scenario.
/// * `as_of` - Valuation date used by each scenario reprice and lookup.
/// * `bump_config_json` - Optional UTF-8 JSON [`BumpSizeConfig`]; `None`
///   uses canonical factor bump sizes.
/// * `n_scenario_points` - Number of evenly spaced shock points per factor;
///   must satisfy the full-repricing engine's scenario-grid constraints.
///
/// # Errors
///
/// Propagates position, factor, and bump parsing failures; an invalid scenario
/// grid; and failures from the full-repricing engine.
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

/// Compute repriced P&L profiles from fully serialized binding inputs.
///
/// Uses [`DEFAULT_PNL_SCENARIO_POINTS`] when `n_scenario_points` is `None`.
/// The returned JSON contains one factor ID, shift grid, and per-position P&L
/// matrix for each shocked factor.
///
/// # Arguments
///
/// * `positions_json` - UTF-8 JSON array of positions; its IDs order P&L rows
///   in the serialized profiles.
/// * `factors_json` - UTF-8 JSON factor-definition array to shock, in
///   serialized profile order.
/// * `market_json` - UTF-8 JSON representation of the pricing
///   [`MarketContext`] for each scenario reprice.
/// * `as_of` - ISO-8601 calendar date parsed as the valuation date.
/// * `bump_config_json` - Optional UTF-8 JSON [`BumpSizeConfig`]; `None`
///   uses canonical bump sizes.
/// * `n_scenario_points` - Optional count of shock-grid points per factor;
///   `None` uses [`DEFAULT_PNL_SCENARIO_POINTS`].
///
/// # Errors
///
/// Propagates JSON/date parsing and repricing failures, and returns validation
/// errors if the output cannot be serialized.
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
    let as_of = finstack_quant_core::dates::parse_iso_date(as_of)?;
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
