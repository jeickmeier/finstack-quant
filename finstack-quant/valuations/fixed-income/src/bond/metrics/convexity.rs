//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::bond::pricing::settlement::QuoteDateContext;
use crate::bond::Bond;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::constants::numerical::ZERO_TOLERANCE;
use finstack_quant_valuations_core::instruments::BondRiskBasis;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};
use std::borrow::Cow;

/// Calculates convexity for bonds.
///
/// Convexity measures the curvature of the price/yield relationship and is
/// computed using the closed-form second derivative of price with respect to yield:
/// ```text
/// Convexity = (1 / P) * d²P/dy² / 100
/// ```
/// where `P` is the yield-implied price and `y` uses the bond's yield compounding
/// convention (street/periodic by default). The `/100` matches the
/// Bloomberg/street quoting convention (verified against the Bloomberg YAS
/// golden): the price impact of a yield move is
/// `ΔP/P ≈ −ModDur·Δy + (1/2)·Convexity·100·Δy²`.
///
/// # Quote-Date Convention
///
/// Convexity is computed relative to the **quote date** (settlement date when
/// `settlement_days` is set, otherwise `as_of`), consistent with YTM and duration.
/// Time to each cashflow is measured from the quote date.
///
/// # Dependencies
///
/// Requires `Ytm` metric to be computed first.
pub(crate) struct ConvexityCalculator;

impl MetricCalculator for ConvexityCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Ytm]
    }

    fn dynamic_dependencies<'a>(&'a self, context: &MetricContext) -> Cow<'a, [MetricId]> {
        let Ok(bond) = context.instrument_as::<Bond>() else {
            return Cow::Borrowed(self.dependencies());
        };
        let has_options = bond.return_floor.is_some()
            || bond.call_put.as_ref().is_some_and(|cp| cp.has_options());
        if has_options && super::bond_risk_basis(context) == BondRiskBasis::CallableOas {
            Cow::Borrowed(&[])
        } else {
            Cow::Borrowed(self.dependencies())
        }
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;

        // Callable/OAS model risk is opt-in. The default matches Bloomberg YAS
        // Workout risk: quoted-yield convexity on maturity/workout cashflows.
        let has_options = bond.return_floor.is_some()
            || bond.call_put.as_ref().is_some_and(|cp| cp.has_options());
        if has_options && super::bond_risk_basis(context) == BondRiskBasis::CallableOas {
            return Ok(super::effective::effective_convexity(bond, context, None)? / 100.0);
        }
        let workout = super::context_workout_path(context)?;
        let bond: &Bond = context.instrument_as()?;

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

        let comp = crate::bond::pricing::quote_conversions::YieldCompounding::Street;
        let frequency = bond.cashflow_spec.frequency();

        // Compute quote-date context (settlement date) for yield-based convexity
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        let (yield_rate, risk_flows, quote_date) =
            if let Some((workout_yield, workout_flows, workout_quote_date)) = workout {
                (workout_yield, Cow::Owned(workout_flows), workout_quote_date)
            } else {
                (ytm, Cow::Borrowed(flows.as_slice()), quote_ctx.quote_date)
            };

        // Calculate price from flows using quote_date to ensure consistency with YTM
        let timed = crate::bond::pricing::quote_conversions::bond_flow_times(
            bond,
            risk_flows.as_ref(),
            quote_date,
        )?;
        let price = crate::bond::pricing::quote_conversions::price_from_ytm_timed(
            &timed, frequency, yield_rate, comp,
        )?;
        if price.abs() < ZERO_TOLERANCE {
            return Ok(0.0);
        }

        // Calculate convexity using quote_date as time origin. Street
        // compounding: DF = (1 + y/m)^(-c) with c = m·t, so
        // d²DF/dy² = c·(c + 1) / (m + y)² · DF.
        let m = crate::bond::pricing::quote_conversions::periods_per_year(frequency)?.max(1.0);
        let mut d2_price = finstack_quant_core::math::summation::NeumaierAccumulator::new();
        for &(_, t, amount) in &timed {
            if t <= 0.0 {
                continue;
            }
            let df = crate::bond::pricing::quote_conversions::df_from_yield(
                yield_rate, t, comp, frequency,
            )?;
            let c = m * t;
            let denom = m + yield_rate;
            d2_price.add(amount.amount() * c * (c + 1.0) / (denom * denom) * df);
        }

        Ok(d2_price.total() / price / 100.0)
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
    fn convexity_returns_zero_for_effectively_zero_price() {
        let as_of = date!(2025 - 01 - 01);
        let mut bond = Bond::fixed(
            "CVX-NEAR-ZERO",
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
                &[MetricId::Convexity],
                finstack_quant_valuations_core::instruments::PricingOptions::default(),
            )
            .expect("convexity result");
        assert_eq!(*result.measures.get("convexity").expect("convexity"), 0.0);
    }
}
