//! Pre-execution [`ScenarioSpec`] and [`OperationSpec`] validation.

use super::operations::OperationSpec;
use super::scenario::ScenarioSpec;
use super::types::*;
use serde::{Deserialize, Serialize};

/// Structural validation diagnostics for a scenario specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ScenarioValidationReport {
    /// Whether the specification passed all structural checks.
    pub valid: bool,
    /// Failures in specification and operation order.
    pub errors: Vec<String>,
    /// Advisory diagnostics associated with the specification.
    pub warnings: Vec<String>,
    /// Whether at least one operation needs an instrument inventory.
    pub requires_instruments: bool,
    /// Whether applying the scenario can modify instrument state.
    pub mutates_instruments: bool,
    /// Number of operations in the submitted specification.
    pub operation_count: usize,
}

/// Check a scenario's structure and report diagnostics without applying shocks.
///
/// # Arguments
/// * `spec` - Scenario whose identifiers, timing and operations are checked.
///   Market and instrument lookups occur only when the scenario is applied.
#[must_use]
pub fn validate_scenario_spec(spec: &ScenarioSpec) -> ScenarioValidationReport {
    let errors: Vec<_> = spec
        .validation_errors()
        .map(|error| error.to_string())
        .collect();
    ScenarioValidationReport {
        valid: errors.is_empty(),
        errors,
        warnings: Vec::new(),
        requires_instruments: spec.requires_instruments(),
        mutates_instruments: spec.mutates_instruments(),
        operation_count: spec.operations.len(),
    }
}

impl ScenarioSpec {
    /// Parse a scenario specification from JSON and validate it.
    ///
    /// The single ingest path for host bindings: strict serde decoding
    /// (unknown fields are rejected) followed by [`ScenarioSpec::validate`].
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical `ScenarioSpec` JSON document.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`](crate::Error::Validation) prefixed with
    /// `Failed to parse ScenarioSpec JSON:` when `json` does not decode, or
    /// the error [`ScenarioSpec::validate`] returns for an inconsistent spec.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_scenarios::ScenarioSpec;
    ///
    /// let spec = ScenarioSpec::from_json(r#"{"id":"base","operations":[]}"#)?;
    /// assert_eq!(spec.id, "base");
    /// # Ok::<(), finstack_quant_scenarios::Error>(())
    /// ```
    pub fn from_json(json: &str) -> crate::error::Result<Self> {
        let spec: Self = serde_json::from_str(json).map_err(|error| {
            crate::error::Error::Validation(format!("Failed to parse ScenarioSpec JSON: {error}"))
        })?;
        spec.validate()?;
        Ok(spec)
    }

    /// Validate the scenario specification for consistency.
    ///
    /// Checks for:
    /// - Non-empty ID
    /// - Valid operations (recursively)
    ///
    /// # Returns
    ///
    /// `Ok(())` when the scenario has a non-empty identifier, contains at most
    /// one [`OperationSpec::TimeRollForward`], and every contained operation
    /// passes [`OperationSpec::validate`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Validation`] if the scenario ID is empty,
    /// more than one time-roll operation is present, or any contained operation
    /// is invalid.
    pub fn validate(&self) -> crate::error::Result<()> {
        self.validation_errors().next().map_or(Ok(()), Err)
    }

    fn validation_errors(&self) -> impl Iterator<Item = crate::error::Error> + '_ {
        use crate::error::Error;

        let id_error = self
            .id
            .trim()
            .is_empty()
            .then(|| Error::Validation("Scenario ID cannot be empty".into()));
        let time_roll_count = self
            .operations
            .iter()
            .filter(|op| matches!(op, OperationSpec::TimeRollForward { .. }))
            .count();
        let roll_error = (time_roll_count > 1).then(|| {
            Error::Validation(format!(
                "Scenario contains {time_roll_count} TimeRollForward operations; at most one is allowed"
            ))
        });
        let operation_errors = self.operations.iter().enumerate().filter_map(|(i, op)| {
            op.validate()
                .err()
                .map(|error| Error::Validation(format!("Operation {i}: {error}")))
        });
        id_error
            .into_iter()
            .chain(roll_error)
            .chain(operation_errors)
    }
}

