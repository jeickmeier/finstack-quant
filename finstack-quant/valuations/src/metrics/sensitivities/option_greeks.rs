//! Generic option greeks metric adapters.
//!
//! These calculators eliminate the per-instrument explosion of:
//! `metrics/{delta,gamma,vega,rho,theta,vanna,volga}.rs`
//! by delegating to the consolidated [`OptionGreeksProvider`] trait.
//! Equity's native provider is used only for its Black-Scholes pricing model;
//! explicitly selected alternate models use the context's repricing dispatch.

use std::marker::PhantomData;
use std::sync::Arc;

use crate::instruments::common_impl::traits::{
    GreekBumps, Instrument, OptionGreekKind, OptionGreeks, OptionGreeksProvider,
    OptionGreeksRequest,
};
use crate::metrics::{metric_not_found, MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::Result;

/// Selected-model derivative to parallel decimal curve rates. HW single-curve
/// products bump the discount curve; multi-curve caps bump their projection
/// curve while holding discounting fixed. Parameters are held or recalibrated
/// exactly as in each selected pricer's normal market-input resolution.
pub fn selected_rate_greek(
    context: &MetricContext,
    curve_id: &finstack_quant_core::types::CurveId,
    second: bool,
) -> Result<f64> {
    use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
    let config = super::config::resolve(context)?;
    let bump_bp = config.rate_bump_bp;
    let h = bump_bp * 1e-4;
    let price = |direction: f64| {
        let market = context.curves.bump([MarketBump::Curve {
            id: curve_id.clone(),
            spec: BumpSpec::parallel_bp(bump_bp * direction),
        }])?;
        context.reprice_raw(&market, context.as_of)
    };
    let up = price(1.0)?;
    let down = price(-1.0)?;
    if second {
        let base = context.reprice_raw(&context.curves, context.as_of)?;
        Ok((up - 2.0 * base + down) / (h * h))
    } else {
        Ok((up - down) / (2.0 * h))
    }
}

pub struct OptionGreekCalculator<I> {
    kind: OptionGreekKind,
    _phantom: PhantomData<I>,
}

impl<I> OptionGreekCalculator<I> {
    fn new(kind: OptionGreekKind) -> Self {
        Self {
            kind,
            _phantom: PhantomData,
        }
    }

    pub fn delta() -> Self {
        Self::new(OptionGreekKind::Delta)
    }

    pub fn gamma() -> Self {
        Self::new(OptionGreekKind::Gamma)
    }

    pub fn vega() -> Self {
        Self::new(OptionGreekKind::Vega)
    }

    pub fn theta() -> Self {
        Self::new(OptionGreekKind::Theta)
    }

    pub fn rho() -> Self {
        Self::new(OptionGreekKind::Rho)
    }

    pub fn foreign_rho() -> Self {
        Self::new(OptionGreekKind::ForeignRho)
    }

    pub fn vanna() -> Self {
        Self::new(OptionGreekKind::Vanna)
    }

    pub fn volga() -> Self {
        Self::new(OptionGreekKind::Volga)
    }
}

impl<I> MetricCalculator for OptionGreekCalculator<I>
where
    I: Instrument + OptionGreeksProvider + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let metric_id = self.kind.metric_id();
        if let Some(value) = context.computed.get(&metric_id) {
            return Ok(*value);
        }

        let native_model = context.instrument.default_model();
        if context
            .pricing_model()
            .is_some_and(|model| model != native_model)
        {
            context.instrument_as::<I>()?;
            let instrument = Arc::clone(&context.instrument);
            if let Some(inst) = instrument.as_any().downcast_ref::<I>() {
                if let Some(value) = inst.selected_model_greek(self.kind, context)? {
                    context.computed.insert(metric_id, value);
                    return Ok(value);
                }
            }
        }

        let base_pv =
            matches!(self.kind, OptionGreekKind::Volga).then(|| context.base_value.amount());
        let bumps = GreekBumps::from(&super::config::resolve(context)?);
        let inst: &I = context.instrument_as()?;
        let greeks = inst.option_greeks(
            &context.curves,
            context.as_of,
            &OptionGreeksRequest {
                greek: self.kind,
                base_pv,
                bumps,
            },
        )?;
        store_available_greeks(context, greeks);
        context
            .computed
            .get(&metric_id)
            .copied()
            .ok_or_else(|| metric_not_found(metric_id))
    }
}

