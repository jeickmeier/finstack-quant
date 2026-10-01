//! JSON-facing helpers for factor-model bindings.

use super::positions::{parse_positions_json, pricing_positions};
use super::{
    DeltaBasedEngine, FactorPnlProfile, FactorSensitivityEngine, FullRepricingEngine,
    SensitivityMatrix,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::factor::{BumpSizeConfig, FactorDefinition};
use serde::{Deserialize, Deserializer, Serialize};

/// Default scenario count for symmetric P&L profile grids.
///
/// `5` produces `[-2, -1, 0, 1, 2]`.
pub const DEFAULT_PNL_SCENARIO_POINTS: usize = 5;

/// Canonical monetary sensitivity JSON shared by Python and WASM.
///
/// The matrix serializes as nested position-by-factor rows. Its canonical
/// deserializer rejects incorrect dimensions, non-finite entries, and unknown
/// fields before the data reaches a risk engine.
#[derive(Debug, Clone, Serialize)]
pub struct SensitivityMatrixJson {
    /// Reporting currency of every monetary sensitivity.
    pub base_currency: Currency,
    /// Validated sensitivity axes and nested rows, flattened into this object.
    #[serde(flatten)]
    pub matrix: SensitivityMatrix,
}

// Flattened serde structs do not reliably reject leftover unknown fields.
// Parse the complete monetary wire explicitly, then use the canonical matrix
// constructor for all shape and value validation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MonetarySensitivityWire {
    base_currency: Currency,
    position_ids: Vec<String>,
    factor_ids: Vec<finstack_quant_models::factor::FactorId>,
    data: Vec<Vec<f64>>,
}

impl<'de> Deserialize<'de> for SensitivityMatrixJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let wire = MonetarySensitivityWire::deserialize(deserializer)?;
        let matrix = SensitivityMatrix::from_rows(wire.position_ids, wire.factor_ids, wire.data)
            .map_err(serde::de::Error::custom)?;
        Ok(Self {
            base_currency: wire.base_currency,
            matrix,
        })
    }
}

