//! Variance swap metric registration; the vega calculator has its own file.

mod vega;

pub(crate) use vega::VegaCalculator;

use crate::variance_swap::VarianceSwap;
use finstack_quant_core::market_data::bumps::BumpSpec;
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register variance swap metrics with the registry.
pub(crate) fn register_variance_swap_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::VarianceSwap,
        metrics: [
            (Vega, VegaCalculator),
            (Dv01, Dv01Calculator),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            (VarianceVega, VarianceVegaCalculator),
            (ExpectedVariance, ExpectedVarianceCalculator),
            (RealizedVariance, RealizedVarianceCalculator),
            (VarianceNotional, VarianceNotionalCalculator),
            (VarianceStrikeVol, StrikeVolCalculator),
            (VarianceTimeToMaturity, TimeToMaturityCalculator),
        ]
    };
    Ok(())
}

/// Variance swap DV01 expressed as PV change per 1bp parallel discount-curve bump.
pub(crate) struct Dv01Calculator;

impl MetricCalculator for Dv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let swap: &VarianceSwap = context.instrument_as()?;
        let bump_bp =
            finstack_quant_valuations_core::metrics::sensitivities::config::resolve(context)?
                .rate_bump_bp;

        let base_pv = context.base_value.amount();
        let bumped_ctx = context.curves.as_ref().bump([MarketBump::Curve {
            id: swap.discount_curve_id.clone(),
            spec: BumpSpec::parallel_bp(bump_bp),
        }])?;
        let bumped_pv = swap.value(&bumped_ctx, context.as_of)?.amount();

        Ok((bumped_pv - base_pv) / bump_bp)
    }
}

/// Calculate the expected variance (blend of realized and forward).
pub(crate) struct ExpectedVarianceCalculator;

impl MetricCalculator for ExpectedVarianceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;
        let as_of = context.as_of;

        if as_of >= swap.final_observation_date()? {
            // At maturity, expected variance equals realized variance
            return swap.partial_realized_variance(&context.curves, as_of);
        }

        // If not started, expected variance is purely forward variance
        if as_of < swap.start_date {
            return swap.remaining_forward_variance(&context.curves, as_of);
        }

        // Partially observed: defer to the same seasoned blend the pricer feeds
        // into the payoff, so this metric always matches the variance implied by
        // the swap's PV. Both terms are weighted by contractual sample counts,
        // the basis settlement uses (W-33).
        swap.seasoned_expected_variance(&context.curves, as_of)
    }
}

/// Calculate variance notional.
pub(crate) struct VarianceNotionalCalculator;

impl MetricCalculator for VarianceNotionalCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;
        Ok(swap.notional.amount())
    }
}

/// Calculate the current realized variance to date.
pub(crate) struct RealizedVarianceCalculator;

impl MetricCalculator for RealizedVarianceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;
        // Use the instrument's own method which handles policy/market data consistently
        swap.partial_realized_variance(&context.curves, context.as_of)
    }
}

/// Calculate strike in volatility terms.
pub(crate) struct StrikeVolCalculator;

impl MetricCalculator for StrikeVolCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;
        Ok(swap.strike_variance.sqrt())
    }
}

/// Calculate time to maturity in years.
pub(crate) struct TimeToMaturityCalculator;

impl MetricCalculator for TimeToMaturityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;
        let as_of = context.as_of;

        let final_observation = swap.final_observation_date()?;
        if as_of >= final_observation {
            return Ok(0.0);
        }

        swap.day_count
            .year_fraction(as_of, final_observation, Default::default())
    }
}

/// Calculate variance vega (sensitivity to 1 point change in variance).
///
/// This is the clean `∂PV/∂V` of the forward-variance leg:
/// `variance_vega = DF · variance_notional · remaining_fraction · side`.
///
/// It satisfies the chain-rule identity with
/// [`vega::VegaCalculator`] (W-34):
///
/// ```text
/// vega = variance_vega · 2·σ_K · 0.01
/// ```
///
/// where `σ` is the current remaining forward volatility used by the PV vega.
/// Strike volatility remains the basis for quoted-notional conversion
/// ([`VarianceSwap::vega_to_variance_notional`]).
pub(crate) struct VarianceVegaCalculator;

impl MetricCalculator for VarianceVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;
        // `compute_pv` weights forward variance by the unobserved share of
        // contractual samples, so its derivative uses the same weight.
        let remaining_fraction = 1.0 - swap.realized_fraction_by_observations(context.as_of)?;
        let disc = context
            .curves
            .get_discount(swap.discount_curve_id.as_str())?;
        let df = disc.df_between_dates(context.as_of, swap.effective_settlement_date()?)?;
        Ok(df * swap.notional.amount() * remaining_fraction * swap.side.sign())
    }
}

