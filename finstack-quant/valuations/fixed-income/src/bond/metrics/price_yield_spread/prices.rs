//! Settlement-anchored bond clean and dirty price metrics.

use crate::bond::pricing::quote_conversions::settlement_dirty_from_quote_overrides;
use crate::bond::pricing::settlement::{model_dirty_at_quote_date, settlement_date};
use crate::bond::Bond;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};

/// Full settlement dirty value in currency: the configured quote driver
/// normalized at the quote date, otherwise the model NPV forward-valued from
/// `as_of` after removing any detached cashflows.
fn settlement_dirty(context: &MetricContext) -> finstack_quant_core::Result<f64> {
    let bond: &Bond = context.instrument_as()?;
    let pricing_dispatch = context.clone_pricer_dispatch();
    if let Some(dirty) = settlement_dirty_from_quote_overrides(
        bond,
        &context.curves,
        context.as_of,
        crate::bond::metrics::active_model(context, bond),
        Some(&pricing_dispatch),
    )? {
        return Ok(dirty);
    }
    model_dirty_at_quote_date(
        bond,
        &context.curves,
        context.as_of,
        settlement_date(bond, context.as_of)?,
        context.base_value.amount(),
    )
}

/// Calculates the full settlement dirty price in currency units.
///
/// A configured quote driver is normalized directly at the quote date. Without
/// one, the model NPV is forward-valued from `as_of` after removing any
/// detached cashflows.
pub(crate) struct DirtyPriceCalculator;

impl MetricCalculator for DirtyPriceCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Accrued]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        settlement_dirty(context)
    }
}

/// Calculates the settlement clean price in currency units.
///
/// The result is always `settlement_dirty - accrued(settlement)`, independent
/// of whether the dirty value originated from a market quote or model NPV.
pub(crate) struct CleanPriceCalculator;

impl MetricCalculator for CleanPriceCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Accrued]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let dirty = settlement_dirty(context)?;
        Ok(dirty - context.computed_metric(&MetricId::Accrued)?)
    }
}
