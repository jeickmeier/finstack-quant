//! FX option delta convention metrics.

use crate::fx_option::pricer::FxOptionGreeks;
use crate::fx_option::FxOption;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Cache model results without publishing sibling metrics into the registry.
/// The request drops this instrument-owned cache when its instrument changes.
fn convention_greeks(context: &mut MetricContext) -> Result<FxOptionGreeks> {
    if let Some(greeks) = context.ext::<FxOptionGreeks>() {
        return Ok(*greeks);
    }
    let option: &FxOption = context.instrument_as()?;
    let greeks = crate::fx_option::pricer::compute_greeks(option, &context.curves, context.as_of)?;
    context.set_ext(greeks);
    Ok(greeks)
}

/// Forward delta calculator for FX options.
pub(crate) struct DeltaForwardCalculator;

impl MetricCalculator for DeltaForwardCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        Ok(convention_greeks(context)?.delta_forward)
    }
}

/// Premium-adjusted spot delta calculator for FX options.
pub(crate) struct DeltaPremiumAdjustedSpotCalculator;

impl MetricCalculator for DeltaPremiumAdjustedSpotCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        Ok(convention_greeks(context)?.delta_premium_adjusted_spot)
    }
}

/// Premium-adjusted forward delta calculator for FX options.
pub(crate) struct DeltaPremiumAdjustedForwardCalculator;