fn store_available_greeks(context: &mut MetricContext, greeks: OptionGreeks) {
    for (metric, value) in [
        (MetricId::Delta, greeks.delta),
        (MetricId::Gamma, greeks.gamma),
        (MetricId::Vega, greeks.vega),
        (MetricId::Theta, greeks.theta),
        (MetricId::Rho, greeks.rho_bp),
        (MetricId::ForeignRho, greeks.foreign_rho_bp),
        (MetricId::Vanna, greeks.vanna),
        (MetricId::Volga, greeks.volga),
    ] {
        if let Some(value) = value {
            context.computed.entry(metric).or_insert(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::instruments::{EquityOption, OptionType, PricingOptions};
    use crate::metrics::MetricId;
    use crate::pricer::{standard_pricer_registry, ModelKey};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::{
        context::MarketContext, scalars::MarketScalar, surfaces::VolSurface,
        term_structures::DiscountCurve,
    };
    use time::macros::date;

    fn fixture() -> (EquityOption, MarketContext) {
        let option = EquityOption::european(
            "MODEL-GREEKS",
            "EQ",
            100.0,
            date!(2026 - 01 - 01),
            1.0,
            Currency::USD,
            OptionType::Call,
        )
        .expect("option");
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(date!(2025 - 01 - 01))
            .knots([(0.0, 1.0), (3.0, 1.0)])
            .build()
            .expect("discount curve");
        let surface = VolSurface::builder("EQUITY-VOL")
            .expiries(&[1.0])
            .strikes(&[100.0])
            .row(&[0.20])
            .build()
            .expect("surface");
        let mut market = MarketContext::new()
            .insert(curve)
            .insert_surface(surface)
            .insert_price("EQUITY-SPOT", MarketScalar::Unitless(100.0))
            .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(0.0));
        for (name, value) in [
            ("HESTON_KAPPA", 2.0),
            ("HESTON_THETA", 0.04),
            ("HESTON_SIGMA_V", 0.6),
            ("HESTON_RHO", -0.7),
            ("HESTON_V0", 0.04),
        ] {
            market = market.insert_price(name, MarketScalar::Unitless(value));
        }
        (option, market)
    }

    fn price(option: &EquityOption, market: &MarketContext, model: ModelKey) -> f64 {
        standard_pricer_registry()
            .price_with_metrics(
                option,
                model,
                market,
                date!(2025 - 01 - 01),
                &[],
                PricingOptions::default(),
            )
            .expect("PV")
            .value
            .amount()
    }

    #[test]
    fn selected_heston_greeks_differentiate_selected_price() {
        let (mut option, market) = fixture();
        let model = ModelKey::HestonFourier;
        let base = price(&option, &market, model);
        // Independent smaller spot difference checks convergence, rather than
        // simply reproducing the calculator's chosen bump.
        let up = price(
            &option,
            &market
                .clone()
                .insert_price("EQUITY-SPOT", MarketScalar::Unitless(100.01)),
            model,
        );
        let down = price(
            &option,
            &market
                .clone()
                .insert_price("EQUITY-SPOT", MarketScalar::Unitless(99.99)),
            model,
        );
        let delta = (up - down) / 0.02;
        let gamma = (up - 2.0 * base + down) / 0.0001;
        for bump in [0.01, 0.001] {
            option
                .metric_pricing_overrides
                .bump_config
                .spot_bump_decimal = Some(bump);
            for metrics in [
                vec![MetricId::Delta, MetricId::Vega, MetricId::Gamma],
                vec![MetricId::Gamma, MetricId::Vega, MetricId::Delta],
            ] {
                let result = standard_pricer_registry()
                    .price_with_metrics(
                        &option,
                        model,
                        &market,
                        date!(2025 - 01 - 01),
                        &metrics,
                        PricingOptions::default(),
                    )
                    .expect("Heston Greeks");
                assert!((result.measures[&MetricId::Delta] - delta).abs() < 5e-4);
                assert!((result.measures[&MetricId::Gamma] - gamma).abs() < 1e-4);
                assert_eq!(
                    result.measures[&MetricId::Vega],
                    0.0,
                    "fixed Heston parameters do not depend on the auxiliary vol surface"
                );
            }
        }
        let native = standard_pricer_registry()
            .price_with_metrics(
                &option,
                ModelKey::Black76,
                &market,
                date!(2025 - 01 - 01),
                &[MetricId::Delta, MetricId::Gamma, MetricId::Vega],
                PricingOptions::default(),
            )
            .expect("native Greeks");
        assert_eq!(
            native.measures[&MetricId::Delta],
            option
                .delta(&market, date!(2025 - 01 - 01))
                .expect("native delta")
        );
        assert!((native.measures[&MetricId::Delta] - delta).abs() > 0.05);
    }

    #[test]
    fn selected_rough_and_mc_models_keep_their_pricing_dispatch() {
        let (mut option, mut market) = fixture();
        option.instrument_pricing_overrides.model_config.mc_paths = Some(512);
        option
            .instrument_pricing_overrides
            .model_config
            .mc_seed_scenario = Some("greeks_crn".into());
        for (name, value) in [
            ("ROUGH_HESTON_KAPPA", 2.0),
            ("ROUGH_HESTON_THETA", 0.04),
            ("ROUGH_HESTON_SIGMA_V", 0.3),
            ("ROUGH_HESTON_RHO", -0.7),
            ("ROUGH_HESTON_V0", 0.04),
            ("ROUGH_HESTON_HURST", 0.1),
            ("ROUGH_BERGOMI_ETA", 0.8),
            ("ROUGH_BERGOMI_HURST", 0.1),
            ("ROUGH_BERGOMI_RHO", -0.7),
        ] {
            market = market.insert_price(name, MarketScalar::Unitless(value));
        }
        // This is a dispatch/CRN regression, not a statistical certification
        // of each model: all three stencil prices use the same sampled paths.
        for model in [
            ModelKey::MonteCarloHeston,
            ModelKey::RoughHestonFourier,
            ModelKey::MonteCarloRoughHeston,
            ModelKey::MonteCarloRoughBergomi,
        ] {
            let base = price(&option, &market, model);
            let up = price(
                &option,
                &market
                    .clone()
                    .insert_price("EQUITY-SPOT", MarketScalar::Unitless(101.0)),
                model,
            );
            let down = price(
                &option,
                &market
                    .clone()
                    .insert_price("EQUITY-SPOT", MarketScalar::Unitless(99.0)),
                model,
            );
            let result = standard_pricer_registry()
                .price_with_metrics(
                    &option,
                    model,
                    &market,
                    date!(2025 - 01 - 01),
                    &[MetricId::Delta, MetricId::Gamma],
                    PricingOptions::default(),
                )
                .expect("selected model Greeks");
            assert!(
                (result.measures[&MetricId::Delta] - (up - down) / 2.0).abs() < 1e-9,
                "delta lost {model:?} dispatch or common random numbers"
            );
            assert!(
                (result.measures[&MetricId::Gamma] - (up - 2.0 * base + down)).abs() < 1e-9,
                "gamma lost {model:?} dispatch or common random numbers"
            );
        }
    }

    #[test]
    fn selected_heston_theta_preserves_per_day_reporting_basis() {
        let (mut option, market) = fixture();
        let theta = |option: &EquityOption| {
            standard_pricer_registry()
                .price_with_metrics(
                    option,
                    ModelKey::HestonFourier,
                    &market,
                    date!(2025 - 01 - 01),
                    &[MetricId::Theta],
                    PricingOptions::default(),
                )
                .expect("Heston theta")
                .measures[&MetricId::Theta]
        };
        let calendar = theta(&option);
        assert!(calendar.abs() > 1e-6);
        option.metric_pricing_overrides.theta_day_basis =
            Some(crate::instruments::ThetaDayBasis::Trading252);
        let trading = theta(&option);
        assert!((trading / calendar - 365.0 / 252.0).abs() < 1e-12);
        // The native equity theta API is per day even when generic carry
        // metrics request a longer horizon.
        option.metric_pricing_overrides.theta_period =
            Some(finstack_quant_core::dates::Tenor::weekly());
        assert!((theta(&option) - trading).abs() < 1e-12);

        option.expiry = date!(2025 - 01 - 02);
        let last_day = theta(&option);
        assert!(
            last_day.is_finite() && last_day < 0.0,
            "last live day theta must not require a future expiry observation"
        );
    }
}
