//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::constants::numerical::ZERO_TOLERANCE;
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use std::borrow::Cow;

/// Calculates Macaulay duration for bonds.
///
/// Macaulay duration is the weighted average time to receive cashflows, where
/// weights are the present values of each cashflow:
/// ```text
/// D_mac = Σ (t_i * PV(CF_i)) / Price
/// ```
///
/// # Quote-Date Convention
///
/// Macaulay duration is computed relative to the **quote date** (settlement date
/// when `settlement_days` is set, otherwise `as_of`), consistent with YTM.
/// Time to each cashflow is measured from the quote date.
///
/// # Dependencies
///
/// Requires `Ytm` metric to be computed first.
pub(crate) struct MacaulayDurationCalculator;

impl MetricCalculator for MacaulayDurationCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Ytm]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let workout = super::context_workout_path(context)?;
        let ytm = context
            .computed
            .get(&MetricId::Ytm)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:Ytm".to_string(),
                })
            })?;

        // YTM dependency ensures cashflows are already built and cached
        let flows: &Vec<(Date, Money)> = context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "context.cashflows".to_string(),
            })
        })?;

        let bond: &Bond = context.instrument_as()?;

        // Compute quote-date context (settlement date) for yield-based duration
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        let (yield_rate, risk_flows, quote_date) =
            if let Some((workout_yield, workout_flows, workout_quote_date)) = workout {
                (workout_yield, Cow::Owned(workout_flows), workout_quote_date)
            } else {
                (ytm, Cow::Borrowed(flows.as_slice()), quote_ctx.quote_date)
            };

        // Calculate price from flows using quote_date to ensure consistency with YTM
        let price =
            crate::instruments::fixed_income::bond::pricing::quote_conversions::price_from_ytm(
                bond,
                risk_flows.as_ref(),
                quote_date,
                yield_rate,
            )?;
        if price.abs() < ZERO_TOLERANCE {
            return Ok(0.0);
        }

        // Calculate Macaulay duration using quote_date as time origin
        let mut weighted_time = finstack_quant_core::math::summation::NeumaierAccumulator::new();

        for &(date, amount) in risk_flows.as_ref() {
            if date <= quote_date {
                continue;
            }
            let t = bond
                .cashflow_spec
                .day_count()
                .year_fraction(
                    quote_date,
                    date,
                    finstack_quant_core::dates::DayCountContext {
                        frequency: Some(bond.cashflow_spec.frequency()),
                        coupon_period: crate::instruments::fixed_income::bond::pricing::quote_conversions::icma_reference_period(
                            bond.cashflow_spec.day_count(), bond.cashflow_spec.frequency(),
                            risk_flows.iter().map(|(d, _)| *d), quote_date,
                        ),
                        ..Default::default()
                    },
                )?
                .max(0.0);
            let df = crate::instruments::fixed_income::bond::pricing::quote_conversions::df_from_yield(
                yield_rate,
                t,
                crate::instruments::fixed_income::bond::pricing::quote_conversions::YieldCompounding::Street,
                bond.cashflow_spec.frequency(),
            )?;
            weighted_time.add(t * amount.amount() * df);
        }

        Ok(weighted_time.total() / price)
    }
}

#[cfg(test)]
mod tests {
    use crate::instruments::fixed_income::bond::Bond;
    use crate::instruments::Instrument;
    use crate::metrics::MetricId;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use time::macros::date;

    #[test]
    fn duration_mac_returns_zero_for_effectively_zero_price() {
        let as_of = date!(2025 - 01 - 01);
        let mut bond = Bond::fixed(
            "DUR-NEAR-ZERO",
            Money::new(1e-12, Currency::USD).expect("valid money fixture"),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            as_of,
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bond.instrument_pricing_overrides =
            crate::instruments::InstrumentPricingOverrides::default()
                .with_quoted_clean_price(100.0);

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.8)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);

        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::DurationMac],
                crate::instruments::PricingOptions::default(),
            )
            .expect("duration result");
        assert_eq!(
            *result.measures.get("duration_mac").expect("duration_mac"),
            0.0
        );
    }
}
