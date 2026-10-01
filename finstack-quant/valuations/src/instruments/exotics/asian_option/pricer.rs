//! Asian option pricers (Monte Carlo and analytical).

// Common imports for all pricers
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::exotics::asian_option::types::AsianOption;
use crate::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

// MC-specific imports
use finstack_quant_models::monte_carlo::engine::PathCaptureConfig;
use finstack_quant_models::monte_carlo::payoff::asian::{AsianCall, AsianPut};
use finstack_quant_models::monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_quant_models::monte_carlo::process::gbm::{GbmParams, GbmProcess};
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::monte_carlo::variance_reduction::control_variate::apply_control_variate;

/// Exact simulation grid and one event index for every future fixing.
/// Repeated indices retain contractual observations sharing a model time.
pub(super) struct FixingGrid {
    pub(super) time_grid: finstack_quant_models::monte_carlo::TimeGrid,
    pub(super) fixing_steps: Vec<usize>,
}

impl FixingGrid {
    fn future_times(&self) -> Vec<f64> {
        self.fixing_steps
            .iter()
            .map(|&step| self.time_grid.times()[step])
            .collect()
    }
}

/// Insert contractual fixing times into the simulation grid, retaining their
/// multiplicity when the instrument's day count maps dates to the same time.
pub(super) fn map_fixings_to_steps(
    fixing_dates: &[Date],
    day_count: finstack_quant_core::dates::DayCount,
    as_of: Date,
    t: f64,
    base_num_steps: usize,
) -> finstack_quant_core::Result<FixingGrid> {
    let fixing_times = fixing_dates
        .iter()
        .filter(|&&date| date > as_of)
        .map(|&date| day_count.year_fraction(as_of, date, DayCountContext::default()))
        .collect::<finstack_quant_core::Result<Vec<_>>>()?;
    let time_grid = finstack_quant_models::monte_carlo::TimeGrid::uniform_with_required_times(
        t,
        base_num_steps.max(1) as f64 / t,
        base_num_steps.max(1),
        &fixing_times,
    )?;
    let fixing_steps = fixing_times
        .iter()
        .map(|&time| {
            time_grid
                .times()
                .iter()
                .position(|&node| (node - time).abs() < 1e-10)
                .ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "Asian fixing time {time} is outside the simulation grid"
                    ))
                })
        })
        .collect::<finstack_quant_core::Result<Vec<_>>>()?;
    Ok(FixingGrid {
        time_grid,
        fixing_steps,
    })
}

/// Cumulative asset carry at each contractual date, in the instrument's clock.
/// The same schedule drives the simulation and geometric log moments.
fn asian_drift_and_fixing_multipliers(
    asian: &AsianOption,
    market: &MarketContext,
    as_of: Date,
    q: f64,
) -> finstack_quant_core::Result<(
    finstack_quant_models::monte_carlo::process::gbm::DriftSchedule,
    Vec<f64>,
)> {
    let discount = market.get_discount(asian.discount_curve_id.as_str())?;
    let mut dates: Vec<Date> = asian
        .fixing_dates
        .iter()
        .copied()
        .filter(|&date| date > as_of)
        .collect();
    dates.push(asian.expiry);
    dates.sort_unstable();
    dates.dedup();
    let mut times = vec![0.0];
    let mut cumulative = vec![0.0];
    for date in dates {
        let time = asian
            .day_count
            .year_fraction(as_of, date, DayCountContext::default())?;
        let carry = -discount.df_between_dates(as_of, date)?.ln() - q * time;
        let last = times.len() - 1;
        if time.total_cmp(&times[last]) != std::cmp::Ordering::Equal {
            times.push(time);
            cumulative.push(carry);
        }
    }
    let drift =
        finstack_quant_models::monte_carlo::process::gbm::DriftSchedule::new(times, cumulative)?;
    let multipliers = asian
        .fixing_dates
        .iter()
        .filter(|&&date| date > as_of)
        .map(|&date| {
            let time = asian
                .day_count
                .year_fraction(as_of, date, DayCountContext::default())?;
            let carry = -discount.df_between_dates(as_of, date)?.ln() - q * time;
            Ok((carry - drift.cumulative(time)).exp())
        })
        .collect::<finstack_quant_core::Result<Vec<_>>>()?;
    Ok((drift, multipliers))
}

/// Compensated (Neumaier) sample mean of `xs`.
///
/// The control-variate machinery sums up to `num_paths` per-path discounted
/// payoffs; a naive `iter().sum()` loses low-order bits at large path counts.
/// Compensated summation keeps the mean accurate (W-05).
fn compensated_mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut acc = finstack_quant_core::math::NeumaierAccumulator::new();
    for &x in xs {
        acc.add(x);
    }
    acc.total() / xs.len() as f64
}

/// Compensated (Neumaier) unbiased sample variance of `xs`.
fn compensated_variance(xs: &[f64], mean: f64) -> f64 {
    let n = xs.len();
    if n < 2 {
        return 0.0;
    }
    let mut acc = finstack_quant_core::math::NeumaierAccumulator::new();
    for &x in xs {
        let d = x - mean;
        acc.add(d * d);
    }
    acc.total() / (n as f64 - 1.0)
}

/// Compensated (Neumaier) unbiased sample covariance of `xs` and `ys`.
///
/// Replaces the naive-sum covariance so the control-variate coefficient stays
/// accurate at large path counts (W-05).
fn compensated_covariance(xs: &[f64], ys: &[f64], mean_x: f64, mean_y: f64) -> f64 {
    let n = xs.len().min(ys.len());
    if n < 2 {
        return 0.0;
    }
    let mut acc = finstack_quant_core::math::NeumaierAccumulator::new();
    for (&x, &y) in xs.iter().zip(ys.iter()) {
        acc.add((x - mean_x) * (y - mean_y));
    }
    acc.total() / (n as f64 - 1.0)
}

/// Closed-form value of a **seasoned** geometric-average Asian option, used as
/// the analytic control variate for the seasoned arithmetic Asian MC (W-07).
///
/// The Monte Carlo geometric payoff computes `G = exp((Σ ln S_i) / N)` where
/// the sum runs over all `N` fixings — `m` already observed (contributing the
/// fixed quantity `hist_prod_log = Σ ln S_past`) and `k` future fixings at
/// times `future_times` (under the simulated GBM). `ln G` is therefore normal,
/// so `G` is lognormal and the option has a Black-style closed form.
///
/// With `ln S_{t_i} = ln S_0 + (r - q - σ²/2) t_i + σ W_{t_i}`:
/// * `μ = [hist_prod_log + Σ_i (ln S_0 + (r-q-σ²/2) t_i)] / N`
/// * `v = σ² · ΣᵢΣⱼ min(t_i, t_j) / N²`   (variance of `ln G`)
/// * `E[G] = exp(μ + v/2)`
///
/// The fallback drift uses `r` and discounting uses `df` so the value is consistent
/// with how the MC simulates and discounts (the unseasoned case reduces to the
/// standard Kemna-Vorst control). When the MC attaches a time-varying drift
/// schedule (curve-implied forwards on a non-flat curve), the same schedule
/// MUST be supplied here: the control variate is only unbiased when the
/// analytic control equals the true mean of the simulated geometric payoff,
/// so `E[ln S_{tᵢ}]` has to use the schedule's cumulative drift, not the
/// maturity-averaged constant `r − q`. Returns `df · max(forward_intrinsic,
/// 0)` in the degenerate zero-variance case.
#[allow(clippy::too_many_arguments)]
fn seasoned_geometric_asian_control(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    df: f64,
    hist_prod_log: f64,
    hist_count: usize,
    future_times: &[f64],
    is_call: bool,
    drift_schedule: Option<&finstack_quant_models::monte_carlo::process::gbm::DriftSchedule>,
    fixing_multipliers: Option<&[f64]>,
) -> f64 {
    let k = future_times.len();
    let n_total = hist_count + k;
    if n_total == 0 {
        return 0.0;
    }
    let n = n_total as f64;
    let ln_spot = spot.ln();
    let constant_drift = r - q - 0.5 * sigma * sigma;

    // Mean of ln(G): fixed past contribution + future drift contribution.
    // With a drift schedule, E[ln S_{tᵢ}] = ln S + M(tᵢ) − σ²tᵢ/2 where
    // M(t) is the schedule's cumulative log-drift — identical to what the MC
    // engine applies step by step.
    let mut mean_acc = finstack_quant_core::math::NeumaierAccumulator::new();
    mean_acc.add(hist_prod_log);
    for (index, &t_i) in future_times.iter().enumerate() {
        let drift_to_t = match drift_schedule {
            Some(schedule) => schedule.cumulative(t_i) - 0.5 * sigma * sigma * t_i,
            None => constant_drift * t_i,
        };
        let log_multiplier = fixing_multipliers.map_or(0.0, |scales| scales[index].ln());
        mean_acc.add(ln_spot + drift_to_t + log_multiplier);
    }
    let mu = mean_acc.total() / n;

    // Variance of ln(G): σ² ΣᵢΣⱼ min(t_i,t_j) / N².
    let mut cov_acc = finstack_quant_core::math::NeumaierAccumulator::new();
    for &t_i in future_times {
        for &t_j in future_times {
            cov_acc.add(t_i.min(t_j));
        }
    }
    let var_ln_g = sigma * sigma * cov_acc.total() / (n * n);

    // Degenerate (no future fixings or zero vol): G is deterministic.
    if var_ln_g <= 0.0 || k == 0 {
        let g = mu.exp();
        let intrinsic = if is_call {
            (g - strike).max(0.0)
        } else {
            (strike - g).max(0.0)
        };
        return df * intrinsic;
    }

    let std = var_ln_g.sqrt();
    let expected_g = (mu + 0.5 * var_ln_g).exp();
    let d1 = (mu + var_ln_g - strike.ln()) / std;
    let d2 = d1 - std;
    let norm_cdf = finstack_quant_core::math::norm_cdf;
    let price = if is_call {
        expected_g * norm_cdf(d1) - strike * norm_cdf(d2)
    } else {
        strike * norm_cdf(-d2) - expected_g * norm_cdf(-d1)
    };
    df * price.max(0.0)
}

