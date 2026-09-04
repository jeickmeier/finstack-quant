//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::instruments::fixed_income::bond::metrics::effective::{
    option_risk_bond_and_base_price, option_risk_curve_id,
};
use crate::instruments::Bond;
use crate::instruments::BondRiskBasis;
use crate::metrics::sensitivities::cs01::sensitivity_central_diff;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::market_data::context::BumpSpec;
use finstack_quant_core::types::CurveId;
use std::borrow::Cow;

/// Calculates option-aware bond DV01.
///
/// Bonds with call, put, or return-floor rights and market price quotes must not
/// reprice bumped scenarios from the fixed clean price. Convert the quote into
/// the equivalent constant-OAS model input, then bump the tree curve and reprice.
pub(crate) struct BondDv01Calculator;

impl MetricCalculator for BondDv01Calculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::DurationMod, MetricId::Ytm]
    }

    fn dynamic_dependencies<'a>(&'a self, context: &MetricContext) -> Cow<'a, [MetricId]> {
        let Ok(bond) = context.instrument_as::<Bond>() else {
            return Cow::Borrowed(self.dependencies());
        };
        let has_options = bond.return_floor.is_some()
            || bond.call_put.as_ref().is_some_and(|cp| cp.has_options());
        let has_price_driver = bond
            .instrument_pricing_overrides
            .market_quotes
            .has_price_driver();
        if super::bond_risk_basis(context) == BondRiskBasis::CallableOas
            || (!has_options && !has_price_driver)
        {
            Cow::Borrowed(&[])
        } else {
            Cow::Borrowed(self.dependencies())
        }
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let has_options = bond.return_floor.is_some()
            || bond.call_put.as_ref().is_some_and(|cp| cp.has_options());
        let basis = super::bond_risk_basis(context);

        if basis != BondRiskBasis::CallableOas {
            if !bond
                .instrument_pricing_overrides
                .market_quotes
                .has_price_driver()
                && !has_options
            {
                return crate::metrics::UnifiedDv01Calculator::<Bond>::new(
                    crate::metrics::Dv01CalculatorConfig::parallel_combined(),
                )
                .calculate(context);
            }

            // Default (Workout) basis: a bond with embedded exercise rights is handled by
            // the yield-basis DV01, the dollar analogue of `DurationMod`. This
            // keeps it consistent with `DurationMod`, `Convexity` and
            // `YieldDv01` on this basis (they all use the quoted-yield /
            // workout convention), and — when a market price is quoted — the
            // workout path feeding `DurationMod` captures the embedded option.
            //
            // The previous behaviour cloned the bond with `call_put = None`
            // and curve-bumped that bullet, which silently discarded the
            // option and reported a curve sensitivity inconsistent in both
            // methodology and units with the rest of the Workout-basis family.
            let duration_mod = context
                .computed
                .get(&MetricId::DurationMod)
                .copied()
                .ok_or_else(|| crate::metrics::metric_not_found(MetricId::DurationMod))?;
            let ytm = context
                .computed
                .get(&MetricId::Ytm)
                .copied()
                .ok_or_else(|| crate::metrics::metric_not_found(MetricId::Ytm))?;
            return super::yield_dv01::yield_basis_dv01(bond, context, duration_mod, ytm);
        }

        let has_price_driver = bond
            .instrument_pricing_overrides
            .market_quotes
            .has_price_driver();
        if !has_price_driver {
            return crate::metrics::UnifiedDv01Calculator::<Bond>::new(
                crate::metrics::Dv01CalculatorConfig::parallel_combined(),
            )
            .calculate(context);
        }

        if !has_options {
            return super::risk_view::with_bond_risk_view(context, |ctx| {
                crate::metrics::UnifiedDv01Calculator::<Bond>::new(
                    crate::metrics::Dv01CalculatorConfig::parallel_combined(),
                )
                .calculate(ctx)
            });
        }

        let (risk_bond, _) = option_risk_bond_and_base_price(bond, context)?;
        let curve_id = option_risk_curve_id(&risk_bond);

        curve_bump_dv01(&risk_bond, context, &curve_id)
    }
}

fn curve_bump_dv01(
    bond: &Bond,
    context: &MetricContext,
    curve_id: &CurveId,
) -> finstack_quant_core::Result<f64> {
    let defaults = crate::metrics::sensitivities::config::from_context_or_default(
        context.get_config(),
        context.get_metric_overrides(),
    )?;
    let bump_bp = defaults.rate_bump_bp;
    if bump_bp.abs() <= f64::EPSILON {
        return Ok(0.0);
    }

    let market_up = context.curves.bump([MarketBump::Curve {
        id: curve_id.clone(),
        spec: BumpSpec::parallel_bp(bump_bp),
    }])?;
    let market_down = context.curves.bump([MarketBump::Curve {
        id: curve_id.clone(),
        spec: BumpSpec::parallel_bp(-bump_bp),
    }])?;

    let pv_up = context.reprice_instrument_raw(bond, &market_up, context.as_of)?;
    let pv_down = context.reprice_instrument_raw(bond, &market_down, context.as_of)?;
    Ok(sensitivity_central_diff(pv_up, pv_down, bump_bp))
}

/// Bucketed DV01 calculator for bonds that handles the case where a market price
/// quote (e.g. `quoted_clean_price`) is set.
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
            crate::metrics::UnifiedDv01Calculator::<Bond>::new(
                crate::metrics::Dv01CalculatorConfig::triangular_key_rate(),
            )
            .calculate(ctx)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::pricer::{
        InstrumentType, ModelKey, Pricer, PricerKey, PricerRegistry, PricingDispatch, PricingError,
    };
    use crate::results::ValuationResult;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::StubKind;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::Rate;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
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
                Money::new(777.0, Currency::USD),
            ))
        }
    }

    #[test]
    fn callable_oas_dv01_reprices_through_selected_custom_registry() {
        let as_of = date!(2025 - 01 - 15);
        let curve_id = CurveId::new("USD-OIS");
        let bond = Bond::fixed(
            "DV01-DISPATCH",
            Money::new(1_000.0, Currency::USD),
            Rate::from_decimal(0.04),
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
            StubKind::ShortFront,
            curve_id.clone(),
        )
        .expect("valid bond");
        let market = MarketContext::new().insert(
            DiscountCurve::builder(curve_id.clone())
                .base_date(as_of)
                .knots([(0.0, 1.0), (5.0, 0.82), (10.0, 0.67)])
                .build()
                .expect("valid discount curve"),
        );

        let native_context = MetricContext::new(
            Arc::new(bond.clone()),
            Arc::new(market.clone()),
            as_of,
            Money::new(1_000.0, Currency::USD),
            MetricContext::default_config(),
        );
        let native_dv01 = curve_bump_dv01(&bond, &native_context, &curve_id).expect("native DV01");
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
            Arc::new(bond.clone()),
            Arc::new(market),
            as_of,
            Money::new(777.0, Currency::USD),
            MetricContext::default_config(),
        );
        context.set_pricer_dispatch(PricingDispatch::registered(
            ModelKey::HazardRate,
            Arc::new(registry),
        ));

        let selected_dv01 =
            curve_bump_dv01(&bond, &context, &curve_id).expect("selected-model DV01");
        assert_eq!(selected_dv01, 0.0);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "up and down legs must both use the selected pricer"
        );
    }
}
