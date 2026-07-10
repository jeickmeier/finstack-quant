//! Price calculators for structured credit instruments.

use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::Result;

/// Calculates dirty price as percentage of par (includes accrued interest).
///
/// Dirty price is the market value including accrued interest, expressed as
/// a percentage of the original notional. This is the actual transaction price.
///
/// # Formula
///
/// Dirty Price = (NPV / Original Notional) × 100
///
/// Where NPV is the net present value of all future cashflows.
///
pub struct DirtyPriceCalculator;

impl MetricCalculator for DirtyPriceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Use the base NPV that was already computed
        let npv = context.base_value.amount();

        // Get the original notional
        // For structured credit, this is typically stored in the pool or tranche
        let notional = get_original_notional(context)?;

        if notional == 0.0 {
            return Ok(0.0);
        }

        // Dirty price = (NPV / Notional) × 100
        let dirty_price = (npv / notional) * 100.0;

        Ok(dirty_price)
    }
}

/// Calculates clean price as percentage of par (excludes accrued interest).
///
/// Clean price is the market convention for quoting structured credit instruments.
/// It equals the dirty price minus accrued interest (converted to price points).
///
/// # Formula
///
/// Clean Price = Dirty Price - (Accrued / Notional) × 100
///
pub struct CleanPriceCalculator;

impl MetricCalculator for CleanPriceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Get dirty price from computed metrics
        let dirty = context
            .computed
            .get(&MetricId::DirtyPrice)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:DirtyPrice".to_string(),
                })
            })?;

        // Get accrued interest in currency units
        let accrued = context
            .computed
            .get(&MetricId::Accrued)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:Accrued".to_string(),
                })
            })?;

        // Convert accrued to price points
        let notional = get_original_notional(context)?;
        let accrued_points = if notional > 0.0 {
            (accrued / notional) * 100.0
        } else {
            0.0
        };

        // Clean price = Dirty price - Accrued (in points)
        let clean_price = dirty - accrued_points;

        Ok(clean_price)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::DirtyPrice, MetricId::Accrued]
    }
}

/// Helper to get the original notional from the context.
///
/// For structured credit, the notional **must** be set when creating the
/// `MetricContext` (pool original balance for deal-level, tranche original
/// balance for tranche-level). Using `base_value` as a fallback produces
/// self-referential prices (dirty price ≈ 100% always) and silently wrong
/// Z-spread / CS01 / duration values.
pub(crate) fn get_original_notional(context: &MetricContext) -> Result<f64> {
    context
        .notional
        .map(|n| n.amount())
        .filter(|&n| n > 0.0)
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "MetricContext.notional must be set for structured credit price metrics. \
                 Set it to pool total balance (deal-level) or tranche original balance \
                 (tranche-level) when constructing the metric context."
                    .to_string(),
            )
        })
}