/// JSON shape returned by WASM P&L profile helpers.
#[derive(Debug, Clone, Serialize)]
pub struct FactorPnlProfileJson {
    /// Reporting currency of each P&L amount.
    pub base_currency: Currency,
    /// Ordered position identifiers for each P&L row.
    pub position_ids: Vec<String>,
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
pub(crate) fn parse_factor_definitions_json(json: &str) -> Result<Vec<FactorDefinition>> {
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
pub(crate) fn parse_bump_config_json(json: Option<&str>) -> Result<BumpSizeConfig> {
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
/// All sensitivities are converted to `base_currency` on each bumped market.
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
/// * `base_currency` - Reporting currency used for every bumped PV and returned exposure.
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
    base_currency: Currency,
    bump_config_json: Option<&str>,
) -> Result<SensitivityMatrix> {
    let parsed_positions = parse_positions_json(positions_json)?;
    let positions = pricing_positions(&parsed_positions);
    let factors = parse_factor_definitions_json(factors_json)?;
    let bump_config = parse_bump_config_json(bump_config_json)?;
    let engine = DeltaBasedEngine::new(bump_config);
    engine.compute_sensitivities(&positions, &factors, market, as_of, base_currency)
}

/// Compute repriced P&L profiles from JSON binding inputs.
///
/// Each factor is shifted across `n_scenario_points` around its configured
/// bump. The resulting profiles hold per-position P&L rows indexed by shift;
/// their units are amounts in the explicitly supplied `base_currency`.
///
/// # Arguments
///
/// * `positions_json` - UTF-8 JSON array of positions; its IDs order the P&L
///   rows in each factor profile.
/// * `factors_json` - UTF-8 JSON factor-definition array to shock and
///   profile, in the returned profile order.
/// * `market` - Typed market snapshot used to fully reprice every scenario.
/// * `as_of` - Valuation date used by each scenario reprice and lookup.
/// * `base_currency` - Reporting currency used for every bumped PV and returned exposure.
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
    base_currency: Currency,
    bump_config_json: Option<&str>,
    n_scenario_points: usize,
) -> Result<Vec<FactorPnlProfile>> {
    let parsed_positions = parse_positions_json(positions_json)?;
    let positions = pricing_positions(&parsed_positions);
    let factors = parse_factor_definitions_json(factors_json)?;
    let bump_config = parse_bump_config_json(bump_config_json)?;
    let engine = FullRepricingEngine::new(bump_config, n_scenario_points)?;
    engine.compute_pnl_profiles(&positions, &factors, market, as_of, base_currency)
}

impl SensitivityMatrixJson {
    /// Serialize a matrix with its explicit monetary reporting unit.
    ///
    /// # Arguments
    ///
    /// * `matrix` - Position-by-factor monetary sensitivities to serialize.
    /// * `base_currency` - Currency in which all matrix entries were calculated.
    pub fn from_matrix(matrix: &SensitivityMatrix, base_currency: Currency) -> Self {
        Self {
            base_currency,
            matrix: matrix.clone(),
        }
    }
}

impl From<&FactorPnlProfile> for FactorPnlProfileJson {
    fn from(profile: &FactorPnlProfile) -> Self {
        Self {
            base_currency: profile.base_currency,
            position_ids: profile.position_ids.clone(),
            factor_id: profile.factor_id.to_string(),
            shifts: profile.shifts.clone(),
            position_pnls: profile.position_pnls.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_models::factor::{FactorId, FactorType, MarketMapping};
    use finstack_quant_valuations::instruments::{Equity, InstrumentEnvelope, InstrumentJson};
    use std::sync::Arc;

    #[test]
    fn monetary_matrix_wire_roundtrip_preserves_reporting_currency_and_rows() {
        let matrix = SensitivityMatrix::from_rows(
            vec!["A".into()],
            vec![FactorId::new("FX")],
            vec![vec![2.5]],
        )
        .expect("valid matrix");
        let wire = SensitivityMatrixJson::from_matrix(&matrix, Currency::EUR);
        let value = serde_json::to_value(&wire).expect("serialize");
        assert_eq!(
            value,
            serde_json::json!({
                "base_currency": "EUR", "position_ids": ["A"],
                "factor_ids": ["FX"], "data": [[2.5]],
            })
        );
        let restored: SensitivityMatrixJson = serde_json::from_value(value).expect("deserialize");
        assert_eq!(restored.base_currency, Currency::EUR);
        assert_eq!(restored.matrix, matrix);
    }

    #[test]
    fn monetary_matrix_wire_rejects_missing_currency_and_unknown_fields() {
        for value in [
            serde_json::json!({"position_ids": [], "factor_ids": [], "data": []}),
            serde_json::json!({"base_currency": "BAD", "position_ids": [], "factor_ids": [], "data": []}),
            serde_json::json!({"base_currency": "USD", "position_ids": [], "factor_ids": [], "data": [], "n_factors": 0}),
        ] {
            assert!(serde_json::from_value::<SensitivityMatrixJson>(value).is_err());
        }
    }

    #[test]
    fn reporting_currency_is_explicit_and_independent_of_position_order() {
        let as_of = time::macros::date!(2025 - 01 - 01);
        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quotes(&[(Currency::EUR, Currency::USD, 1.1)])
            .unwrap();
        let market = MarketContext::new().insert_fx(FxMatrix::new(provider));
        let mut positions: Vec<_> = [("USD", Currency::USD), ("EUR", Currency::EUR)]
            .into_iter()
            .map(|(id, currency)| {
                let mut equity = Equity::new(id, id, currency);
                equity.quoted_spot = Some(100.0);
                serde_json::json!({
                    "id": id,
                    "instrument": InstrumentEnvelope::new(InstrumentJson::Equity(equity)),
                    "weight": 1.0
                })
            })
            .collect();
        let factors = serde_json::to_string(&vec![FactorDefinition {
            id: FactorId::new("EURUSD"),
            factor_type: FactorType::Fx,
            market_mapping: MarketMapping::FxRate {
                pair: (Currency::EUR, Currency::USD),
            },
            description: None,
        }])
        .unwrap();
        let json = serde_json::to_string(&positions).unwrap();
        let first = compute_factor_sensitivities_from_json(
            &json,
            &factors,
            &market,
            as_of,
            Currency::USD,
            None,
        )
        .unwrap();
        let profiles =
            compute_pnl_profiles_from_json(&json, &factors, &market, as_of, Currency::USD, None, 3)
                .unwrap();
        positions.reverse();
        let second = compute_factor_sensitivities_from_json(
            &serde_json::to_string(&positions).unwrap(),
            &factors,
            &market,
            as_of,
            Currency::USD,
            None,
        )
        .unwrap();
        assert_eq!(first.delta(0, 0), 0.0);
        assert!(first.delta(1, 0).abs() > 1.0);
        assert_eq!(first.delta(1, 0), second.delta(0, 0));
        assert_eq!(first.delta(0, 0), second.delta(1, 0));
        assert_eq!(
            SensitivityMatrixJson::from_matrix(&first, Currency::USD).base_currency,
            Currency::USD
        );
        let profile = FactorPnlProfileJson::from(&profiles[0]);
        assert_eq!(profile.base_currency, Currency::USD);
        assert_eq!(profile.position_ids, vec!["USD", "EUR"]);
    }
}
