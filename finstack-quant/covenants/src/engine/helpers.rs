use super::types::{BoundKind, CovenantSpec, CovenantType, ThresholdTest};
use finstack_quant_core::dates::Date;

pub(super) struct SpecEvaluation {
    pub(super) passed: bool,
    pub(super) actual_value: Option<f64>,
    pub(super) threshold: Option<f64>,
    pub(super) headroom: Option<f64>,
    pub(super) detail: Option<String>,
}

/// Relative headroom: signed distance from the threshold, normalized by
/// `|threshold|` so the sign convention (positive = cushion, negative =
/// deficit) is preserved for negative thresholds too. A zero threshold falls
/// back to an absolute distance (denominator 1).
pub(crate) fn headroom_for(bound: Option<BoundKind>, value: f64, threshold: f64) -> f64 {
    if !value.is_finite() || !threshold.is_finite() {
        return f64::NAN;
    }

    let denom = if threshold.abs() < f64::EPSILON {
        1.0
    } else {
        threshold.abs()
    };

    match bound {
        Some(BoundKind::AtMost) => (threshold - value) / denom,
        Some(BoundKind::AtLeast) => (value - threshold) / denom,
        None => 0.0,
    }
}

/// Whether a springing trigger is met, including the NaN-activates convention.
///
/// `NaN <= t` and `NaN >= t` are both false, which would deactivate the
/// covenant and report a pass on undefined data. A NaN trigger therefore
/// activates so the covenant's own NaN handling decides the outcome.
pub(crate) fn springing_condition_met(metric: &str, value: f64, test: ThresholdTest) -> bool {
    if value.is_nan() {
        tracing::warn!(
            metric,
            "springing condition metric is NaN; activating the covenant"
        );
        return true;
    }
    match test {
        ThresholdTest::Maximum(threshold) => value <= threshold,
        ThresholdTest::Minimum(threshold) => value >= threshold,
    }
}

/// Resolve the metric selected by a covenant. An explicit identifier owns the
/// lookup; only specs without one use their type's conventional metric name.
pub(crate) fn spec_metric_name(spec: &CovenantSpec) -> Option<&str> {
    spec.metric_id
        .as_ref()
        .map(|id| id.as_str())
        .or_else(|| spec.covenant.covenant_type.default_metric_name())
        .or(match &spec.covenant.covenant_type {
            CovenantType::Custom { metric, .. } => Some(metric.as_str()),
            CovenantType::Basket { name, .. } => Some(name.as_str()),
            _ => None,
        })
}

/// Shared point-in-time and forecast breach convention.
pub(crate) fn is_covenant_breached(
    covenant_type: &CovenantType,
    value: f64,
    threshold: f64,
) -> bool {
    if value.is_nan() {
        return true;
    }
    if covenant_type.is_ratio_max() && value < 0.0 {
        return true;
    }
    match covenant_type.bound_kind() {
        Some(BoundKind::AtMost) => value > threshold,
        Some(BoundKind::AtLeast) => value < threshold,
        None => false,
    }
}

/// Trait for instruments that can be mutated by covenant consequences.
pub trait InstrumentMutator: Send + Sync {
    /// Set default status.
    fn set_default_status(
        &mut self,
        is_default: bool,
        as_of: Date,
    ) -> finstack_quant_core::Result<()>;

    /// Increase interest rate.
    fn increase_rate(&mut self, increase: f64) -> finstack_quant_core::Result<()>;

    /// Set cash sweep percentage.
    fn set_cash_sweep(&mut self, percentage: f64) -> finstack_quant_core::Result<()>;

    /// Block distributions.
    fn set_distribution_block(&mut self, blocked: bool) -> finstack_quant_core::Result<()>;

    /// Change maturity date.
    fn set_maturity(&mut self, new_maturity: Date) -> finstack_quant_core::Result<()>;
}