/// Asian option Monte Carlo pricer.
pub struct AsianOptionMcPricer {
    config: PathDependentPricerConfig,
}

impl AsianOptionMcPricer {
    /// Create a new Asian option MC pricer with default config.
    pub fn new() -> Self {
        Self {
            config: PathDependentPricerConfig::default(),
        }
    }

    /// Price an Asian option using Monte Carlo.
    fn price_internal(
        &self,
        inst: &AsianOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        inst.validate_past_fixings(as_of)?;
        let t = inst
            .day_count
            .year_fraction(as_of, inst.expiry, DayCountContext::default())?;

        let (hist_sum, hist_prod_log, hist_count) = inst.accumulated_state(as_of);

        if t <= 0.0 {
            // Expired: use realized average
            let average = if hist_count > 0 {
                match inst.averaging_method {
                    crate::instruments::exotics::asian_option::types::AveragingMethod::Arithmetic => {
                        hist_sum / hist_count as f64
                    }
                    crate::instruments::exotics::asian_option::types::AveragingMethod::Geometric => {
                        (hist_prod_log / hist_count as f64).exp()
                    }
                }
            } else {
                let spot_scalar = curves.get_price(&inst.spot_id)?;
                match spot_scalar {
                    finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                    finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
                }
            };

            let intrinsic = match inst.option_type {
                crate::instruments::OptionType::Call => (average - inst.strike).max(0.0),
                crate::instruments::OptionType::Put => (inst.strike - average).max(0.0),
            };
            return Money::new(intrinsic * inst.quantity, inst.currency);
        }

        let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;
        let discount_factor = disc_curve.df_between_dates(as_of, inst.expiry)?;
        // Keep drift consistent with date-based discounting: exp(-r * t) == DF(as_of, maturity).
        // A non-positive/non-finite df means a corrupted curve; error rather
        // than silently pricing at a zero rate.
        let r = crate::instruments::common_impl::helpers::zero_rate_from_df(
            discount_factor,
            t,
            "AsianOption discount curve",
        )?;

        let spot_scalar = curves.get_price(&inst.spot_id)?;
        let spot = match spot_scalar {
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        let q = crate::instruments::common_impl::helpers::resolve_optional_dividend_yield(
            curves,
            inst.div_yield_id.as_ref(),
        )?;

        // Get volatility (override → surface)
        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &inst.instrument_pricing_overrides.market_quotes,
            curves,
            inst.vol_surface_id.as_str(),
            t,
            inst.strike,
        )?;

        let gbm_params = GbmParams::new(r, q, sigma)?;
        let process = GbmProcess::new(gbm_params);

        let base_cfg = crate::instruments::common_impl::helpers::merged_path_config(
            &self.config,
            &inst.instrument_pricing_overrides,
        )?;

        // Exact event times prevent time-grid resolution from changing the
        // average. Equal model times share a simulated state but retain weight.
        let base_num_steps =
            ((t * base_cfg.steps_per_year).round() as usize).max(base_cfg.min_steps);
        let fixing_grid =
            map_fixings_to_steps(&inst.fixing_dates, inst.day_count, as_of, t, base_num_steps)?;
        let (drift, fixing_multipliers) =
            asian_drift_and_fixing_multipliers(inst, curves, as_of, q)?;
        let drift_schedule = std::sync::Arc::new(drift);
        let process = process.with_drift_schedule(std::sync::Arc::clone(&drift_schedule));
        let future_fixing_times = fixing_grid.future_times();
        let time_grid = fixing_grid.time_grid;
        let fixing_steps = fixing_grid.fixing_steps;

        let averaging = inst.averaging_method;

        // Derive deterministic seed from instrument ID and scenario
        use finstack_quant_models::monte_carlo::seed;

        let seed = if let Some(ref scenario) = inst
            .instrument_pricing_overrides
            .model_config
            .mc_seed_scenario
        {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };

        let mut config = base_cfg;
        config.seed = seed;

        // If arithmetic averaging, apply geometric-Asian control variate for variance reduction
        let result_money =
            match (inst.averaging_method, inst.option_type) {
                (
                    crate::instruments::exotics::asian_option::types::AveragingMethod::Arithmetic,
                    crate::instruments::OptionType::Call,
                ) => {
                    // Use path capture to get per-path discounted payoffs for covariance
                    let mut cfg_cap = config;
                    cfg_cap.path_capture = PathCaptureConfig::all().with_payoffs();
                    let pricer_cap = PathDependentPricer::new(cfg_cap);

                    // Arithmetic payoff
                    let arith_payoff = AsianCall::with_history(
                    inst.strike,
                    inst.quantity,
                    finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod::Arithmetic,
                    fixing_steps.clone(),
                    hist_sum,
                    hist_prod_log,
                    hist_count,
                )?.with_fixing_multipliers(&fixing_multipliers)?;
                    let arith_full = pricer_cap.price_with_paths_and_grid(
                        &process,
                        spot,
                        time_grid.clone(),
                        &arith_payoff,
                        inst.currency,
                        discount_factor,
                    )?;

                    // Geometric payoff (same RNG via same seed)
                    let geom_payoff = AsianCall::with_history(
                    inst.strike,
                    inst.quantity,
                    finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod::Geometric,
                    fixing_steps,
                    hist_sum,
                    hist_prod_log,
                    hist_count,
                )?.with_fixing_multipliers(&fixing_multipliers)?;
                    let geom_full = pricer_cap.price_with_paths_and_grid(
                        &process,
                        spot,
                        time_grid,
                        &geom_payoff,
                        inst.currency,
                        discount_factor,
                    )?;

                    // Extract per-path discounted payoffs
                    // paths should be Some when path_capture is enabled in price_with_paths
                    let xs: Vec<f64> = arith_full
                        .paths
                        .as_ref()
                        .ok_or_else(|| {
                            finstack_quant_core::Error::Validation(
                                "Path capture enabled but paths not captured".into(),
                            )
                        })?
                        .paths
                        .iter()
                        .map(|p| p.final_value)
                        .collect();
                    let ys: Vec<f64> = geom_full
                        .paths
                        .as_ref()
                        .ok_or_else(|| {
                            finstack_quant_core::Error::Validation(
                                "Path capture enabled but paths not captured".into(),
                            )
                        })?
                        .paths
                        .iter()
                        .map(|p| p.final_value)
                        .collect();

                    let n = xs.len();
                    // Compensated (Neumaier) sample moments — the control-variate
                    // estimator sums up to `num_paths` terms, so naive summation
                    // would lose precision at large path counts (W-05).
                    let mean_x = compensated_mean(&xs);
                    let mean_y = compensated_mean(&ys);
                    let var_x = compensated_variance(&xs, mean_x);
                    let var_y = compensated_variance(&ys, mean_y);
                    let cov_xy = compensated_covariance(&xs, &ys, mean_x, mean_y);

                    // Analytical value of geometric Asian (control).
                    //
                    // The standard geometric-Asian closed form has no seasoning
                    // adjustment, so it is only a valid control when the option is
                    // unseasoned. For a seasoned arithmetic Asian the seasoning-
                    // aware analytic control variate is used instead (see below).

                    // Seasoning-aware analytic control variate (W-07). The
                    // geometric Asian is priced on the *exact* future fixing times
                    // the MC samples, with the past fixings' fixed log-product
                    // folded in, so it is the true mean of the simulated geometric
                    // payoff for both seasoned and unseasoned options. The seasoned
                    // path therefore keeps the variance reduction instead of
                    // discarding the geometric pass.
                    // The MC payoffs (xs/ys) are notional-scaled while the
                    // closed form is per unit notional, so the control mean must
                    // be scaled to the same units before the CV adjustment.
                    let control_analytical = inst.quantity
                        * seasoned_geometric_asian_control(
                            spot,
                            inst.strike,
                            r,
                            q,
                            sigma,
                            discount_factor,
                            hist_prod_log,
                            hist_count,
                            &future_fixing_times,
                            true,
                            Some(drift_schedule.as_ref()),
                            Some(&fixing_multipliers),
                        );
                    let adj = apply_control_variate(
                        mean_x,
                        var_x,
                        mean_y,
                        var_y,
                        cov_xy,
                        control_analytical,
                        n,
                    );
                    MoneyEstimate::from_estimate(adj, inst.currency)?.mean
                }
                (
                    crate::instruments::exotics::asian_option::types::AveragingMethod::Arithmetic,
                    crate::instruments::OptionType::Put,
                ) => {
                    let mut cfg_cap = config;
                    cfg_cap.path_capture = PathCaptureConfig::all().with_payoffs();
                    let pricer_cap = PathDependentPricer::new(cfg_cap);

                    let arith_payoff = AsianPut::with_history(
                    inst.strike,
                    inst.quantity,
                    finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod::Arithmetic,
                    fixing_steps.clone(),
                    hist_sum,
                    hist_prod_log,
                    hist_count,
                )?.with_fixing_multipliers(&fixing_multipliers)?;
                    let arith_full = pricer_cap.price_with_paths_and_grid(
                        &process,
                        spot,
                        time_grid.clone(),
                        &arith_payoff,
                        inst.currency,
                        discount_factor,
                    )?;

                    let geom_payoff = AsianPut::with_history(
                    inst.strike,
                    inst.quantity,
                    finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod::Geometric,
                    fixing_steps,
                    hist_sum,
                    hist_prod_log,
                    hist_count,
                )?.with_fixing_multipliers(&fixing_multipliers)?;
                    let geom_full = pricer_cap.price_with_paths_and_grid(
                        &process,
                        spot,
                        time_grid,
                        &geom_payoff,
                        inst.currency,
                        discount_factor,
                    )?;

                    // paths should be Some when path_capture is enabled in price_with_paths
                    let xs: Vec<f64> = arith_full
                        .paths
                        .as_ref()
                        .ok_or_else(|| {
                            finstack_quant_core::Error::Validation(
                                "Path capture enabled but paths not captured".into(),
                            )
                        })?
                        .paths
                        .iter()
                        .map(|p| p.final_value)
                        .collect();
                    let ys: Vec<f64> = geom_full
                        .paths
                        .as_ref()
                        .ok_or_else(|| {
                            finstack_quant_core::Error::Validation(
                                "Path capture enabled but paths not captured".into(),
                            )
                        })?
                        .paths
                        .iter()
                        .map(|p| p.final_value)
                        .collect();
                    let n = xs.len();
                    // Compensated (Neumaier) sample moments (W-05).
                    let mean_x = compensated_mean(&xs);
                    let mean_y = compensated_mean(&ys);
                    let var_x = compensated_variance(&xs, mean_x);
                    let var_y = compensated_variance(&ys, mean_y);
                    let cov_xy = compensated_covariance(&xs, &ys, mean_x, mean_y);

                    // Seasoning-aware analytic control variate (W-07) — see the
                    // call branch above for the rationale.
                    // Scale the per-unit closed form to the notional-scaled MC
                    // payoff units (see the call branch above).
                    let control_analytical = inst.quantity
                        * seasoned_geometric_asian_control(
                            spot,
                            inst.strike,
                            r,
                            q,
                            sigma,
                            discount_factor,
                            hist_prod_log,
                            hist_count,
                            &future_fixing_times,
                            false,
                            Some(drift_schedule.as_ref()),
                            Some(&fixing_multipliers),
                        );
                    let adj = apply_control_variate(
                        mean_x,
                        var_x,
                        mean_y,
                        var_y,
                        cov_xy,
                        control_analytical,
                        n,
                    );
                    MoneyEstimate::from_estimate(adj, inst.currency)?.mean
                }
                // Geometric averaging (no CV needed) or fallback path
                _ => {
                    let pricer = PathDependentPricer::new(config);
                    match inst.option_type {
                        crate::instruments::OptionType::Call => {
                            let payoff = AsianCall::with_history(
                                inst.strike,
                                inst.quantity,
                                averaging,
                                fixing_steps,
                                hist_sum,
                                hist_prod_log,
                                hist_count,
                            )?
                            .with_fixing_multipliers(&fixing_multipliers)?;
                            pricer
                                .price_with_grid(
                                    &process,
                                    spot,
                                    time_grid,
                                    &payoff,
                                    inst.currency,
                                    discount_factor,
                                )?
                                .mean
                        }
                        crate::instruments::OptionType::Put => {
                            let payoff = AsianPut::with_history(
                                inst.strike,
                                inst.quantity,
                                averaging,
                                fixing_steps,
                                hist_sum,
                                hist_prod_log,
                                hist_count,
                            )?
                            .with_fixing_multipliers(&fixing_multipliers)?;
                            pricer
                                .price_with_grid(
                                    &process,
                                    spot,
                                    time_grid,
                                    &payoff,
                                    inst.currency,
                                    discount_factor,
                                )?
                                .mean
                        }
                    }
                }
            };

        Ok(result_money)
    }
}

