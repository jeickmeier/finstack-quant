//! Problem, decision-space, constraint, and solver types for portfolio optimization.
//!
use crate::types::{AttributeTest, AttributeValue};
use finstack_quant_valuations::metrics::MetricId;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// How optimization weights are defined.
///
/// # Conventions
///
/// `ValueWeight` and `NotionalWeight` are normalized portfolio shares, whereas
/// `UnitScaling` is a direct multiplier on current quantity for existing
/// positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightingScheme {
    /// `w_i` is share of gross portfolio base-currency PV.
    ///
    /// Weights are computed against `gross_pv_base = Σ |pv_i|`, so for a
    /// long-only portfolio `∑ w_i = 1`. When shorts are permitted the sum can
    /// differ from `1` because shorts enter the denominator as absolute value
    /// while contributing negatively to `w_i`.
    ///
    /// Quantity reconstruction converts the target PV share back to
    /// `Position::quantity` via `pv_per_unit`, which is base-currency PV per
    /// 1.0 of [`crate::position::Position::scale_factor`] (a `Percentage`
    /// holding of `50` uses scale `0.5`, not raw `50`). Each candidate must
    /// have non-zero `pv_per_unit`. Zero-PV candidates are rejected at
    /// decision-space construction time under this scheme because their
    /// implied quantity would be undefined.
    /// Held existing positions retain their exact current quantity, including
    /// holdings with zero or negligible PV.
    ValueWeight,

    /// `w_i` is share of signed deal notional; still normalized so `∑ w_i = 1`
    /// on a long-only book (shorts enter the denominator as absolute value).
    ///
    /// Per-position notional is the absolute deal notional converted to portfolio
    /// base currency at the valuation date, multiplied by `scale_factor()`.
    /// Instruments that do not expose `notional()` fail under this scheme —
    /// there is no silent fallback to `scale_factor()` as a dollar proxy.
    /// Quantity reconstruction divides the target notional share by the
    /// instrument deal notional and writes the result back in
    /// `Position::quantity` units (lots, shares, or percentage points).
    NotionalWeight,

    /// `w_i` scales the current quantity (e.g. units or face value).
    ///
    /// Unlike `ValueWeight` and `NotionalWeight`, this is not a PV share.
    /// For existing positions, `w_i = 1.0` means "keep the current quantity",
    /// `w_i = 0.5` means "halve it", and `w_i = 0.0` means "close it".
    /// Existing multipliers are nonnegative by default; an explicit budget
    /// or weight bound controls increases above one. Turnover is measured as
    /// the sum of absolute changes in these multipliers.
    /// For new candidates, `w_i` is interpreted directly as the target quantity
    /// because there is no live quantity to scale.
    UnitScaling,
}

/// Where a per‑position scalar metric comes from.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PerPositionMetric {
    /// Directly from `ValuationResult::measures` using a standard `MetricId`.
    ///
    /// Examples:
    /// - `Metric(MetricId::DurationMod)` for modified duration
    /// - `Metric(MetricId::Ytm)` for yield to maturity
    /// - `Metric(MetricId::Dv01)` for DV01
    Metric(MetricId),

    /// From `ValuationResult::measures` using a string key (for custom or
    /// bucketed metrics stored by name).
    CustomKey(String),

    /// Use the base currency PV of the position (after scaling).
    PvBase,

    /// Use the native‑currency PV of the position (after scaling).
    ///
    /// Values are returned in each position's own currency and are therefore
    /// **not commensurable** across a mixed-currency portfolio. The LP
    /// coefficient builder rejects `PvNative` inside aggregated objectives
    /// ([`MetricExpr::WeightedSum`] or [`MetricExpr::ValueWeightedAverage`]) —
    /// use [`PerPositionMetric::PvBase`] in those contexts so all positions
    /// are summed in a single numeraire.
    PvNative,

    /// Numeric attribute value from position attributes.
    Attribute(String),

    /// 1.0 if the attribute test passes, 0.0 otherwise.
    AttributeIndicator(AttributeTest),

    /// Constant scalar for all positions.
    Constant(f64),
}

impl PerPositionMetric {
    /// Resolve a metric against one decision item's values and attributes.
    ///
    /// # Arguments
    ///
    /// * `measures` - Instrument metric values keyed by canonical metric name.
    /// * `attributes` - Position attributes supplying numeric values or indicator tests.
    /// * `pv_base` - Held present value in the portfolio reporting currency.
    /// * `pv_native` - Held present value in the instrument's native currency.
    pub(crate) fn resolve(
        &self,
        measures: &IndexMap<String, f64>,
        attributes: &IndexMap<String, AttributeValue>,
        pv_base: f64,
        pv_native: f64,
    ) -> Option<f64> {
        match self {
            Self::Metric(id) => measures.get(id.as_str()).copied(),
            Self::CustomKey(key) => measures.get(key).copied(),
            Self::PvBase => Some(pv_base),
            Self::PvNative => Some(pv_native),
            Self::Attribute(key) => attributes.get(key).and_then(AttributeValue::as_number),
            Self::AttributeIndicator(test) => {
                Some(if test.evaluate(attributes) { 1.0 } else { 0.0 })
            }
            Self::Constant(value) => Some(*value),
        }
    }
}

