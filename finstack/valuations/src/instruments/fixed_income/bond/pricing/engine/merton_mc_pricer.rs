//! Merton Monte Carlo structural credit pricer for PIK bonds.
//!
//! Prices a bond using the Merton structural credit MC engine. The
//! [`MertonMcConfig`] must be set on the bond's `pricing_overrides.model_config`.
//!
//! Returns PV plus MC-specific structural measures (expected loss, default rate,
//! etc.). Standard spread/yield metrics (z-spread, YTM, durations) are available
//! via `PricerRegistry::price_with_metrics`, which runs the generic metrics
//! pipeline against the MC model price.
//!
//! When the optional `calibration` field is set on the config, the pricer first
//! calibrates a structural parameter (barrier or asset vol) to a market quote
//! using low-path MC with common random numbers, then re-prices with the
//! calibrated model at full path count.

use super::merton_mc::MertonMcConfig;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::bond::pricing::time_basis::{
    bond_cashflow_dfs_on_model_grid, bond_model_maturity_years,
    implied_flat_discount_rate_from_curve,
};
use crate::instruments::fixed_income::bond::types::Bond;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use indexmap::IndexMap;

/// Model maturity and flat discount rate for Merton MC, on consistent time bases.
struct MertonMcTimeBasis {
    mat_years: f64,
    discount_rate: f64,
}

fn merton_mc_time_basis(
    bond: &Bond,
    disc: &DiscountCurve,
    as_of: finstack_core::dates::Date,
    ctx: &PricingErrorContext,
) -> Result<MertonMcTimeBasis, PricingError> {
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
    as_of: finstack_core::dates::Date,
    mat_years: f64,
    ctx: &PricingErrorContext,
) -> Result<(), PricingError> {
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
        as_of: finstack_core::dates::Date,
    ) -> Result<ValuationResult, PricingError> {
        use finstack_core::money::Money;

        let bond = instrument
            .as_any()
            .downcast_ref::<Bond>()
            .ok_or_else(|| PricingError::type_mismatch(InstrumentType::Bond, instrument.key()))?;

        let ctx = PricingErrorContext::new()
            .instrument_id(bond.id())
            .instrument_type(InstrumentType::Bond)
            .model(ModelKey::MertonMc)
            .curve_id(bond.discount_curve_id.as_str());

        let mc_override = bond
            .pricing_overrides
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
            use super::merton_mc::calibration::calibrate_parameter_to_market;
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
        as_of: finstack_core::dates::Date,
    ) -> Result<f64, PricingError> {
        let bond = instrument
            .as_any()
            .downcast_ref::<Bond>()
            .ok_or_else(|| PricingError::type_mismatch(InstrumentType::Bond, instrument.key()))?;

        let ctx = PricingErrorContext::new()
            .instrument_id(bond.id())
            .instrument_type(InstrumentType::Bond)
            .model(ModelKey::MertonMc)
            .curve_id(bond.discount_curve_id.as_str());

        let mc_override = bond
            .pricing_overrides
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
            use super::merton_mc::calibration::calibrate_parameter_to_market;
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
