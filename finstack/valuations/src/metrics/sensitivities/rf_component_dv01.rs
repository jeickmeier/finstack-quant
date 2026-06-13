//! DV01 for instruments discounted at a flat instrument-level rate
//! (WACC / property discount rate) rather than a market curve.
//!
//! DCF equity and real-estate DCF valuations always discount at their own
//! rate ; they have no direct curve sensitivity. Their
//! rate risk comes from the risk-free component embedded in that rate
//! (`wacc = rf + risk_premium`). Because the decomposition is additive,
//! `∂PV/∂rf = ∂PV/∂rate`, so DV01 is computed by central-difference bumps of
//! the discount rate itself.
//!
//! Bucketed (key-rate) DV01 applies the rf bump tenor-by-tenor with the same
//! triangular bucket weights used by the curve-based key-rate calculator
//! (flat half-wings at the first and last buckets), so the bucket weights
//! partition unity and the bucketed values sum exactly to the parallel DV01.

use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::math::neumaier_sum;
use std::marker::PhantomData;

/// Instruments whose PV is discounted at a flat instrument-level rate with an
/// additive risk-free component that can be bumped per tenor.
pub(crate) trait RfComponentPriced: Instrument {
    /// Present value with the discount rate bumped by `bump_at(t)` (absolute,
    /// decimal) at each cashflow tenor `t` (in years). `bump_at = |_| 0.0`
    /// must reproduce the unbumped PV.
    fn pv_with_rf_bump(
        &self,
        market: &MarketContext,
        as_of: Date,
        bump_at: &dyn Fn(f64) -> f64,
    ) -> finstack_core::Result<f64>;
}

/// DV01 calculation mode for [`RfComponentDv01Calculator`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RfDv01Mode {
    /// Single scalar from a parallel rf bump.
    Parallel,
    /// Key-rate buckets via triangular tenor weights (stored as series).
    Bucketed,
}

/// DV01 calculator bumping the risk-free component inside a flat discount
/// rate .
pub(crate) struct RfComponentDv01Calculator<I> {
    mode: RfDv01Mode,
    _phantom: PhantomData<I>,
}

impl<I> RfComponentDv01Calculator<I> {
    pub(crate) fn new(mode: RfDv01Mode) -> Self {
        Self {
            mode,
            _phantom: PhantomData,
        }
    }
}

/// Triangular bucket weight at tenor `t` for bucket `i` of a strictly
/// increasing grid, with flat half-wings at the ends so the weights
/// partition unity over all tenors.
fn triangular_weight(buckets: &[f64], i: usize, t: f64) -> f64 {
    let last = buckets.len() - 1;
    let target = buckets[i];
    match (i == 0, i == last) {
        (true, true) => 1.0,
        (true, false) => {
            let next = buckets[i + 1];
            if t <= target {
                1.0
            } else if t < next {
                (next - t) / (next - target)
            } else {
                0.0
            }
        }
        (false, true) => {
            let prev = buckets[i - 1];
            if t >= target {
                1.0
            } else if t > prev {
                (t - prev) / (target - prev)
            } else {
                0.0
            }
        }
        (false, false) => {
            let prev = buckets[i - 1];
            let next = buckets[i + 1];
            if t > prev && t <= target {
                (t - prev) / (target - prev)
            } else if t > target && t < next {
                (next - t) / (next - target)
            } else {
                0.0
            }
        }
    }
}

impl<I> MetricCalculator for RfComponentDv01Calculator<I>
where
    I: RfComponentPriced + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;
        let bump_bp = defaults.rate_bump_bp;
        let delta = bump_bp / 10_000.0;
        let market = context.curves.as_ref();
        let as_of = context.as_of;

        match self.mode {
            RfDv01Mode::Parallel => {
                let pv_up = instrument.pv_with_rf_bump(market, as_of, &|_| delta)?;
                let pv_down = instrument.pv_with_rf_bump(market, as_of, &|_| -delta)?;
                Ok((pv_up - pv_down) / (2.0 * bump_bp))
            }
            RfDv01Mode::Bucketed => {
                let buckets = defaults.dv01_buckets_years.as_slice();
                for win in buckets.windows(2) {
                    if win[1] <= win[0] {
                        return Err(finstack_core::Error::Validation(format!(
                            "key-rate buckets must be strictly increasing, got {buckets:?}"
                        )));
                    }
                }

                let mut series: Vec<(std::borrow::Cow<'static, str>, f64)> =
                    Vec::with_capacity(buckets.len());
                for (i, &target) in buckets.iter().enumerate() {
                    let pv_up = instrument.pv_with_rf_bump(market, as_of, &|t| {
                        delta * triangular_weight(buckets, i, t)
                    })?;
                    let pv_down = instrument.pv_with_rf_bump(market, as_of, &|t| {
                        -delta * triangular_weight(buckets, i, t)
                    })?;
                    let dv01 = (pv_up - pv_down) / (2.0 * bump_bp);
                    series.push((sens_config::format_bucket_label_cow(target), dv01));
                }

                let total = neumaier_sum(series.iter().map(|(_, v)| *v));
                context.store_bucketed_series(MetricId::BucketedDv01, series);
                Ok(total)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangular_weights_partition_unity() {
        let buckets = [0.25, 1.0, 5.0, 10.0];
        for &t in &[0.0, 0.1, 0.25, 0.5, 1.0, 3.0, 5.0, 7.5, 10.0, 30.0] {
            let sum: f64 = (0..buckets.len())
                .map(|i| triangular_weight(&buckets, i, t))
                .sum();
            assert!(
                (sum - 1.0).abs() < 1e-12,
                "weights at t={t} must sum to 1, got {sum}"
            );
        }
    }

    #[test]
    fn single_bucket_is_parallel() {
        let buckets = [5.0];
        for &t in &[0.0, 5.0, 100.0] {
            assert!((triangular_weight(&buckets, 0, t) - 1.0).abs() < 1e-15);
        }
    }
}
