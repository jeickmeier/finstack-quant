//! Vanilla swaption pricer implementation.

use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::swaption::Swaption;
use crate::instruments::VolatilityModel;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::market_data::context::MarketContext;

/// European swaption pricer using the Black-76 formula.
pub struct SimpleSwaptionBlackPricer;

impl SimpleSwaptionBlackPricer {
    /// Create a Black-76 swaption pricer.
    pub fn new() -> Self {
        Self
    }
}

impl Default for SimpleSwaptionBlackPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for SimpleSwaptionBlackPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Swaption, ModelKey::Black76)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let swaption =
            crate::pricer::expect_inst::<Swaption>(instrument, InstrumentType::Swaption)?;

        price_swaption(
            swaption,
            market,
            as_of,
            ModelKey::Black76,
            VolatilityModel::Black,
        )
    }
}

/// European swaption pricer using the Bachelier normal formula.
pub struct SimpleSwaptionNormalPricer;

impl SimpleSwaptionNormalPricer {
    /// Create a Bachelier normal swaption pricer.
    pub fn new() -> Self {
        Self
    }
}

impl Default for SimpleSwaptionNormalPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for SimpleSwaptionNormalPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Swaption, ModelKey::Normal)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let swaption =
            crate::pricer::expect_inst::<Swaption>(instrument, InstrumentType::Swaption)?;

        price_swaption(
            swaption,
            market,
            as_of,
            ModelKey::Normal,
            VolatilityModel::Normal,
        )
    }
}

fn price_swaption(
    swaption: &Swaption,
    market: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
    model: ModelKey,
    expected_vol_model: VolatilityModel,
) -> std::result::Result<ValuationResult, PricingError> {
    let context = PricingErrorContext::from_instrument(swaption).model(model);
    if swaption.vol_model != expected_vol_model {
        return Err(PricingError::invalid_input_with_context(
            format!(
                "swaption volatility model `{}` is incompatible with requested pricing model `{model}`",
                swaption.vol_model
            ),
            context,
        ));
    }

    if let Some(pv) = swaption
        .terminal_value(market, as_of)
        .map_err(|error| PricingError::from_core(error, context.clone()))?
    {
        return Ok(ValuationResult::stamped(swaption.id(), as_of, pv));
    }

    let pv = if swaption.sabr_params.is_some() {
        swaption.price_sabr(market, as_of).map_err(|error| {
            PricingError::model_failure_with_context(error.to_string(), context.clone())
        })?
    } else {
        // Use Act/365F for the option time-to-expiry so the vol-surface
        // pillar lookup is on the same time axis used by both pricing kernels.
        let time_to_expiry = year_fraction(
            finstack_quant_core::dates::DayCount::Act365F,
            as_of,
            swaption.expiry,
        )
        .map_err(|error| {
            PricingError::model_failure_with_context(error.to_string(), context.clone())
        })?;
        let forward = swaption.forward_swap_rate(market, as_of).map_err(|error| {
            PricingError::model_failure_with_context(error.to_string(), context.clone())
        })?;
        let vol = swaption
            .resolve_volatility(market, forward, time_to_expiry)
            .map_err(|error| {
                PricingError::missing_market_data_with_context(error.to_string(), context.clone())
            })?;

        match expected_vol_model {
            VolatilityModel::Black => swaption.price_black(market, vol, as_of),
            VolatilityModel::Normal => swaption.price_normal(market, vol, as_of),
        }
        .map_err(|error| PricingError::model_failure_with_context(error.to_string(), context))?
    };

    Ok(ValuationResult::stamped(swaption.id(), as_of, pv))
}

#[cfg(test)]
mod tests {
    #[allow(dead_code, unused_imports)]
    mod date_support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/date.rs"
        ));
    }
    #[allow(dead_code, unused_imports)]
    mod discount_forward_curve_support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/discount_forward_curves.rs"
        ));
    }

    use super::*;
    use crate::instruments::{
        Instrument, InstrumentPricingOverrides, PricingOptions, ScenarioPricingOverrides,
    };
    use date_support::date;
    use discount_forward_curve_support::flat_discount_with_tenor;
    use finstack_quant_models::SabrParameters;

    #[test]
    fn swaption_default_matches_declared_volatility_model_for_pv_and_raw() {
        let as_of = date(2025, 1, 1);
        let mut black = Swaption::example().expect("example");
        black.instrument_pricing_overrides =
            InstrumentPricingOverrides::default().with_implied_volatility(0.20);
        let mut normal = black.clone();
        normal.vol_model = VolatilityModel::Normal;
        let mut sabr = black.clone();
        sabr.sabr_params = Some(SabrParameters {
            alpha: 0.20,
            beta: 0.5,
            rho: -0.3,
            nu: 0.4,
            shift: None,
        });
        let market = MarketContext::new().insert(flat_discount_with_tenor(
            black.get_discount_curve_id().as_str(),
            as_of,
            0.03,
            10.0,
        ));
        let registry = crate::pricer::standard_pricer_registry();

        for (swaption, expected_model) in [
            (&black, ModelKey::Black76),
            (&normal, ModelKey::Normal),
            (&sabr, ModelKey::Black76),
        ] {
            let default = swaption
                .price_with_metrics(&market, as_of, &[], PricingOptions::default())
                .expect("default swaption price");
            let explicit = registry
                .price_with_metrics(
                    swaption,
                    expected_model,
                    &market,
                    as_of,
                    &[],
                    PricingOptions::default(),
                )
                .expect("explicit swaption price");
            let default_raw = swaption
                .value_raw(&market, as_of)
                .expect("default swaption raw price");
            let explicit_raw = registry
                .price_raw(swaption, expected_model, &market, as_of)
                .expect("explicit swaption raw price");

            assert_eq!(swaption.default_model(), expected_model);
            assert_eq!(default.value, explicit.value);
            assert_eq!(default_raw, explicit_raw);
        }
    }

    #[test]
    fn black76_registry_applies_swaption_scenario_once_for_pv_and_raw() {
        let as_of = date(2025, 1, 1);
        let mut baseline = Swaption::example().expect("example");
        baseline.instrument_pricing_overrides =
            InstrumentPricingOverrides::default().with_implied_volatility(0.20);
        let market = MarketContext::new().insert(flat_discount_with_tenor(
            baseline.get_discount_curve_id().as_str(),
            as_of,
            0.03,
            10.0,
        ));
        let registry = crate::pricer::standard_pricer_registry();

        let baseline_result = baseline
            .price_with_metrics(
                &market,
                as_of,
                &[],
                PricingOptions::default().with_model(ModelKey::Black76),
            )
            .expect("baseline swaption registry price");
        let baseline_raw = registry
            .price_raw(&baseline, ModelKey::Black76, &market, as_of)
            .expect("baseline swaption registry raw price");

        let mut shocked = baseline;
        shocked.scenario_pricing_overrides =
            ScenarioPricingOverrides::default().with_scenario_price_shock_decimal(-0.10);
        let shocked_result = shocked
            .price_with_metrics(
                &market,
                as_of,
                &[],
                PricingOptions::default().with_model(ModelKey::Black76),
            )
            .expect("shocked swaption registry price");
        let shocked_raw = registry
            .price_raw(&shocked, ModelKey::Black76, &market, as_of)
            .expect("shocked swaption registry raw price");

        let expected_pv = baseline_result.value.amount() * 0.90;
        let expected_raw = baseline_raw * 0.90;
        assert!((shocked_result.value.amount() - expected_pv).abs() < 1e-8);
        assert!((shocked_raw - expected_raw).abs() < 1e-8);
    }
}
