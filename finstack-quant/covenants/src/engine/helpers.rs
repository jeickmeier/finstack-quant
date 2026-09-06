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

/// Whether a springing trigger is met.
pub(crate) fn springing_condition_met(value: f64, test: ThresholdTest) -> bool {
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

fn is_negative_gross_leverage(covenant_type: &CovenantType, value: f64) -> bool {
    matches!(
        covenant_type,
        CovenantType::MaxDebtToEbitda { .. }
            | CovenantType::MaxTotalLeverage { .. }
            | CovenantType::MaxSeniorLeverage { .. }
    ) && value < 0.0
}

/// Shared point-in-time and forecast breach convention.
pub(crate) fn is_covenant_breached(
    covenant_type: &CovenantType,
    value: f64,
    threshold: f64,
) -> bool {
    if value.is_nan() || is_negative_gross_leverage(covenant_type, value) {
        return true;
    }
    match covenant_type.bound_kind() {
        Some(BoundKind::AtMost) => value > threshold,
        Some(BoundKind::AtLeast) => value < threshold,
        None => false,
    }
}

pub(crate) fn validate_metric(name: &str, value: f64) -> finstack_quant_core::Result<()> {
    if !value.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "metric '{name}' must be finite"
        )));
    }
    Ok(())
}

pub(crate) fn validated_ratio_value(
    spec: &CovenantSpec,
    value: f64,
    denominator: Option<f64>,
) -> f64 {
    if denominator.is_some_and(|d| d <= 0.0)
        || is_negative_gross_leverage(&spec.covenant.covenant_type, value)
    {
        f64::NAN
    } else {
        value
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

    /// Record or execute an additional collateral requirement.
    ///
    /// # Arguments
    ///
    /// * `description` - Contractual collateral requirement to record on the target.
    /// * `as_of` - Effective date of the requirement.
    ///
    /// # Errors
    ///
    /// Return an error when the target cannot represent the requirement; the
    /// engine then leaves this consequence outstanding for a later retry.
    fn require_collateral(
        &mut self,
        description: &str,
        as_of: Date,
    ) -> finstack_quant_core::Result<()>;

    /// Change maturity date.
    fn set_maturity(&mut self, new_maturity: Date) -> finstack_quant_core::Result<()>;
}