#[cfg(test)]
mod expected_variance_tests {
    use super::*;
    use crate::variance_swap::pricer::compute_pv;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations_core::instruments::PayReceive;
    use finstack_quant_valuations_core::instruments::{Attributes, Instrument};
    use std::sync::Arc;
    use time::macros::date;

    /// W-33 regression: the `ExpectedVariance` metric must equal the expected
    /// variance the swap is actually priced on, i.e. `pv = payoff(metric) · df`.
    ///
    /// The seasoned blend weights realized and forward variance by contractual
    /// sample counts. For a weekend-skipping daily schedule that differs from a
    /// calendar-time blend, so this pins the metric to the PV and guards against
    /// the metric and pricer drifting apart again.
    #[test]
    fn expected_variance_metric_matches_pv_for_weekend_skipping_schedule() {
        let start = date!(2025 - 01 - 06); // Monday
        let maturity = date!(2025 - 06 - 30); // Monday
        let as_of = date!(2025 - 04 - 18); // Friday, mid-life

        let mut swap = VarianceSwap::builder()
            .id(InstrumentId::new("VARSPX-EV"))
            .underlying_ticker("SPX".to_string())
            .spot_id(finstack_quant_core::types::PriceId::new("SPX"))
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(finstack_quant_core::types::PriceId::new(
                "SPX-DIVYIELD",
            )))
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_frequency(Tenor::daily())
            .observation_calendar_id("USNY".into())
            .realized_var_method(finstack_quant_core::math::stats::RealizedVarMethod::CloseToClose)
            .price_series_policy(crate::variance_swap::EquityPriceSeriesPolicy::Adjusted)
            .side(PayReceive::Receive)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("expected-variance swap");
        swap.instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.20);

        // Non-trivial close path on every past (weekday) observation so realized
        // variance differs from the scalar-vol forward variance.
        let obs: Vec<_> = swap
            .observation_dates()
            .expect("observation schedule")
            .into_iter()
            .filter(|&d| d <= as_of)
            .enumerate()
            .map(|(i, d)| (d, 100.0 * (1.0 + 0.002 * (i as f64 % 4.0 - 1.5))))
            .collect();
        let series = ScalarTimeSeries::new("SPX", obs, None).expect("series");
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, 0.96)])
            .build()
            .expect("discount curve");
        let market = MarketContext::new().insert(disc).insert_series(series);

        // Everything that borrows `market` must be evaluated before it is moved
        // into the `MetricContext`.
        let pv = compute_pv(&swap, &market, as_of).expect("seasoned pv");
        let df = (market.get_discount("USD-OIS").expect("curve").as_ref())
            .df_between_dates(as_of, swap.maturity)
            .expect("df");

        // Calendar-time blend, used only to prove the schedule is non-uniform so
        // this test is a real regression guard.
        let w = swap.time_elapsed_fraction(as_of);
        let count_realized = swap
            .partial_realized_variance(&market, as_of)
            .expect("obs-count realized");
        let forward = swap
            .remaining_forward_variance(&market, as_of)
            .expect("forward");
        let count_blend = count_realized * w + forward * (1.0 - w);

        let base_value = swap.value(&market, as_of).expect("base value");
        let instrument: Arc<dyn Instrument> = Arc::new(swap.clone());
        let mut ctx = MetricContext::new(
            instrument,
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );
        let metric = ExpectedVarianceCalculator
            .calculate(&mut ctx)
            .expect("expected variance");

        // 1) The metric, fed through the payoff and discounted, reproduces the PV
        //    exactly: the reported expected variance is the one the swap prices on.
        let pv_from_metric = swap.payoff(metric).expect("valid payoff").amount() * df;
        let tol = 1e-6 * pv.amount().abs().max(1.0);
        assert!(
            (pv.amount() - pv_from_metric).abs() <= tol,
            "expected-variance metric must reproduce the PV: pv={} payoff(metric)*df={}",
            pv.amount(),
            pv_from_metric,
        );

        // 2) The weekend-skipping schedule makes the calendar-time blend
        //    materially different, so the sample-count basis is load-bearing.
        assert!(
            (metric - count_blend).abs() / count_blend.abs().max(1e-12) > 1e-3,
            "metric ({metric}) must differ from the calendar-time blend ({count_blend})"
        );
    }
}
