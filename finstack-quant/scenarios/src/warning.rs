//! Structured warnings emitted during scenario application.
//!
//! [`ApplicationReport`] carries a `Vec<Warning>` so callers can match on
//! warning categories without parsing free-text strings.

use serde::{Deserialize, Serialize};
use std::fmt;

use finstack_quant_valuations::pricer::InstrumentType;

/// A single warning emitted by an adapter, the engine, or a downstream helper.
///
/// New variants will be added over time; pattern matches on `Warning` should
/// always include a wildcard arm.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Warning {
    /// Par-spread shocks used the approximation delta hazard = delta spread / (1 - recovery).
    HazardSpreadFirstOrder {
        /// Hazard curve affected by the approximate spread shock.
        curve_id: String,
        /// Decimal recovery fraction used in the loss-given-default conversion.
        recovery_rate: f64,
    },

    /// A discount curve was resolved heuristically (currency-prefix or single-
    /// curve fallback) instead of via an explicit `discount_curve_id`.
    DiscountCurveHeuristic {
        /// Curve identifier the heuristic was applied to.
        for_curve: String,
        /// The discount curve selected by the heuristic.
        chosen_discount: String,
        /// Reason text describing the heuristic path.
        reason: String,
    },

    /// A commodity price-curve shock was outside the typical percent-of-forward
    /// stress range (approximately `[-80, +200]` percent).
    CommodityShockOutsideRange {
        /// Curve identifier.
        curve_id: String,
        /// Detail describing whether the trigger was a parallel or node shock
        /// and which knot(s) were extreme.
        detail: String,
    },

    /// Equity identifier was missing from the market context; no bump applied.
    EquityNotFound {
        /// Equity price identifier.
        id: String,
    },

    /// Rate binding produced no statement update because the target node has
    /// no explicit forecast values.
    RateBindingNoForecastValues {
        /// Statement node identifier.
        node_id: String,
        /// Curve identifier from which the rate would have been extracted.
        curve_id: String,
    },

    /// Rate binding evaluation failed.
    RateBindingFailed {
        /// Statement node identifier.
        node_id: String,
        /// Curve identifier.
        curve_id: String,
        /// Underlying error message.
        reason: String,
    },

    /// Statement node had no forecast values to modify.
    StatementNodeNoValues {
        /// Statement node identifier.
        node_id: String,
        /// Operation that was attempted (e.g. `"forecast_percent"`).
        op: String,
    },

    /// Statement operation failed.
    StatementOpFailed {
        /// Statement node identifier.
        node_id: String,
        /// Operation name.
        op: String,
        /// Underlying error message.
        reason: String,
    },

    /// Model evaluator emitted a warning during re-evaluation.
    ModelEvaluation {
        /// Free-text detail from the evaluator (already includes context).
        detail: String,
    },

    /// Model re-evaluation failed.
    ModelReevaluationFailed {
        /// Underlying error message.
        reason: String,
    },

    /// FX shock left a non-pivot direct quote inconsistent with the pivot-
    /// triangulated cross.
    FxTriangulationInconsistent {
        /// Free-text detail naming the inconsistent pair and computed values.
        detail: String,
    },

    /// Instrument shock fell back to attribute metadata because no pricing
    /// override was exposed by the instrument's pricer.
    InstrumentShockFallback {
        /// Whether the shock is a price (`"price"`) or spread (`"spread"`).
        shock_kind: String,
        /// Canonical instrument type that could not apply the shock directly.
        inst_type: InstrumentType,
        /// Instrument label (id / name / unidentified).
        label: String,
    },

    /// Instrument shock was requested but no instruments were supplied via the
    /// execution context.
    InstrumentShockNoPortfolio {
        /// Whether the shock is a price (`"price"`) or spread (`"spread"`).
        shock_kind: String,
        /// Whether the filter was by type (`"type"`) or attribute (`"attr"`).
        filter: String,
    },

    /// Instrument shock found no instruments matching the filter.
    InstrumentShockNoMatch {
        /// Free-text description of the filter (e.g. `IndexMap` debug form).
        filter_desc: String,
    },

    /// Correlation shock requested but no instruments supplied.
    CorrelationShockNoPortfolio,

    /// Correlation shock found no `StructuredCredit` instruments with a
    /// correlation structure.
    CorrelationShockNoMatch,

    /// Correlation bump clamped one or more correlation parameters.
    CorrelationClamped {
        /// Instrument identifier.
        instrument_id: String,
        /// Detail string describing what was clamped.
        detail: String,
    },

    /// A shock targeting a curve node by tenor had to extrapolate beyond the
    /// curve's pillar range.
    TenorExtrapolated {
        /// Curve identifier.
        curve_id: String,
        /// Free-text describing the tenor and the gap.
        detail: String,
    },

    /// An interpolated node bump split its shock across two pillars of a
    /// par-CDS curve that is rebuilt by solve-to-par recalibration. On that
    /// path the 1/Σw² delivery correction is only first-order — the weighted
    /// targets are snapped to the nearest calibration quotes before
    /// re-solving, so the realized shock at the requested tenor may differ
    /// from the requested size. Direct-shift curves (discount, inflation,
    /// forward, commodity) calibrate onto the native interpolant and do not
    /// emit this warning. Use `TenorMatchMode::Exact` at pillar tenors for
    /// pillar-accurate par-CDS bucket risk.
    InterpolatedNodeBumpFirstOrder {
        /// Curve identifier.
        curve_id: String,
        /// Free-text describing the tenor and adjacent pillars.
        detail: String,
    },

    /// Bucket-correlation shock matched no detachment buckets.
    BaseCorrBucketNoMatch {
        /// Surface identifier.
        surface_id: String,
    },

    /// Vol-surface arbitrage warning detected post-shock.
    VolSurfaceArbitrage {
        /// Volatility-surface identifier.
        vol_surface_id: String,
        /// Free-text from `ArbitrageViolation` Display.
        detail: String,
    },

    /// Vol-surface negative bucket shock that may produce non-positive vols.
    VolSurfaceLargeNegativeShock {
        /// Volatility-surface identifier.
        vol_surface_id: String,
        /// Percent shock value.
        pct: f64,
        /// Whether this was a parallel (`false`) or bucket (`true`) shock.
        bucket: bool,
    },

    /// A hierarchy-targeted operation expanded to zero curves. This is almost
    /// always a configuration mistake — the target path exists but has no
    /// curves attached to it, or the tag filter excluded everything.
    HierarchyNoMatch {
        /// Hierarchy path the operation targeted, joined with `/`.
        target_path: String,
        /// Operation kind name (e.g., `"HierarchyCurveParallelBp"`).
        op_kind: String,
    },

    /// A hierarchy-resolved identifier was skipped because it does not exist
    /// in the market collection targeted by the operation kind (e.g. a
    /// `HierarchyCurveParallelBp { curve_kind: Discount }` matched an id that
    /// is an equity price or vol surface, not a discount curve). The remaining
    /// resolved identifiers are still applied.
    HierarchyResolvedIdSkipped {
        /// The resolved identifier that was skipped.
        curve_id: String,
        /// Operation kind name (e.g., `"HierarchyCurveParallelBp"`).
        op_kind: String,
    },

    /// An instrument's valuation failed while computing carry during a
    /// `TimeRollForward` operation. The instrument is excluded from the
    /// roll-forward carry aggregation but the roll itself proceeds.
    TimeRollInstrumentFailed {
        /// Instrument identifier.
        instrument_id: String,
        /// Underlying valuation error message.
        reason: String,
    },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HazardSpreadFirstOrder { curve_id, recovery_rate } => write!(f,
                "First-order par-spread shock for '{curve_id}' uses delta hazard = delta spread / (1 - {recovery_rate}); quotes are not rebootstrap-reconciled"),
            Warning::DiscountCurveHeuristic { reason, .. } => f.write_str(reason),
            Warning::CommodityShockOutsideRange { detail, .. }
            | Warning::FxTriangulationInconsistent { detail }
            | Warning::TenorExtrapolated { detail, .. }
            | Warning::InterpolatedNodeBumpFirstOrder { detail, .. } => f.write_str(detail),
            Warning::EquityNotFound { id } => write!(f, "Equity {id}: not found in market data"),
            Warning::RateBindingNoForecastValues { node_id, curve_id } => write!(
                f,
                "Rate binding {node_id}->{curve_id}: node has no forecast values to assign"
            ),
            Warning::RateBindingFailed {
                node_id,
                curve_id,
                reason,
            } => write!(f, "Rate binding {node_id}->{curve_id}: {reason}"),
            Warning::StatementNodeNoValues { node_id, op } => write!(
                f,
                "Statement node '{node_id}' has no forecast values to modify (op: {op})"
            ),
            Warning::StatementOpFailed {
                node_id,
                op,
                reason,
            } => write!(f, "Statement {op} for node {node_id}: {reason}"),
            Warning::ModelEvaluation { detail } => write!(f, "Model evaluation: {detail}"),
            Warning::ModelReevaluationFailed { reason } => {
                write!(f, "Model re-evaluation: {reason}")
            }
            Warning::InstrumentShockFallback {
                shock_kind,
                inst_type,
                label,
            } => write!(
                f,
                "Instrument {shock_kind} shock fell back to metadata for instrument '{label}' \
                 (type {inst_type}): the pricer does not expose get_scenario_pricing_overrides_mut(), \
                 so the shock is recorded under scenario_{shock_kind}_shock_* but will not affect \
                 valuation unless the downstream consumer reads that metadata."
            ),
            Warning::InstrumentShockNoPortfolio { shock_kind, filter } => write!(
                f,
                "Instrument {filter} {shock_kind} shock requested but no instruments provided"
            ),
            Warning::InstrumentShockNoMatch { filter_desc } => {
                write!(f, "No instruments matched attribute filter {filter_desc}")
            }
            Warning::CorrelationShockNoPortfolio => {
                f.write_str("Correlation shock requested but no instruments provided")
            }
            Warning::CorrelationShockNoMatch => f.write_str(
                "Correlation shock: no StructuredCredit instruments with correlation structure found",
            ),
            Warning::CorrelationClamped {
                instrument_id,
                detail,
            } => write!(f, "Correlation bump for '{instrument_id}': {detail}"),
            Warning::BaseCorrBucketNoMatch { surface_id } => {
                write!(f, "BaseCorrBucketPts on {surface_id} matched no detachment buckets")
            }
            Warning::VolSurfaceArbitrage {
                vol_surface_id,
                detail,
            } => write!(
                f,
                "Vol surface '{vol_surface_id}' post-shock arbitrage warning: {detail}"
            ),
            Warning::VolSurfaceLargeNegativeShock {
                vol_surface_id,
                pct,
                bucket,
            } => write!(
                f,
                "Vol surface '{vol_surface_id}': Large negative {kind}shock ({pct:.1}%) may produce \
                 non-positive vols or calendar spread arbitrage. Consider using check_arbitrage() \
                 to validate post-shock surface.",
                kind = if *bucket { "bucket " } else { "" },
            ),
            Warning::HierarchyNoMatch {
                target_path,
                op_kind,
            } => write!(
                f,
                "Hierarchy operation '{op_kind}' targeting '{target_path}' matched no curves; \
                 the target path may exist but be empty, or its tag filter excluded everything."
            ),
            Warning::HierarchyResolvedIdSkipped { curve_id, op_kind } => write!(
                f,
                "Hierarchy operation '{op_kind}' resolved id '{curve_id}', which does not exist \
                 in the market collection that operation targets; skipped. Check that the \
                 hierarchy node groups only identifiers of the targeted kind."
            ),
            Warning::TimeRollInstrumentFailed {
                instrument_id,
                reason,
            } => write!(
                f,
                "Time roll-forward: valuation of instrument '{instrument_id}' failed ({reason}); \
                 it was excluded from the roll-forward carry aggregation."
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instrument_fallback_serializes_canonical_instrument_type() {
        let warning = Warning::InstrumentShockFallback {
            shock_kind: "price".to_string(),
            inst_type: InstrumentType::Bond,
            label: "BOND-1".to_string(),
        };

        let json = serde_json::to_value(warning).expect("warning should serialize");
        assert_eq!(json["kind"], "instrument_shock_fallback");
        assert_eq!(json["inst_type"], "bond");
    }
}