/// How to handle missing metrics for a position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingMetricPolicy {
    /// Treat missing as 0.0 (default, appropriate for duration‑like metrics).
    #[default]
    Zero,

    /// Freeze the position at its current quantity and drop missing inputs from metric
    /// coefficient vectors (`WeightedSum` and `ValueWeightedAverage` alike).
    ///
    /// Missing metrics are not treated as zero in the objective or in
    /// constraint rows: the coefficient is `0` and the name is omitted from
    /// a `ValueWeightedAverage` denominator, matching “excluded from
    /// constraint evaluation”. This applies to standard metrics, custom keys,
    /// and numeric attributes only where the expression's filter matches.
    Exclude,

    /// Fail with error if any required metric is missing.
    Strict,
}

/// Portfolio‑level scalar metric expressed in terms of position metrics + weights.
///
/// These expressions are intentionally restricted to linear or linearized forms
/// so they can be represented by the LP-based optimizer.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum MetricExpr {
    /// `sum_i w_i * m_i`, where `m_i` comes from a `PerPositionMetric`.
    WeightedSum {
        /// Per‑position metric to aggregate.
        metric: PerPositionMetric,
        /// Optional filter restricting which positions contribute.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filter: Option<super::universe::PositionFilter>,
    },

    /// Value‑weighted average: `sum_i w_i * m_i`, with implicit `sum_i w_i == 1`.
    /// This is appropriate for duration or yield when weights are `ValueWeight`.
    ValueWeightedAverage {
        /// Per‑position metric to average.
        metric: PerPositionMetric,
        /// Optional filter restricting which positions contribute.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filter: Option<super::universe::PositionFilter>,
    },
}

/// Optimization direction and target.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Objective {
    /// Maximize a portfolio‑level metric.
    Maximize(MetricExpr),
    /// Minimize a portfolio‑level metric.
    Minimize(MetricExpr),
}

// ── Numerical tolerances ───────────────────────────────────────────────────
//
// The optimizer mixes several different scales (raw quantities, notionals,
// base-currency PVs, fractional weights, constraint slacks) in the same LP.
// Picking a single tolerance for all of them is wrong — a 1e-9 weight is
// genuinely negligible, while a 1e-9 base-currency-PV may not be. The
// constants below name each tolerance by what it gates, so future changes
// are easier to reason about and harder to miss.
//
// # Conventions
//
// - **`PV_PER_UNIT_TOL`**: smallest `|pv_per_unit|` we will divide by when
//   reconstructing implied quantities under `ValueWeight`. Below this we
//   either reject the candidate (decision-space) or set the implied
//   quantity to zero (LP solver) — see the call sites for which. Held existing
//   positions bypass reconstruction and retain their exact current quantity.
// - **`MIN_WEIGHT_TOL`**: smallest `|min_weight|` that we treat as
//   "non-zero" when classifying candidates as long-only or
//   long-short-eligible.
// - **`GROSS_BASE_TOL`**: smallest gross base-currency / notional value
//   below which weight normalization collapses to zero (avoids divide-by-
//   zero on hedged-flat books).
// - **`WEIGHT_TOL`**: smallest absolute weight we treat as a real position
//   when filtering the trade list output.
// - **`SLACK_TOL`**: smallest absolute constraint slack at which a
//   constraint is reported as binding in the result envelope.
//
// These are deliberately *not* a single global tolerance. The PV scale
// (`PV_PER_UNIT_TOL = 1e-12`) and the weight scale (`WEIGHT_TOL = 1e-9`)
// and the constraint-slack scale (`SLACK_TOL = 1e-6`) all live at
// different orders of magnitude because the quantities they gate live at
// different orders of magnitude.

/// Smallest absolute `pv_per_unit` we will divide by when reconstructing
/// implied quantities. Used by both decision-space candidate filtering
/// and LP-solver quantity reconstruction.
pub const PV_PER_UNIT_TOL: f64 = 1e-12;

/// Smallest `|min_weight|` we treat as non-zero when classifying
/// candidates as "long-only-eligible" vs "shortable".
pub const MIN_WEIGHT_TOL: f64 = 1e-12;

/// Smallest absolute gross base-currency PV / notional below which the
/// weight-normalization denominator is treated as effectively zero.
/// Hedged-flat portfolios with cancelling longs and shorts can land here
/// even when individual positions are far from zero.
pub const GROSS_BASE_TOL: f64 = 1e-6;

/// Smallest absolute weight change we report in the post-solve trade list.
/// Candidate additions below this target weight are omitted from the rebalanced
/// portfolio; held existing quantities are preserved regardless of their weight.
pub const WEIGHT_TOL: f64 = 1e-9;

/// Smallest absolute constraint slack at which a constraint is reported
/// as binding in the result envelope.
pub const SLACK_TOL: f64 = 1e-6;
