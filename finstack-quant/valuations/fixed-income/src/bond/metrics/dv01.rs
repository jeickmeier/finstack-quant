//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::bond::Bond;
use finstack_quant_valuations_core::metrics::sensitivities::dv01::{
    Dv01CalculatorConfig, UnifiedDv01Calculator,
};
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};
use std::borrow::Cow;
use std::sync::Arc;

/// Combined direct rate DV01 on the bond's selected holder valuation basis.
pub(crate) struct BondDv01Calculator;

impl MetricCalculator for BondDv01Calculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::ZSpread]
    }

    fn dynamic_dependencies<'a>(&'a self, context: &MetricContext) -> Cow<'a, [MetricId]> {
        BondBucketedDv01Calculator.dynamic_dependencies(context)
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        super::risk_view::with_bond_risk_view(context, |ctx| {
            direct_rate_dv01(ctx, Dv01CalculatorConfig::parallel_combined())
        })
    }
}

fn direct_rate_dv01(
    context: &mut MetricContext,
    config: Dv01CalculatorConfig,
) -> finstack_quant_core::Result<f64> {
    let bond = context.instrument_as::<Bond>()?;
    let model = &bond.instrument_pricing_overrides.model_config;
    let calculator = UnifiedDv01Calculator::new(config);
    if model.government_curve_id.is_none() && model.asw_forward_curve_id.is_none() {
        return calculator.calculate(context);
    }
    // These references belong to spread quotes. The direct rate experiment
    // reprices the holder cashflows with the selected model's curves.
    let mut rate_bond = bond.clone();
    let model = &mut rate_bond.instrument_pricing_overrides.model_config;
    model.government_curve_id = None;
    model.asw_forward_curve_id = None;
    context.with_instrument(Arc::new(rate_bond), |ctx| calculator.calculate(ctx))
}

/// Bucketed DV01 calculator for bonds that handles the case where a market price
/// quote (e.g. `quoted_clean_price_pct`) is set.
///
/// When no price driver is present the behaviour is identical to
/// `UnifiedDv01Calculator` with `triangular_key_rate` config. When a price
/// driver is present the bond is first converted to a spread-pinned clone
/// (Z-spread for plain bonds, OAS for callables, hazard-shifted for credit)
/// so that curve bumps in the key-rate loop move the PV rather than returning
/// the fixed quoted price.
pub(crate) struct BondBucketedDv01Calculator;

impl MetricCalculator for BondBucketedDv01Calculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::ZSpread]
    }

    fn dynamic_dependencies<'a>(&'a self, context: &MetricContext) -> Cow<'a, [MetricId]> {
        let Ok(bond) = context.instrument_as::<Bond>() else {
            return Cow::Borrowed(self.dependencies());
        };
        let needs_z_spread = super::risk_view::plain_rate_quote_requires_z_spread(context, bond);
        if needs_z_spread {
            Cow::Borrowed(self.dependencies())
        } else {
            Cow::Borrowed(&[])
        }
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        super::risk_view::with_bond_risk_view(context, |ctx| {
            direct_rate_dv01(ctx, Dv01CalculatorConfig::triangular_key_rate())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::StubKind;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, Rate};
    use finstack_quant_valuations_core::instruments::Instrument;
    use finstack_quant_valuations_core::pricer::{
        InstrumentType, ModelKey, Pricer, PricerKey, PricerRegistry, PricingDispatch, PricingError,
    };
    use finstack_quant_valuations_core::results::ValuationResult;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use time::macros::date;

    struct FixedHazardBondPricer {
        calls: Arc<AtomicUsize>,
    }

    impl Pricer for FixedHazardBondPricer {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Bond, ModelKey::HazardRate)
        }

        fn price_dyn(
            &self,
            instrument: &dyn Instrument,
            _market: &MarketContext,
            as_of: finstack_quant_core::dates::Date,
        ) -> std::result::Result<ValuationResult, PricingError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(ValuationResult::stamped(
                instrument.id(),
                as_of,
                Money::from((777_i64, Currency::USD)),
            ))
        }
    }

    #[test]
    fn direct_bond_dv01_reprices_through_selected_custom_registry() {
        let as_of = date!(2025 - 01 - 15);
        let curve_id = CurveId::new("USD-OIS");
        let bond = Bond::fixed(
            "DV01-DISPATCH",
            Money::from((1_000_i64, Currency::USD)),
            Rate::from_decimal(0.04).expect("valid rate fixture"),
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
            StubKind::ShortFront,
            curve_id.clone(),
        )
        .expect("valid bond");
        let market = MarketContext::new()
            .try_insert(
                DiscountCurve::builder(curve_id)
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (5.0, 0.82), (10.0, 0.67)])
                    .day_count(finstack_quant_core::dates::DayCount::Act365F)
                    .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                    .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                    .build()
                    .expect("valid discount curve"),
            )
            .expect("insert market curve");

        let mut native_context = MetricContext::new(
            Arc::new(bond.clone()),
            Arc::new(market.clone()),
            as_of,
            Money::from((1_000_i64, Currency::USD)),
            MetricContext::default_config(),
        );
        let native_dv01 = BondDv01Calculator
            .calculate(&mut native_context)
            .expect("native DV01");
        assert!(
            native_dv01.abs() > 1.0e-9,
            "control path must have non-zero DV01"
        );

        let calls = Arc::new(AtomicUsize::new(0));
        let mut registry = PricerRegistry::new();
        registry
            .register(FixedHazardBondPricer {
                calls: Arc::clone(&calls),
            })
            .expect("unique custom pricer");
        let mut context = MetricContext::new(
            Arc::new(bond),
            Arc::new(market),
            as_of,
            Money::from((777_i64, Currency::USD)),
            MetricContext::default_config(),
        );
        context.set_pricer_dispatch(PricingDispatch::registered(
            ModelKey::HazardRate,
            Arc::new(registry),
        ));

        let selected_dv01 = BondDv01Calculator
            .calculate(&mut context)
            .expect("selected-model DV01");
        assert_eq!(selected_dv01, 0.0);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "up and down legs must both use the selected pricer"
        );
    }
    #[test]
    fn quoted_bond_uses_one_direct_curve_basis_for_parallel_and_bucketed_dv01() {
        let as_of = date!(2025 - 01 - 15);
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.81), (10.0, 0.64)])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("discount curve");
        let market = MarketContext::new().try_insert(curve).expect("market");
        let mut bond = Bond::fixed(
            "QUOTE-RISK",
            Money::from((1_000_i64, Currency::USD)),
            Rate::from_decimal(0.04).unwrap(),
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
            StubKind::ShortFront,
            "USD-OIS",
        )
        .unwrap();
        bond.instrument_pricing_overrides = bond
            .instrument_pricing_overrides
            .with_quoted_clean_price_pct(98.0);
        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Dv01, MetricId::BucketedDv01],
                Default::default(),
            )
            .expect("quote-centered risk");
        let parallel = result.measures["dv01"];
        let bucketed = result.measures["bucketed_dv01"];
        let tolerance = 2e-4 * parallel.abs();
        assert!(
            parallel < 0.0 && (parallel - bucketed).abs() < tolerance,
            "{parallel} vs {bucketed}, tolerance {tolerance}"
        );
    }
}
