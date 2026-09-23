use super::{
    BarrierCrossing, MertonMcConfig, MertonMcResult, PathStatistics, PikMode, PikSchedule,
};
use finstack_quant_core::math::random::{Pcg64Rng, RandomNumberGenerator};
use finstack_quant_core::{InputError, Result};
use finstack_quant_models::credit::{AssetDynamics, BarrierType, CreditState};
use smallvec::SmallVec;

/// Contractual terms of the bond leg simulated by [`MertonMcEngine`].
///
/// Times are on the bond model clock (years from the valuation date under the
/// bond coupon day count). Every remaining coupon is paid in full, so the
/// simulated PV is a DIRTY value; `accrued` converts it to clean.
#[derive(Debug, Clone)]
pub(crate) struct MertonBondTerms {
    /// Initial face amount in currency units.
    pub(crate) notional: f64,
    /// Annual coupon rate as a decimal.
    pub(crate) coupon_rate: f64,
    /// Horizon (final redemption) time in years; strictly positive.
    pub(crate) maturity_years: f64,
    /// Remaining coupons as `(payment time, accrual year fraction)`, ascending,
    /// with every payment time in `(0, maturity_years]`.
    pub(crate) coupons: Vec<(f64, f64)>,
    /// Accrued interest at the valuation date in currency units.
    pub(crate) accrued: f64,
}

/// Merton Monte Carlo pricing engine for PIK bonds.
pub struct MertonMcEngine;

