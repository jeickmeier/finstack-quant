//! Dollar roll risk and carry metrics.
//!
//! Standard rate-sensitivity metrics (DV01, bucketed DV01, theta) plus
//! carry-specific analytics: implied financing rate and roll specialness.

use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry};

/// Implied financing rate metric calculator.
///
/// Computes the annualized implied repo rate from the dollar roll drop,
/// expected coupon income, and principal paydown between settlement dates.
/// Uses the MBS cashflow engine for carry inputs.
///
/// Uses 0.5% monthly SMM as the prepayment assumption.
pub(crate) struct ImpliedFinancingRateCalculator;

impl MetricCalculator for ImpliedFinancingRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let roll: &crate::instruments::DollarRoll = context.instrument_as()?;
        let result = super::carry::implied_financing_rate(roll, 0.005)?;
        Ok(result.implied_rate)
    }
}

/// Roll specialness metric calculator.
///
/// Returns specialness in basis points (repo rate - implied financing rate).
/// Positive means rolling is cheaper than repo financing.
///
/// Uses 0.5% monthly SMM and resolves financing over the actual roll interval.
pub(crate) struct RollSpecialnessCalculator;

impl MetricCalculator for RollSpecialnessCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let roll: &crate::instruments::DollarRoll = context.instrument_as()?;
        let front = roll.front_settle_date()?;
        let back = roll.back_settle_date()?;
        let accrual = finstack_quant_core::dates::DayCount::Act360.year_fraction(
            front,
            back,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        let repo_rate = if let Some(id) = &roll.repo_curve_id {
            let curve = context.curves.get_forward(id)?;
            let t1 = curve.day_count().year_fraction(
                curve.base_date(),
                front,
                finstack_quant_core::dates::DayCountContext::default(),
            )?;
            let t2 = curve.day_count().year_fraction(
                curve.base_date(),
                back,
                finstack_quant_core::dates::DayCountContext::default(),
            )?;
            curve.rate_between(t1, t2)? * (t2 - t1) / accrual
        } else {
            let curve = context.curves.get_discount(&roll.discount_curve_id)?;
            (1.0 / curve.df_between_dates(front, back)? - 1.0) / accrual
        };
        super::carry::roll_specialness(roll, 0.005, repo_rate)
    }
}

/// Register dollar roll metrics with the registry.
pub(crate) fn register_dollar_roll_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::DollarRoll,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::DollarRoll,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::DollarRoll,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            (ImpliedFinancingRate, ImpliedFinancingRateCalculator),
            (RollSpecialness, RollSpecialnessCalculator)
        ]
    }
    Ok(())
}

#[cfg(test)]
mod production_mortgage_audit {
    use super::*;
    use finstack_quant_core::{
        currency::Currency,
        market_data::{
            context::MarketContext,
            term_structures::{DiscountCurve, ForwardCurve},
        },
        money::Money,
    };
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn specialness_resolves_the_supplied_repo_curve() {
        let as_of = date!(2026 - 03 - 01);
        let mut roll = crate::instruments::DollarRoll::example().expect("roll");
        roll.front_settlement_date = Some(date!(2026 - 03 - 11));
        roll.back_settlement_date = Some(date!(2026 - 04 - 11));
        roll.repo_curve_id = Some("REPO".into());
        let carry = crate::instruments::fixed_income::dollar_roll::carry::implied_financing_rate(
            &roll, 0.005,
        )
        .expect("carry");
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (5.0, 1.0)])
                    .build()
                    .expect("curve"),
            )
            .insert(
                ForwardCurve::builder("REPO", 31.0 / 360.0)
                    .base_date(as_of)
                    .day_count(finstack_quant_core::dates::DayCount::Act360)
                    .knots([(0.0, 0.0375), (5.0, 0.0375)])
                    .build()
                    .expect("forward"),
            );
        let mut context = MetricContext::new(
            Arc::new(roll),
            Arc::new(market),
            as_of,
            Money::from((0_i64, Currency::USD)),
            MetricContext::default_config(),
        );
        let actual = RollSpecialnessCalculator
            .calculate(&mut context)
            .expect("specialness");
        let expected = (0.0375 - carry.implied_rate) * 10000.0;
        assert!(
            (actual - expected).abs() < 1e-7,
            "{actual} versus {expected}"
        );
    }
}
