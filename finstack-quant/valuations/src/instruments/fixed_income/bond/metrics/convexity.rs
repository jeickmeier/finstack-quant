use crate::constants::numerical::ZERO_TOLERANCE;
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::Bond;
use crate::instruments::BondRiskBasis;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
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
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::metrics::{MetricRegistry, MetricId, MetricContext};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// // Convexity is computed automatically when requesting bond metrics
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub(crate) struct ConvexityCalculator;

impl MetricCalculator for ConvexityCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Ytm]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;

        // Callable/OAS model risk is opt-in. The default matches Bloomberg YAS
        // Workout risk: quoted-yield convexity on maturity/workout cashflows.
        let has_options = bond.call_put.as_ref().is_some_and(|cp| cp.has_options());
        if has_options && super::bond_risk_basis(context) == BondRiskBasis::CallableOas {
            return Ok(super::effective::effective_convexity(
                bond,
                context.curves.as_ref(),
                context.as_of,
                None,
            )? / 100.0);
        }

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

        let comp =
            crate::instruments::fixed_income::bond::pricing::quote_conversions::YieldCompounding::Street;
        let freq = bond.cashflow_spec.frequency();

        // Compute quote-date context (settlement date) for yield-based convexity
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        let (yield_rate, risk_flows, quote_date) =
            if let Some((workout_yield, workout_flows, workout_quote_date)) =
                super::quoted_workout_path(bond, context.curves.as_ref(), context.as_of, flows)?
            {
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

        // Calculate convexity using quote_date as time origin
        let mut d2_price = 0.0;
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
                        ..Default::default()
                    },
                )?
                .max(0.0);
            let df_second = df_second_derivative(yield_rate, t, comp, freq)?;
            d2_price += amount.amount() * df_second;
        }

        Ok(d2_price / price / 100.0)
    }
}

fn df_second_derivative(
    ytm: f64,
    t: f64,
    comp: crate::instruments::fixed_income::bond::pricing::quote_conversions::YieldCompounding,
    freq: finstack_quant_core::dates::Tenor,
) -> finstack_quant_core::Result<f64> {
    use crate::instruments::fixed_income::bond::pricing::quote_conversions::{
        df_from_yield, periods_per_year, YieldCompounding,
    };

    if t <= 0.0 {
        return Ok(0.0);
    }

    let df = df_from_yield(ytm, t, comp, freq)?;
    Ok(match comp {
        YieldCompounding::Simple => {
            let denom = 1.0 + ytm * t;
            2.0 * t * t / (denom * denom * denom)
        }
        YieldCompounding::Annual => {
            let denom = 1.0 + ytm;
            t * (t + 1.0) / (denom * denom) * df
        }
        YieldCompounding::Periodic(m) => {
            let m = m as f64;
            let c = m * t;
            let denom = m + ytm;
            c * (c + 1.0) / (denom * denom) * df
        }
        YieldCompounding::Continuous => t * t * df,
        YieldCompounding::Street => {
            let m = periods_per_year(freq)?.max(1.0);
            let c = m * t;
            let denom = m + ytm;
            c * (c + 1.0) / (denom * denom) * df
        }
        YieldCompounding::TreasuryActual => {
            let m = periods_per_year(freq)?.max(1.0);
            let period_length = 1.0 / m;

            if t <= period_length {
                let denom = 1.0 + ytm * t;
                2.0 * t * t / (denom * denom * denom)
            } else {
                let n_full = (t * m).floor();
                let stub_time = t - n_full / m;
                if stub_time <= 1e-10 {
                    let c = m * t;
                    let denom = m + ytm;
                    c * (c + 1.0) / (denom * denom) * df
                } else {
                    let df_stub = 1.0 / (1.0 + ytm * stub_time);
                    let df_periodic = (1.0 + ytm / m).powf(-n_full);

                    let df_stub_prime = -stub_time / (1.0 + ytm * stub_time).powi(2);
                    let df_stub_second =
                        2.0 * stub_time * stub_time / (1.0 + ytm * stub_time).powi(3);

                    let denom = m + ytm;
                    let df_periodic_prime = -(n_full / denom) * df_periodic;
                    let df_periodic_second =
                        n_full * (n_full + 1.0) / (denom * denom) * df_periodic;

                    df_stub_second * df_periodic
                        + 2.0 * df_stub_prime * df_periodic_prime
                        + df_stub * df_periodic_second
                }
            }
        }
    })
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
    fn convexity_returns_zero_for_effectively_zero_price() {
        let as_of = date!(2025 - 01 - 01);
        let mut bond = Bond::fixed(
            "CVX-NEAR-ZERO",
            Money::new(1e-12, Currency::USD),
            0.05,
            as_of,
            date!(2030 - 01 - 01),
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
                &[MetricId::Convexity],
                crate::instruments::PricingOptions::default(),
            )
            .expect("convexity result");
        assert_eq!(*result.measures.get("convexity").expect("convexity"), 0.0);
    }
}