impl MertonMcEngine {
    /// Price a bond with structural credit model via Monte Carlo.
    ///
    /// Asset paths evolve under the risk-neutral measure using the Merton
    /// model's `risk_free_rate` as drift (GBM only). Cashflows are discounted
    /// using `config.cashflow_dfs` when available, falling back to
    /// `exp(-discount_rate * t)`. The flat `discount_rate` is always used for
    /// the Merton risk-neutral drift.
    ///
    /// Per-coupon PIK behavior is controlled by `config.pik_schedule`:
    /// - `PikMode::Cash` → coupon paid in cash
    /// - `PikMode::Pik` → coupon accreted to notional
    /// - `PikMode::Split` → coupon split between cash and PIK
    /// - `PikMode::Toggle` → deferred to `config.toggle_model`
    ///
    /// Coupons sit on a regular grid anchored backward from maturity. When
    /// `maturity_years` is not a whole number of periods the valuation date
    /// falls inside the first period: that coupon is paid in full, so
    /// `dirty_price_pct` is the simulated PV and `clean_price_pct` subtracts
    /// the accrued interest for the elapsed part of the period.
    ///
    /// # Arguments
    ///
    /// * `notional` - Bond face amount in major currency units used as the repayment basis
    /// * `coupon_rate` - Annual coupon rate as a decimal (for example 0.08 for 8%)
    /// * `maturity_years` - Time to maturity in years under the pricing day-count;
    ///   must be positive
    /// * `coupon_frequency` - Number of coupon periods per year (for example 2 for
    ///   semi-annual); must be at least 1
    /// * `config` - Monte Carlo configuration including path count and PIK schedule
    /// * `discount_rate` - Continuous discount rate for Merton drift and the flat-rate
    ///   fallback when `cashflow_dfs` is not set
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid (e.g., non-GBM
    /// dynamics, invalid PIK split fractions).
    pub fn price(
        notional: f64,
        coupon_rate: f64,
        maturity_years: f64,
        coupon_frequency: usize,
        config: &MertonMcConfig,
        discount_rate: f64,
    ) -> Result<MertonMcResult> {
        if coupon_frequency == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "Merton MC requires coupon_frequency >= 1".to_string(),
            ));
        }
        let coupons = Self::coupon_schedule(maturity_years, coupon_frequency);
        let period = 1.0 / coupon_frequency as f64;
        let elapsed = coupons
            .first()
            .map_or(0.0, |&(t_first, _)| (period - t_first).max(0.0));
        let terms = MertonBondTerms {
            notional,
            coupon_rate,
            maturity_years,
            coupons,
            accrued: notional * coupon_rate * elapsed,
        };
        Self::price_terms(&terms, config, discount_rate)
    }

    /// Price explicit bond terms (coupon grid, horizon and accrued).
    ///
    /// This is the engine behind [`MertonMcEngine::price`] and
    /// `Bond::price_merton_mc`; see [`MertonBondTerms`] for the inputs.
    pub(crate) fn price_terms(
        terms: &MertonBondTerms,
        config: &MertonMcConfig,
        discount_rate: f64,
    ) -> Result<MertonMcResult> {
        let (notional, coupon_rate, maturity_years) =
            (terms.notional, terms.coupon_rate, terms.maturity_years);
        if !matches!(config.merton.dynamics(), AssetDynamics::GeometricBrownian) {
            return Err(InputError::Invalid.into());
        }
        Self::validate_pik_schedule(&config.pik_schedule)?;
        // A single path has no variance estimate (division by n-1 = 0) and
        // no meaningful MC statistics.
        if config.num_paths < 2 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Merton MC requires num_paths >= 2 (got {}); a single path has \
                 no variance estimate",
                config.num_paths
            )));
        }
        // A non-positive maturity yields `dt = maturity_years / total_steps <= 0`
        // (with `total_steps` floored at 1), so `sqrt_dt` becomes 0 or NaN and the
        // simulation degenerates into zero-variance paths with a divide-by-zero in
        // the Brownian-bridge crossing probability — returning a garbage price
        // rather than an error. Expired / same-day bonds must be rejected here (the
        // calibration path already guards this); price them as PV = 0 upstream.
        if !maturity_years.is_finite() || maturity_years <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Merton MC requires maturity_years > 0 (got {maturity_years}); \
                 an expired or same-day bond has PV = 0 and cannot be simulated"
            )));
        }
        if !notional.is_finite() || notional <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Merton MC requires a positive finite notional (got {notional})"
            )));
        }

        let num_paths = config.num_paths;
        // Size the grid so it ends EXACTLY at maturity: rounding the step
        // count against a fixed dt = 1/steps_per_year left stub maturities
        // with a horizon ≠ maturity (coupons placed beyond the simulated
        // horizon, first-passage defaults triggering after maturity).
        let total_steps = (maturity_years * config.time_steps_per_year as f64)
            .ceil()
            .max(1.0) as usize;
        let dt = maturity_years / total_steps as f64;
        let sqrt_dt = dt.sqrt();
        let sigma = config.merton.asset_vol();
        let r = config.merton.risk_free_rate();
        let mu = r - config.merton.payout_rate() - 0.5 * sigma * sigma;

        // Remaining coupon grid, shared with the risk-free leg.
        let coupon_schedule = terms.coupons.as_slice();
        let num_coupons = coupon_schedule.len();

        let dfs_ref = config.cashflow_dfs.as_deref();

        // Barrier parameters
        let debt_barrier = config.merton.debt_barrier();
        let (barrier_type, barrier_growth_rate) = match config.merton.barrier_type() {
            BarrierType::FirstPassage {
                barrier_growth_rate,
            } => (
                BarrierType::FirstPassage {
                    barrier_growth_rate: *barrier_growth_rate,
                },
                *barrier_growth_rate,
            ),
            BarrierType::Terminal => (BarrierType::Terminal, 0.0),
        };

        // Determine how many base paths (for antithetic)
        let n_base = if config.antithetic {
            num_paths.div_ceil(2)
        } else {
            num_paths
        };

        let mut path_pvs: Vec<f64> = Vec::with_capacity(num_paths);

        // Accumulators for statistics
        let mut total_defaults: usize = 0;
        let mut total_default_time: f64 = 0.0;
        let mut total_terminal_notional: f64 = 0.0;
        let mut total_recovery_pct: f64 = 0.0;
        let mut total_pik_elections: usize = 0;
        let mut total_coupon_periods: usize = 0;
        let mut surviving_paths: usize = 0;

        // Random-draw buffers are created once per worker thread inside the
        // parallel `map_init` below and reused across the paths that thread
        // handles, so there is still no per-path buffer allocation.

        // First-passage barriers depend only on the time step (deterministic in
        // `t`), so precompute them once rather than evaluating two `exp()` per step
        // per path. Empty for `Terminal`, which uses the flat `debt_barrier`.
        let first_passage_barriers: Vec<f64> = match barrier_type {
            BarrierType::FirstPassage { .. } => (0..=total_steps)
                .map(|i| debt_barrier * (barrier_growth_rate * (i as f64 * dt)).exp())
                .collect(),
            BarrierType::Terminal => Vec::new(),
        };

        // The base-path loop is embarrassingly parallel: every path draws from
        // an independent per-path RNG stream (`new_with_stream(seed, path_idx)`).
        // We simulate the paths concurrently and collect one small vector of
        // per-leg outcomes per base path. `into_par_iter` over a range is
        // order-preserving, and the outcomes are folded back into the running
        // accumulators on a single thread in the exact serial order below — so
        // every statistic, including the path-PV sum that sets the price, stays
        // bit-identical to the serial implementation regardless of thread count.
        #[derive(Default)]
        struct LegOutcome {
            pv: f64,
            defaulted_at: Option<f64>,
            recovery_pct: f64,
            survived: bool,
            terminal_notional: f64,
            pik_elections: usize,
            coupon_periods: usize,
        }

        let init_bufs = || {
            (
                vec![0.0f64; total_steps],
                vec![0.0f64; total_steps],
                vec![0.0f64; num_coupons],
            )
        };
        let simulate_base =
            |(normals, uniforms, toggle_uniforms): &mut (Vec<f64>, Vec<f64>, Vec<f64>),
             path_idx| {
                // Per-path RNG for determinism
                let mut rng = Pcg64Rng::new_with_stream(config.seed, path_idx as u64);

                // Fill per-thread buffers with random draws so that antithetic
                // pairs share identical randomness (normals are sign-flipped;
                // uniforms are reused).
                for n in normals.iter_mut() {
                    *n = rng.normal(0.0, 1.0);
                }
                // Brownian-bridge crossing checks
                for u in uniforms.iter_mut() {
                    *u = rng.uniform();
                }
                // Toggle decision draws — one per coupon date, shared across the
                // antithetic pair to preserve variance-reduction symmetry.
                for tu in toggle_uniforms.iter_mut() {
                    *tu = rng.uniform();
                }

                // Antithetic decision derived purely from `path_idx`. With
                // `n_base = num_paths.div_ceil(2)`, only the final base path
                // (when `num_paths` is odd) emits a single leg, so the serial
                // `path_pvs.len() + 1 < num_paths` test is exactly
                // `2 * path_idx + 1 < num_paths`.
                let signs: &[f64] = if config.antithetic && 2 * path_idx + 1 < num_paths {
                    &[1.0, -1.0]
                } else {
                    &[1.0]
                };

                let mut legs: SmallVec<[LegOutcome; 2]> = SmallVec::new();
                for &sign in signs {
                    let mut leg = LegOutcome::default();
                    let mut v = config.merton.asset_value();
                    let mut n_current = notional;
                    let mut defaulted = false;
                    let mut path_pv = 0.0;
                    let mut path_pik_elections: usize = 0;
                    let mut path_coupon_periods: usize = 0;
                    let mut coupon_idx: usize = 0;

                    for (step, &normal_draw) in normals.iter().enumerate().take(total_steps) {
                        let t = (step + 1) as f64 * dt;
                        let z = normal_draw * sign;
                        // For the antithetic path (sign = -1) the trajectory is
                        // sign-flipped, so the crossing probability `p` is
                        // different.  Proper antithetic variance reduction requires
                        // the complementary uniform `1 - u` so that the two path
                        // legs remain negatively correlated and the variance
                        // reduction is not partially defeated by correlated
                        // default decisions.
                        let u = if sign < 0.0 {
                            1.0 - uniforms[step]
                        } else {
                            uniforms[step]
                        };

                        let v_prev = v;
                        let barrier_prev = match barrier_type {
                            BarrierType::FirstPassage { .. } => first_passage_barriers[step],
                            BarrierType::Terminal => debt_barrier,
                        };

                        // 1. Evolve asset value (GBM)
                        v *= (mu * dt + sigma * sqrt_dt * z).exp();

                        // 2. Check default
                        match barrier_type {
                            BarrierType::Terminal => {
                                let is_final_step = step + 1 == total_steps;
                                if is_final_step && v < debt_barrier {
                                    let recovery_rate = config
                                        .dynamic_recovery
                                        .as_ref()
                                        .map_or(config.default_recovery_rate, |dr| {
                                            dr.recovery_at_notional(n_current)
                                        });
                                    let recovery_cashflow = recovery_rate * n_current;
                                    let df = Self::df_at_time(dfs_ref, t, discount_rate);
                                    path_pv += recovery_cashflow * df;
                                    defaulted = true;
                                    leg.defaulted_at = Some(t);
                                    leg.recovery_pct = recovery_rate;
                                    break;
                                }
                            }
                            BarrierType::FirstPassage { .. } => {
                                let barrier = first_passage_barriers[step + 1];
                                let crossed = if v < barrier {
                                    true
                                } else if matches!(
                                    config.barrier_crossing,
                                    BarrierCrossing::BrownianBridge
                                ) && sigma > 0.0
                                    && dt > 0.0
                                {
                                    let barrier_now = barrier;
                                    let x0 = (v_prev / barrier_prev).ln();
                                    let x1 = (v / barrier_now).ln();
                                    if x0 > 0.0 && x1 > 0.0 {
                                        let denom = sigma * sigma * dt;
                                        let p = (-2.0 * x0 * x1 / denom).exp();
                                        u < p
                                    } else {
                                        true
                                    }
                                } else {
                                    false
                                };

                                if crossed {
                                    let recovery_rate = config
                                        .dynamic_recovery
                                        .as_ref()
                                        .map_or(config.default_recovery_rate, |dr| {
                                            dr.recovery_at_notional(n_current)
                                        });
                                    let recovery_cashflow = recovery_rate * n_current;
                                    let df = Self::df_at_time(dfs_ref, t, discount_rate);
                                    path_pv += recovery_cashflow * df;
                                    defaulted = true;
                                    leg.defaulted_at = Some(t);
                                    leg.recovery_pct = recovery_rate;
                                    break;
                                }
                            }
                        }

                        // 3. At coupon dates (using pre-computed schedule)
                        while coupon_idx < coupon_schedule.len()
                            && t >= coupon_schedule[coupon_idx].0 - dt * 0.5
                        {
                            let (coupon_t, accrual) = coupon_schedule[coupon_idx];
                            let coupon_amount = n_current * coupon_rate * accrual;
                            path_coupon_periods += 1;
                            let df = Self::df_at_time(dfs_ref, coupon_t, discount_rate);

                            match config.pik_schedule.mode_at(coupon_t) {
                                PikMode::Cash => {
                                    path_pv += coupon_amount * df;
                                }
                                PikMode::Pik => {
                                    n_current += coupon_amount;
                                    path_pik_elections += 1;
                                }
                                PikMode::Split {
                                    cash_fraction,
                                    pik_fraction,
                                } => {
                                    path_pv += coupon_amount * cash_fraction * df;
                                    n_current += coupon_amount * pik_fraction;
                                    if pik_fraction > 0.0 {
                                        path_pik_elections += 1;
                                    }
                                }
                                PikMode::Toggle => {
                                    if let Some(ref toggle) = config.toggle_model {
                                        let leverage = n_current / v;
                                        let hazard_rate =
                                            config.endogenous_hazard.as_ref().map_or_else(
                                                || {
                                                    let pd =
                                                        config.merton.default_probability(coupon_t);
                                                    if coupon_t > 0.0 {
                                                        -(1.0 - pd).ln() / coupon_t
                                                    } else {
                                                        0.0
                                                    }
                                                },
                                                |eh| eh.hazard_at_leverage(leverage),
                                            );
                                        let remaining = maturity_years - coupon_t;
                                        let dd = if sigma > 0.0 && remaining > 0.0 {
                                            let sqrt_remaining = remaining.sqrt();
                                            ((v / n_current).ln()
                                                + (r - config.merton.payout_rate()
                                                    - 0.5 * sigma * sigma)
                                                    * remaining)
                                                / (sigma * sqrt_remaining)
                                        } else {
                                            0.0
                                        };

                                        let state = CreditState {
                                            hazard_rate,
                                            distance_to_default: Some(dd),
                                            leverage,
                                            accreted_notional: n_current,
                                            coupon_due: coupon_amount,
                                            asset_value: Some(v),
                                        };

                                        let tu =
                                            toggle_uniforms.get(coupon_idx).copied().unwrap_or(0.5);
                                        if toggle.should_pik_with_uniform(&state, tu) {
                                            n_current += coupon_amount;
                                            path_pik_elections += 1;
                                        } else {
                                            path_pv += coupon_amount * df;
                                        }
                                    } else {
                                        path_pv += coupon_amount * df;
                                    }
                                }
                            }

                            coupon_idx += 1;
                        }
                    }

                    // 4. Terminal payment (if survived)
                    if !defaulted {
                        let df = Self::df_at_time(dfs_ref, maturity_years, discount_rate);
                        path_pv += n_current * df;
                        leg.survived = true;
                        leg.terminal_notional = n_current;
                    }

                    leg.pik_elections = path_pik_elections;
                    leg.coupon_periods = path_coupon_periods;
                    leg.pv = path_pv;
                    legs.push(leg);
                }
                legs
            };

        #[cfg(not(target_arch = "wasm32"))]
        let outcomes: Vec<SmallVec<[LegOutcome; 2]>> = {
            use rayon::prelude::*;
            (0..n_base)
                .into_par_iter()
                .map_init(init_bufs, simulate_base)
                .collect()
        };

        #[cfg(target_arch = "wasm32")]
        let outcomes: Vec<SmallVec<[LegOutcome; 2]>> = {
            let mut bufs = init_bufs();
            (0..n_base)
                .map(|path_idx| simulate_base(&mut bufs, path_idx))
                .collect()
        };

        // Serial, order-preserving fold into the running accumulators. This
        // reproduces the exact serial accumulation sequence (base path, then
        // leg, then statistic), so every aggregate — including the path-PV sum
        // and the default-time / recovery statistics — is bit-identical.
        for legs in outcomes {
            for leg in legs {
                if let Some(t) = leg.defaulted_at {
                    total_defaults += 1;
                    total_default_time += t;
                    total_recovery_pct += leg.recovery_pct;
                }
                if leg.survived {
                    surviving_paths += 1;
                    total_terminal_notional += leg.terminal_notional;
                }
                total_pik_elections += leg.pik_elections;
                total_coupon_periods += leg.coupon_periods;
                path_pvs.push(leg.pv);
            }
        }

        // Trim to exact num_paths in case antithetic generated extras
        path_pvs.truncate(num_paths);

        // Aggregate statistics
        let actual_paths = path_pvs.len() as f64;
        let mean_pv = path_pvs.iter().sum::<f64>() / actual_paths;
        let dirty_price_pct = mean_pv / notional * 100.0;
        let clean_price_pct = dirty_price_pct - terms.accrued / notional * 100.0;

        // PIK-aware risk-free PV for expected loss calculation
        let risk_free_pv =
            Self::risk_free_pv_spread(terms, discount_rate, &config.pik_schedule, dfs_ref, 0.0);
        let expected_loss = if risk_free_pv > 0.0 {
            1.0 - mean_pv / risk_free_pv
        } else {
            0.0
        };

        // Effective spread: solve for the constant spread s such that the
        // risk-free PV with every cashflow's discount factor bumped by
        // e^{-s·t} equals mean_pv. Solving on the SAME discount basis as
        // mean_pv (term-structure DFs when set) keeps curve shape out of the
        // spread.
        let effective_spread_bp = Self::solve_effective_spread(
            terms,
            discount_rate,
            mean_pv,
            &config.pik_schedule,
            dfs_ref,
        );

        // Unexpected loss (std dev of path PVs / notional) — a distribution
        // measure over individual paths, so it always uses the raw PVs.
        let variance = path_pvs
            .iter()
            .map(|&pv| (pv - mean_pv).powi(2))
            .sum::<f64>()
            / (actual_paths - 1.0);
        let std_dev = variance.sqrt();
        let unexpected_loss = std_dev / notional;

        // Standard error of the MEAN estimate. Antithetic legs are
        // negatively correlated by construction, so the i.i.d. samples are
        // the PAIR AVERAGES (adjacent in path order), not the individual
        // legs — treating 2N legs as independent misstates the SE.
        let standard_error = if config.antithetic {
            let pair_means: Vec<f64> = path_pvs
                .chunks(2)
                .map(|pair| pair.iter().sum::<f64>() / pair.len() as f64)
                .collect();
            let m = pair_means.len() as f64;
            let pair_mean = pair_means.iter().sum::<f64>() / m;
            let pair_var = if pair_means.len() > 1 {
                pair_means
                    .iter()
                    .map(|&x| (x - pair_mean).powi(2))
                    .sum::<f64>()
                    / (m - 1.0)
            } else {
                0.0
            };
            (pair_var / m).sqrt() / notional * 100.0
        } else {
            std_dev / actual_paths.sqrt() / notional * 100.0
        };

        // Expected shortfall at 95% (average of worst 5% of paths)
        // Sort in place — mean_pv and variance are already computed above.
        path_pvs.sort_by(|a, b| a.total_cmp(b));
        let cutoff = (0.05 * actual_paths).ceil() as usize;
        let cutoff = cutoff.max(1);
        let es_sum: f64 = path_pvs.iter().take(cutoff).sum();
        let expected_shortfall_95 = es_sum / cutoff as f64 / notional * 100.0;

        // Average PIK fraction
        let average_pik_fraction = if total_coupon_periods > 0 {
            total_pik_elections as f64 / total_coupon_periods as f64
        } else {
            0.0
        };

        // Path statistics
        let default_rate = total_defaults as f64 / actual_paths;
        let avg_default_time = if total_defaults > 0 {
            total_default_time / total_defaults as f64
        } else {
            0.0
        };
        let avg_terminal_notional = if surviving_paths > 0 {
            total_terminal_notional / surviving_paths as f64
        } else {
            notional
        };
        let avg_recovery_pct = if total_defaults > 0 {
            total_recovery_pct / total_defaults as f64
        } else {
            0.0
        };
        let pik_exercise_rate = average_pik_fraction;

        Ok(MertonMcResult {
            clean_price_pct,
            dirty_price_pct,
            expected_loss,
            unexpected_loss,
            expected_shortfall_95,
            average_pik_fraction,
            effective_spread_bp,
            path_statistics: PathStatistics {
                default_rate,
                avg_default_time,
                avg_terminal_notional,
                avg_recovery_pct,
                pik_exercise_rate,
            },
            num_paths: path_pvs.len(),
            standard_error,
        })
    }

    /// Validate PIK split fractions (non-negative, summing to 1) and, for
    /// `Stepped` schedules, that step times are finite and strictly
    /// ascending — `mode_at` does a linear scan that silently returns the
    /// wrong mode for out-of-order entries.
    fn validate_pik_schedule(schedule: &PikSchedule) -> Result<()> {
        let check = |mode: &PikMode| -> Result<()> {
            if let PikMode::Split {
                cash_fraction,
                pik_fraction,
            } = mode
            {
                if *cash_fraction < 0.0
                    || *pik_fraction < 0.0
                    || (*cash_fraction + *pik_fraction - 1.0).abs() > 1e-10
                {
                    return Err(InputError::Invalid.into());
                }
            }
            Ok(())
        };
        match schedule {
            PikSchedule::Uniform(mode) => check(mode)?,
            PikSchedule::Stepped(steps) => {
                let mut prev_t = f64::NEG_INFINITY;
                for &(t, ref mode) in steps {
                    if !t.is_finite() || t <= prev_t {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "PikSchedule::Stepped times must be finite and strictly \
                             ascending; entry at t = {t} follows t = {prev_t}"
                        )));
                    }
                    prev_t = t;
                    check(mode)?;
                }
            }
        }
        Ok(())
    }

    /// Log-linear interpolation of discount factors with flat-rate fallback.
    fn df_at_time(cashflow_dfs: Option<&[(f64, f64)]>, t: f64, flat_rate: f64) -> f64 {
        let Some(dfs) = cashflow_dfs.filter(|d| !d.is_empty()) else {
            return (-flat_rate * t).exp();
        };
        if t <= 0.0 {
            return 1.0;
        }
        if t <= dfs[0].0 {
            let (t0, df0) = dfs[0];
            if t0 > 0.0 {
                let r0 = -df0.ln() / t0;
                return (-r0 * t).exp();
            }
            return (-flat_rate * t).exp();
        }
        let last = dfs.len() - 1;
        if t >= dfs[last].0 {
            let (tn, dfn) = dfs[last];
            if tn > 0.0 {
                let rn = -dfn.ln() / tn;
                return (-rn * t).exp();
            }
            return (-flat_rate * t).exp();
        }
        let idx = dfs.partition_point(|&(ti, _)| ti < t);
        let (t0, df0) = dfs[idx - 1];
        let (t1, df1) = dfs[idx];
        if (t1 - t0).abs() < 1e-15 {
            return df0;
        }
        let w = (t - t0) / (t1 - t0);
        (df0.ln() * (1.0 - w) + df1.ln() * w).exp()
    }

    /// Regular coupon grid anchored backward from maturity for the synthetic
    /// [`MertonMcEngine::price`] entry point.
    ///
    /// Returns `(time, accrual)` pairs with `accrual = 1 / coupon_frequency`
    /// for every coupon. `maturity_years` is read as the remaining life of a
    /// bond on a regular schedule, so when it is not a whole number of
    /// periods the valuation date sits inside the first period and that
    /// coupon is still paid in full (dirty valuation); the elapsed part of
    /// the period is the accrued interest.
    fn coupon_schedule(maturity_years: f64, coupon_frequency: usize) -> Vec<(f64, f64)> {
        let period = 1.0 / coupon_frequency as f64;
        // FP tolerance so aligned maturities (e.g. exactly 10 semi-annual
        // periods) do not gain an extra coupon.
        let n = ((maturity_years / period) - 1e-9).ceil().max(1.0) as usize;
        (1..=n)
            .map(|k| (maturity_years - (n - k) as f64 * period, period))
            .collect()
    }

    /// PIK-aware risk-free PV (no-default scenario) with each cashflow's
    /// discount factor bumped by `e^{-spread·t}` on top of the base discount
    /// basis (term-structure DFs when set, flat rate otherwise).
    ///
    /// For Toggle periods the risk-free scenario assumes cash payment
    /// (zero hazard implies no PIK trigger).
    fn risk_free_pv_spread(
        terms: &MertonBondTerms,
        discount_rate: f64,
        pik_schedule: &PikSchedule,
        cashflow_dfs: Option<&[(f64, f64)]>,
        spread: f64,
    ) -> f64 {
        let mut pv = 0.0;
        let mut n = terms.notional;

        for &(t, accrual) in &terms.coupons {
            let df = Self::df_at_time(cashflow_dfs, t, discount_rate) * (-spread * t).exp();
            let coupon = n * terms.coupon_rate * accrual;

            match pik_schedule.mode_at(t) {
                PikMode::Cash | PikMode::Toggle => {
                    pv += coupon * df;
                }
                PikMode::Pik => {
                    n += coupon;
                }
                PikMode::Split {
                    cash_fraction,
                    pik_fraction,
                } => {
                    pv += coupon * cash_fraction * df;
                    n += coupon * pik_fraction;
                }
            }
        }
        pv += n
            * Self::df_at_time(cashflow_dfs, terms.maturity_years, discount_rate)
            * (-spread * terms.maturity_years).exp();
        pv
    }

    /// Solve for the constant spread `s` (in bp) such that the PIK-aware
    /// risk-free PV with every cashflow's discount factor bumped by
    /// `e^{-s·t}` equals `target_pv`.
    ///
    /// The base discount basis (term-structure `cashflow_dfs` when set) is
    /// the SAME one used to compute `target_pv`, so curve shape cancels out
    /// of the spread instead of leaking into it.
    fn solve_effective_spread(
        terms: &MertonBondTerms,
        discount_rate: f64,
        target_pv: f64,
        pik_schedule: &PikSchedule,
        cashflow_dfs: Option<&[(f64, f64)]>,
    ) -> f64 {
        let (notional, maturity_years) = (terms.notional, terms.maturity_years);
        if target_pv <= 0.0 || maturity_years <= 0.0 {
            return 0.0;
        }
        let pv_at =
            |s: f64| Self::risk_free_pv_spread(terms, discount_rate, pik_schedule, cashflow_dfs, s);
        let rf_pv = pv_at(0.0);
        if target_pv >= rf_pv {
            return 0.0;
        }

        // Newton-Raphson with ZCB-approximation initial guess
        let mut s = (rf_pv / target_pv).ln() / maturity_years;
        let bump = 1e-4;
        for _ in 0..50 {
            let pv = pv_at(s);
            let f = pv - target_pv;
            if f.abs() < 1e-10 * notional {
                break;
            }
            let pv_up = pv_at(s + bump);
            let dpv = (pv_up - pv) / bump;
            if dpv.abs() < 1e-15 {
                break;
            }
            s -= f / dpv;
            if s < 0.0 {
                s = 0.0;
            }
        }
        s * 10_000.0
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