impl Default for AsianOptionMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for AsianOptionMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::AsianOption, ModelKey::MonteCarloGBM)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let asian = expect_inst::<AsianOption>(instrument, InstrumentType::AsianOption)?;

        let pv = self.price_internal(asian, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(asian.id(), as_of, pv))
    }
}

/// Present value using Monte Carlo.
pub(crate) fn compute_pv(
    inst: &AsianOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    let pricer = AsianOptionMcPricer::new();
    pricer.price_internal(inst, curves, as_of)
}

use crate::instruments::common_impl::helpers::collect_black_scholes_inputs;
use finstack_quant_models::closed_form::asian::arithmetic_asian_tw_price_times;

/// Geometric Asian option analytical pricer.
pub struct AsianOptionAnalyticalGeometricPricer;

impl AsianOptionAnalyticalGeometricPricer {
    /// Create a new analytical geometric Asian option pricer
    pub fn new() -> Self {
        Self
    }
}

impl Default for AsianOptionAnalyticalGeometricPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for AsianOptionAnalyticalGeometricPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::AsianOption, ModelKey::AsianGeometricBS)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let asian = expect_inst::<AsianOption>(instrument, InstrumentType::AsianOption)?;

        // Use standardized input collection
        let (spot, r, q, sigma, t) = collect_black_scholes_inputs(
            &asian.spot_id,
            &asian.discount_curve_id,
            asian.div_yield_id.as_ref(),
            &asian.vol_surface_id,
            asian.strike,
            asian.expiry,
            asian.day_count,
            market,
            as_of,
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        asian.validate_past_fixings(as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        let (sum, log_prod, count) = asian.accumulated_state(as_of);
        let total_fixings = asian.fixing_dates.len();

        if t <= 0.0 {
            // Handle expired option using realized average
            let average = if count > 0 {
                match asian.averaging_method {
                    crate::instruments::exotics::asian_option::types::AveragingMethod::Arithmetic => {
                        sum / count as f64
                    }
                    crate::instruments::exotics::asian_option::types::AveragingMethod::Geometric => {
                        (log_prod / count as f64).exp()
                    }
                }
            } else {
                // Fallback if no fixings recorded (unlikely for expired option)
                spot
            };

            let intrinsic = match asian.option_type {
                crate::instruments::OptionType::Call => (average - asian.strike).max(0.0),
                crate::instruments::OptionType::Put => (asian.strike - average).max(0.0),
            };
            return Ok(ValuationResult::stamped(
                asian.id(),
                as_of,
                Money::new(intrinsic * asian.quantity, asian.currency).map_err(|error| {
                    crate::pricer::PricingError::from_core(
                        error,
                        crate::pricer::PricingErrorContext::from_instrument(asian),
                    )
                })?,
            ));
        }

        // Seasoned Geometric Asian requires adjusted strike formula not yet implemented.
        // The adjustment involves: K_eff = (n·K - G_past) / (n - m) where G_past is the
        // geometric average of past fixings. For now, fall back to Monte Carlo.
        if count > 0 {
            return Err(PricingError::model_failure_with_context(
                format!(
                    "Seasoned Geometric Asian analytical pricing not supported ({} of {} fixings already observed). \
                    For seasoned options, use Monte Carlo pricing via `npv_mc()` method or set \
                    `averaging_method = Arithmetic` which supports seasoning via Turnbull-Wakeman.",
                    count, total_fixings
                ),
                PricingErrorContext::default(),
            ));
        }

        // Price on the actual fixing schedule. Falling back to the
        // equal-spacing formula would misprice contracts whose fixings are
        // not uniformly spaced over [as_of, expiry] (e.g. averaging windows
        // concentrated near expiry).
        let fixing_times = fixing_year_fractions(asian, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        let df = (-r * t).exp();
        let is_call = matches!(asian.option_type, crate::instruments::OptionType::Call);
        let (drift, multipliers) = asian_drift_and_fixing_multipliers(asian, market, as_of, q)
            .map_err(|error| {
                PricingError::from_core(error, PricingErrorContext::from_instrument(asian))
            })?;
        let price = seasoned_geometric_asian_control(
            spot,
            asian.strike,
            r,
            q,
            sigma,
            df,
            0.0,
            0,
            &fixing_times,
            is_call,
            Some(&drift),
            Some(&multipliers),
        );

        let pv = Money::new(price * asian.quantity, asian.currency).map_err(|error| {
            crate::pricer::PricingError::from_core(
                error,
                crate::pricer::PricingErrorContext::from_instrument(asian),
            )
        })?;
        Ok(ValuationResult::stamped(asian.id(), as_of, pv))
    }
}

/// Year fractions (instrument day count) from `as_of` to each strictly-future
/// fixing date, sorted ascending (fixing dates are stored sorted).
fn fixing_year_fractions(
    asian: &AsianOption,
    as_of: Date,
) -> finstack_quant_core::Result<Vec<f64>> {
    let mut times = Vec::with_capacity(asian.fixing_dates.len());
    for date in &asian.fixing_dates {
        if *date > as_of {
            times.push(
                asian
                    .day_count
                    .year_fraction(as_of, *date, DayCountContext::default())?,
            );
        }
    }
    Ok(times)
}

/// Arithmetic Asian option semi-analytical pricer (Turnbull-Wakeman).
pub struct AsianOptionSemiAnalyticalTwPricer;

impl AsianOptionSemiAnalyticalTwPricer {
    /// Create a new Turnbull-Wakeman approximation Asian option pricer
    pub fn new() -> Self {
        Self
    }
}

impl Default for AsianOptionSemiAnalyticalTwPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for AsianOptionSemiAnalyticalTwPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::AsianOption, ModelKey::AsianTurnbullWakeman)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        use crate::instruments::common_impl::helpers::collect_black_scholes_inputs_df;

        let asian = expect_inst::<AsianOption>(instrument, InstrumentType::AsianOption)?;

        // Use DF-based input collection for time-consistent discounting
        let bs_inputs = collect_black_scholes_inputs_df(
            &asian.spot_id,
            &asian.discount_curve_id,
            asian.div_yield_id.as_ref(),
            &asian.vol_surface_id,
            asian.strike,
            asian.expiry,
            asian.day_count,
            market,
            as_of,
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let spot = bs_inputs.spot;
        let df_expiry = bs_inputs.df;
        let q = bs_inputs.q;
        let sigma = bs_inputs.sigma;
        let t = bs_inputs.t;

        asian.validate_past_fixings(as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        let (sum, _, count) = asian.accumulated_state(as_of);
        let total_fixings = asian.fixing_dates.len();

        if t <= 0.0 {
            let average = if count > 0 { sum / count as f64 } else { spot };
            let intrinsic = match asian.option_type {
                crate::instruments::OptionType::Call => (average - asian.strike).max(0.0),
                crate::instruments::OptionType::Put => (asian.strike - average).max(0.0),
            };
            return Ok(ValuationResult::stamped(
                asian.id(),
                as_of,
                Money::new(intrinsic * asian.quantity, asian.currency).map_err(|error| {
                    crate::pricer::PricingError::from_core(
                        error,
                        crate::pricer::PricingErrorContext::from_instrument(asian),
                    )
                })?,
            ));
        }

        let future_fixings = total_fixings.saturating_sub(count);
        if future_fixings == 0 {
            // Deterministic case (all fixings past, but not expired?)
            let average = sum / total_fixings as f64;
            let payoff = match asian.option_type {
                crate::instruments::OptionType::Call => (average - asian.strike).max(0.0),
                crate::instruments::OptionType::Put => (asian.strike - average).max(0.0),
            };
            // Use the date-based DF from inputs
            return Ok(ValuationResult::stamped(
                asian.id(),
                as_of,
                Money::new(payoff * df_expiry * asian.quantity, asian.currency).map_err(
                    |error| {
                        crate::pricer::PricingError::from_core(
                            error,
                            crate::pricer::PricingErrorContext::from_instrument(asian),
                        )
                    },
                )?,
            ));
        }

        let n = total_fixings as f64;
        let m = future_fixings as f64;
        let k = asian.strike;

        let numerator = n * k - sum;
        let k_eff = numerator / m;
        let scale = m / n;

        let price = if k_eff < 0.0 {
            match asian.option_type {
                crate::instruments::OptionType::Call => {
                    // Deep ITM: PV = DF_T * (Expected_Avg - K)
                    // Expected_Avg = (Past_Sum + Sum(F_i)) / N
                    // F_i = forward price for fixing date i
                    //     = S * exp(-q*t_i) / df_i  (GK forward formula)
                    //
                    // We compute each forward using date-based DFs for consistency.
                    let disc_curve = market
                        .get_discount(asian.discount_curve_id.as_str())
                        .map_err(|e| {
                            PricingError::model_failure_with_context(
                                e.to_string(),
                                PricingErrorContext::default(),
                            )
                        })?;

                    let mut sum_fwd = 0.0;
                    for date in &asian.fixing_dates {
                        if *date > as_of {
                            let t_i = asian
                                .day_count
                                .year_fraction(as_of, *date, DayCountContext::default())
                                .map_err(|e| {
                                    PricingError::model_failure_with_context(
                                        e.to_string(),
                                        PricingErrorContext::default(),
                                    )
                                })?;
                            // Get date-based DF for this fixing
                            let df_i = disc_curve.df_between_dates(as_of, *date).map_err(|e| {
                                PricingError::model_failure_with_context(
                                    e.to_string(),
                                    PricingErrorContext::default(),
                                )
                            })?;
                            // GK forward: F_i = S * exp(-q*t_i) / df_i
                            let forward_i = spot * (-q * t_i).exp() / df_i;
                            sum_fwd += forward_i;
                        }
                    }
                    let expected_avg = (sum + sum_fwd) / n;
                    // Use date-based DF for final discounting
                    (expected_avg - k).max(0.0) * df_expiry
                }
                crate::instruments::OptionType::Put => {
                    // Deep ITM put (k_eff < 0): past fixings already push average above K.
                    // Compute expected average using forwards and return discounted payoff.
                    // PV = DF_T * max(K - Expected_Avg, 0)
                    let disc_curve = market
                        .get_discount(asian.discount_curve_id.as_str())
                        .map_err(|e| {
                            PricingError::model_failure_with_context(
                                e.to_string(),
                                PricingErrorContext::default(),
                            )
                        })?;

                    let mut sum_fwd = 0.0;
                    for date in &asian.fixing_dates {
                        if *date > as_of {
                            let t_i = asian
                                .day_count
                                .year_fraction(as_of, *date, DayCountContext::default())
                                .map_err(|e| {
                                    PricingError::model_failure_with_context(
                                        e.to_string(),
                                        PricingErrorContext::default(),
                                    )
                                })?;
                            let df_i = disc_curve.df_between_dates(as_of, *date).map_err(|e| {
                                PricingError::model_failure_with_context(
                                    e.to_string(),
                                    PricingErrorContext::default(),
                                )
                            })?;
                            let forward_i = spot * (-q * t_i).exp() / df_i;
                            sum_fwd += forward_i;
                        }
                    }
                    let expected_avg = (sum + sum_fwd) / n;
                    (k - expected_avg).max(0.0) * df_expiry
                }
            }
        } else {
            // Price the remaining-fixings option on the ACTUAL future fixing
            // times: seasoned schedules are concentrated near expiry, so the
            // equal-spacing-over-[0, t] assumption misstates the average's
            // variance.
            let future_times = fixing_year_fractions(asian, as_of).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
            let is_call = matches!(asian.option_type, crate::instruments::OptionType::Call);
            let unscaled = arithmetic_asian_tw_price_times(
                spot,
                k_eff,
                t,
                df_expiry,
                q,
                sigma,
                &future_times,
                is_call,
            )
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
            unscaled * scale
        };

        let pv = Money::new(price * asian.quantity, asian.currency).map_err(|error| {
            crate::pricer::PricingError::from_core(
                error,
                crate::pricer::PricingErrorContext::from_instrument(asian),
            )
        })?;
        Ok(ValuationResult::stamped(asian.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::exotics::asian_option::{AsianOption, AveragingMethod};
    use crate::instruments::OptionType;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
    use finstack_quant_models::closed_form::asian::{geometric_asian_call, geometric_asian_put};
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn market(as_of: Date, spot: f64, vol: f64, rate: f64, div_yield: f64) -> MarketContext {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("discount curve");

        let vol_surface = VolSurface::builder("SPX-VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .build()
            .expect("vol surface");

        MarketContext::new()
            .insert(discount)
            .insert_surface(vol_surface)
            .insert_price("SPX-SPOT", MarketScalar::Unitless(spot))
            .insert_price("SPX-DIV", MarketScalar::Unitless(div_yield))
    }

    fn market_without_vol(as_of: Date, spot: f64, rate: f64, div_yield: f64) -> MarketContext {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("discount curve");

        MarketContext::new()
            .insert(discount)
            .insert_price("SPX-SPOT", MarketScalar::Unitless(spot))
            .insert_price("SPX-DIV", MarketScalar::Unitless(div_yield))
    }

    fn asian_option(
        averaging: AveragingMethod,
        option_type: OptionType,
        expiry: Date,
        strike: f64,
        fixing_dates: Vec<Date>,
    ) -> AsianOption {
        AsianOption::builder()
            .id(InstrumentId::new("ASIAN-TEST"))
            .underlying_ticker("SPX".to_string())
            .strike(strike)
            .option_type(option_type)
            .averaging_method(averaging)
            .expiry(expiry)
            .fixing_dates(fixing_dates)
            .quantity(1.0)
            .currency(Currency::USD)
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(PriceId::new("SPX-DIV")))
            .attributes(Default::default())
            .build()
            .expect("asian option")
    }

    #[test]
    fn geometric_pricer_matches_kemna_vorst_call_benchmark() {
        let as_of = date(2025, 1, 2);
        let expiry = date(2026, 1, 2);
        let fixing_dates = vec![
            date(2025, 4, 2),
            date(2025, 7, 2),
            date(2025, 10, 2),
            date(2026, 1, 2),
        ];

        let spot = 100.0;
        let strike = 100.0;
        let vol = 0.20;
        let rate = 0.05;
        let div_yield = 0.00;

        let market = market(as_of, spot, vol, rate, div_yield);
        let option = asian_option(
            AveragingMethod::Geometric,
            OptionType::Call,
            expiry,
            strike,
            fixing_dates.clone(),
        );

        let pv = option.value(&market, as_of).expect("asian pv").amount();

        let t = option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        // Benchmark on the ACTUAL fixing times (calendar quarters are not
        // exactly uniform under ACT/365F), matching the pricer's schedule-
        // aware formula.
        let times: Vec<f64> = fixing_dates
            .iter()
            .map(|d| {
                option
                    .day_count
                    .year_fraction(as_of, *d, DayCountContext::default())
                    .expect("year fraction")
            })
            .collect();
        let expected = finstack_quant_models::closed_form::asian::geometric_asian_price_times(
            spot,
            strike,
            t,
            (-rate * t).exp(),
            div_yield,
            vol,
            &times,
            true,
        )
        .expect("valid schedule");
        // And it must stay close to the uniform-grid Kemna-Vorst benchmark.
        let kv = geometric_asian_call(spot, strike, t, rate, div_yield, vol, fixing_dates.len());
        let expected_money = Money::new(expected, Currency::USD)
            .expect("valid money fixture")
            .amount();

        assert!((pv - expected_money).abs() < 1e-12);
        assert!((pv - kv).abs() < 0.05, "pv {pv} far from Kemna-Vorst {kv}");
    }

    /// `model_config.mc_antithetic` must reach the Asian Monte Carlo engine
    /// through the shared `merged_path_config`: toggling it changes the
    /// simulated streams, so the PV moves, while both estimates stay within
    /// Monte Carlo noise of the discrete-fixing geometric (Kemna-Vorst) price.
    ///
    /// Tolerance: the discounted ATM 1y geometric-Asian payoff has a standard
    /// deviation below 7, so 20,000 estimators give a standard error below
    /// 0.05; 0.2 is four standard errors.
    #[test]
    fn asian_honours_mc_antithetic() {
        let as_of = date(2025, 1, 2);
        let expiry = date(2026, 1, 2);
        let fixing_dates = vec![
            date(2025, 4, 2),
            date(2025, 7, 2),
            date(2025, 10, 2),
            date(2026, 1, 2),
        ];
        let (spot, strike, vol, rate) = (100.0, 100.0, 0.20, 0.05);
        let market = market(as_of, spot, vol, rate, 0.0);
        let mut option = asian_option(
            AveragingMethod::Geometric,
            OptionType::Call,
            expiry,
            strike,
            fixing_dates.clone(),
        );
        option.instrument_pricing_overrides.model_config.mc_paths = Some(20_000);

        let pv_with = |antithetic: bool| {
            let mut inst = option.clone();
            inst.instrument_pricing_overrides.model_config.mc_antithetic = Some(antithetic);
            AsianOptionMcPricer::new()
                .price_internal(&inst, &market, as_of)
                .expect("mc price")
                .amount()
        };
        let plain = pv_with(false);
        let antithetic = pv_with(true);

        let t = option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let times: Vec<f64> = fixing_dates
            .iter()
            .map(|d| {
                option
                    .day_count
                    .year_fraction(as_of, *d, DayCountContext::default())
                    .expect("year fraction")
            })
            .collect();
        let reference = finstack_quant_models::closed_form::asian::geometric_asian_price_times(
            spot,
            strike,
            t,
            (-rate * t).exp(),
            0.0,
            vol,
            &times,
            true,
        )
        .expect("valid schedule");

        assert_ne!(
            plain.to_bits(),
            antithetic.to_bits(),
            "mc_antithetic must change the simulated streams"
        );
        for pv in [plain, antithetic] {
            assert!(
                (pv - reference).abs() < 0.2,
                "MC geometric Asian {pv} outside 4 standard errors of {reference}"
            );
        }
    }

    #[test]
    fn geometric_pricer_matches_kemna_vorst_put_benchmark() {
        let as_of = date(2025, 1, 2);
        let expiry = date(2026, 1, 2);
        let fixing_dates = vec![date(2025, 6, 2), date(2026, 1, 2)];

        let spot = 100.0;
        let strike = 110.0;
        let vol = 0.25;
        let rate = 0.03;
        let div_yield = 0.01;

        let market = market(as_of, spot, vol, rate, div_yield);
        let option = asian_option(
            AveragingMethod::Geometric,
            OptionType::Put,
            expiry,
            strike,
            fixing_dates.clone(),
        );

        let pv = option.value(&market, as_of).expect("asian pv").amount();

        let t = option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let times: Vec<f64> = fixing_dates
            .iter()
            .map(|d| {
                option
                    .day_count
                    .year_fraction(as_of, *d, DayCountContext::default())
                    .expect("year fraction")
            })
            .collect();
        let expected = finstack_quant_models::closed_form::asian::geometric_asian_price_times(
            spot,
            strike,
            t,
            (-rate * t).exp(),
            div_yield,
            vol,
            &times,
            false,
        )
        .expect("valid schedule");
        let kv = geometric_asian_put(spot, strike, t, rate, div_yield, vol, fixing_dates.len());
        let expected_money = Money::new(expected, Currency::USD)
            .expect("valid money fixture")
            .amount();

        assert!((pv - expected_money).abs() < 1e-12);
        // With only 2 fixings the calendar schedule (≈0.41y, 1y) deviates
        // visibly from the uniform grid (0.5y, 1y); the prices stay close
        // but not equal.
        assert!((pv - kv).abs() < 0.5, "pv {pv} far from Kemna-Vorst {kv}");
    }

    #[test]
    fn turnbull_wakeman_respects_fully_realized_average_payoff() {
        let as_of = date(2025, 7, 1);
        let expiry = date(2025, 12, 31);
        let fixing_dates = vec![
            date(2025, 1, 31),
            date(2025, 2, 28),
            date(2025, 3, 31),
            date(2025, 4, 30),
            date(2025, 5, 31),
            date(2025, 6, 30),
        ];

        let mut option = asian_option(
            AveragingMethod::Arithmetic,
            OptionType::Call,
            expiry,
            100.0,
            fixing_dates.clone(),
        );
        option.past_fixings = fixing_dates
            .iter()
            .copied()
            .zip([102.0, 101.0, 103.0, 104.0, 100.0, 105.0])
            .collect();

        let market = market(as_of, 100.0, 0.20, 0.05, 0.0);
        let pv = option.value(&market, as_of).expect("asian pv").amount();

        let average = [102.0, 101.0, 103.0, 104.0, 100.0, 105.0]
            .iter()
            .sum::<f64>()
            / fixing_dates.len() as f64;
        let payoff = (average - 100.0).max(0.0);
        let df = market.get_discount("USD-OIS").expect("discount").df(option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction"));
        let expected_money = Money::new(payoff * df, Currency::USD)
            .expect("valid money fixture")
            .amount();

        assert!((pv - expected_money).abs() < 1e-12);
    }

    #[test]
    fn mc_pricer_expired_uses_realized_arithmetic_average() {
        let as_of = date(2025, 6, 30);
        let fixing_dates = vec![date(2025, 4, 30), date(2025, 5, 31), date(2025, 6, 30)];
        let mut option = asian_option(
            AveragingMethod::Arithmetic,
            OptionType::Call,
            as_of,
            100.0,
            fixing_dates.clone(),
        );
        option.past_fixings = fixing_dates
            .iter()
            .copied()
            .zip([90.0, 110.0, 120.0])
            .collect();

        let pv = AsianOptionMcPricer::new()
            .price_internal(&option, &market(as_of, 999.0, 0.20, 0.05, 0.0), as_of)
            .expect("expired MC price")
            .amount();

        let expected = ((90.0_f64 + 110.0 + 120.0) / 3.0 - 100.0).max(0.0);
        assert!((pv - expected).abs() < 1e-12);
    }

    #[test]
    fn mc_pricer_expired_without_fixings_errors() {
        let as_of = date(2025, 6, 30);
        let option = asian_option(
            AveragingMethod::Geometric,
            OptionType::Put,
            as_of,
            100.0,
            vec![date(2025, 3, 31), date(2025, 6, 30)],
        );

        let err = AsianOptionMcPricer::new()
            .price_internal(&option, &market(as_of, 80.0, 0.20, 0.05, 0.0), as_of)
            .expect_err("expired Asian option must require realized fixings");
        assert!(err.to_string().contains("missing a past fixing"));
    }

    #[test]
    fn mc_price_dyn_wraps_market_data_errors() {
        let as_of = date(2025, 1, 2);
        let expiry = date(2025, 7, 2);
        let option = asian_option(
            AveragingMethod::Arithmetic,
            OptionType::Call,
            expiry,
            100.0,
            vec![date(2025, 3, 2), expiry],
        );

        let err = AsianOptionMcPricer::new()
            .price_dyn(&option, &market_without_vol(as_of, 100.0, 0.05, 0.0), as_of)
            .expect_err("missing vol surface should be wrapped");
        assert!(err.to_string().contains("SPX-VOL"));
    }

    #[test]
    fn analytical_geometric_expired_uses_realized_average_and_ignores_spot() {
        let as_of = date(2025, 6, 30);
        let fixing_dates = vec![date(2025, 5, 31), as_of];
        let mut option = asian_option(
            AveragingMethod::Geometric,
            OptionType::Call,
            as_of,
            100.0,
            fixing_dates.clone(),
        );
        option.past_fixings = fixing_dates.iter().copied().zip([100.0, 121.0]).collect();

        let pricer = AsianOptionAnalyticalGeometricPricer::new();
        let low_spot_pv = pricer
            .price_dyn(&option, &market(as_of, 50.0, 0.20, 0.05, 0.0), as_of)
            .expect("expired geometric Asian price at low current spot")
            .value
            .amount();
        let high_spot_pv = pricer
            .price_dyn(&option, &market(as_of, 999.0, 0.20, 0.05, 0.0), as_of)
            .expect("expired geometric Asian price at high current spot")
            .value
            .amount();

        // sqrt(100 * 121) = 110, so the realized call payoff is exactly 10.
        assert!(
            (low_spot_pv - 10.0).abs() < 1e-12,
            "expired geometric payoff should be 10, got {low_spot_pv}"
        );
        assert!(
            (high_spot_pv - low_spot_pv).abs() < 1e-12,
            "expired geometric payoff must ignore current spot: low={low_spot_pv}, high={high_spot_pv}"
        );
    }

    #[test]
    fn analytical_geometric_expired_without_fixings_errors() {
        let as_of = date(2025, 6, 30);
        let option = asian_option(
            AveragingMethod::Geometric,
            OptionType::Call,
            as_of,
            100.0,
            vec![date(2025, 3, 31), date(2025, 6, 30)],
        );

        let err = AsianOptionAnalyticalGeometricPricer::new()
            .price_dyn(&option, &market(as_of, 125.0, 0.20, 0.05, 0.0), as_of)
            .expect_err("expired Asian option must require realized fixings");
        assert!(err.to_string().contains("missing a past fixing"));
    }

    #[test]
    fn analytical_geometric_price_dyn_wraps_market_data_errors() {
        let as_of = date(2025, 1, 2);
        let expiry = date(2025, 7, 2);
        let option = asian_option(
            AveragingMethod::Geometric,
            OptionType::Call,
            expiry,
            100.0,
            vec![date(2025, 3, 2), expiry],
        );

        let err = AsianOptionAnalyticalGeometricPricer::new()
            .price_dyn(&option, &market_without_vol(as_of, 100.0, 0.05, 0.0), as_of)
            .expect_err("missing vol surface should be wrapped");
        assert!(err.to_string().contains("SPX-VOL"));
    }

    #[test]
    fn analytical_geometric_rejects_seasoned_option() {
        let as_of = date(2025, 7, 1);
        let expiry = date(2025, 12, 31);
        let fixing_dates = vec![
            date(2025, 1, 31),
            date(2025, 3, 31),
            date(2025, 6, 30),
            date(2025, 9, 30),
            expiry,
        ];

        let mut option = asian_option(
            AveragingMethod::Geometric,
            OptionType::Call,
            expiry,
            100.0,
            fixing_dates.clone(),
        );
        option.past_fixings = fixing_dates
            .iter()
            .take(3)
            .copied()
            .zip([101.0, 103.0, 102.0])
            .collect();

        let err = AsianOptionAnalyticalGeometricPricer::new()
            .price_dyn(&option, &market(as_of, 100.0, 0.20, 0.05, 0.0), as_of)
            .expect_err("seasoned geometric analytical pricing should be rejected");
        assert!(err
            .to_string()
            .contains("Seasoned Geometric Asian analytical pricing not supported"));
    }

    #[test]
    fn turnbull_wakeman_all_fixings_past_put_discounts_deterministic_payoff() {
        let as_of = date(2025, 7, 1);
        let expiry = date(2025, 12, 31);
        let fixing_dates = vec![
            date(2025, 1, 31),
            date(2025, 2, 28),
            date(2025, 3, 31),
            date(2025, 4, 30),
            date(2025, 5, 31),
            date(2025, 6, 30),
        ];

        let mut option = asian_option(
            AveragingMethod::Arithmetic,
            OptionType::Put,
            expiry,
            100.0,
            fixing_dates.clone(),
        );
        option.past_fixings = fixing_dates
            .iter()
            .copied()
            .zip([92.0, 95.0, 97.0, 96.0, 94.0, 98.0])
            .collect();

        let market = market(as_of, 100.0, 0.20, 0.05, 0.0);
        let pv = AsianOptionSemiAnalyticalTwPricer::new()
            .price_dyn(&option, &market, as_of)
            .expect("deterministic TW price")
            .value
            .amount();

        let average =
            [92.0, 95.0, 97.0, 96.0, 94.0, 98.0].iter().sum::<f64>() / fixing_dates.len() as f64;
        let payoff = (100.0 - average).max(0.0);
        let df = market.get_discount("USD-OIS").expect("discount").df(option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction"));
        let expected = Money::new(payoff * df, Currency::USD)
            .expect("valid money fixture")
            .amount();

        assert!((pv - expected).abs() < 1e-12);
    }

    #[test]
    fn turnbull_wakeman_negative_effective_strike_call_uses_forward_average_branch() {
        let as_of = date(2025, 4, 2);
        let expiry = date(2025, 10, 2);
        let fixing_dates = vec![
            date(2025, 1, 2),
            date(2025, 2, 2),
            date(2025, 3, 2),
            date(2025, 4, 2),
            date(2025, 5, 2),
            date(2025, 6, 2),
            date(2025, 7, 2),
            date(2025, 8, 2),
            date(2025, 9, 2),
            date(2025, 10, 2),
        ];

        let mut option = asian_option(
            AveragingMethod::Arithmetic,
            OptionType::Call,
            expiry,
            100.0,
            fixing_dates.clone(),
        );
        option.past_fixings = fixing_dates
            .iter()
            .take(4)
            .copied()
            .zip([250.0, 240.0, 260.0, 255.0])
            .collect();

        let market = market(as_of, 100.0, 0.20, 0.05, 0.01);
        let pv = AsianOptionSemiAnalyticalTwPricer::new()
            .price_dyn(&option, &market, as_of)
            .expect("deep ITM TW price")
            .value
            .amount();

        let disc_curve = market.get_discount("USD-OIS").expect("discount");
        let df_expiry = disc_curve.df(option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction"));
        let spot = 100.0;
        let q = 0.01;
        let sum_past = [250.0, 240.0, 260.0, 255.0].iter().sum::<f64>();
        let n = fixing_dates.len() as f64;
        let mut sum_fwd = 0.0;
        for date in fixing_dates.iter().copied().filter(|d| *d > as_of) {
            let t_i = option
                .day_count
                .year_fraction(as_of, date, DayCountContext::default())
                .expect("fixing year fraction");
            let df_i = disc_curve
                .df_between_dates(as_of, date)
                .expect("date-based discount factor");
            sum_fwd += spot * (-q * t_i).exp() / df_i;
        }
        let expected_avg = (sum_past + sum_fwd) / n;
        let expected = Money::new(
            (expected_avg - option.strike).max(0.0) * df_expiry,
            Currency::USD,
        )
        .expect("valid money fixture")
        .amount();

        assert!((pv - expected).abs() < 1e-12);
    }

    #[test]
    fn turnbull_wakeman_negative_effective_strike_put_clamps_to_zero() {
        let as_of = date(2025, 4, 2);
        let expiry = date(2025, 10, 2);
        let fixing_dates = vec![
            date(2025, 1, 2),
            date(2025, 2, 2),
            date(2025, 3, 2),
            date(2025, 4, 2),
            date(2025, 5, 2),
            date(2025, 6, 2),
            date(2025, 7, 2),
            date(2025, 8, 2),
            date(2025, 9, 2),
            date(2025, 10, 2),
        ];

        let mut option = asian_option(
            AveragingMethod::Arithmetic,
            OptionType::Put,
            expiry,
            100.0,
            fixing_dates.clone(),
        );
        option.past_fixings = fixing_dates
            .iter()
            .take(4)
            .copied()
            .zip([250.0, 240.0, 260.0, 255.0])
            .collect();

        let pv = AsianOptionSemiAnalyticalTwPricer::new()
            .price_dyn(&option, &market(as_of, 100.0, 0.20, 0.05, 0.01), as_of)
            .expect("deep OTM TW put should price")
            .value
            .amount();

        assert_eq!(pv, 0.0);
    }

    #[test]
    fn mc_pricer_expired_without_fixings_rejects_price_scalar_fallback() {
        let as_of = date(2025, 6, 30);
        let option = asian_option(
            AveragingMethod::Arithmetic,
            OptionType::Call,
            as_of,
            100.0,
            vec![date(2025, 3, 31), date(2025, 6, 30)],
        );

        let market = market(as_of, 80.0, 0.20, 0.05, 0.0).insert_price(
            "SPX-SPOT",
            MarketScalar::Price(Money::from((125_i64, Currency::USD))),
        );

        let err = AsianOptionMcPricer::new()
            .price_internal(&option, &market, as_of)
            .expect_err("price scalar must not replace past fixing history");
        assert!(err.to_string().contains("missing a past fixing"));
    }

    #[test]
    fn turnbull_wakeman_price_dyn_wraps_market_data_errors() {
        let as_of = date(2025, 1, 2);
        let expiry = date(2025, 7, 2);
        let option = asian_option(
            AveragingMethod::Arithmetic,
            OptionType::Put,
            expiry,
            100.0,
            vec![date(2025, 3, 2), expiry],
        );

        let err = AsianOptionSemiAnalyticalTwPricer::new()
            .price_dyn(&option, &market_without_vol(as_of, 100.0, 0.05, 0.0), as_of)
            .expect_err("missing vol surface should be wrapped");
        assert!(err.to_string().contains("SPX-VOL"));
    }

    /// Distinct fixing times are inserted exactly even on a coarse base grid;
    /// no contractual observation is lost or shifted to an arbitrary free slot.
    #[test]
    fn w04_close_fixings_map_to_distinct_steps() {
        let as_of = date(2025, 1, 1);
        // Twelve monthly fixings within one year.
        let fixing_dates: Vec<Date> = (1..=12).map(|m| date(2025, m, 15)).collect();
        let t = DayCount::Act365F
            .year_fraction(as_of, date(2026, 1, 1), DayCountContext::default())
            .expect("year fraction");

        // A deliberately coarse base grid (4 steps) cannot resolve 12 monthly
        // fixings — the naive round()+dedup() would merge several of them.
        let grid = map_fixings_to_steps(&fixing_dates, DayCount::Act365F, as_of, t, 4)
            .expect("fixing grid");

        assert_eq!(
            grid.fixing_steps.len(),
            12,
            "all 12 distinct monthly fixings must survive as distinct steps; \
             got {} — the coarse-grid round()+dedup() merged some",
            grid.fixing_steps.len()
        );
        // Steps must be strictly increasing (distinct) and within the grid.
        for w in grid.fixing_steps.windows(2) {
            assert!(
                w[1] > w[0],
                "fixing steps must be strictly increasing/distinct, got {:?}",
                grid.fixing_steps
            );
        }
        assert!(
            grid.fixing_steps
                .iter()
                .all(|&s| s >= 1 && s <= grid.time_grid.num_steps()),
            "every fixing step must lie in [1, num_steps={}]",
            grid.time_grid.num_steps()
        );
    }

    /// W-04: when fixings are well separated the helper does not blow up the
    /// grid — it returns at least the base step count and keeps each fixing
    /// distinct.
    #[test]
    fn w04_well_separated_fixings_keep_base_grid() {
        let as_of = date(2025, 1, 1);
        let fixing_dates = vec![date(2025, 4, 1), date(2025, 8, 1), date(2026, 1, 1)];
        let t = DayCount::Act365F
            .year_fraction(as_of, date(2026, 1, 1), DayCountContext::default())
            .expect("year fraction");
        let grid = map_fixings_to_steps(&fixing_dates, DayCount::Act365F, as_of, t, 252)
            .expect("fixing grid");
        assert_eq!(grid.fixing_steps.len(), 3);
        assert!(grid.time_grid.num_steps() >= 252);
    }

    /// W-05: the compensated covariance/variance helpers must stay accurate
    /// even when the per-path payoffs carry a large constant offset that would
    /// swamp a naive sum of squared deviations.
    #[test]
    fn w05_compensated_moments_are_accurate_with_large_offset() {
        // Values with a huge offset: naive Σ(x-mean)² loses precision badly.
        let offset = 1e9;
        let xs: Vec<f64> = (0..10_000).map(|i| offset + (i % 7) as f64).collect();
        let ys: Vec<f64> = (0..10_000).map(|i| offset + (i % 5) as f64).collect();

        let mean_x = compensated_mean(&xs);
        let var_x = compensated_variance(&xs, mean_x);
        let cov = compensated_covariance(&xs, &ys, mean_x, compensated_mean(&ys));

        // Reference variance of the repeating pattern 0..7 (independent of the
        // offset). Compute it directly on the de-offset values.
        let centered: Vec<f64> = xs.iter().map(|v| v - offset).collect();
        let ref_mean = centered.iter().sum::<f64>() / centered.len() as f64;
        let ref_var = centered.iter().map(|v| (v - ref_mean).powi(2)).sum::<f64>()
            / (centered.len() as f64 - 1.0);

        assert!(
            (var_x - ref_var).abs() < 1e-6,
            "compensated variance {var_x} must match reference {ref_var} \
             despite the 1e9 offset"
        );
        assert!(cov.is_finite(), "compensated covariance must be finite");
    }

    /// W-07: `seasoned_geometric_asian_control` must reduce to the standard
    /// Kemna-Vorst geometric Asian value when the option is unseasoned
    /// (`hist_count == 0`) and the fixings are equally spaced — the standard
    /// closed form is the special case of the seasoned formula.
    #[test]
    fn w07_seasoned_geometric_control_reduces_to_kemna_vorst_unseasoned() {
        let spot = 100.0;
        let strike = 100.0;
        let r = 0.05_f64;
        let q = 0.0;
        let sigma = 0.20;
        let t = 1.0_f64;
        let n = 12usize;
        let df = (-r * t).exp();

        // Equally spaced fixings t_i = i*T/n, i = 1..n.
        let future_times: Vec<f64> = (1..=n).map(|i| i as f64 * t / n as f64).collect();

        let seasoned_call = seasoned_geometric_asian_control(
            spot,
            strike,
            r,
            q,
            sigma,
            df,
            0.0,
            0,
            &future_times,
            true,
            None,
            None,
        );
        let kv_call = geometric_asian_call(spot, strike, t, r, q, sigma, n);
        assert!(
            (seasoned_call - kv_call).abs() < 1e-9,
            "unseasoned seasoned-control call {seasoned_call} must equal \
             Kemna-Vorst {kv_call}"
        );

        let seasoned_put = seasoned_geometric_asian_control(
            spot,
            strike,
            r,
            q,
            sigma,
            df,
            0.0,
            0,
            &future_times,
            false,
            None,
            None,
        );
        let kv_put = geometric_asian_put(spot, strike, t, r, q, sigma, n);
        assert!(
            (seasoned_put - kv_put).abs() < 1e-9,
            "unseasoned seasoned-control put {seasoned_put} must equal \
             Kemna-Vorst {kv_put}"
        );
    }

    /// W-07: with past fixings folded in, the seasoned geometric control must
    /// stay finite, non-negative, and respond correctly to the realized
    /// geometric average — high past fixings lift a call's value.
    #[test]
    fn w07_seasoned_geometric_control_uses_history() {
        let spot = 100.0;
        let strike = 100.0;
        let r = 0.05;
        let q = 0.0;
        let sigma = 0.20;
        let df = (-r * 0.5_f64).exp();
        // Three future fixings in the back half of the year.
        let future_times = vec![0.6, 0.8, 1.0];

        // hist_prod_log = Σ ln(S_past) for two past fixings.
        let low_hist = 2.0 * 90.0_f64.ln();
        let high_hist = 2.0 * 130.0_f64.ln();

        let call_low = seasoned_geometric_asian_control(
            spot,
            strike,
            r,
            q,
            sigma,
            df,
            low_hist,
            2,
            &future_times,
            true,
            None,
            None,
        );
        let call_high = seasoned_geometric_asian_control(
            spot,
            strike,
            r,
            q,
            sigma,
            df,
            high_hist,
            2,
            &future_times,
            true,
            None,
            None,
        );

        assert!(call_low.is_finite() && call_low >= 0.0);
        assert!(call_high.is_finite() && call_high >= 0.0);
        assert!(
            call_high > call_low,
            "a seasoned geometric Asian call with higher realized past fixings \
             ({call_high}) must be worth more than one with lower past fixings \
             ({call_low})"
        );
    }

    #[test]
    fn coincident_model_times_preserve_all_asian_fixings() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2025, 1, 31);
        let curves = market(as_of, 100.0, 0.0, 0.0, 0.0);
        for averaging in [AveragingMethod::Arithmetic, AveragingMethod::Geometric] {
            for option_type in [OptionType::Call, OptionType::Put] {
                let strike = if option_type == OptionType::Call {
                    95.0
                } else {
                    105.0
                };
                let mut option = asian_option(
                    averaging,
                    option_type,
                    expiry,
                    strike,
                    vec![as_of, date(2025, 1, 30), expiry],
                );
                option.day_count = DayCount::ThirtyE360;
                option.past_fixings = vec![(as_of, 90.0)];
                option.instrument_pricing_overrides.model_config.mc_paths = Some(16);
                let average = match averaging {
                    AveragingMethod::Arithmetic => (90.0 + 100.0 + 100.0) / 3.0,
                    AveragingMethod::Geometric => {
                        ((90.0_f64.ln() + 2.0 * 100.0_f64.ln()) / 3.0).exp()
                    }
                };
                let expected = if option_type == OptionType::Call {
                    average - strike
                } else {
                    strike - average
                };
                let actual = option.npv_mc(&curves, as_of).expect("MC price").amount();
                assert!(
                    (actual - expected).abs() < 1e-10,
                    "{averaging:?} {option_type:?}: {actual} vs {expected}"
                );
            }
        }
    }

    #[test]
    fn asian_grid_contains_exact_close_fixings_and_equal_time_weights() {
        let as_of = date(2025, 1, 1);
        let dates = [date(2025, 12, 30), date(2025, 12, 31)];
        let grid = map_fixings_to_steps(&dates, DayCount::Act365F, as_of, 1.0, 2).expect("grid");
        for (&fixing, &time) in dates.iter().zip(grid.future_times().iter()) {
            let expected = DayCount::Act365F
                .year_fraction(as_of, fixing, DayCountContext::default())
                .expect("time");
            assert!((time - expected).abs() < 1e-14);
        }
        let expiry = date(2025, 1, 31);
        let t = DayCount::ThirtyE360
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("time");
        let grid = map_fixings_to_steps(
            &[date(2025, 1, 30), expiry],
            DayCount::ThirtyE360,
            as_of,
            t,
            2,
        )
        .expect("grid");
        assert_eq!(grid.fixing_steps.len(), 2);
        assert_eq!(grid.fixing_steps[0], grid.fixing_steps[1]);
    }

    #[test]
    fn geometric_asian_uses_dated_forward_and_separate_payment_discount() {
        let as_of = date(2025, 1, 1);
        let fixing = date(2025, 7, 2);
        let expiry = date(2026, 1, 1);
        let mut option = asian_option(
            AveragingMethod::Geometric,
            OptionType::Call,
            expiry,
            95.0,
            vec![fixing],
        );
        option.instrument_pricing_overrides.model_config.mc_paths = Some(16);
        for terminal_df in [0.9, 0.8] {
            let curve = DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (0.5, 1.0), (1.0, terminal_df), (2.0, 0.7)])
                .build()
                .expect("curve");
            let curves = market(as_of, 100.0, 0.0, 0.0, 0.0).insert(curve);
            let expected = 5.0 * terminal_df;
            let actual = AsianOptionAnalyticalGeometricPricer::new()
                .price_dyn(&option, &curves, as_of)
                .expect("analytical")
                .value
                .amount();
            let mc = option.npv_mc(&curves, as_of).expect("MC").amount();
            assert!((actual - expected).abs() < 1e-10, "{actual} vs {expected}");
            assert!((mc - expected).abs() < 1e-10, "{mc} vs {expected}");
        }
    }

    #[test]
    fn coincident_asian_times_retain_each_dated_forward() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2025, 1, 31);
        let earlier = date(2025, 1, 30);
        let curves = market(as_of, 100.0, 0.0, 0.05, 0.0);
        let discount = curves.get_discount("USD-OIS").expect("curve");
        let first = 100.0 / discount.df_between_dates(as_of, earlier).expect("first DF");
        let second = 100.0 / discount.df_between_dates(as_of, expiry).expect("second DF");
        let payment_df = discount
            .df_between_dates(as_of, expiry)
            .expect("payment DF");
        assert_ne!(first, second);
        for averaging in [AveragingMethod::Arithmetic, AveragingMethod::Geometric] {
            let mut option = asian_option(
                averaging,
                OptionType::Call,
                expiry,
                95.0,
                vec![as_of, earlier, expiry],
            );
            option.day_count = DayCount::ThirtyE360;
            option.past_fixings = vec![(as_of, 90.0)];
            option.instrument_pricing_overrides.model_config.mc_paths = Some(16);
            let average = match averaging {
                AveragingMethod::Arithmetic => (90.0 + first + second) / 3.0,
                AveragingMethod::Geometric => {
                    ((90.0_f64.ln() + first.ln() + second.ln()) / 3.0).exp()
                }
            };
            let expected = (average - 95.0) * payment_df;
            let actual = option.npv_mc(&curves, as_of).expect("MC").amount();
            assert!(
                (actual - expected).abs() < 1e-10,
                "{averaging:?}: {actual} vs {expected}"
            );
        }
    }

    #[test]
    fn heston_asian_preserves_coincident_dates_and_dated_forwards() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2025, 1, 31);
        let earlier = date(2025, 1, 30);
        let curves = market(as_of, 100.0, 0.0, 0.05, 0.0)
            .insert_price("HESTON_KAPPA", MarketScalar::Unitless(2.0))
            .insert_price("HESTON_THETA", MarketScalar::Unitless(1e-20))
            .insert_price("HESTON_SIGMA_V", MarketScalar::Unitless(1e-12))
            .insert_price("HESTON_RHO", MarketScalar::Unitless(0.0))
            .insert_price("HESTON_V0", MarketScalar::Unitless(1e-20));
        let discount = curves.get_discount("USD-OIS").expect("curve");
        let first = 100.0 / discount.df_between_dates(as_of, earlier).expect("first DF");
        let second = 100.0 / discount.df_between_dates(as_of, expiry).expect("second DF");
        let payment_df = discount
            .df_between_dates(as_of, expiry)
            .expect("payment DF");
        let pricer = crate::instruments::exotics::asian_option::heston_mc_pricer::AsianOptionHestonMcPricer::new();
        for averaging in [AveragingMethod::Arithmetic, AveragingMethod::Geometric] {
            let mut option = asian_option(
                averaging,
                OptionType::Call,
                expiry,
                95.0,
                vec![as_of, earlier, expiry],
            );
            option.day_count = DayCount::ThirtyE360;
            option.past_fixings = vec![(as_of, 90.0)];
            option.instrument_pricing_overrides.model_config.mc_paths = Some(16);
            let average = match averaging {
                AveragingMethod::Arithmetic => (90.0 + first + second) / 3.0,
                AveragingMethod::Geometric => {
                    ((90.0_f64.ln() + first.ln() + second.ln()) / 3.0).exp()
                }
            };
            let expected = (average - 95.0) * payment_df;
            let actual = pricer
                .price_dyn(&option, &curves, as_of)
                .expect("Heston MC")
                .value
                .amount();
            assert!(
                (actual - expected).abs() < 1e-6,
                "{averaging:?}: {actual} vs {expected}"
            );
        }
    }

    #[test]
    fn positive_vol_coincident_fixing_control_matches_log_moment_oracle_and_mc() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2025, 1, 31);
        let first = date(2025, 1, 30);
        let sigma = 0.35;
        let mut option = asian_option(
            AveragingMethod::Geometric,
            OptionType::Call,
            expiry,
            95.0,
            vec![as_of, first, expiry],
        );
        option.day_count = DayCount::ThirtyE360;
        option.past_fixings = vec![(as_of, 90.0)];
        let curves = market(as_of, 100.0, sigma, 0.04, 0.0);
        let discount = curves.get_discount("USD-OIS").expect("curve");
        let df = discount
            .df_between_dates(as_of, expiry)
            .expect("payment DF");
        let f1 = 100.0 / discount.df_between_dates(as_of, first).expect("first DF");
        let f2 = 100.0 / df;
        let t = 29.0 / 360.0;
        // Both future fixings share the same Brownian increment: their sum
        // has variance 4*sigma^2*T, rather than 2*sigma^2*T.
        let variance = 4.0 * sigma * sigma * t / 9.0;
        let mean_log = (90.0_f64.ln() + f1.ln() + f2.ln()) / 3.0 - sigma * sigma * t / 3.0;
        let expected_g = (mean_log + 0.5 * variance).exp();
        let d2 = (mean_log - option.strike.ln()) / variance.sqrt();
        let expected = df
            * (expected_g * finstack_quant_core::math::norm_cdf(d2 + variance.sqrt())
                - option.strike * finstack_quant_core::math::norm_cdf(d2));
        let grid = map_fixings_to_steps(&option.fixing_dates, option.day_count, as_of, t, 2)
            .expect("grid");
        let times = grid.future_times();
        let (drift, multipliers) =
            asian_drift_and_fixing_multipliers(&option, &curves, as_of, 0.0).expect("carry");
        let control = seasoned_geometric_asian_control(
            100.0,
            option.strike,
            -df.ln() / t,
            0.0,
            sigma,
            df,
            90.0_f64.ln(),
            1,
            &times,
            true,
            Some(&drift),
            Some(&multipliers),
        );
        assert!(
            (control - expected).abs() < 1e-12,
            "control={control}, oracle={expected}"
        );
        let process = GbmProcess::new(GbmParams::new(-df.ln() / t, 0.0, sigma).expect("GBM"))
            .with_drift_schedule(std::sync::Arc::new(drift));
        let payoff = AsianCall::with_history(
            option.strike,
            1.0,
            AveragingMethod::Geometric,
            grid.fixing_steps,
            90.0,
            90.0_f64.ln(),
            1,
        )
        .expect("payoff")
        .with_fixing_multipliers(&multipliers)
        .expect("dated scaling");
        let config = PathDependentPricerConfig {
            num_paths: 40_000,
            seed: 20260930,
            use_parallel: false,
            use_sobol: false,
            antithetic: false,
            ..Default::default()
        };
        let result = PathDependentPricer::new(config)
            .price_with_grid(&process, 100.0, grid.time_grid, &payoff, Currency::USD, df)
            .expect("MC");
        assert!(
            (result.mean.amount() - expected).abs() < 5.0 * result.stderr,
            "MC={}, oracle={expected}, sampling stderr={}",
            result.mean.amount(),
            result.stderr
        );
    }

    #[test]
    fn nonflat_multifixing_geometric_asian_matches_joint_normal_moment_oracle() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2026, 1, 1);
        let t1 = 91.0 / 365.0;
        let t2 = 273.0 / 365.0;
        let (sigma, q) = (0.30, 0.01);
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (t1, 0.99), (t2, 0.90), (1.0, 0.85)])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("curve");
        let curves = market(as_of, 100.0, sigma, 0.0, q).insert(curve);
        // For two distinct observations, Cov(log S1, log S2)=sigma^2*t1.
        let f1 = 100.0 * (-q * t1).exp() / 0.99;
        let f2 = 100.0 * (-q * t2).exp() / 0.90;
        let mean_log = 0.5 * (f1.ln() + f2.ln()) - 0.25 * sigma * sigma * (t1 + t2);
        let variance = 0.25 * sigma * sigma * (3.0 * t1 + t2);
        let expected_g = (mean_log + 0.5 * variance).exp();
        let d2 = (mean_log - 100.0_f64.ln()) / variance.sqrt();
        let d1 = d2 + variance.sqrt();
        for option_type in [OptionType::Call, OptionType::Put] {
            let option = asian_option(
                AveragingMethod::Geometric,
                option_type,
                expiry,
                100.0,
                vec![date(2025, 4, 2), date(2025, 10, 1)],
            );
            let expected = 0.85
                * match option_type {
                    OptionType::Call => {
                        expected_g * finstack_quant_core::math::norm_cdf(d1)
                            - 100.0 * finstack_quant_core::math::norm_cdf(d2)
                    }
                    OptionType::Put => {
                        100.0 * finstack_quant_core::math::norm_cdf(-d2)
                            - expected_g * finstack_quant_core::math::norm_cdf(-d1)
                    }
                };
            let actual = AsianOptionAnalyticalGeometricPricer::new()
                .price_dyn(&option, &curves, as_of)
                .expect("analytic")
                .value
                .amount();
            assert!(
                (actual - expected).abs() < 1e-11,
                "{option_type:?}: {actual} vs {expected}"
            );
        }
    }
}
