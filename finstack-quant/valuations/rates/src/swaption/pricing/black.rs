//! Vanilla swaption pricer implementation.

use crate::VolatilityModel;
use crate::swaption::Swaption;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use finstack_quant_valuations_core::results::ValuationResult;

/// European swaption pricer using the Black-76 formula.
pub struct SimpleSwaptionBlackPricer;

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
        let swaption = finstack_quant_valuations_core::pricer::expect_inst::<Swaption>(
            instrument,
            InstrumentType::Swaption,
        )?;

        price_swaption(
            swaption,
            market,
            as_of,
            ModelKey::Black76,
            VolatilityModel::Black,
        )
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<f64, PricingError> {
        let swaption = finstack_quant_valuations_core::pricer::expect_inst::<Swaption>(
            instrument,
            InstrumentType::Swaption,
        )?;

        price_swaption_raw(
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
        let swaption = finstack_quant_valuations_core::pricer::expect_inst::<Swaption>(
            instrument,
            InstrumentType::Swaption,
        )?;

        price_swaption(
            swaption,
            market,
            as_of,
            ModelKey::Normal,
            VolatilityModel::Normal,
        )
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<f64, PricingError> {
        let swaption = finstack_quant_valuations_core::pricer::expect_inst::<Swaption>(
            instrument,
            InstrumentType::Swaption,
        )?;

        price_swaption_raw(
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
    let pv_raw = price_swaption_raw(swaption, market, as_of, model, expected_vol_model)?;
    let pv =
        finstack_quant_core::money::Money::from_model_value(pv_raw, swaption.notional.currency())
            .map_err(|error| PricingError::from_core(error, context))?;
    Ok(ValuationResult::stamped(swaption.id(), as_of, pv))
}

fn price_swaption_raw(
    swaption: &Swaption,
    market: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
    model: ModelKey,
    expected_vol_model: VolatilityModel,
) -> std::result::Result<f64, PricingError> {
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
        .terminal_value_raw(market, as_of)
        .map_err(|error| PricingError::from_core(error, context.clone()))?
    {
        return Ok(pv);
    }

    let pv = if swaption.sabr_params.is_some() {
        swaption
            .price_sabr_before_expiry_raw(market, as_of)
            .map_err(|error| {
                PricingError::model_failure_with_context(error.to_string(), context.clone())
            })?
    } else {
        let forward = swaption.forward_swap_rate(market, as_of).map_err(|error| {
            PricingError::model_failure_with_context(error.to_string(), context.clone())
        })?;
        let (quote, time_to_expiry) = swaption
            .resolve_volatility_quote_on_date(market, forward, as_of)
            .map_err(|error| PricingError::from_core(error, context.clone()))?;

        swaption
            .price_quote_at_raw(market, quote, as_of, forward, time_to_expiry)
            .map_err(|error| PricingError::model_failure_with_context(error.to_string(), context))?
    };

    Ok(pv)
}

#[cfg(test)]
mod tests {
    use finstack_quant_valuations_test_support::date as date_support;
    use finstack_quant_valuations_test_support::discount_forward_curves as discount_forward_curve_support;

    use super::*;
    use date_support::date;
    use discount_forward_curve_support::flat_discount_with_tenor;
    use finstack_quant_models::volatility::sabr::SabrParameters;
    use finstack_quant_valuations_core::instruments::pricing_overrides::{
        InstrumentPricingOverrides, ScenarioPricingOverrides,
    };
    use finstack_quant_valuations_core::instruments::{Instrument, PricingOptions};

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
        let market = MarketContext::new()
            .try_insert(flat_discount_with_tenor(
                black.get_discount_curve_id().as_str(),
                as_of,
                0.03,
                10.0,
            ))
            .expect("insert market curve");
        let registry = crate::RATES.pricers();

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
    fn dated_swaption_requires_cube_clock_agreement() {
        use finstack_quant_core::dates::{DayCount, DayCountContextState, VolatilityClock};
        use finstack_quant_core::market_data::surfaces::{SabrParameterData, VolCube};
        use finstack_quant_models::volatility::VolatilityConvention;

        let as_of = date(2025, 1, 1);
        let swaption = Swaption::example().expect("swaption");
        let discount =
            flat_discount_with_tenor(swaption.get_discount_curve_id().as_str(), as_of, 0.03, 10.0);
        let market = MarketContext::new()
            .try_insert(discount)
            .expect("insert discount curve");
        let node = SabrParameterData::new(0.2, 1.0, 0.0, -0.25, Some(0.03)).expect("SABR node");
        let tail = SabrParameterData::new(0.4, 1.0, 0.0, -0.25, Some(0.03)).expect("SABR node");
        let cube = VolCube::from_grid(
            swaption.vol_surface_id.as_str(),
            &[1.0, 4.0],
            &[1.0, 10.0],
            &[node, node, tail, tail],
            &[0.03; 4],
        )
        .expect("cube");
        let missing = market.clone().insert_vol_cube(cube.clone());
        assert!(
            swaption
                .resolve_volatility(&missing, 0.03, 2.0)
                .expect("unqualified numeric quote")
                > 0.0
        );

        let clock = |reference_date, day_count| {
            VolatilityClock::try_new(reference_date, day_count, DayCountContextState::default())
                .expect("volatility clock")
        };
        let registry = crate::RATES.pricers();
        let matching = market.clone().insert_vol_cube(
            cube.clone()
                .with_expiry_clock(clock(as_of, DayCount::Act365F)),
        );
        let priced = swaption.value_raw(&matching, as_of).expect("dated price");
        assert!(priced > 0.0);
        assert_eq!(
            priced,
            registry
                .price_raw(&swaption, ModelKey::Black76, &matching, as_of)
                .expect("registry price")
        );
        assert_eq!(
            swaption
                .greek_inputs(&matching, as_of)
                .expect("Greek inputs")
                .expect("pre-expiry inputs")
                .convention,
            VolatilityConvention::ShiftedLognormal { shift: 0.03 }
        );

        let horizon = date(2025, 7, 1);
        let rolled_market = matching.condition(horizon).expect("conditioned rates");
        assert!(
            swaption.value_raw(&rolled_market, horizon).is_err(),
            "an ordinary horizon price must still qualify the source clock"
        );
        let held = swaption
            .theta_observed_state(&matching, as_of, horizon)
            .expect("theta state")
            .expect("frozen cube quote");
        let held = held
            .as_any()
            .downcast_ref::<Swaption>()
            .expect("held swaption");
        let (opening_quote, opening_time) = swaption
            .resolve_volatility_quote_on_date(&matching, 0.03, as_of)
            .expect("opening quote");
        let (held_quote, held_time) = held
            .resolve_volatility_quote_on_date(&rolled_market, 0.03, horizon)
            .expect("held quote");
        assert_eq!(held_quote.sigma, opening_quote.sigma);
        assert_eq!(held_quote.convention, opening_quote.convention);
        assert!(held_time < opening_time);

        let serialized = serde_json::to_value(held).expect("serialized held swaption");
        assert!(serialized.get("theta_cube_source_date").is_none());
        let restored: Swaption = serde_json::from_value(serialized).expect("restored swaption");
        assert!(
            restored.value_raw(&rolled_market, horizon).is_err(),
            "restored trades must pass ordinary source-clock admission"
        );

        let stale_clock = clock(date(2024, 12, 31), DayCount::Act365F);
        let wrong_basis = clock(as_of, DayCount::Act360);
        for rejected in [
            missing,
            market
                .clone()
                .insert_vol_cube(cube.clone().with_expiry_clock(stale_clock)),
            market.insert_vol_cube(cube.with_expiry_clock(wrong_basis)),
        ] {
            let error = swaption
                .value_raw(&rejected, as_of)
                .expect_err("incompatible expiry clock");
            assert!(error.to_string().contains("expiry_clock"), "{error}");
            assert!(swaption.greek_inputs(&rejected, as_of).is_err());
            assert!(
                registry
                    .price_raw(&swaption, ModelKey::Black76, &rejected, as_of)
                    .expect_err("registry must reject the expiry clock")
                    .to_string()
                    .contains("expiry_clock")
            );
            let mut scalar = swaption.clone();
            scalar.instrument_pricing_overrides =
                InstrumentPricingOverrides::default().with_implied_volatility(0.2);
            assert!(scalar.value_raw(&rejected, as_of).is_ok());
            let after_expiry = swaption.expiry.next_day().expect("next day");
            assert_eq!(
                swaption
                    .value_raw(&rejected, after_expiry)
                    .expect("expired"),
                0.0
            );
        }
    }

    #[test]
    fn black76_registry_applies_swaption_scenario_once_for_pv_and_raw() {
        let as_of = date(2025, 1, 1);
        let mut baseline = Swaption::example().expect("example");
        baseline.instrument_pricing_overrides =
            InstrumentPricingOverrides::default().with_implied_volatility(0.20);
        let market = MarketContext::new()
            .try_insert(flat_discount_with_tenor(
                baseline.get_discount_curve_id().as_str(),
                as_of,
                0.03,
                10.0,
            ))
            .expect("insert market curve");
        let registry = crate::RATES.pricers();

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
