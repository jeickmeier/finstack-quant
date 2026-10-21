//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::bond::Bond;
use finstack_quant_valuations_core::constants::numerical::ZERO_TOLERANCE;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};

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
        let ytm = context.computed_metric(&MetricId::Ytm)?;
        // YTM dependency ensures cashflows are already built and cached
        let super::YieldRiskBasis {
            yield_rate,
            timed,
            price,
        } = super::yield_risk_basis(context, ytm)?;
        let bond: &Bond = context.instrument_as()?;
        if price.abs() < ZERO_TOLERANCE {
            return Ok(0.0);
        }

        // Calculate Macaulay duration using quote_date as time origin
        let mut weighted_time = finstack_quant_core::math::summation::NeumaierAccumulator::new();

        for &(_, t, amount) in &timed {
            if t <= 0.0 {
                continue;
            }
            let df = crate::bond::pricing::quote_conversions::df_from_yield(
                yield_rate,
                t,
                crate::bond::pricing::quote_conversions::YieldCompounding::Street,
                bond.cashflow_spec.frequency(),
            )?;
            weighted_time.add(t * amount.amount() * df);
        }

        Ok(weighted_time.total() / price)
    }
}

#[cfg(test)]
mod tests {
    use crate::bond::Bond;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations_core::instruments::Instrument;
    use finstack_quant_valuations_core::metrics::MetricId;
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
            finstack_quant_valuations_core::instruments::InstrumentPricingOverrides::default()
                .with_quoted_clean_price_pct(100.0);

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
                finstack_quant_valuations_core::instruments::PricingOptions::default(),
            )
            .expect("duration result");
        assert_eq!(
            *result.measures.get("duration_mac").expect("duration_mac"),
            0.0
        );
    }
}
