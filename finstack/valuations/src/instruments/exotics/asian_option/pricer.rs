//! Asian option pricers (Monte Carlo and analytical).

// Common imports for all pricers
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::exotics::asian_option::types::AsianOption;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, PricingResult,
};
use crate::results::ValuationResult;
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

// MC-specific imports
use finstack_monte_carlo::engine::PathCaptureConfig;
use finstack_monte_carlo::payoff::asian::{AsianCall, AsianPut};
use finstack_monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_monte_carlo::process::gbm::{GbmParams, GbmProcess};
use finstack_monte_carlo::results::MoneyEstimate;
use finstack_monte_carlo::variance_reduction::control_variate::apply_control_variate;

/// Result of mapping an Asian option's fixing dates onto a uniform MC time
/// grid.
pub(super) struct FixingGrid {
    /// Number of uniform time steps for the MC simulation.
    pub(super) num_steps: usize,
    /// Distinct, sorted grid step indices — one per future fixing date.
    pub(super) fixing_steps: Vec<usize>,
}

impl FixingGrid {
    /// Effective future-fixing times on the MC grid: step `k` corresponds to
    /// the simulated time `k · T / num_steps`. These are the times the MC
    /// actually samples the spot at — and the times the analytic control
    /// variate must use to stay consistent with the simulation.
    fn future_times(&self, t: f64) -> Vec<f64> {
        if self.num_steps == 0 {
            return Vec::new();
        }
        let dt = t / self.num_steps as f64;
        self.fixing_steps
            .iter()
            .map(|&step| step as f64 * dt)
            .collect()
    }
}

