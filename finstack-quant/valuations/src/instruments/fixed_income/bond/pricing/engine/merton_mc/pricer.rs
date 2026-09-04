use super::calibration::calibrate_parameter_to_market;
use super::MertonMcConfig;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::bond::pricing::time_basis::{
    bond_cashflow_dfs_on_model_grid, bond_model_maturity_years,
    implied_flat_discount_rate_from_curve,
};
use crate::instruments::fixed_income::bond::types::Bond;
use crate::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use indexmap::IndexMap;

// Pricer Registry Adapter

/// Model maturity and flat discount rate for Merton MC, on consistent time bases.
struct MertonMcTimeBasis {
    mat_years: f64,
    discount_rate: f64,
}

fn merton_mc_time_basis(
    bond: &Bond,
    disc: &DiscountCurve,
    as_of: finstack_quant_core::dates::Date,
    ctx: &PricingErrorContext,
) -> std::result::Result<MertonMcTimeBasis, PricingError> {
    let mat_years = bond_model_maturity_years(bond, as_of)
        .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;
    let discount_rate =
        implied_flat_discount_rate_from_curve(disc, as_of, bond.maturity, mat_years)
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;
    Ok(MertonMcTimeBasis {
        mat_years,
        discount_rate,
    })
}

fn populate_cashflow_dfs_if_needed(
    config: &mut MertonMcConfig,
    disc: &DiscountCurve,
    bond: &Bond,
    as_of: finstack_quant_core::dates::Date,
    mat_years: f64,
    ctx: &PricingErrorContext,
) -> std::result::Result<(), PricingError> {
    if config.cashflow_dfs.is_none() && mat_years > 0.0 {
        let dfs = bond_cashflow_dfs_on_model_grid(
            disc,
            as_of,
            bond.maturity,
            mat_years,
            config.time_steps_per_year,
        )
        .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;
        config.cashflow_dfs = Some(dfs);
    }
    Ok(())
}

/// Merton structural Monte Carlo pricer for (PIK) bonds.
///
/// Registered under `PricerKey::new(InstrumentType::Bond, ModelKey::MertonMc)`;
/// see the module-level docs for configuration details and metric outputs.
pub(crate) struct SimpleBondMertonMcPricer;

