//! Autocallable Monte Carlo pricer.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::autocallable::monte_carlo::AutocallablePayoff;
use crate::instruments::equity::autocallable::types::Autocallable;
use crate::instruments::equity::piecewise_gbm::{bootstrap_forward_gbm, PiecewiseExactGbm};
use crate::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_models::monte_carlo::engine::{McEngine, McEngineConfig};
use finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricerConfig;
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_models::monte_carlo::TimeGrid;

/// Autocallable Monte Carlo pricer.
pub struct AutocallableMcPricer {
    config: PathDependentPricerConfig,
}

impl AutocallableMcPricer {
    /// Create an autocallable MC pricer with antithetic variance reduction.
    pub fn new() -> Self {
        Self {
            config: PathDependentPricerConfig::default().with_antithetic(true),
        }
    }

    /// Price an autocallable using Monte Carlo.
    fn price_internal(
        &self,
        inst: &Autocallable,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<(
        finstack_quant_core::money::Money,
        Option<finstack_quant_models::monte_carlo::results::MoneyEstimate>,
    )> {
        inst.validate()?;
        if as_of > inst.expiry {
            return Ok((Money::from((0_i64, inst.notional.currency())), None));
        }
        let initial_spot = crate::instruments::common_impl::helpers::scalar_price_amount(
            curves.get_price(&inst.spot_id)?,
            inst.notional.currency(),
        )?;

        let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;

        // Use explicit expiry as the contractual settlement/maturity date.
        //
        // Two-clock convention (INVARIANTS.md §4): model/vol time is measured on
        // the instrument's day count (the vol-surface calibration basis,
        // typically Act/365F for equity), while discounting uses exact
        // date-based factors on the curve's own basis.
        let final_date = inst.expiry;
        let model_day_count = inst.day_count;
        let t = model_day_count.year_fraction(as_of, final_date, DayCountContext::default())?;
        let discount_factor = disc_curve.df_between_dates(as_of, final_date)?;

        // Reference (strike-set) level S_0 for barrier and payoff ratios.
        // For a new trade this is the spot at as_of; seasoned trades must
        // carry the observed initial fixing.
        let s0 = inst.initial_level.unwrap_or(initial_spot);

        // Deterministic evaluation of past observation dates (seasoned trade).
        //
        // Every observation date on or before as_of must have an observed
        // fixing: evaluating past dates against simulated spot would both
        // randomize already-known outcomes and future-value past cashflows
        // (df_ratio > 1).
        let n_past = inst
            .observation_dates
            .iter()
            .take_while(|&&d| d <= as_of)
            .count();
        if n_past > 0 && inst.initial_level.is_none() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Autocallable '{}' has {} past observation dates but no initial_level; \
                 the strike-set level is required to evaluate past barriers",
                inst.id, n_past
            )));
        }
        let mut past_min_spot = f64::INFINITY;
        let mut past_max_spot = f64::NEG_INFINITY;
        let mut prior_memory_coupons = 0.0;
        let mut fixed_coupon_expiry_ratio = 0.0;
        for idx in 0..n_past {
            let obs_date = inst.observation_dates[idx];
            let payment_date = inst.payment_dates[idx];
            let fix = inst.fixing_on(obs_date).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "Autocallable '{}': observation date {} is on or before as_of {} but has \
                     no entry in past_fixings; provide the observed fixing to price this \
                     seasoned trade",
                    inst.id, obs_date, as_of
                ))
            })?;
            past_min_spot = past_min_spot.min(fix);
            past_max_spot = past_max_spot.max(fix);

            if fix >= s0 * inst.coupon_barriers[idx] {
                let coupon = inst.coupons[idx]
                    + if inst.memory_coupons {
                        prior_memory_coupons
                    } else {
                        0.0
                    };
                if payment_date > as_of {
                    let payment_df = disc_curve.df_between_dates(as_of, payment_date)?;
                    fixed_coupon_expiry_ratio += coupon * payment_df / discount_factor;
                }
                prior_memory_coupons = 0.0;
            } else if inst.memory_coupons {
                prior_memory_coupons += inst.coupons[idx];
            }

            if fix >= s0 * inst.autocall_barriers[idx] {
                if payment_date <= as_of {
                    return Ok((Money::from((0_i64, inst.notional.currency())), None));
                }
                let payment_df = disc_curve.df_between_dates(as_of, payment_date)?;
                let pv = inst.notional.amount()
                    * (payment_df + fixed_coupon_expiry_ratio * discount_factor);
                return Ok((Money::new(pv, inst.notional.currency())?, None));
            }
        }

        if n_past == inst.observation_dates.len() {
            let last_obs = inst.observation_dates[n_past - 1];
            let final_fixing = inst.fixing_on(last_obs).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "Autocallable '{}': missing fixing for final observation date {}",
                    inst.id, last_obs
                ))
            })?;
            let payoff = AutocallablePayoff::new(
                vec![],
                vec![],
                vec![],
                vec![],
                inst.memory_coupons,
                inst.final_barrier,
                inst.final_payoff_type,
                inst.participation_rate,
                inst.cap_level,
                inst.notional.amount(),
                s0,
                vec![],
            )?;
            let redemption_ratio = payoff.final_payoff_ratio(final_fixing, past_min_spot);
            let pv = (redemption_ratio + fixed_coupon_expiry_ratio)
                * inst.notional.amount()
                * discount_factor;
            return Ok((Money::new(pv, inst.notional.currency())?, None));
        }

        let future_dates = &inst.observation_dates[n_past..];
        let future_payment_dates = &inst.payment_dates[n_past..];
        let future_barriers = inst.autocall_barriers[n_past..].to_vec();
        let future_coupon_barriers = inst.coupon_barriers[n_past..].to_vec();
        let future_coupons = inst.coupons[n_past..].to_vec();

        // Dividend yield from scalar id if provided
        //
        // When a dividend yield ID is explicitly provided, we require the lookup to succeed
        // and return a unitless scalar. Silent fallback to 0.0 would mask market data
        // configuration errors.
        let q = if let Some(div_id) = &inst.div_yield_id {
            let ms = curves.get_price(div_id.as_str()).map_err(|e| {
                finstack_quant_core::Error::Validation(format!(
                    "Failed to fetch dividend yield '{}': {}",
                    div_id, e
                ))
            })?;
            match ms {
                finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Dividend yield '{}' should be a unitless scalar, got Price({})",
                        div_id,
                        m.currency()
                    )));
                }
            }
        } else {
            0.0
        };

        // Map remaining (future) observation dates to model times on the
        // instrument clock (matches the vol-surface calibration basis).
        let observation_times: Vec<f64> = future_dates
            .iter()
            .map(|&date| model_day_count.year_fraction(as_of, date, DayCountContext::default()))
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;

        // Bootstrap a piecewise-constant forward GBM over the autocall observation
        // schedule (plus the final maturity) so the simulation carries the term
        // structure of volatility and rates. A single flat-vol GBM misprices the
        // knock-in put and the path-dependent autocall probabilities when the
        // surface/curve is not flat; for a flat surface this reduces exactly to the
        // previous constant-GBM process.
        //
        // Check points carry (model_time, date) pairs: the vol surface is
        // sampled on the instrument clock while discount factors come from
        // exact curve dates.
        let mut check_points: Vec<(f64, Date)> = observation_times
            .iter()
            .copied()
            .zip(future_dates.iter().copied())
            .filter(|&(ct, _)| ct > 0.0)
            .collect();
        check_points.push((t, final_date));
        check_points.sort_by(|a, b| a.0.total_cmp(&b.0));
        check_points.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-10);
        let process = bootstrap_forward_gbm(
            disc_curve.as_ref(),
            curves,
            &inst.instrument_pricing_overrides.market_quotes,
            inst.vol_surface_id.as_str(),
            as_of,
            initial_spot,
            q,
            &check_points,
            &format!("Autocallable {}", inst.id),
        )?;

        // Convert each observation's contractual payment to expiry-value units
        // because the engine applies `DF(as_of, expiry)` once to the path payoff.
        let payment_df_ratios: Vec<f64> = future_payment_dates
            .iter()
            .map(|&payment_date| {
                let payment_df = disc_curve.df_between_dates(as_of, payment_date)?;
                Ok(payment_df / discount_factor)
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;

        let payoff = AutocallablePayoff::new(
            observation_times.clone(),
            future_barriers,
            future_coupon_barriers,
            future_coupons,
            inst.memory_coupons,
            inst.final_barrier,
            inst.final_payoff_type,
            inst.participation_rate,
            inst.cap_level,
            inst.notional.amount(),
            s0,
            payment_df_ratios,
        )?
        .with_seasoned_state(
            past_min_spot,
            past_max_spot,
            prior_memory_coupons,
            fixed_coupon_expiry_ratio,
        );

        // Derive deterministic seed from instrument ID and scenario.
        use finstack_quant_models::monte_carlo::seed;
        let seed = if let Some(ref scenario) = inst.metric_pricing_overrides.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };

        let merged_cfg = crate::instruments::common_impl::helpers::merged_path_config(
            &self.config,
            &inst.instrument_pricing_overrides,
        )?;

        // Identical grid to the previous constant-GBM path (uniform steps plus the
        // observation dates as required times), so a flat surface reproduces the
        // prior prices bit-for-bit.
        let time_grid = TimeGrid::uniform_with_required_times(
            t,
            merged_cfg.steps_per_year,
            merged_cfg.min_steps,
            &observation_times,
        )?;

        // Mirror `PathDependentPricer::price_with_grid` (non-Sobol path) but drive
        // the piecewise process with its exact discretization.
        let engine_config = McEngineConfig {
            num_paths: merged_cfg.num_paths,
            time_grid,
            target_ci_half_width: inst
                .instrument_pricing_overrides
                .model_config
                .mc_target_ci_half_width,
            use_parallel: merged_cfg.use_parallel,
            chunk_size: Some(merged_cfg.chunk_size),
            path_capture: merged_cfg.path_capture.clone(),
            antithetic: merged_cfg.antithetic,
        };
        let engine = McEngine::new(engine_config);
        let rng = PhiloxRng::new(seed);
        let disc = PiecewiseExactGbm::new();
        let initial_state = vec![initial_spot];

        let result = engine.price(
            &rng,
            &process,
            &disc,
            &initial_state,
            &payoff,
            inst.notional.currency(),
            discount_factor,
        )?;

        let mean = result.mean;
        Ok((mean, Some(result)))
    }
}

impl Default for AutocallableMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for AutocallableMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Autocallable, ModelKey::MonteCarloGBM)
    }

    #[tracing::instrument(
        name = "autocallable.mc.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn crate::instruments::common_impl::traits::Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let autocallable = expect_inst::<Autocallable>(instrument, InstrumentType::Autocallable)?;

        let (pv, estimate) = self
            .price_internal(autocallable, market, as_of)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::from_instrument(autocallable)
                        .model(ModelKey::MonteCarloGBM),
                )
            })?;

        let mut result = ValuationResult::stamped(autocallable.id(), as_of, pv);
        if let Some(estimate) = estimate {
            crate::instruments::common_impl::helpers::attach_mc_diagnostics(&mut result, &estimate);
        }
        Ok(result)
    }
}

/// Present value using Monte Carlo.
pub(crate) fn compute_pv(
    inst: &Autocallable,
    curves: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    let pricer = AutocallableMcPricer::new();
    Ok(pricer.price_internal(inst, curves, as_of)?.0)
}
