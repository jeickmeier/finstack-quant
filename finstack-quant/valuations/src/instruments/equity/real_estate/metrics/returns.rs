//! Real estate return-style metrics (IRR, MOIC, cash-on-cash).

use crate::instruments::equity::real_estate::RealEstateAsset;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::cashflow::xirr_with_daycount;
use finstack_quant_core::Error as CoreError;
use std::collections::BTreeMap;

fn require_purchase_price(asset: &RealEstateAsset) -> finstack_quant_core::Result<f64> {
    let px = asset
        .purchase_price
        .ok_or_else(|| CoreError::Validation("purchase_price is required".into()))?;
    if px.currency() != asset.currency {
        return Err(CoreError::Validation(
            "purchase_price currency does not match instrument currency".into(),
        ));
    }
    Ok(px.amount())
}

fn build_unlevered_cashflow_map(
    asset: &RealEstateAsset,
    as_of: finstack_quant_core::dates::Date,
) -> finstack_quant_core::Result<BTreeMap<finstack_quant_core::dates::Date, f64>> {
    let purchase_price = require_purchase_price(asset)?;

    let mut flows: BTreeMap<finstack_quant_core::dates::Date, f64> = BTreeMap::new();

    // Initial acquisition outflow at as_of.
    let acq_cost = asset.acquisition_cost_total()?;
    *flows.entry(as_of).or_insert(0.0) += -(purchase_price + acq_cost);

    // Interim unlevered flows (NOI - CapEx).
    for (d, a) in asset.unlevered_flows(as_of)? {
        *flows.entry(d).or_insert(0.0) += a;
    }

    // Terminal sale proceeds at terminal date (if configured).
    if let Some((d, sale)) = asset.terminal_sale_proceeds(as_of)? {
        *flows.entry(d).or_insert(0.0) += sale;
    } else {
        return Err(CoreError::Validation(
            "terminal_cap_rate is required to compute terminal sale proceeds".into(),
        ));
    }

    Ok(flows)
}

/// Unlevered IRR (XIRR-style) computed from:
/// - initial purchase price + acquisition_cost at `as_of` (negative)
/// - unlevered net cash flows `NOI - CapEx`
/// - terminal sale proceeds from exit cap rate (positive)
#[derive(Debug, Default)]
pub(super) struct UnleveredIrr;

impl MetricCalculator for UnleveredIrr {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let asset = context
            .instrument
            .as_any()
            .downcast_ref::<RealEstateAsset>()
            .ok_or_else(|| {
                CoreError::Validation("UnleveredIrr: instrument type mismatch".into())
            })?;

        let flows = build_unlevered_cashflow_map(asset, context.as_of)?;
        let flows_vec: Vec<(finstack_quant_core::dates::Date, f64)> = flows.into_iter().collect();

        xirr_with_daycount(&flows_vec, asset.day_count, None)
    }
}

/// Unlevered multiple (MOIC-like): total inflows / total outflows (absolute).
#[derive(Debug, Default)]
pub(super) struct UnleveredMultiple;

impl MetricCalculator for UnleveredMultiple {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let asset = context
            .instrument
            .as_any()
            .downcast_ref::<RealEstateAsset>()
            .ok_or_else(|| {
                CoreError::Validation("UnleveredMultiple: instrument type mismatch".into())
            })?;

        let flows = build_unlevered_cashflow_map(asset, context.as_of)?;
        let mut inflows = 0.0;
        let mut outflows = 0.0;
        for (_d, a) in flows {
            if a >= 0.0 {
                inflows += a;
            } else {
                outflows += -a;
            }
        }
        if outflows <= 0.0 {
            return Err(CoreError::Validation(
                "UnleveredMultiple: total outflows must be positive".into(),
            ));
        }
        Ok(inflows / outflows)
    }
}

/// Unlevered first-period cash-on-cash: first `NOI - CapEx` cash flow divided by
/// purchase price (+ acquisition cost).
#[derive(Debug, Default)]
pub(super) struct UnleveredCashOnCashFirst;

impl MetricCalculator for UnleveredCashOnCashFirst {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let asset = context
            .instrument
            .as_any()
            .downcast_ref::<RealEstateAsset>()
            .ok_or_else(|| {
                CoreError::Validation("UnleveredCashOnCashFirst: instrument type mismatch".into())
            })?;

        let purchase_price = require_purchase_price(asset)?;
        let acq_cost = asset.acquisition_cost_total()?;
        let denom = purchase_price + acq_cost;
        if denom <= 0.0 {
            return Err(CoreError::Validation(
                "UnleveredCashOnCashFirst: denominator must be positive".into(),
            ));
        }

        let mut flows = asset.unlevered_flows(context.as_of)?;
        flows.sort_by_key(|(d, _)| *d);
        let first = flows.first().map(|(_, a)| *a).ok_or_else(|| {
            CoreError::Validation("UnleveredCashOnCashFirst: missing flows".into())
        })?;

        Ok(first / denom)
    }
}
