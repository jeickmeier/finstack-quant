//! Expected variance metric (blend of realized and forward).

use super::super::types::VarianceSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Calculate the expected variance (blend of realized and forward).
pub(crate) struct ExpectedVarianceCalculator;

impl MetricCalculator for ExpectedVarianceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;
        let as_of = context.as_of;

        if as_of >= swap.maturity {
            // At maturity, expected variance equals realized variance
            return swap.partial_realized_variance(&context.curves, as_of);
        }

        // If not started, expected variance is purely forward variance
        if as_of < swap.start_date {
            return swap.remaining_forward_variance(&context.curves, as_of);
        }

        // Partially observed: defer to the same seasoned blend the pricer feeds
        // into the payoff, so this metric always matches the variance implied by
        // the swap's PV. The realized term is annualized on the day-count time
        // basis (V_accrued / t_elapsed) to match the blend weight `w`, rather than
        // the observation-count basis of `partial_realized_variance`, which would
        // disagree for non-uniform schedules (W-33).
        swap.seasoned_expected_variance(&context.curves, as_of)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::instruments::equity::variance_swap::pricer::{
        compute_pv, partial_realized_variance, remaining_forward_variance,
    };
    use crate::instruments::equity::variance_swap::types::PayReceive;
    use finstack_core::currency::Currency;
    use finstack_core::dates::{DayCount, Tenor};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::scalars::ScalarTimeSeries;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::Money;
    use finstack_core::types::{CurveId, InstrumentId};
    use std::sync::Arc;
    use time::macros::date;

    /// W-33 regression: the `ExpectedVariance` metric must equal the expected
    /// variance the swap is actually priced on, i.e. `pv = payoff(metric) · df`.
    ///
    /// The realized term in the seasoned blend is annualized on the day-count
    /// time basis (`V_accrued / t_elapsed`), not the observation-count basis of
    /// `partial_realized_variance`. For a weekend-skipping daily schedule the two
    /// genuinely differ, so this pins the metric to the PV and guards against the
    /// metric and pricer drifting apart again.
    #[test]
    fn expected_variance_metric_matches_pv_for_weekend_skipping_schedule() {
        let start = date!(2025 - 01 - 06); // Monday
        let maturity = date!(2025 - 06 - 30); // Monday
        let as_of = date!(2025 - 04 - 18); // Friday, mid-life

        let swap = VarianceSwap::builder()
            .id(InstrumentId::new("VARSPX-EV"))
            .underlying_ticker("SPX".to_string())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_freq(Tenor::daily())
            .realized_var_method(finstack_core::math::stats::RealizedVarMethod::CloseToClose)
            .side(PayReceive::Receive)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("expected-variance swap");

        // Non-trivial close path on every past (weekday) observation so realized
        // variance differs from the forward variance (= strike, no vol surface).
        let obs: Vec<_> = swap
            .observation_dates()
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
        let t = swap
            .day_count
            .year_fraction(as_of, swap.maturity, Default::default())
            .expect("year fraction");
        let df = market.get_discount("USD-OIS").expect("curve").df(t);

        // Observation-count blend — the old, wrong basis — used only to prove the
        // schedule is non-uniform so this test is a real regression guard.
        let w = swap.time_elapsed_fraction(as_of);
        let count_realized =
            partial_realized_variance(&swap, &market, as_of).expect("obs-count realized");
        let forward = remaining_forward_variance(&swap, &market, as_of).expect("forward");
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
        let pv_from_metric = swap.payoff(metric).amount() * df;
        let tol = 1e-6 * pv.amount().abs().max(1.0);
        assert!(
            (pv.amount() - pv_from_metric).abs() <= tol,
            "expected-variance metric must reproduce the PV: pv={} payoff(metric)*df={}",
            pv.amount(),
            pv_from_metric,
        );

        // 2) The weekend-skipping schedule makes the observation-count blend
        //    materially different, so the time-basis fix is load-bearing.
        assert!(
            (metric - count_blend).abs() / count_blend.abs().max(1e-12) > 1e-3,
            "metric ({metric}) must differ from the observation-count blend ({count_blend})"
        );
    }
}
