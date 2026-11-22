//! Weight risk calculator for baskets.
//!
//! Computes sensitivity to weight changes for each constituent.
//! For each constituent, bumps its weight by 1bp and adjusts other weights
//! proportionally to preserve the original total weight, then measures the
//! impact on basket PV.
//!
//! # Formula
//! ```text
//! WeightRisk_i = Notional * bump_size * (mark_i - sum_{j != i}(w_j * mark_j) / sum_{j != i}(w_j))
//! ```
//! Where `bump_size` is 1bp (`0.0001`) change in weight.

use super::super::pricer::marked_constituent;
use crate::instruments::basket::Basket;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::{Result, money::Money};

/// Standard weight bump: 1bp (0.0001)
const WEIGHT_BUMP: f64 = 0.0001;

/// Weight risk calculator for baskets.
pub(crate) struct WeightRiskCalculator;

impl MetricCalculator for WeightRiskCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let basket: &Basket = context.instrument_as()?;
        basket.validate()?;

        if basket.constituents.iter().any(|c| c.units.is_some()) {
            return Err(finstack_quant_core::Error::Validation(
                "WeightRisk requires weight-based constituents; explicit units do not define a financed weight direction".into(),
            ));
        }

        let total_weight: f64 = basket.constituents.iter().map(|c| c.weight).sum();
        for constituent in &basket.constituents {
            let total_other_weight = total_weight - constituent.weight;
            if total_other_weight.abs() <= f64::EPSILON {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "WeightRisk cannot fund a {WEIGHT_BUMP} reallocation into '{}' from the other weights",
                    constituent.id
                )));
            }
        }

        let as_of = context.as_of;
        let marks: Vec<Money> = basket
            .constituents
            .iter()
            .map(|constituent| {
                marked_constituent(basket, constituent, context.curves.as_ref(), as_of)
            })
            .collect::<Result<Vec<_>>>()?;

        let total_weighted_mark: f64 = basket
            .constituents
            .iter()
            .zip(&marks)
            .map(|(constituent, mark)| constituent.weight * mark.amount())
            .sum();
        let notional = basket.notional.amount();

        let mut series: Vec<(String, f64)> = Vec::with_capacity(basket.constituents.len());
        let mut total_risk = 0.0;

        for (constituent, mark) in basket.constituents.iter().zip(&marks) {
            let total_other_weight = total_weight - constituent.weight;
            let mark_amount = mark.amount();
            let other_weighted_mark = total_weighted_mark - constituent.weight * mark_amount;
            // Bumping w_i by WEIGHT_BUMP and scaling w_{j != i} by
            // (total_other_weight - WEIGHT_BUMP) / total_other_weight shifts NAV by
            // WEIGHT_BUMP * (mark_i - other_weighted_mark / total_other_weight);
            // fixed accrued expenses cancel out.
            let risk =
                notional * WEIGHT_BUMP * (mark_amount - other_weighted_mark / total_other_weight);

            series.push((constituent.id.clone(), risk));
            total_risk += risk;
        }

        context.store_bucketed_series(MetricId::WeightRisk, series);

        Ok(total_risk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::Instrument;
    use crate::instruments::basket::{
        BasketAssetType, BasketConstituent, BasketExpense, BasketPricingConfig,
        ConstituentReference,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::types::{Attributes, CurveId, InstrumentId, PriceId};
    use std::sync::Arc;

    #[test]
    fn linear_weight_risk_matches_explicit_bump_and_reprice() {
        let constituents = vec![
            BasketConstituent {
                id: "AAPL".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: PriceId::new("AAPL"),
                    asset_type: BasketAssetType::Equity,
                },
                weight: 0.5,
                units: None,
                ticker: Some("AAPL".to_string()),
            },
            BasketConstituent {
                id: "MSFT".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: PriceId::new("MSFT"),
                    asset_type: BasketAssetType::Equity,
                },
                weight: 0.3,
                units: None,
                ticker: Some("MSFT".to_string()),
            },
            BasketConstituent {
                id: "GOOGL".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: PriceId::new("GOOGL"),
                    asset_type: BasketAssetType::Equity,
                },
                weight: 0.2,
                units: None,
                ticker: Some("GOOGL".to_string()),
            },
        ];

        let basket = Basket::builder()
            .id(InstrumentId::new("BASKET-3"))
            .constituents(constituents)
            .expense_ratio(0.0025)
            .expense(BasketExpense::Accrued {
                amount: Money::new(2.5, Currency::USD).expect("valid expense"),
            })
            .reporting_currency(Currency::USD)
            .notional(Money::from((100_000_i64, Currency::USD)))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Attributes::new())
            .pricing_config(BasketPricingConfig::default())
            .build()
            .expect("valid basket");

        let market = Arc::new(
            MarketContext::new()
                .insert_price("AAPL", MarketScalar::unitless(150.0).expect("valid scalar"))
                .insert_price("MSFT", MarketScalar::unitless(300.0).expect("valid scalar"))
                .insert_price(
                    "GOOGL",
                    MarketScalar::unitless(2800.0).expect("valid scalar"),
                ),
        );
        let as_of = time::macros::date!(2025 - 01 - 01);
        let base_pv = basket.value(market.as_ref(), as_of).expect("base pv");

        let mut context = MetricContext::new(
            Arc::new(basket.clone()),
            Arc::clone(&market),
            as_of,
            base_pv,
            MetricContext::default_config(),
        );
        let total_risk = WeightRiskCalculator
            .calculate(&mut context)
            .expect("weight risk");

        let mut expected_total = 0.0;
        for (idx, constituent) in basket.constituents.iter().enumerate() {
            let total_other_weight: f64 = basket
                .constituents
                .iter()
                .enumerate()
                .filter_map(|(i, c)| (i != idx).then_some(c.weight))
                .sum();
            let mut bumped = basket.clone();
            bumped.constituents[idx].weight += WEIGHT_BUMP;
            let scale_factor = (total_other_weight - WEIGHT_BUMP) / total_other_weight;
            for (i, c) in bumped.constituents.iter_mut().enumerate() {
                if i != idx {
                    c.weight *= scale_factor;
                }
            }
            let pv_bumped = bumped
                .value(market.as_ref(), as_of)
                .expect("bumped pv")
                .amount();
            let expected = pv_bumped - base_pv.amount();
            expected_total += expected;

            let actual = context.computed
                [&MetricId::composite(&MetricId::WeightRisk, &[constituent.id.as_str()])];
            assert!(
                (actual - expected).abs() < 1e-6,
                "constituent {} mismatch: actual={actual}, expected={expected}",
                constituent.id
            );
        }

        assert!(
            (total_risk - expected_total).abs() < 1e-6,
            "total risk mismatch: actual={total_risk}, expected={expected_total}"
        );
    }
}