/// Map an Asian option's future fixing dates onto a uniform MC time grid,
/// guaranteeing **one distinct grid step per fixing date** (W-04).
///
/// The naive mapping `step = round(t_i / T · num_steps)` followed by `dedup()`
/// silently merges two distinct fixings whenever the grid is too coarse to
/// resolve the gap between them — the MC then averages over fewer observations
/// than the contract specifies. This helper refines the grid (increases
/// `num_steps`) until every future fixing rounds to a distinct step; if the
/// fixings are pathologically close it falls back to assigning consecutive
/// steps so the count is still preserved.
///
/// Returns the chosen `num_steps` and the sorted distinct step indices. The
/// length of `fixing_steps` is the **effective** future-fixing count that MC
/// actually averages over (W-06) — it excludes fixings at or before `as_of`.
pub(super) fn map_fixings_to_distinct_steps(
    fixing_dates: &[Date],
    day_count: finstack_core::dates::DayCount,
    as_of: Date,
    t: f64,
    base_num_steps: usize,
) -> finstack_core::Result<FixingGrid> {
    // Collect future fixing times (strictly after as_of, on or before expiry).
    let mut fixing_times: Vec<f64> = Vec::new();
    for &fixing_date in fixing_dates {
        let fixing_t = day_count.year_fraction(as_of, fixing_date, DayCountContext::default())?;
        if fixing_t > 0.0 && fixing_t <= t {
            fixing_times.push(fixing_t);
        }
    }
    fixing_times.sort_by(|a, b| a.total_cmp(b));

    let n_fixings = fixing_times.len();
    if n_fixings == 0 || t <= 0.0 {
        return Ok(FixingGrid {
            num_steps: base_num_steps.max(1),
            fixing_steps: Vec::new(),
        });
    }

    // The grid must have at least one step per fixing.
    let mut num_steps = base_num_steps.max(n_fixings);

    // Round each fixing time onto the grid; refine until all steps are
    // distinct. Cap the refinement so a degenerate schedule cannot blow up the
    // grid size.
    let max_steps = base_num_steps.saturating_mul(8).max(n_fixings * 4).max(16);
    let round_steps = |num_steps: usize| -> Vec<usize> {
        fixing_times
            .iter()
            .map(|&ft| {
                let step = (ft / t * num_steps as f64).round() as usize;
                step.clamp(1, num_steps)
            })
            .collect::<Vec<_>>()
    };

    loop {
        let steps = round_steps(num_steps);
        let mut distinct = steps.clone();
        distinct.sort_unstable();
        distinct.dedup();
        if distinct.len() == n_fixings {
            return Ok(FixingGrid {
                num_steps,
                fixing_steps: distinct,
            });
        }
        if num_steps >= max_steps {
            break;
        }
        num_steps = (num_steps * 2).min(max_steps);
    }

    // Fallback: fixings too close to resolve even on the refined grid. Assign
    // consecutive steps so the effective fixing count is still preserved
    // (W-04/W-06). Anchor on the rounded positions, then push collisions to the
    // next free slot.
    num_steps = max_steps.max(n_fixings);
    let rounded = round_steps(num_steps);
    let mut assigned: Vec<usize> = Vec::with_capacity(n_fixings);
    let mut next_free = 1usize;
    for &want in &rounded {
        let slot = want.max(next_free).min(num_steps);
        assigned.push(slot);
        next_free = slot + 1;
    }
    // If the tail overflowed past num_steps, repack from the top.
    if assigned.last().copied().unwrap_or(0) > num_steps {
        for (i, slot) in assigned.iter_mut().enumerate() {
            *slot = num_steps - (n_fixings - 1 - i);
        }
    }
    Ok(FixingGrid {
        num_steps,
        fixing_steps: assigned,
    })
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
    let mut acc = finstack_core::math::NeumaierAccumulator::new();
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
    let mut acc = finstack_core::math::NeumaierAccumulator::new();
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
    let mut acc = finstack_core::math::NeumaierAccumulator::new();
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
/// The drift uses `r` and discounting uses `df` so the value is consistent
/// with how the MC simulates and discounts (the unseasoned case reduces to the
/// standard Kemna-Vorst control). Returns `df · max(forward_intrinsic, 0)` in
/// the degenerate zero-variance case.
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
) -> f64 {
    let k = future_times.len();
    let n_total = hist_count + k;
    if n_total == 0 {
        return 0.0;
    }
    let n = n_total as f64;
    let ln_spot = spot.ln();
    let drift = r - q - 0.5 * sigma * sigma;

    // Mean of ln(G): fixed past contribution + future drift contribution.
    let mut mean_acc = finstack_core::math::NeumaierAccumulator::new();
    mean_acc.add(hist_prod_log);
    for &t_i in future_times {
        mean_acc.add(ln_spot + drift * t_i);
    }
    let mu = mean_acc.total() / n;

    // Variance of ln(G): σ² ΣᵢΣⱼ min(t_i,t_j) / N².
    let mut cov_acc = finstack_core::math::NeumaierAccumulator::new();
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
    let norm_cdf = finstack_core::math::norm_cdf;
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

    fn merged_path_config(&self, inst: &AsianOption) -> PathDependentPricerConfig {
        let mut c = self.config.clone();
        if let Some(n) = inst.pricing_overrides.model_config.mc_paths {
            if n > 0 {
                c.num_paths = n;
            }
        }
        c
    }

    /// Price an Asian option using Monte Carlo.
    fn price_internal(
        &self,
        inst: &AsianOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<Money> {
        // Get time to maturity
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
                // Fallback
                let spot_scalar = curves.get_price(&inst.spot_id)?;
                match spot_scalar {
                    finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                    finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
                }
            };

            let intrinsic = match inst.option_type {
                crate::instruments::OptionType::Call => (average - inst.strike).max(0.0),
                crate::instruments::OptionType::Put => (inst.strike - average).max(0.0),
            };
            return Ok(Money::new(
                intrinsic * inst.notional.amount(),
                inst.notional.currency(),
            ));
        }

        // Get discount curve
        let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;
        let discount_factor = disc_curve.df_between_dates(as_of, inst.expiry)?;
        // Keep drift consistent with date-based discounting: exp(-r * t) == DF(as_of, maturity).
        let r = if t > 0.0 && discount_factor > 0.0 {
            -discount_factor.ln() / t
        } else {
            0.0
        };

        // Get spot
        let spot_scalar = curves.get_price(&inst.spot_id)?;
        let spot = match spot_scalar {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        // Get dividend yield
        let q = crate::instruments::common_impl::helpers::resolve_optional_dividend_yield(
            curves,
            inst.div_yield_id.as_ref(),
        )?;

        // Get volatility (override → surface)
        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &inst.pricing_overrides.market_quotes,
            curves,
            inst.vol_surface_id.as_str(),
            t,
            inst.strike,
        )?;

        // Create GBM process
        let gbm_params = GbmParams::new(r, q, sigma)?;
        let process = GbmProcess::new(gbm_params);

        let base_cfg = self.merged_path_config(inst);

        // Map fixing dates to time steps, guaranteeing one distinct grid step
        // per fixing (W-04). The grid is refined as needed so no two distinct
        // fixings are silently merged.
        let steps_per_year = base_cfg.steps_per_year;
        let base_num_steps = ((t * steps_per_year).round() as usize).max(base_cfg.min_steps);
        let fixing_grid = map_fixings_to_distinct_steps(
            &inst.fixing_dates,
            inst.day_count,
            as_of,
            t,
            base_num_steps,
        )?;
        let num_steps = fixing_grid.num_steps;

        // Time-varying drift: project each MC step with the curve-implied
        // forward drift so per-fixing spots (which drive the Asian average)
        // are unbiased on a non-flat rate curve. On a flat curve this is
        // bit-equivalent to the constant `(r - q)` drift.
        let process = process.with_drift_schedule(std::sync::Arc::new(
            crate::instruments::common_impl::helpers::build_gbm_drift_schedule(
                disc_curve.as_ref(),
                r,
                q,
                t,
                num_steps,
            )?,
        ));

        // Effective future-fixing times on the MC grid. The seasoned geometric
        // control variate (W-07) is built on exactly these times, so its
        // implied fixing count is the *effective* future-fixing count — not
        // the contract total — which is the W-06 fix.
        let future_fixing_times = fixing_grid.future_times(t);
        let fixing_steps = fixing_grid.fixing_steps;

        // Create payoff
        let averaging = match inst.averaging_method {
            crate::instruments::exotics::asian_option::types::AveragingMethod::Arithmetic => {
                finstack_monte_carlo::payoff::asian::AveragingMethod::Arithmetic
            }
            crate::instruments::exotics::asian_option::types::AveragingMethod::Geometric => {
                finstack_monte_carlo::payoff::asian::AveragingMethod::Geometric
            }
        };

        // Derive deterministic seed from instrument ID and scenario
        use finstack_monte_carlo::seed;

        let seed = if let Some(ref scenario) = inst.pricing_overrides.metrics.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };

        // Create config with derived seed
        let mut config = base_cfg;
        config.seed = seed;

        // If arithmetic averaging, apply geometric-Asian control variate for variance reduction
        let result_money = match (inst.averaging_method, inst.option_type) {
            (
                crate::instruments::exotics::asian_option::types::AveragingMethod::Arithmetic,
                crate::instruments::OptionType::Call,
            ) => {
                // Use path capture to get per-path discounted payoffs for covariance
                let mut cfg_cap = config.clone();
                cfg_cap.path_capture = PathCaptureConfig::all().with_payoffs();
                let pricer_cap = PathDependentPricer::new(cfg_cap);

                // Arithmetic payoff
                let arith_payoff = AsianCall::with_history(
                    inst.strike,
                    inst.notional.amount(),
                    finstack_monte_carlo::payoff::asian::AveragingMethod::Arithmetic,
                    fixing_steps.clone(),
                    hist_sum,
                    hist_prod_log,
                    hist_count,
                );
                let arith_full = pricer_cap.price_with_paths(
                    &process,
                    spot,
                    t,
                    num_steps,
                    &arith_payoff,
                    inst.notional.currency(),
                    discount_factor,
                )?;

                // Geometric payoff (same RNG via same seed)
                let geom_payoff = AsianCall::with_history(
                    inst.strike,
                    inst.notional.amount(),
                    finstack_monte_carlo::payoff::asian::AveragingMethod::Geometric,
                    fixing_steps.clone(),
                    hist_sum,
                    hist_prod_log,
                    hist_count,
                );
                let geom_full = pricer_cap.price_with_paths(
                    &process,
                    spot,
                    t,
                    num_steps,
                    &geom_payoff,
                    inst.notional.currency(),
                    discount_factor,
                )?;

                // Extract per-path discounted payoffs
                // paths should be Some when path_capture is enabled in price_with_paths
                let xs: Vec<f64> = arith_full
                    .paths
                    .as_ref()
                    .ok_or_else(|| {
                        finstack_core::Error::Validation(
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
                        finstack_core::Error::Validation(
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
                let control_analytical = seasoned_geometric_asian_control(
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
                MoneyEstimate::from_estimate(adj, inst.notional.currency()).mean
            }
            (
                crate::instruments::exotics::asian_option::types::AveragingMethod::Arithmetic,
                crate::instruments::OptionType::Put,
            ) => {
                let mut cfg_cap = config.clone();
                cfg_cap.path_capture = PathCaptureConfig::all().with_payoffs();
                let pricer_cap = PathDependentPricer::new(cfg_cap);

                let arith_payoff = AsianPut::with_history(
                    inst.strike,
                    inst.notional.amount(),
                    finstack_monte_carlo::payoff::asian::AveragingMethod::Arithmetic,
                    fixing_steps.clone(),
                    hist_sum,
                    hist_prod_log,
                    hist_count,
                );
                let arith_full = pricer_cap.price_with_paths(
                    &process,
                    spot,
                    t,
                    num_steps,
                    &arith_payoff,
                    inst.notional.currency(),
                    discount_factor,
                )?;

                let geom_payoff = AsianPut::with_history(
                    inst.strike,
                    inst.notional.amount(),
                    finstack_monte_carlo::payoff::asian::AveragingMethod::Geometric,
                    fixing_steps.clone(),
                    hist_sum,
                    hist_prod_log,
                    hist_count,
                );
                let geom_full = pricer_cap.price_with_paths(
                    &process,
                    spot,
                    t,
                    num_steps,
                    &geom_payoff,
                    inst.notional.currency(),
                    discount_factor,
                )?;

                // paths should be Some when path_capture is enabled in price_with_paths
                let xs: Vec<f64> = arith_full
                    .paths
                    .as_ref()
                    .ok_or_else(|| {
                        finstack_core::Error::Validation(
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
                        finstack_core::Error::Validation(
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
                let control_analytical = seasoned_geometric_asian_control(
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
                MoneyEstimate::from_estimate(adj, inst.notional.currency()).mean
            }
            // Geometric averaging (no CV needed) or fallback path
            _ => {
                let pricer = PathDependentPricer::new(config);
                match inst.option_type {
                    crate::instruments::OptionType::Call => {
                        let payoff = AsianCall::with_history(
                            inst.strike,
                            inst.notional.amount(),
                            averaging,
                            fixing_steps,
                            hist_sum,
                            hist_prod_log,
                            hist_count,
                        );
                        pricer
                            .price(
                                &process,
                                spot,
                                t,
                                num_steps,
                                &payoff,
                                inst.notional.currency(),
                                discount_factor,
                            )?
                            .mean
                    }
                    crate::instruments::OptionType::Put => {
                        let payoff = AsianPut::with_history(
                            inst.strike,
                            inst.notional.amount(),
                            averaging,
                            fixing_steps,
                            hist_sum,
                            hist_prod_log,
                            hist_count,
                        );
                        pricer
                            .price(
                                &process,
                                spot,
                                t,
                                num_steps,
                                &payoff,
                                inst.notional.currency(),
                                discount_factor,
                            )?
                            .mean
                    }
                }
            }
        };

        Ok(result_money)
    }

    /// Price with LRM Greeks (delta, vega) convenience.
    #[allow(clippy::too_many_lines)]
    #[allow(dead_code)] // May be used by external bindings or tests
    pub(crate) fn price_with_lrm_greeks_internal(
        &self,
        inst: &AsianOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<(Money, Option<(f64, f64)>)> {
        // Reuse the same setup as price_internal
        let t = inst
            .day_count
            .year_fraction(as_of, inst.expiry, DayCountContext::default())?;

        let (hist_sum, hist_prod_log, hist_count) = inst.accumulated_state(as_of);

        if t <= 0.0 {
            // Expired, return 0 value/greeks (or intrinsic if we care about cashflows, but greeks usually 0)
            // For simplicity, we return 0 value if expired in LRM greeks context as sensitivity is 0.
            // Or should we return intrinsic? Since this is pricing + greeks, we should return value.
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
                    finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                    finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
                }
            };
            let intrinsic = match inst.option_type {
                crate::instruments::OptionType::Call => (average - inst.strike).max(0.0),
                crate::instruments::OptionType::Put => (inst.strike - average).max(0.0),
            };

            return Ok((
                Money::new(intrinsic * inst.notional.amount(), inst.notional.currency()),
                None,
            ));
        }

        let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;
        let discount_factor = disc_curve.df_between_dates(as_of, inst.expiry)?;
        // Keep drift consistent with date-based discounting: exp(-r * t) == DF(as_of, maturity).
        let r = if t > 0.0 && discount_factor > 0.0 {
            -discount_factor.ln() / t
        } else {
            0.0
        };

        let spot_scalar = curves.get_price(&inst.spot_id)?;
        let spot = match spot_scalar {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        let q = crate::instruments::common_impl::helpers::resolve_optional_dividend_yield(
            curves,
            inst.div_yield_id.as_ref(),
        )?;

        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &inst.pricing_overrides.market_quotes,
            curves,
            inst.vol_surface_id.as_str(),
            t,
            inst.strike,
        )?;

        let gbm_params = GbmParams::new(r, q, sigma)?;
        let process = GbmProcess::new(gbm_params);

        let base_cfg = self.merged_path_config(inst);

        let steps_per_year = base_cfg.steps_per_year;
        let base_num_steps = ((t * steps_per_year).round() as usize).max(base_cfg.min_steps);

        // Fixing steps mapping — one distinct grid step per fixing (W-04).
        let fixing_grid = map_fixings_to_distinct_steps(
            &inst.fixing_dates,
            inst.day_count,
            as_of,
            t,
            base_num_steps,
        )?;
        let num_steps = fixing_grid.num_steps;
        let fixing_steps = fixing_grid.fixing_steps;

        // Time-varying drift: project each MC step with the curve-implied
        // forward drift so per-fixing spots (which drive the Asian average)
        // are unbiased on a non-flat rate curve. On a flat curve this is
        // bit-equivalent to the constant `(r - q)` drift.
        let process = process.with_drift_schedule(std::sync::Arc::new(
            crate::instruments::common_impl::helpers::build_gbm_drift_schedule(
                disc_curve.as_ref(),
                r,
                q,
                t,
                num_steps,
            )?,
        ));

        let averaging = match inst.averaging_method {
            crate::instruments::exotics::asian_option::types::AveragingMethod::Arithmetic => {
                finstack_monte_carlo::payoff::asian::AveragingMethod::Arithmetic
            }
            crate::instruments::exotics::asian_option::types::AveragingMethod::Geometric => {
                finstack_monte_carlo::payoff::asian::AveragingMethod::Geometric
            }
        };

        // Seed handling
        use finstack_monte_carlo::seed;
        let seed = if let Some(ref scenario) = inst.pricing_overrides.metrics.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };
        let mut cfg = base_cfg;
        cfg.seed = seed;
        let pricer = PathDependentPricer::new(cfg);

        let (est, greeks) = match inst.option_type {
            crate::instruments::OptionType::Call => {
                let payoff = finstack_monte_carlo::payoff::asian::AsianCall::with_history(
                    inst.strike,
                    inst.notional.amount(),
                    averaging,
                    fixing_steps,
                    hist_sum,
                    hist_prod_log,
                    hist_count,
                );
                pricer.price_with_lrm_greeks(
                    &process,
                    spot,
                    t,
                    num_steps,
                    &payoff,
                    inst.notional.currency(),
                    discount_factor,
                    r,
                    q,
                    sigma,
                )?
            }
            crate::instruments::OptionType::Put => {
                let payoff = finstack_monte_carlo::payoff::asian::AsianPut::with_history(
                    inst.strike,
                    inst.notional.amount(),
                    averaging,
                    fixing_steps,
                    hist_sum,
                    hist_prod_log,
                    hist_count,
                );
                pricer.price_with_lrm_greeks(
                    &process,
                    spot,
                    t,
                    num_steps,
                    &payoff,
                    inst.notional.currency(),
                    discount_factor,
                    r,
                    q,
                    sigma,
                )?
            }
        };

        Ok((est.mean, greeks))
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
    ) -> PricingResult<ValuationResult> {
        let asian = instrument
            .as_any()
            .downcast_ref::<AsianOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::AsianOption, instrument.key())
            })?;

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
) -> finstack_core::Result<Money> {
    let pricer = AsianOptionMcPricer::new();
    pricer.price_internal(inst, curves, as_of)
}

/// Present value with LRM Greeks via Monte Carlo.
#[allow(dead_code)] // May be used by external bindings or tests
pub fn npv_with_lrm_greeks(
    inst: &AsianOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_core::Result<(Money, Option<(f64, f64)>)> {
    let pricer = AsianOptionMcPricer::new();
    pricer.price_with_lrm_greeks_internal(inst, curves, as_of)
}

// ========================= ANALYTICAL PRICERS =========================

use crate::instruments::common_impl::helpers::collect_black_scholes_inputs;
use crate::models::closed_form::asian::{
    arithmetic_asian_call_tw, arithmetic_asian_put_tw, geometric_asian_call, geometric_asian_put,
};

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
    ) -> PricingResult<ValuationResult> {
        let asian = instrument
            .as_any()
            .downcast_ref::<AsianOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::AsianOption, instrument.key())
            })?;

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
                Money::new(
                    intrinsic * asian.notional.amount(),
                    asian.notional.currency(),
                ),
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

        let price = match asian.option_type {
            crate::instruments::OptionType::Call => {
                geometric_asian_call(spot, asian.strike, t, r, q, sigma, total_fixings)
            }
            crate::instruments::OptionType::Put => {
                geometric_asian_put(spot, asian.strike, t, r, q, sigma, total_fixings)
            }
        };

        let pv = Money::new(price * asian.notional.amount(), asian.notional.currency());
        Ok(ValuationResult::stamped(asian.id(), as_of, pv))
    }
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
    ) -> PricingResult<ValuationResult> {
        use crate::instruments::common_impl::helpers::collect_black_scholes_inputs_df;

        let asian = instrument
            .as_any()
            .downcast_ref::<AsianOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::AsianOption, instrument.key())
            })?;

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
        // Derive r_eff for TW formula (which still needs a rate for moment calculations)
        let r = bs_inputs.r_eff();

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
                Money::new(
                    intrinsic * asian.notional.amount(),
                    asian.notional.currency(),
                ),
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
                Money::new(
                    payoff * df_expiry * asian.notional.amount(),
                    asian.notional.currency(),
                ),
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
            let unscaled = match asian.option_type {
                crate::instruments::OptionType::Call => {
                    arithmetic_asian_call_tw(spot, k_eff, t, r, q, sigma, future_fixings)
                }
                crate::instruments::OptionType::Put => {
                    arithmetic_asian_put_tw(spot, k_eff, t, r, q, sigma, future_fixings)
                }
            };
            unscaled * scale
        };

        let pv = Money::new(price * asian.notional.amount(), asian.notional.currency());
        Ok(ValuationResult::stamped(asian.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::exotics::asian_option::{AsianOption, AveragingMethod};
    use crate::instruments::OptionType;
    use crate::models::closed_form::asian::{geometric_asian_call, geometric_asian_put};
    use finstack_core::currency::Currency;
    use finstack_core::dates::{Date, DayCount, DayCountContext};
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
    use finstack_core::types::{CurveId, InstrumentId};
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
            .notional(Money::new(1.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(Default::default())
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
        let expected =
            geometric_asian_call(spot, strike, t, rate, div_yield, vol, fixing_dates.len());
        let expected_money = Money::new(expected, Currency::USD).amount();

        assert!((pv - expected_money).abs() < 1e-12);
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
        let expected =
            geometric_asian_put(spot, strike, t, rate, div_yield, vol, fixing_dates.len());
        let expected_money = Money::new(expected, Currency::USD).amount();

        assert!((pv - expected_money).abs() < 1e-12);
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
        let expected_money = Money::new(payoff * df, Currency::USD).amount();

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
    fn mc_pricer_expired_without_fixings_falls_back_to_spot() {
        let as_of = date(2025, 6, 30);
        let option = asian_option(
            AveragingMethod::Geometric,
            OptionType::Put,
            as_of,
            100.0,
            vec![date(2025, 3, 31), date(2025, 6, 30)],
        );

        let pv = AsianOptionMcPricer::new()
            .price_internal(&option, &market(as_of, 80.0, 0.20, 0.05, 0.0), as_of)
            .expect("expired MC price")
            .amount();

        assert!((pv - 20.0).abs() < 1e-12);
    }

    #[test]
    fn expired_lrm_wrapper_returns_intrinsic_and_no_greeks() {
        let as_of = date(2025, 6, 30);
        let fixing_dates = vec![date(2025, 5, 31), date(2025, 6, 30)];
        let mut option = asian_option(
            AveragingMethod::Arithmetic,
            OptionType::Put,
            as_of,
            100.0,
            fixing_dates.clone(),
        );
        option.past_fixings = fixing_dates.iter().copied().zip([95.0, 85.0]).collect();

        let (pv, greeks) =
            npv_with_lrm_greeks(&option, &market(as_of, 999.0, 0.20, 0.05, 0.0), as_of)
                .expect("expired lrm price");

        assert!((pv.amount() - 10.0).abs() < 1e-12);
        assert_eq!(greeks, None);
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
    fn analytical_geometric_expired_without_fixings_falls_back_to_spot() {
        let as_of = date(2025, 6, 30);
        let option = asian_option(
            AveragingMethod::Geometric,
            OptionType::Call,
            as_of,
            100.0,
            vec![date(2025, 3, 31), date(2025, 6, 30)],
        );

        let pv = AsianOptionAnalyticalGeometricPricer::new()
            .price_dyn(&option, &market(as_of, 125.0, 0.20, 0.05, 0.0), as_of)
            .expect("expired analytical price")
            .value
            .amount();

        assert!((pv - 25.0).abs() < 1e-12);
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
            .take(2)
            .copied()
            .zip([101.0, 103.0])
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
        let expected = Money::new(payoff * df, Currency::USD).amount();

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
    fn mc_pricer_expired_without_fixings_uses_price_scalar_spot_fallback() {
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
            MarketScalar::Price(Money::new(125.0, Currency::USD)),
        );

        let pv = AsianOptionMcPricer::new()
            .price_internal(&option, &market, as_of)
            .expect("expired MC price")
            .amount();

        assert!((pv - 25.0).abs() < 1e-12);
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

    /// W-04: distinct fixing dates must map to distinct grid steps. With a
    /// coarse base grid the naive `round()` mapping collapses fixings that are
    /// close together onto the same step; the helper must refine the grid (or
    /// fall back to consecutive steps) so the effective fixing count is never
    /// silently reduced.
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
        let grid = map_fixings_to_distinct_steps(&fixing_dates, DayCount::Act365F, as_of, t, 4)
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
                .all(|&s| s >= 1 && s <= grid.num_steps),
            "every fixing step must lie in [1, num_steps={}]",
            grid.num_steps
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
        let grid = map_fixings_to_distinct_steps(&fixing_dates, DayCount::Act365F, as_of, t, 252)
            .expect("fixing grid");
        assert_eq!(grid.fixing_steps.len(), 3);
        assert!(grid.num_steps >= 252);
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

    /// W-07: the seasoned arithmetic Asian MC must keep the geometric control
    /// variate rather than discarding it. The control-variate price must agree
    /// with a plain-MC reference within Monte Carlo error — i.e. the seasoning-
    /// aware control does not bias the estimate.
    #[test]
    fn w07_seasoned_arithmetic_asian_cv_matches_plain_mc() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2026, 1, 1);
        let fixing_dates = vec![
            date(2025, 4, 1),
            date(2025, 7, 1),
            date(2025, 10, 1),
            expiry,
        ];
        let mut option = asian_option(
            AveragingMethod::Arithmetic,
            OptionType::Call,
            expiry,
            100.0,
            fixing_dates.clone(),
        );
        // Season the option: the first fixing is already observed.
        option.past_fixings = vec![(date(2025, 4, 1), 103.0)];

        let market = market(as_of, 100.0, 0.20, 0.05, 0.0);

        // Control-variate price (seasoned path, W-07).
        let cv_pv = AsianOptionMcPricer::new()
            .price_internal(&option, &market, as_of)
            .expect("seasoned CV price")
            .amount();

        // Plain-MC reference: a geometric-averaging Asian shares the seasoned
        // accumulation path but takes the non-CV branch, so use a direct
        // arithmetic MC via the LRM wrapper (no control variate applied).
        let (plain_pv, _) = AsianOptionMcPricer::new()
            .price_with_lrm_greeks_internal(&option, &market, as_of)
            .expect("plain MC price");

        let diff = (cv_pv - plain_pv.amount()).abs();
        assert!(
            cv_pv.is_finite() && cv_pv > 0.0,
            "seasoned CV price must be finite and positive, got {cv_pv}"
        );
        // The CV is unbiased, so it must agree with plain MC within a few
        // standard errors. Both use the same seed-derived RNG.
        assert!(
            diff < 0.5,
            "seasoned arithmetic Asian control-variate price {cv_pv} must agree \
             with plain MC {} within MC error; diff={diff}",
            plain_pv.amount()
        );
    }
}