impl MetricCalculator for DeltaPremiumAdjustedForwardCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        Ok(convention_greeks(context)?.delta_premium_adjusted_forward)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx_option::pricer::{compute_greeks, COMPUTE_GREEKS_CALLS};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations_core::metrics::{MetricId, MetricRegistry};
    use finstack_quant_valuations_core::pricer::InstrumentType;
    use std::sync::Arc;
    use time::macros::date;

    fn market() -> MarketContext {
        let as_of = date!(2025 - 01 - 01);
        let provider = SimpleFxProvider::new();
        provider
            .set_quotes(&[(Currency::EUR, Currency::USD, 1.2)])
            .expect("FX quote");
        let mut market = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(provider)));
        for (id, rate) in [("USD-OIS", 0.03_f64), ("EUR-OIS", 0.01_f64)] {
            market = market.insert(
                DiscountCurve::builder(id)
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, (-rate).exp())])
                    .build()
                    .expect("discount curve"),
            );
        }
        market.insert_surface(
            VolSurface::builder("EURUSD-VOL")
                .expiries(&[0.5, 1.0])
                .strikes(&[1.0, 1.2])
                .row(&[0.15, 0.15])
                .row(&[0.15, 0.15])
                .build()
                .expect("surface"),
        )
    }

    #[test]
    fn convention_metrics_compute_once_and_preserve_model_values_in_every_order() {
        let as_of = date!(2025 - 01 - 01);
        let calculators: [&dyn MetricCalculator; 3] = [
            &DeltaForwardCalculator,
            &DeltaPremiumAdjustedSpotCalculator,
            &DeltaPremiumAdjustedForwardCalculator,
        ];
        for premium in [Currency::EUR, Currency::USD] {
            for (expiry, strike) in [
                (date!(2026 - 01 - 01), 1.12),
                (as_of, 1.2),
                (date!(2024 - 12 - 31), 1.12),
                (date!(2026 - 01 - 01), 0.0),
            ] {
                let mut option = FxOption::example().expect("option");
                option.expiry = expiry;
                option.strike = strike;
                option.delta_convention.premium_currency = premium;
                let market = Arc::new(market());
                let expected = compute_greeks(&option, &market, as_of).expect("model greeks");
                let expected = [
                    expected.delta_forward,
                    expected.delta_premium_adjusted_spot,
                    expected.delta_premium_adjusted_forward,
                ];
                for order in [[0, 1, 2], [2, 0, 1], [1, 2, 0]] {
                    let mut context = MetricContext::new(
                        Arc::new(option.clone()),
                        Arc::clone(&market),
                        as_of,
                        Money::from((0_i64, Currency::USD)),
                        MetricContext::default_config(),
                    );
                    COMPUTE_GREEKS_CALLS.with(|count| count.set(0));
                    for index in order {
                        assert_eq!(
                            calculators[index].calculate(&mut context).expect("delta"),
                            expected[index]
                        );
                    }
                    COMPUTE_GREEKS_CALLS.with(|count| assert_eq!(count.get(), 1));
                    assert!(
                        context.computed.is_empty(),
                        "only the registry publishes metrics"
                    );
                }
            }
        }
    }

    #[test]
    fn convention_cache_preserves_registry_replacements_in_both_request_orders() {
        struct Replacement;
        impl MetricCalculator for Replacement {
            fn calculate(&self, _context: &mut MetricContext) -> Result<f64> {
                Ok(123.0)
            }
        }

        let mut registry = MetricRegistry::new();
        super::super::register_fx_option_metrics(&mut registry).expect("registry");
        registry.replace_metric(
            MetricId::DeltaPremiumAdjustedSpot,
            Arc::new(Replacement),
            &[InstrumentType::FxOption],
        );
        let mut option = FxOption::example().expect("option");
        option.expiry = date!(2026 - 01 - 01);
        let market = Arc::new(market());
        let as_of = date!(2025 - 01 - 01);
        let expected = compute_greeks(&option, &market, as_of)
            .expect("greeks")
            .delta_forward;
        for requested in [
            [MetricId::DeltaForward, MetricId::DeltaPremiumAdjustedSpot],
            [MetricId::DeltaPremiumAdjustedSpot, MetricId::DeltaForward],
        ] {
            let mut context = MetricContext::new(
                Arc::new(option.clone()),
                Arc::clone(&market),
                as_of,
                Money::from((0_i64, Currency::USD)),
                MetricContext::default_config(),
            );
            COMPUTE_GREEKS_CALLS.with(|count| count.set(0));
            let values = registry.compute(&requested, &mut context).expect("metrics");
            assert_eq!(values.get(&MetricId::DeltaForward), Some(&expected));
            assert_eq!(
                values.get(&MetricId::DeltaPremiumAdjustedSpot),
                Some(&123.0)
            );
            assert!(!context
                .computed
                .contains_key(&MetricId::DeltaPremiumAdjustedForward));
            COMPUTE_GREEKS_CALLS.with(|count| assert_eq!(count.get(), 1));
        }
    }

    #[test]
    fn changing_instrument_invalidates_convention_greeks() {
        let mut option = FxOption::example().expect("option");
        option.expiry = date!(2026 - 01 - 01);
        option.delta_convention.premium_currency = Currency::EUR;
        let market = Arc::new(market());
        let as_of = date!(2025 - 01 - 01);
        let mut changed = option.clone();
        changed.strike = 1.3;
        let expected = compute_greeks(&changed, &market, as_of).expect("changed greeks");
        let mut context = MetricContext::new(
            Arc::new(option),
            Arc::clone(&market),
            as_of,
            Money::from((0_i64, Currency::USD)),
            MetricContext::default_config(),
        );
        COMPUTE_GREEKS_CALLS.with(|count| count.set(0));
        let original = DeltaForwardCalculator
            .calculate(&mut context)
            .expect("original delta");
        assert!(context.ext::<FxOptionGreeks>().is_some());
        context.set_instrument(Arc::new(changed));
        assert!(context.ext::<FxOptionGreeks>().is_none());
        assert_eq!(
            DeltaPremiumAdjustedForwardCalculator
                .calculate(&mut context)
                .expect("changed premium delta"),
            expected.delta_premium_adjusted_forward
        );
        assert_eq!(
            DeltaForwardCalculator
                .calculate(&mut context)
                .expect("changed forward delta"),
            expected.delta_forward
        );
        assert_ne!(original, expected.delta_forward);
        COMPUTE_GREEKS_CALLS.with(|count| assert_eq!(count.get(), 2));
        assert!(context.computed.is_empty());
    }
}
