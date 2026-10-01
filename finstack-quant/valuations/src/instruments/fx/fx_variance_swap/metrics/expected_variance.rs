//! Expected contractual variance, shared with the PV calculation.

use super::super::types::FxVarianceSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Calculate the expected variance (blend of realized and forward).
pub(crate) struct ExpectedVarianceCalculator;

impl MetricCalculator for ExpectedVarianceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<FxVarianceSwap>()?;
        crate::instruments::fx::fx_variance_swap::pricer::expected_variance(
            swap,
            &context.curves,
            context.as_of,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::instruments::fx::fx_variance_swap::types::PayReceive;
    use crate::metrics::{MetricCalculator, MetricContext, MetricId};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::stats::RealizedVarMethod;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use std::sync::Arc;
    use time::macros::date;

    /// The metric uses the contractual return-sample normalization, identical
    /// to PV even for a weekend gap before the last fixing.
    #[test]
    fn expected_variance_uses_contractual_sample_normalization() {
        let start = date!(2025 - 01 - 06); // Monday
        let maturity = date!(2025 - 06 - 30); // Monday
        let as_of = date!(2025 - 06 - 27); // Friday, near maturity

        let usd = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.03_f64).exp())])
            .build()
            .expect("usd curve");
        let eur = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.01_f64).exp())])
            .build()
            .expect("eur curve");
        let provider = SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("valid rate");

        let swap = FxVarianceSwap::builder()
            .id(InstrumentId::new("FXVAR-EXPVAR-SEASON"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .spot_id("EURUSD".into())
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_frequency(Tenor::daily())
            .base_calendar_id("TARGET2".into())
            .quote_calendar_id("USNY".into())
            .realized_var_method(RealizedVarMethod::CloseToClose)
            .side(PayReceive::Receive)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("seasoned fx swap");

        let count_w = swap
            .realized_fraction_by_observations(as_of)
            .expect("observation fraction");
        let time_w = swap.time_elapsed_fraction(as_of).expect("time fraction");
        assert!(
            (count_w - time_w).abs() > 1e-4,
            "schedule must make count weight ({count_w}) differ from time weight ({time_w})"
        );

        let past: Vec<_> = swap
            .observation_dates()
            .expect("observation schedule")
            .into_iter()
            .filter(|&d| d <= as_of)
            .collect();
        let obs: Vec<_> = past
            .iter()
            .enumerate()
            .map(|(i, &d)| (d, 1.10 * (1.0 + 0.002 * (i as f64 % 3.0 - 1.0))))
            .collect();
        let series = ScalarTimeSeries::new("EURUSD", obs, None).expect("series");
        let surface = VolSurface::builder("EURUSD-VOL")
            .expiries(&[1.0])
            .strikes(&[0.9, 1.1, 1.3])
            .row(&[0.12, 0.10, 0.12])
            .build()
            .expect("surface");
        let market = MarketContext::new()
            .insert(usd)
            .insert(eur)
            .insert_fx(FxMatrix::new(Arc::new(provider)))
            .insert_series(series)
            .insert_surface(surface);

        let realized = swap
            .partial_realized_variance(&market, as_of)
            .expect("realized");
        let forward = swap
            .remaining_forward_variance(&market, as_of)
            .expect("forward");
        let expected_count = realized * count_w + forward * (1.0 - count_w);

        // The same expected contractual variance drives metric and PV.
        let expected_shared = crate::instruments::fx::fx_variance_swap::pricer::expected_variance(
            &swap, &market, as_of,
        )
        .expect("seasoned blend");
        let dom = market.get_discount("USD-OIS").expect("curve");
        let df = dom
            .df_between_dates(as_of, swap.maturity)
            .expect("date-based df");

        let instrument: Arc<dyn Instrument> = Arc::new(swap.clone());
        let base_value = swap.value(&market, as_of).expect("base value");
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

        assert!(
            (metric - expected_shared).abs() < 1e-9,
            "expected variance must use the shared sample-normalized value: metric={metric} \
             sample-normalized={expected_shared}"
        );
        let pv_from_metric = swap.payoff(metric).expect("valid payoff").amount() * df;
        assert!(
            (base_value.amount() - pv_from_metric).abs()
                < 1e-6 * base_value.amount().abs().max(1.0),
            "payoff(metric) discounted must reproduce the booked PV: pv={} from_metric={}",
            base_value.amount(),
            pv_from_metric
        );
        assert!(
            (metric - expected_count).abs() < 1e-9,
            "expected variance must preserve the return sample denominator"
        );
        let _ = MetricId::ExpectedVariance;
    }
}