impl Pricer for SimpleBondMertonMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Bond, ModelKey::MertonMc)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let bond = expect_inst::<Bond>(instrument, InstrumentType::Bond)?;

        let ctx = PricingErrorContext::new()
            .instrument_id(bond.id())
            .instrument_type(InstrumentType::Bond)
            .model(ModelKey::MertonMc)
            .curve_id(bond.discount_curve_id.as_str());

        bond.validate_merton_mc_embedded_rights()
            .map_err(|error| PricingError::from_core(error, ctx.clone()))?;

        let mc_override = bond
            .instrument_pricing_overrides
            .model_config
            .merton_mc_config
            .as_ref()
            .ok_or_else(|| {
                PricingError::invalid_input_with_context(
                    "MertonMc pricer requires merton_mc_config on pricing_overrides",
                    ctx.clone(),
                )
            })?;

        let disc = market
            .get_discount(bond.discount_curve_id.as_str())
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;

        let MertonMcTimeBasis {
            mat_years,
            discount_rate,
        } = merton_mc_time_basis(bond, &disc, as_of, &ctx)?;

        // ---- Calibration pass (opt-in) ---------------------------------
        let mut calibration_measures: IndexMap<crate::metrics::MetricId, f64> = IndexMap::new();
        let mut effective_config = if let Some(ref cal_spec) = mc_override.0.calibration {
            let cal_output = calibrate_parameter_to_market(
                bond,
                market,
                as_of,
                discount_rate,
                &mc_override.0,
                cal_spec,
            )
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;

            calibration_measures.insert(
                crate::metrics::MetricId::custom("calibrated_debt_barrier"),
                cal_output.calibrated_merton.debt_barrier(),
            );
            calibration_measures.insert(
                crate::metrics::MetricId::custom("calibrated_asset_vol"),
                cal_output.calibrated_merton.asset_vol(),
            );
            calibration_measures.insert(
                crate::metrics::MetricId::custom("calibration_residual_pv"),
                cal_output.residual_pv,
            );
            calibration_measures.insert(
                crate::metrics::MetricId::custom("calibration_iterations"),
                cal_output.iterations as f64,
            );
            calibration_measures.insert(
                crate::metrics::MetricId::custom("calibration_target_pv"),
                cal_output.target_pv,
            );
            calibration_measures.insert(
                crate::metrics::MetricId::custom("calibration_solved_parameter"),
                cal_output.solved_parameter,
            );

            let mut cfg = mc_override.0.clone();
            cfg.merton = cal_output.calibrated_merton;
            cfg.calibration = None;
            cfg
        } else {
            mc_override.0.clone()
        };

        // Build term-structure discount factors from the curve for cashflow
        // discounting. The flat `discount_rate` is still used for the Merton
        // risk-neutral drift.
        populate_cashflow_dfs_if_needed(
            &mut effective_config,
            &disc,
            bond,
            as_of,
            mat_years,
            &ctx,
        )?;

        // ---- Full pricing pass -----------------------------------------
        let mc_result = bond
            .price_merton_mc(&effective_config, discount_rate, as_of)
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx))?;

        let mc_clean_pct = mc_result.clean_price_pct;
        let pv_amount = mc_clean_pct / 100.0 * bond.notional.amount();
        let pv = Money::new(pv_amount, bond.notional.currency());

        let mut measures = IndexMap::new();
        measures.insert(
            crate::metrics::MetricId::custom("expected_loss"),
            mc_result.expected_loss,
        );
        measures.insert(
            crate::metrics::MetricId::custom("default_rate"),
            mc_result.path_statistics.default_rate,
        );
        measures.insert(
            crate::metrics::MetricId::custom("avg_terminal_notional"),
            mc_result.path_statistics.avg_terminal_notional,
        );
        measures.insert(
            crate::metrics::MetricId::custom("pik_fraction"),
            mc_result.average_pik_fraction,
        );
        measures.insert(
            crate::metrics::MetricId::custom("mc_stderr"),
            mc_result.standard_error,
        );
        measures.insert(
            crate::metrics::MetricId::custom("unexpected_loss"),
            mc_result.unexpected_loss,
        );
        measures.insert(
            crate::metrics::MetricId::custom("expected_shortfall_95"),
            mc_result.expected_shortfall_95,
        );

        for (k, v) in calibration_measures {
            measures.insert(k, v);
        }

        let result = ValuationResult::stamped(bond.id(), as_of, pv);
        Ok(result.with_measures(measures))
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<f64, PricingError> {
        let bond = expect_inst::<Bond>(instrument, InstrumentType::Bond)?;

        let ctx = PricingErrorContext::new()
            .instrument_id(bond.id())
            .instrument_type(InstrumentType::Bond)
            .model(ModelKey::MertonMc)
            .curve_id(bond.discount_curve_id.as_str());

        bond.validate_merton_mc_embedded_rights()
            .map_err(|error| PricingError::from_core(error, ctx.clone()))?;

        let mc_override = bond
            .instrument_pricing_overrides
            .model_config
            .merton_mc_config
            .as_ref()
            .ok_or_else(|| {
                PricingError::invalid_input_with_context(
                    "MertonMc pricer requires merton_mc_config on pricing_overrides",
                    ctx.clone(),
                )
            })?;

        let disc = market
            .get_discount(bond.discount_curve_id.as_str())
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;

        let MertonMcTimeBasis {
            mat_years,
            discount_rate,
        } = merton_mc_time_basis(bond, &disc, as_of, &ctx)?;

        let mut effective_config = if let Some(ref cal_spec) = mc_override.0.calibration {
            let cal_output = calibrate_parameter_to_market(
                bond,
                market,
                as_of,
                discount_rate,
                &mc_override.0,
                cal_spec,
            )
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;

            let mut cfg = mc_override.0.clone();
            cfg.merton = cal_output.calibrated_merton;
            cfg.calibration = None;
            cfg
        } else {
            mc_override.0.clone()
        };

        populate_cashflow_dfs_if_needed(
            &mut effective_config,
            &disc,
            bond,
            as_of,
            mat_years,
            &ctx,
        )?;

        let mc_result = bond
            .price_merton_mc(&effective_config, discount_rate, as_of)
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx))?;

        Ok(mc_result.clean_price_pct / 100.0 * bond.notional.amount())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::bond::{CallPut, CallPutSchedule, ReturnFloorSpec};
    use crate::instruments::PricingOptions;
    use crate::pricer::{standard_pricer_registry, PricingError};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::StubKind;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::types::Rate;
    use finstack_quant_models::credit::MertonModel;
    use time::macros::date;

    fn test_bond_and_config() -> (Bond, MertonMcConfig) {
        let config = MertonMcConfig::new(
            MertonModel::new(200.0, 0.20, 100.0, 0.03).expect("valid Merton model"),
            0.40,
        )
        .expect("valid Merton MC config")
        .num_paths(32)
        .time_steps_per_year(4)
        .antithetic(false)
        .seed(7);
        let mut bond = Bond::fixed(
            "MERTON_OPTION_GUARD",
            Money::new(100.0, Currency::USD),
            Rate::from_decimal(0.05),
            date!(2024 - 01 - 15),
            date!(2029 - 01 - 15),
            StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("valid bond");
        bond.instrument_pricing_overrides = bond
            .instrument_pricing_overrides
            .clone()
            .with_merton_mc(config.clone());
        (bond, config)
    }

    fn test_market() -> MarketContext {
        MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(date!(2025 - 01 - 15))
                .knots([(0.0, 1.0), (10.0, (-0.03_f64 * 10.0).exp())])
                .build()
                .expect("valid discount curve"),
        )
    }

    fn assert_option_guidance(message: &str) {
        assert!(message.contains("does not support embedded call, put, or return-floor rights"));
        assert!(message.contains("'tree'"));
        assert!(message.contains("'rates_credit'"));
    }

    #[test]
    fn public_and_registered_merton_paths_reject_embedded_rights() {
        let as_of = date!(2025 - 01 - 15);
        let market = test_market();
        let (straight, config) = test_bond_and_config();

        let direct = straight
            .price_merton_mc(&config, 0.03, as_of)
            .expect("straight Merton pricing remains supported");
        assert!(direct.clean_price_pct.is_finite());
        let registered_straight = standard_pricer_registry()
            .price_with_metrics(
                &straight,
                ModelKey::MertonMc,
                &market,
                as_of,
                &[],
                PricingOptions::default(),
            )
            .expect("registered straight Merton pricing remains supported");
        assert!(registered_straight.value.amount().is_finite());

        let mut callable = straight.clone();
        callable.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: date!(2027 - 01 - 15),
                end_date: date!(2027 - 01 - 15),
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        let direct_error = callable
            .price_merton_mc(&config, 0.03, as_of)
            .expect_err("typed Merton pricing must reject callable bonds");
        assert_option_guidance(&direct_error.to_string());

        let money_error = standard_pricer_registry()
            .price_with_metrics(
                &callable,
                ModelKey::MertonMc,
                &market,
                as_of,
                &[],
                PricingOptions::default(),
            )
            .expect_err("registered Merton pricing must reject callable bonds");
        assert!(matches!(money_error, PricingError::InvalidInput { .. }));
        assert_option_guidance(&money_error.to_string());

        let mut puttable = straight.clone();
        puttable.call_put = Some(CallPutSchedule {
            calls: Vec::new(),
            puts: vec![CallPut {
                start_date: date!(2027 - 01 - 15),
                end_date: date!(2027 - 01 - 15),
                price_pct_of_par: 110.0,
                make_whole: None,
            }],
        });
        let put_error = puttable
            .price_merton_mc(&config, 0.03, as_of)
            .expect_err("typed Merton pricing must reject puttable bonds");
        assert_option_guidance(&put_error.to_string());

        let mut floored = straight;
        floored.return_floor = Some(ReturnFloorSpec::moic(1.10));
        let floor_error = floored
            .price_merton_mc(&config, 0.03, as_of)
            .expect_err("typed Merton pricing must reject return floors");
        assert_option_guidance(&floor_error.to_string());

        let raw_error = standard_pricer_registry()
            .price_raw(&floored, ModelKey::MertonMc, &market, as_of)
            .expect_err("registered raw Merton pricing must reject return floors");
        assert!(matches!(raw_error, PricingError::InvalidInput { .. }));
        assert_option_guidance(&raw_error.to_string());
    }
}
