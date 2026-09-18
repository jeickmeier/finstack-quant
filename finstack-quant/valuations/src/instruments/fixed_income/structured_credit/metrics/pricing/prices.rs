//! Price calculators for structured credit instruments.

use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::Result;

/// Calculates dirty price as percentage of par (includes accrued interest).
///
/// Dirty price is the market value including accrued interest, expressed as
/// a percentage of the CURRENT face (the factor-adjusted quote basis). This
/// is the actual transaction price.
///
/// # Formula
///
/// Dirty Price = (NPV / Current Face) × 100
///
/// Where NPV is the net present value of all future cashflows.
///
pub struct DirtyPriceCalculator;

impl MetricCalculator for DirtyPriceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let quote = super::super::quote::SettlementQuote::from_context(context)?;
        let flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "structured-credit price requires projected cashflows".into(),
            )
        })?;
        let deal = context
            .instrument_as::<crate::instruments::fixed_income::structured_credit::StructuredCredit>(
            )?;
        let curve = context
            .curves
            .get_discount(deal.discount_curve_id.as_str())?;
        if quote.notional <= 0.0 {
            return Ok(0.0);
        }
        Ok(quote.model_dirty(flows, &curve)? / quote.notional * 100.0)
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
        let dirty = context
            .computed
            .get(&MetricId::DirtyPrice)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:DirtyPrice".to_string(),
                })
            })?;

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
        let notional = quote_notional(context)?;
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

/// The face amount structured-credit prices are quoted against.
///
/// Prices are per CURRENT face (the factor-adjusted secondary-market basis):
/// the tranche's current balance for tranche-level metrics, the sum of the
/// current tranche balances for deal-level metrics. The notional **must** be
/// set when creating the `MetricContext`; using `base_value` as a fallback
/// produces self-referential prices (dirty price ≈ 100% always) and silently
/// wrong Z-spread / CS01 / duration values.
pub(crate) fn quote_notional(context: &MetricContext) -> Result<f64> {
    context
        .notional
        .map(|n| n.amount())
        .filter(|&n| n > 0.0)
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "MetricContext.notional must be set for structured credit price metrics. \
                 Set it to the sum of current tranche balances (deal-level) or the \
                 tranche's current balance (tranche-level) when constructing the \
                 metric context."
                    .to_string(),
            )
        })
}