impl OperationSpec {
    /// Validate the operation specification.
    ///
    /// Checks for:
    /// - Non-NaN numeric values
    /// - Non-empty identifiers
    /// - Logical consistency (e.g. valid currency pairs)
    ///
    /// # Returns
    ///
    /// `Ok(())` when the operation's identifiers, numeric inputs, and variant-
    /// specific invariants are valid.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Validation`] if any identifier is empty,
    /// any numeric field is non-finite, or the variant violates its own
    /// structural rules. FX shocks reject `pct <= -100` because driving a spot
    /// rate to zero would propagate NaNs through triangulation. Equity and
    /// instrument price shocks reject `pct < -100`; an exact -100% wipeout is
    /// allowed, but a negative post-shock price is not.
    pub fn validate(&self) -> crate::error::Result<()> {
        match self {
            OperationSpec::MarketFxPct { base, quote, pct } => {
                if base == quote {
                    return Err(crate::error::Error::Validation(
                        "Base and quote currencies must be different".into(),
                    ));
                }
                check_finite(*pct, "pct")?;
                // FX cannot be driven to zero or negative; a -100% shock would
                // collapse the spot rate and propagate NaNs through triangulation
                // and valuation downstream.
                check_pct_floor(*pct, "pct")?;
            }
            OperationSpec::EquityPricePct { ids, pct } => {
                if ids.is_empty() {
                    return Err(crate::error::Error::Validation(
                        "Equity IDs cannot be empty".into(),
                    ));
                }
                check_finite(*pct, "pct")?;
                check_price_pct_floor(*pct, "pct")?;
            }
            OperationSpec::InstrumentPricePctByAttr { attrs, pct } => {
                if attrs.is_empty() {
                    return Err(crate::error::Error::Validation(
                        "InstrumentPricePctByAttr.attrs must not be empty; an empty map would \
                         silently apply the shock to every instrument. Use \
                         InstrumentPricePctByType to target by instrument type."
                            .into(),
                    ));
                }
                check_finite(*pct, "pct")?;
                check_price_pct_floor(*pct, "pct")?;
            }
            OperationSpec::CurveParallelBp {
                curve_id,
                discount_curve_id,
                bp,
                curve_kind,
            } => {
                check_rate_curve_kind(*curve_kind)?;
                check_id(curve_id.as_str(), "curve_id")?;
                if let Some(discount_curve_id) = discount_curve_id {
                    check_id(discount_curve_id.as_str(), "discount_curve_id")?;
                }
                check_finite(*bp, "bp")?;
            }
            OperationSpec::CurveNodeBp {
                curve_id,
                discount_curve_id,
                nodes,
                curve_kind,
                ..
            } => {
                check_rate_curve_kind(*curve_kind)?;
                check_id(curve_id.as_str(), "curve_id")?;
                if let Some(discount_curve_id) = discount_curve_id {
                    check_id(discount_curve_id.as_str(), "discount_curve_id")?;
                }
                check_nodes(nodes)?;
            }
            OperationSpec::CommodityCurveParallelPct { curve_id, pct } => {
                check_id(curve_id.as_str(), "curve_id")?;
                check_finite(*pct, "pct")?;
            }
            OperationSpec::VolIndexParallelPts { curve_id, points } => {
                check_id(curve_id.as_str(), "curve_id")?;
                check_finite(*points, "points")?;
            }
            OperationSpec::VolIndexNodePts {
                curve_id, nodes, ..
            }
            | OperationSpec::CommodityCurveNodePct {
                curve_id, nodes, ..
            } => {
                check_id(curve_id.as_str(), "curve_id")?;
                check_nodes(nodes)?;
            }
            OperationSpec::BaseCorrParallelPts { surface_id, points }
            | OperationSpec::BaseCorrBucketPts {
                surface_id, points, ..
            } => {
                check_id(surface_id.as_str(), "surface_id")?;
                check_corr_delta(*points, "points")?;
            }
            OperationSpec::VolSurfaceParallelPct {
                vol_surface_id,
                pct,
                ..
            }
            | OperationSpec::VolSurfaceBucketPct {
                vol_surface_id,
                pct,
                ..
            } => {
                check_id(vol_surface_id.as_str(), "vol_surface_id")?;
                check_finite(*pct, "pct")?;
                check_price_pct_floor(*pct, "pct")?;
            }
            OperationSpec::StmtForecastPercent { node_id, pct } => {
                check_id(node_id.as_str(), "node_id")?;
                check_finite(*pct, "pct")?;
            }
            OperationSpec::StmtForecastAssign { node_id, value } => {
                check_id(node_id.as_str(), "node_id")?;
                check_finite(*value, "value")?;
            }
            OperationSpec::RateBinding { binding } => {
                binding.validate()?;
            }
            OperationSpec::InstrumentSpreadBpByAttr { attrs, bp } => {
                if attrs.is_empty() {
                    return Err(crate::error::Error::Validation(
                        "InstrumentSpreadBpByAttr.attrs must not be empty; an empty map would \
                         silently apply the shock to every instrument. Use \
                         InstrumentSpreadBpByType to target by instrument type."
                            .into(),
                    ));
                }
                check_finite(*bp, "bp")?;
            }
            OperationSpec::InstrumentPricePctByType { pct, .. } => {
                check_finite(*pct, "pct")?;
                check_price_pct_floor(*pct, "pct")?;
            }
            OperationSpec::InstrumentSpreadBpByType { bp, .. } => {
                check_finite(*bp, "bp")?;
            }
            OperationSpec::AssetCorrelationPts { delta_pts }
            | OperationSpec::PrepayDefaultCorrelationPts { delta_pts } => {
                check_corr_delta(*delta_pts, "delta_pts")?;
            }
            OperationSpec::HierarchyCurveParallelBp {
                target,
                bp,
                discount_curve_id,
                curve_kind,
            } => {
                check_rate_curve_kind(*curve_kind)?;
                target
                    .validate()
                    .map_err(|err| crate::error::Error::Validation(err.to_string()))?;
                check_finite(*bp, "bp")?;
                if let Some(dcid) = discount_curve_id {
                    check_id(dcid.as_str(), "discount_curve_id")?;
                }
            }
            OperationSpec::HierarchyCommodityCurveParallelPct { target, pct } => {
                target
                    .validate()
                    .map_err(|err| crate::error::Error::Validation(err.to_string()))?;
                check_finite(*pct, "pct")?;
            }
            OperationSpec::HierarchyVolSurfaceParallelPct { target, pct, .. }
            | OperationSpec::HierarchyEquityPricePct { target, pct } => {
                target
                    .validate()
                    .map_err(|err| crate::error::Error::Validation(err.to_string()))?;
                check_finite(*pct, "pct")?;
                check_price_pct_floor(*pct, "pct")?;
            }
            OperationSpec::HierarchyBaseCorrParallelPts { target, points } => {
                target
                    .validate()
                    .map_err(|err| crate::error::Error::Validation(err.to_string()))?;
                check_corr_delta(*points, "points")?;
            }
            OperationSpec::TimeRollForward { period, .. } => {
                if period.trim().is_empty() {
                    return Err(crate::error::Error::Validation(
                        "Time roll period cannot be empty".into(),
                    ));
                }
                finstack_quant_core::dates::Tenor::parse(period.trim()).map_err(|error| {
                    crate::error::Error::InvalidPeriod(format!(
                        "Time roll period {period:?} is not a valid tenor: {error}"
                    ))
                })?;
            }
        }
        Ok(())
    }
}

