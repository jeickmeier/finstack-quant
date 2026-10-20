//! Settlement-anchored bond clean and dirty price metrics.

use crate::bond::pricing::quote_conversions::settlement_dirty_from_quote_overrides;
use crate::bond::pricing::settlement::{model_dirty_at_quote_date, QuoteDateContext};
use crate::bond::Bond;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};

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

        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        model_dirty_at_quote_date(
            bond,
            &context.curves,
            context.as_of,
            quote_ctx.quote_date,
            context.base_value.amount(),
        )
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
        let bond: &Bond = context.instrument_as()?;
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        let pricing_dispatch = context.clone_pricer_dispatch();
        let dirty = if let Some(dirty) = settlement_dirty_from_quote_overrides(
            bond,
            &context.curves,
            context.as_of,
            crate::bond::metrics::active_model(context, bond),
            Some(&pricing_dispatch),
        )? {
            dirty
        } else {
            model_dirty_at_quote_date(
                bond,
                &context.curves,
                context.as_of,
                quote_ctx.quote_date,
                context.base_value.amount(),
            )?
        };
        let accrued = context.computed_metric(&MetricId::Accrued)?;
        debug_assert!(
            (accrued - quote_ctx.accrued_at_quote_date).abs() < 1e-10,
            "accrued dependency must share the settlement quote date"
        );
        Ok(dirty - accrued)
    }
}
