//! Weight risk calculator for baskets.
//!
//! Computes sensitivity to weight changes for each constituent.
//! For each constituent, bumps its weight and adjusts other weights proportionally
//! to maintain sum = 1.0, then measures the impact on basket NAV.
//!
//! # Formula
//! ```text
//! WeightRisk_i = (PV(basket with bumped weight_i) - PV_base) / bump_size
//! ```
//! Where bump_size is typically 1bp (0.0001) change in weight.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::exotics::basket::Basket;
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
            let label = constituent
                .ticker
                .clone()
                .unwrap_or_else(|| constituent.id.clone());

            // Create basket with bumped weight
            let bumped_weight = (constituent.weight + WEIGHT_BUMP).clamp(0.0, 1.0);
            let weight_change = bumped_weight - constituent.weight;

            // If no change (clamped), skip
            if weight_change.abs() < 1e-10 {
                series.push((label, 0.0));
                continue;
            }

            // Adjust other weights proportionally to maintain sum = 1.0
            let mut bumped_basket = basket.clone();
            bumped_basket.constituents[idx].weight = bumped_weight;

            let total_other_weight: f64 = basket
                .constituents
                .iter()
                .enumerate()
                .filter_map(|(i, c)| if i != idx { Some(c.weight) } else { None })
                .sum();

            // Redistribute the weight change proportionally among other constituents
            if total_other_weight > 1e-10 {
                let scale_factor = (1.0 - bumped_weight) / total_other_weight;
                for (i, c) in bumped_basket.constituents.iter_mut().enumerate() {
                    if i != idx {
                        c.weight *= scale_factor;
                    }
                }
            }

            // Reprice with bumped weights
            let pv_bumped = bumped_basket
                .value(context.curves.as_ref(), as_of)?
                .amount();

            // Weight risk = (PV_bumped - PV_base) / weight_change
            // Result is per 1bp change in weight
            let risk = (pv_bumped - base_pv) / weight_change * 10_000.0; // Scale to per 1bp

            series.push((label, risk));
            total_risk += risk;
        }

        // Store as bucketed series
        context.store_bucketed_series(crate::metrics::MetricId::custom("weight_risk"), series);

        Ok(total_risk)
    }
}