pub(super) fn check_rate_curve_kind(kind: CurveKind) -> crate::error::Result<()> {
    if kind == CurveKind::Commodity {
        return Err(crate::error::Error::Validation(
            "Commodity price curves use commodity_curve_parallel_pct, commodity_curve_node_pct or hierarchy_commodity_curve_parallel_pct".into(),
        ));
    }
    Ok(())
}

pub(super) fn check_finite(val: f64, name: &str) -> crate::error::Result<()> {
    if !val.is_finite() {
        Err(crate::error::Error::Validation(format!(
            "Value '{name}' must be finite"
        )))
    } else {
        Ok(())
    }
}

pub(super) fn check_id(id: &str, name: &str) -> crate::error::Result<()> {
    if id.trim().is_empty() {
        Err(crate::error::Error::Validation(format!(
            "Identifier '{name}' cannot be empty"
        )))
    } else {
        Ok(())
    }
}

pub(super) fn check_pct_floor(val: f64, name: &str) -> crate::error::Result<()> {
    if val <= -100.0 {
        Err(crate::error::Error::Validation(format!(
            "Percentage '{name}' must be greater than -100% (got {val:.1}%)"
        )))
    } else {
        Ok(())
    }
}

pub(super) fn check_price_pct_floor(val: f64, name: &str) -> crate::error::Result<()> {
    if val < -100.0 {
        Err(crate::error::Error::Validation(format!(
            "Price percentage '{name}' must be at least -100% (got {val:.1}%)"
        )))
    } else {
        Ok(())
    }
}

pub(super) fn check_nodes(nodes: &[(String, f64)]) -> crate::error::Result<()> {
    if nodes.is_empty() {
        return Err(crate::error::Error::Validation(
            "Curve nodes cannot be empty".into(),
        ));
    }
    for (tenor, bp) in nodes {
        if tenor.trim().is_empty() {
            return Err(crate::error::Error::Validation(
                "Curve node tenor cannot be empty".into(),
            ));
        }
        check_finite(*bp, "bp")?;
    }
    Ok(())
}

/// Correlation shocks are quoted as additive points (1.0 = +100 pts). A Δρ
/// outside `[-2, 2]` cannot produce a valid correlation regardless of base
/// level (since ρ ∈ [-1, 1] implies |Δρ| ≤ 2). Reject such inputs at
/// validation time to surface obvious unit-confusion (e.g. 25 meaning
/// "0.25") before the adapter silently clamps the outcome.
pub(super) fn check_corr_delta(val: f64, name: &str) -> crate::error::Result<()> {
    check_finite(val, name)?;
    if !(-2.0..=2.0).contains(&val) {
        return Err(crate::error::Error::Validation(format!(
            "Correlation delta '{name}' must lie in [-2, 2] points (got {val:.4}); \
             inputs are additive correlation points where 1.0 = +100 pts"
        )));
    }
    Ok(())
}
