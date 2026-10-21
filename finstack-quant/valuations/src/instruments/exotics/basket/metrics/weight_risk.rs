//! Weight risk calculator for baskets.
//!
//! Computes sensitivity to weight changes for each constituent.
//! For each constituent, bumps its weight and adjusts other weights proportionally
//! to preserve the original total weight, then measures the impact on basket NAV.
//!
//! # Formula
//! ```text
//! WeightRisk_i = (PV(basket with bumped weight_i) - PV_base) / bump_size * 1e-4
//! ```
//! Where bump_size is typically 1bp (0.0001) change in weight.

use crate::instruments::exotics::basket::Basket;
use crate::instruments::Instrument;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Standard weight bump: 1bp (0.0001)
const WEIGHT_BUMP: f64 = 0.0001;

/// Weight risk calculator for baskets.
pub(crate) struct WeightRiskCalculator;

impl MetricCalculator for WeightRiskCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let basket: &Basket = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.base_value.amount();

        let mut series: Vec<(String, f64)> = Vec::new();
        let mut total_risk = 0.0;

        // For each constituent, bump its weight and adjust others proportionally
        for (idx, constituent) in basket.constituents.iter().enumerate() {
            let label = constituent.id.clone();

            if constituent.units.is_some() || basket.constituents.iter().any(|c| c.units.is_some())
            {
                return Err(finstack_quant_core::Error::Validation(
                    "WeightRisk requires weight-based constituents; explicit units do not define a financed weight direction".into(),
                ));
            }
            let weight_change = WEIGHT_BUMP;
            let total_other_weight: f64 = basket
                .constituents
                .iter()
                .enumerate()
                .filter_map(|(i, c)| (i != idx).then_some(c.weight))
                .sum();
            if total_other_weight.abs() <= f64::EPSILON {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "WeightRisk cannot fund a {WEIGHT_BUMP} reallocation into '{}' from the other weights", constituent.id
                )));
            }
            let mut bumped_basket = basket.clone();
            bumped_basket.constituents[idx].weight += weight_change;
            let scale_factor = (total_other_weight - weight_change) / total_other_weight;
            for (i, c) in bumped_basket.constituents.iter_mut().enumerate() {
                if i != idx {
                    c.weight *= scale_factor;
                }
            }

            // Reprice with bumped weights
            let pv_bumped = bumped_basket
                .value(context.curves.as_ref(), as_of)?
                .amount();

            // Weight risk = (PV_bumped - PV_base) / weight_change
            // Result is per 1bp change in weight
            let risk = (pv_bumped - base_pv) / weight_change * WEIGHT_BUMP; // Currency per 1bp weight change

            series.push((label, risk));
            total_risk += risk;
        }

        context.store_bucketed_series(crate::metrics::MetricId::WeightRisk, series);

        Ok(total_risk)
    }
}
