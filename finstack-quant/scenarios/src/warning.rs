//! Structured warnings emitted during scenario application.
//!
//! [`ApplicationReport`](crate::engine::ApplicationReport) carries a
//! `Vec<Warning>` instead of a `Vec<String>` so that operations alerting
//! pipelines can pattern-match on warning categories without parsing free-text
//! strings. The `Display` impl produces the same human-readable form as the
//! previous string-based warnings, so existing log lines and UI summaries
//! continue to render the same way.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A single warning emitted by an adapter, the engine, or a downstream helper.
///
/// New variants will be added over time; pattern matches on `Warning` should
/// always include a wildcard arm.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Warning {
    /// Hazard-curve solve-to-par recalibration failed; engine fell back to a
    /// direct hazard-rate shift. CS01 may differ from the recalibrated path.
    HazardRecalibrationFallback {
        /// Curve identifier that failed to recalibrate.
        curve_id: String,
        /// Underlying error message from the recalibration attempt.
        reason: String,
        /// Whether the fallback was triggered by a node-bump (`true`) or a
        /// parallel-bump (`false`) operation.
        node: bool,
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

    /// A commodity-curve shock was outside the typical stress range. Common
    /// trigger: passing a basis-point intent in dollar-points or vice versa.
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
        /// Instrument type printed for diagnostic purposes.
        inst_type: String,
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

    /// Bucket-correlation shock matched no detachment buckets.
    BaseCorrBucketNoMatch {
        /// Surface identifier.
        surface_id: String,
    },

    /// Vol-surface arbitrage warning detected post-shock.
    VolSurfaceArbitrage {
        /// Surface identifier.
        surface_id: String,
        /// Free-text from `ArbitrageViolation` Display.
        detail: String,
    },

    /// Vol-surface negative bucket shock that may produce non-positive vols.
    VolSurfaceLargeNegativeShock {
        /// Surface identifier.
        surface_id: String,
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
            Warning::HazardRecalibrationFallback {
                curve_id,
                reason,
                node,
            } => write!(
                f,
                "Hazard curve '{curve_id}' {kind} shock: par-CDS recalibration failed ({reason}); \
                 applied direct hazard-rate shift instead. Risk-neutral default probabilities will be \
                 additively shifted by the requested bp amount {target} rather than re-solved from \
                 par spreads, which can materially change CS01 for sharply sloped curves. Note \
                 the magnitude bias: by the credit triangle (dLambda ~ dSpread / (1 - R)), a \
                 direct hazard shift of the par-spread bp under-shocks default intensities by \
                 roughly the factor (1 - recovery).",
                kind = if *node { "node" } else { "parallel" },
                target = if *node { "at the targeted pillars" } else { "at each pillar" },
            ),
            Warning::DiscountCurveHeuristic { reason, .. } => f.write_str(reason),
            Warning::CommodityShockOutsideRange { detail, .. }
            | Warning::FxTriangulationInconsistent { detail }
            | Warning::TenorExtrapolated { detail, .. } => f.write_str(detail),
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
                 (type {inst_type}): the pricer does not expose scenario_overrides_mut(), \
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
            Warning::VolSurfaceArbitrage { surface_id, detail } => write!(
                f,
                "Vol surface '{surface_id}' post-shock arbitrage warning: {detail}"
            ),
            Warning::VolSurfaceLargeNegativeShock {
                surface_id,
                pct,
                bucket,
            } => write!(
                f,
                "Vol surface '{surface_id}': Large negative {kind}shock ({pct:.1}%) may produce \
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
