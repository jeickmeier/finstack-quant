//! Expected Loss metric for CDS Index.
//!
//! Calculates the expected loss at maturity for a CDS index using:
//! EL = Notional × PD × LGD
//!
//! For a CDS index, we use the index-level hazard curve and average recovery rate,
//! which represents the expected loss across all constituents assuming equal weighting.

use crate::cds_index::{CdsIndex, IndexPricing};
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Expected Loss calculator for CDS Index.
pub(crate) struct ExpectedLossCalculator;

impl MetricCalculator for ExpectedLossCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let index: &CdsIndex = context.instrument_as()?;

        let as_of = context.as_of;

        // If already at/after maturity, no forward expected loss.
        if as_of >= index.premium_leg.end {
            return Ok(0.0);
        }

        let scale = index.index_factor;
        let notional = index.notional.amount() * scale;

        if index.pricing == IndexPricing::Constituents {
            // Defaulted constituents have already been settled and are
            // removed from forward exposure (their notional is captured by
            // `index_factor`). Exclude them from the forward EL and weight
            // surviving names so they sum to 1.
            let active: Vec<_> = index.constituents.iter().filter(|c| !c.defaulted).collect();
            if active.is_empty() {
                return Ok(0.0);
            }
            let sum_w: f64 = active.iter().map(|c| c.weight).sum();
            if sum_w <= 0.0 {
                return Ok(0.0);
            }

            let mut weighted_el = 0.0;
            for constituent in active {
                let hazard = context
                    .curves
                    .get_hazard(constituent.credit.credit_curve_id.as_str())?;
                let default_prob = hazard.conditional_default_prob(as_of, index.premium_leg.end)?;
                let lgd = 1.0 - constituent.credit.recovery_rate;
                weighted_el += (constituent.weight / sum_w) * default_prob * lgd;
            }

            return Ok(notional * weighted_el);
        }

        // Index-level expected loss using the index hazard curve and recovery.
        let hazard = context
            .curves
            .get_hazard(index.protection_leg.credit_curve_id.as_str())?;
        let default_prob = hazard.conditional_default_prob(as_of, index.premium_leg.end)?;
        let lgd = 1.0 - index.protection_leg.recovery_rate;

        Ok(notional * default_prob * lgd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cds_index::CdsIndexConstituent;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::{context::MarketContext, term_structures::HazardCurve};
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations_core::instruments::CreditParams;
    use std::sync::Arc;
    use time::macros::date;

    fn context(index: CdsIndex, market: MarketContext, as_of: Date) -> MetricContext {
        MetricContext::new(
            Arc::new(index),
            Arc::new(market),
            as_of,
            Money::new(0.0, Currency::USD).expect("mark"),
            MetricContext::default_config(),
        )
    }

    fn hazard(id: &str, rate: f64) -> HazardCurve {
        HazardCurve::builder(id)
            .base_date(date!(1900 - 01 - 01))
            .knots([(100.0, 7.0), (101.0, rate)])
            .recovery_rate(0.4)
            .build()
            .expect("hazard")
    }

    #[test]
    fn expected_loss_single_curve_uses_conditional_pd_and_index_factor() {
        for rate in [0.0, 0.02, 10.0] {
            for factor in [0.0, 0.75] {
                let mut index = CdsIndex::example().expect("index");
                index.premium_leg.end = date!(2026 - 01 - 01);
                index.index_factor = factor;
                index.protection_leg.recovery_rate = 0.4;
                let market = MarketContext::new()
                    .insert(hazard(index.protection_leg.credit_curve_id.as_str(), rate));
                let expected = index.notional.amount() * factor * 0.6 * -(-rate).exp_m1();
                let mut ctx = context(index.clone(), market.clone(), date!(2025 - 01 - 01));
                let actual = ExpectedLossCalculator.calculate(&mut ctx).expect("EL");
                assert!((actual - expected).abs() < 1e-8);
                let mut matured = context(index, market, date!(2026 - 01 - 01));
                assert_eq!(
                    ExpectedLossCalculator
                        .calculate(&mut matured)
                        .expect("matured"),
                    0.0
                );
            }
        }
    }

    #[test]
    fn expected_loss_constituents_preserve_explicit_defaults_and_active_weights() {
        let mut index = CdsIndex::example().expect("index");
        index.premium_leg.end = date!(2026 - 01 - 01);
        index.index_factor = 0.75;
        index.pricing = IndexPricing::Constituents;
        let mut first = CreditParams::corporate_standard("FIRST", "FIRST-HZ");
        first.recovery_rate = 0.2;
        let mut second = CreditParams::corporate_standard("SECOND", "SECOND-HZ");
        second.recovery_rate = 0.5;
        index.constituents = vec![
            CdsIndexConstituent::active(first, 0.2),
            CdsIndexConstituent::active(second, 0.6),
            CdsIndexConstituent {
                credit: CreditParams::corporate_standard("DEFAULTED", "MISSING-HZ"),
                weight: 0.2,
                defaulted: true,
            },
        ];
        let market = MarketContext::new()
            .insert(hazard("FIRST-HZ", 0.02))
            .insert(hazard("SECOND-HZ", 0.04));
        let expected = index.notional.amount()
            * 0.75
            * (0.25 * 0.8 * -(-0.02_f64).exp_m1() + 0.75 * 0.5 * -(-0.04_f64).exp_m1());
        let mut ctx = context(index.clone(), market, date!(2025 - 01 - 01));
        assert!((ExpectedLossCalculator.calculate(&mut ctx).expect("EL") - expected).abs() < 1e-8);
        for constituent in &mut index.constituents {
            constituent.defaulted = true;
        }
        let mut ctx = context(index, MarketContext::new(), date!(2025 - 01 - 01));
        assert_eq!(
            ExpectedLossCalculator.calculate(&mut ctx).expect("settled"),
            0.0
        );
    }
}
