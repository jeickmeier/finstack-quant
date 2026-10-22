//! Real estate return-style metrics (IRR, MOIC, cash-on-cash).

use crate::instruments::equity::real_estate::{pricer, RealEstateAsset};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::cashflow::xirr;
use finstack_quant_core::Error;
use std::collections::BTreeMap;

fn build_unlevered_cashflow_map(
    asset: &RealEstateAsset,
    as_of: finstack_quant_core::dates::Date,
) -> finstack_quant_core::Result<BTreeMap<finstack_quant_core::dates::Date, f64>> {
    let purchase_price = pricer::required_purchase_price(asset)?;

    let mut flows: BTreeMap<finstack_quant_core::dates::Date, f64> = BTreeMap::new();

    // Initial acquisition outflow at as_of.
    let acq_cost = pricer::acquisition_cost_total(asset)?;
    *flows.entry(as_of).or_insert(0.0) += -(purchase_price + acq_cost);

    // Interim unlevered flows (NOI - CapEx).
    for (d, a) in pricer::unlevered_flows(asset, as_of)? {
        *flows.entry(d).or_insert(0.0) += a;
    }

    // Terminal sale proceeds at terminal date (if configured).
    if let Some((d, sale)) = pricer::terminal_sale_proceeds(asset, as_of)? {
        *flows.entry(d).or_insert(0.0) += sale;
    } else {
        return Err(Error::Validation(
            "terminal_cap_rate is required to compute terminal sale proceeds".into(),
        ));
    }

    Ok(flows)
}

/// Unlevered IRR (XIRR-style) computed from:
/// - initial purchase price + Σ acquisition_costs at `as_of` (negative)
/// - unlevered net cash flows `NOI - CapEx`
/// - terminal sale proceeds from exit cap rate (positive)
#[derive(Debug, Default)]
pub(super) struct UnleveredIrr;

impl MetricCalculator for UnleveredIrr {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let asset: &RealEstateAsset = context.instrument_as()?;

        let flows = build_unlevered_cashflow_map(asset, context.as_of)?;
        let flows_vec: Vec<(finstack_quant_core::dates::Date, f64)> = flows.into_iter().collect();

        xirr(&flows_vec, asset.day_count, None)
    }
}

/// Unlevered multiple (MOIC-like): total inflows / total outflows (absolute).
#[derive(Debug, Default)]
pub(super) struct UnleveredMultiple;

impl MetricCalculator for UnleveredMultiple {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let asset: &RealEstateAsset = context.instrument_as()?;

        let flows = build_unlevered_cashflow_map(asset, context.as_of)?;
        super::moic(flows.into_values(), "UnleveredMultiple")
    }
}

/// Unlevered first-period cash-on-cash: first `NOI - CapEx` cash flow divided by
/// purchase price (+ acquisition cost).
#[derive(Debug, Default)]
pub(super) struct UnleveredCashOnCashFirst;

impl MetricCalculator for UnleveredCashOnCashFirst {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let asset: &RealEstateAsset = context.instrument_as()?;

        let purchase_price = pricer::required_purchase_price(asset)?;
        let acq_cost = pricer::acquisition_cost_total(asset)?;
        let denom = purchase_price + acq_cost;
        if denom <= 0.0 {
            return Err(Error::Validation(
                "UnleveredCashOnCashFirst: denominator must be positive".into(),
            ));
        }

        let flows = pricer::unlevered_flows(asset, context.as_of)?;
        let first = flows
            .first()
            .map(|(_, a)| *a)
            .ok_or_else(|| Error::Validation("UnleveredCashOnCashFirst: missing flows".into()))?;

        Ok(first / denom)
    }
}
