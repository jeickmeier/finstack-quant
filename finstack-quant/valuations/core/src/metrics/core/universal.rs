//! The universal metric calculators every family registry shares.

use std::sync::Arc;

use super::{
    ids::MetricId,
    registry::{MetricRegistry, MetricRegistryError},
    traits::{MetricCalculator, MetricContext},
};
use crate::metrics::risk::{GenericExpectedShortfall, GenericHVar};
use crate::metrics::sensitivities::breakeven::BreakevenCalculator;
use crate::metrics::sensitivities::carry_decomposition::CarryDecompositionCalculator;
use crate::metrics::sensitivities::theta::GenericThetaAny;

struct ComputedMetricLookup {
    metric: MetricId,
    dependencies: [MetricId; 1],
}

impl ComputedMetricLookup {
    fn new(metric: MetricId, dependency: MetricId) -> Self {
        Self {
            metric,
            dependencies: [dependency],
        }
    }
}

impl MetricCalculator for ComputedMetricLookup {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        context.computed.get(&self.metric).copied().ok_or_else(|| {
            finstack_quant_core::InputError::NotFound {
                id: format!("metric:{}", self.metric),
            }
            .into()
        })
    }

    fn dependencies(&self) -> &[MetricId] {
        &self.dependencies
    }
}

/// Owner entry for a standard metric a model pricer publishes itself.
///
/// Some models emit a standard metric identifier as a side output of their own
/// pricing pass instead of leaving it to a calculator — structured credit's
/// Monte Carlo `expected_loss`, for example. That value is authoritative, so
/// the metric layer must read it rather than derive a second, possibly
/// disagreeing one. The pricing path seeds those measures into
/// `MetricContext::computed`, which both satisfies the registry's up-front
/// applicability check and supplies the value here.
///
/// The registration still matters: without an entry, the registry would report
/// the metric as inapplicable to an instrument type whose pricers do produce
/// it, and `metrics_for_instrument` would omit it. Applicability is recorded
/// per instrument type, while publication depends on the model actually used,
/// so this calculator fails when the valuation ran under a model that does not
/// publish the metric. Each family registers the entries for the instrument
/// types whose pricers publish the metric.
pub struct PricerPublishedMetric {
    metric: MetricId,
}

impl PricerPublishedMetric {
    /// Owner entry for `metric`, read back from the pricing pass.
    pub fn new(metric: MetricId) -> Self {
        Self { metric }
    }
}

impl MetricCalculator for PricerPublishedMetric {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        context.computed.get(&self.metric).copied().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "Metric '{}' is published by this instrument type's model pricers, \
                 but the model used for this valuation did not produce it",
                self.metric
            ))
        })
    }
}

/// Register the calculators that apply to every instrument type.
///
/// Every family registry starts from these; a family table adds only its
/// instrument-specific owners on top.
///
/// # Arguments
///
/// * `registry` - Registry receiving the universal calculators (theta and
///   its decomposition, carry, breakeven, historical VaR and expected
///   shortfall).
pub fn register_universal_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), MetricRegistryError> {
    registry.register_metric(MetricId::Theta, Arc::new(GenericThetaAny), &[])?;
    registry.register_metric(
        MetricId::ThetaCarry,
        Arc::new(ComputedMetricLookup::new(
            MetricId::ThetaCarry,
            MetricId::Theta,
        )),
        &[],
    )?;
    registry.register_metric(
        MetricId::ThetaRollDown,
        Arc::new(ComputedMetricLookup::new(
            MetricId::ThetaRollDown,
            MetricId::Theta,
        )),
        &[],
    )?;
    // Theta also publishes the horizon it used. It is a requestable metric ID,
    // so it needs a lookup entry like its siblings; without one a caller asking
    // for it by name has no registered owner.
    registry.register_metric(
        MetricId::ThetaPeriodDays,
        Arc::new(ComputedMetricLookup::new(
            MetricId::ThetaPeriodDays,
            MetricId::Theta,
        )),
        &[],
    )?;
    registry.register_metric(
        MetricId::CarryTotal,
        Arc::new(CarryDecompositionCalculator::default()),
        &[],
    )?;
    registry.register_metric(
        MetricId::CouponIncome,
        Arc::new(ComputedMetricLookup::new(
            MetricId::CouponIncome,
            MetricId::CarryTotal,
        )),
        &[],
    )?;
    registry.register_metric(
        MetricId::PullToPar,
        Arc::new(ComputedMetricLookup::new(
            MetricId::PullToPar,
            MetricId::CarryTotal,
        )),
        &[],
    )?;
    registry.register_metric(
        MetricId::RollDown,
        Arc::new(ComputedMetricLookup::new(
            MetricId::RollDown,
            MetricId::CarryTotal,
        )),
        &[],
    )?;
    registry.register_metric(
        MetricId::FundingCost,
        Arc::new(ComputedMetricLookup::new(
            MetricId::FundingCost,
            MetricId::CarryTotal,
        )),
        &[],
    )?;
    registry.register_metric(MetricId::Breakeven, Arc::new(BreakevenCalculator), &[])?;
    registry.register_metric(MetricId::HVar, Arc::new(GenericHVar), &[])?;
    registry.register_metric(
        MetricId::ExpectedShortfall,
        Arc::new(GenericExpectedShortfall),
        &[],
    )?;
    Ok(())
}
