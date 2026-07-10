//! Unified pricing engine for revolving credit facilities.
//!
//! Provides a single pricer that handles both deterministic and stochastic modes:
//! - **Deterministic**: Prices using pre-defined draw/repay events
//! - **Stochastic**: Generates 3-factor MC paths and prices each path deterministically
//!
//! # Architecture
//!
//! Stochastic pricing is implemented as averaging many deterministic path pricings,
//! ensuring consistency between modes and enabling full path capture for distribution analysis.

use finstack_quant_core::dates::{Date, DateExt, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use rayon::prelude::*;

use crate::cashflow::builder::CashFlowSchedule;
use crate::instruments::common_impl::traits::Instrument;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_monte_carlo::estimate::Estimate;
use finstack_quant_monte_carlo::results::{MoneyEstimate, MonteCarloResult};

use super::super::cashflow_engine::{
    CashflowEngine, PathAwareCashflowSchedule, ThreeFactorPathData,
};
use super::super::types::{BaseRateSpec, DrawRepaySpec, RevolvingCredit};
use super::components::compute_upfront_fee_pv;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;

use super::path_generator::generate_three_factor_paths;

/// Result for a single path valuation.
///
/// Contains the present value, optional 3-factor path data, and the detailed cashflow schedule.
#[derive(Debug, Clone)]
pub struct PathResult {
    /// Present value for this path
    pub pv: Money,
    /// 3-factor path data (if from MC)
    pub path_data: Option<ThreeFactorPathData>,
    /// Cashflow schedule for this path
    pub cashflows: CashFlowSchedule,
}

/// Enhanced Monte Carlo results with full path details.
///
/// Extends the standard `MonteCarloResult` with individual path results
/// for distribution analysis and visualization.
#[derive(Debug)]
pub struct EnhancedMonteCarloResult {
    /// Standard MC statistics (mean, std error, CI)
    pub mc_result: MonteCarloResult,
    /// Individual path results for distribution analysis
    pub path_results: Vec<PathResult>,
}

// (no test-only dead-code smoke; keep fields live via real code paths)

/// Unified pricer for revolving credit facilities.
///
/// Handles both deterministic and stochastic pricing using a single implementation.
/// Stochastic pricing generates paths and applies deterministic pricing to each path.
pub struct RevolvingCreditPricer {
    model: ModelKey,
}

impl Default for RevolvingCreditPricer {
    fn default() -> Self {
        Self {
            model: ModelKey::Discounting,
        }
    }
}

/// Resolve the fixing series for a floating-rate facility from the market context.
///
/// Returns `None` for fixed-rate facilities or when no fixing series is present
/// (graceful degradation).
fn resolve_fixings<'a>(
    facility: &RevolvingCredit,
    market: &'a MarketContext,
) -> Option<&'a ScalarTimeSeries> {
    match &facility.base_rate_spec {
        BaseRateSpec::Floating(spec) => {
            finstack_quant_core::market_data::fixings::get_fixing_series(
                market,
                spec.index_id.as_ref(),
            )
            .ok()
        }
        BaseRateSpec::Fixed { .. } => None,
    }
}

impl RevolvingCreditPricer {
    /// Create a new pricer instance with specified model.
    pub fn new(model: ModelKey) -> Self {
        Self { model }
    }
    /// Price a single path (deterministic or from MC).
    ///
    /// This is the core pricing logic used for both modes:
    /// - Discounts all cashflows
    /// - Applies survival weighting (static from hazard curve or dynamic from path)
    /// - Adds upfront fee PV
    ///
    /// # Arguments
    ///
    /// * `facility` - The revolving credit facility
    /// * `market` - Market context with curves
    /// * `as_of` - Valuation date
    /// * `path_schedule` - Cashflow schedule with optional path data
    ///
    /// # Returns
    ///
    /// A `PathResult` with PV, cashflows, and path data
    pub fn price_single_path(
        facility: &RevolvingCredit,
        market: &MarketContext,
        as_of: Date,
        path_schedule: &PathAwareCashflowSchedule,
    ) -> Result<PathResult> {
        let disc_curve = market.get_discount(&facility.discount_curve_id)?;

        // Compute survival probabilities
        let survival_probs = if let Some(ref path_data) = path_schedule.path_data {
            // Dynamic survival from credit spread path
            // Need to compute survival at each cashflow date, not just time points
            let cashflow_dates: Vec<Date> = path_schedule
                .schedule
                .flows
                .iter()
                .map(|cf| cf.date)
                .collect();
            Self::compute_dynamic_survival_at_dates(
                &path_data.credit_spread_path,
                &path_data.time_points,
                &cashflow_dates,
                facility.recovery_rate,
                facility.commitment_date,
                facility.day_count,
            )?
        } else if let Some(ref hazard_id) = facility.credit_curve_id {
            // Static survival from hazard curve
            let hazard = market.get_hazard(hazard_id.as_str())?;
            hazard.survival_at_dates(
                &path_schedule
                    .schedule
                    .flows
                    .iter()
                    .map(|cf| cf.date)
                    .collect::<Vec<_>>(),
            )?
        } else {
            // No credit risk
            vec![1.0; path_schedule.schedule.flows.len()]
        };

        // Survival to the valuation date, from the same source as the
        // cashflow-date survivals. All survival weights are conditioned on
        // survival to `as_of` (divide by S(as_of)): a facility being priced
        // has, by definition, not defaulted yet. Using unconditional
        // survival from commitment/curve-base (the previous behavior)
        // understates PV for seasoned facilities by the factor S(→as_of) —
        // the bond hazard engine establishes the same convention.
        let sp_as_of = if let Some(ref path_data) = path_schedule.path_data {
            Self::compute_dynamic_survival_at_dates(
                &path_data.credit_spread_path,
                &path_data.time_points,
                &[as_of],
                facility.recovery_rate,
                facility.commitment_date,
                facility.day_count,
            )?[0]
        } else if let Some(ref hazard_id) = facility.credit_curve_id {
            let hazard = market.get_hazard(hazard_id.as_str())?;
            let t = hazard.day_count().year_fraction(
                hazard.base_date(),
                as_of,
                finstack_quant_core::dates::DayCountContext::default(),
            )?;
            hazard.sp(t)
        } else {
            1.0
        };
        if !sp_as_of.is_finite() || sp_as_of <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "survival probability to valuation date must be positive and finite, \
                 got {sp_as_of}"
            )));
        }

        // Discounting: static curve by default; pathwise bank account when
        // the path carries a genuinely stochastic short rate (HW σ > 0).
        // With stochastic rates the path's coupons are driven by the
        // simulated r — discounting them on the static curve would erase
        // every rate-level/rate-correlation effect on PV through the
        // numeraire. DF(as_of→t) = exp(−∫ r ds) along the path; when rate
        // vol is zero the static-curve mode is retained exactly as before.
        let pathwise_rates = path_schedule
            .path_data
            .as_ref()
            .filter(|p| p.stochastic_rates);
        // Signed so pre-commitment valuation dates do not error; only the
        // pathwise branch consumes this.
        let t_asof_path = facility.day_count.signed_year_fraction(
            facility.commitment_date,
            as_of,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        let df_asof_to = |date: Date| -> Result<f64> {
            if let Some(p) = pathwise_rates {
                let t = facility.day_count.signed_year_fraction(
                    facility.commitment_date,
                    date,
                    finstack_quant_core::dates::DayCountContext::default(),
                )?;
                Ok(Self::pathwise_bank_account_df(
                    &p.time_points,
                    &p.short_rate_path,
                    t_asof_path,
                    t,
                ))
            } else {
                disc_curve.df_between_dates(as_of, date)
            }
        };

        // Discount cashflows with survival weighting.
        // Anchor PV at `as_of` (not the curve base date) so that rolling the
        // valuation date forward shortens the discount path and produces
        // non-zero theta from the time-value of accruing fees/interest.
        if survival_probs.len() != path_schedule.schedule.flows.len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "survival probability count {} does not match cashflow count {}",
                survival_probs.len(),
                path_schedule.schedule.flows.len()
            )));
        }

        let mut total_pv = 0.0;
        for (cf, survival_uncond) in path_schedule.schedule.flows.iter().zip(&survival_probs) {
            if cf.date < as_of {
                continue;
            }
            let df = df_asof_to(cf.date)?;
            let survival = *survival_uncond / sp_as_of;
            total_pv += cf.amount.amount() * df * survival;
        }

        // Recovery Leg PV — trapezoidal integration on a monthly-or-finer grid.
        // PV_rec = Sum [ Exposure(t) * RecoveryRate * DF(t) * ProbDefault(t-1, t) ]
        if facility.recovery_rate > 0.0 {
            let future_grid = Self::build_recovery_grid(facility, as_of, path_schedule)?;

            if !future_grid.is_empty() {
                let survival_at_grid = if let Some(ref path_data) = path_schedule.path_data {
                    Self::compute_dynamic_survival_at_dates(
                        &path_data.credit_spread_path,
                        &path_data.time_points,
                        &future_grid,
                        facility.recovery_rate,
                        facility.commitment_date,
                        facility.day_count,
                    )?
                } else if let Some(ref hazard_id) = facility.credit_curve_id {
                    let hazard = market.get_hazard(hazard_id.as_str())?;
                    hazard.survival_at_dates(&future_grid)?
                } else {
                    vec![1.0; future_grid.len()]
                };

                let exposure_at_grid =
                    Self::exposure_at_grid(facility, as_of, &future_grid, path_schedule)?;

                // Same source as `sp_as_of` above: integration starts at the
                // valuation date with S(as_of).
                let mut prev_sp = sp_as_of;

                let mut prev_exposure = if path_schedule.path_data.is_some() {
                    facility.drawn_amount.amount()
                } else {
                    super::super::cashflow_engine::calculate_drawn_balance_at_date(facility, as_of)?
                        .amount()
                };

                let mut prev_date = as_of;
                for i in 0..future_grid.len() {
                    let curr_date = future_grid[i];
                    let curr_sp = survival_at_grid[i];
                    let curr_exposure = exposure_at_grid[i];

                    // Default probability conditional on survival to as_of.
                    let prob_default = ((prev_sp - curr_sp) / sp_as_of).max(0.0);

                    let df_prev = df_asof_to(prev_date)?;
                    let df_curr = df_asof_to(curr_date)?;
                    let df_avg = (df_prev + df_curr) / 2.0;
                    let exposure_avg = (prev_exposure + curr_exposure) / 2.0;

                    total_pv += exposure_avg * facility.recovery_rate * df_avg * prob_default;

                    prev_sp = curr_sp;
                    prev_exposure = curr_exposure;
                    prev_date = curr_date;
                }
            }
        }

        // Add upfront fee if applicable
        if let Some(upfront) = facility.fees.upfront_fee {
            total_pv += compute_upfront_fee_pv(
                Some(upfront),
                facility.commitment_date,
                as_of,
                disc_curve.as_ref(),
            )?;
        }

        let result = PathResult {
            pv: Money::new(total_pv, facility.commitment_amount.currency()),
            path_data: path_schedule.path_data.clone(),
            cashflows: path_schedule.schedule.clone(),
        };

        // Keep optional payloads live under `-D dead-code`:
        // callers expect to inspect cashflows and paths, and we also touch them here.
        let _ = result.cashflows.flows.len();
        let _ = result.path_data.is_some();

        Ok(result)
    }

    /// Main pricing entry point.
    ///
    /// Automatically dispatches to deterministic or stochastic pricing based on
    /// the facility's `draw_repay_spec`.
    ///
    /// # Arguments
    ///
    /// * `facility` - The revolving credit facility
    /// * `market` - Market context with curves
    /// * `as_of` - Valuation date
    ///
    /// # Returns
    ///
    /// Present value as `Money`
    pub(crate) fn price(
        facility: &RevolvingCredit,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        match &facility.draw_repay_spec {
            DrawRepaySpec::Deterministic(_) => {
                // Single deterministic path
                let fixings = resolve_fixings(facility, market);
                let engine = CashflowEngine::new(facility, Some(market), as_of, fixings)?;
                let schedule = engine.generate_deterministic()?;
                let result = Self::price_single_path(facility, market, as_of, &schedule)?;
                Ok(result.pv)
            }
            DrawRepaySpec::Stochastic(_) => {
                let enhanced = Self::price_monte_carlo(facility, market, as_of)?;
                Ok(enhanced.mc_result.estimate.mean)
            }
        }
    }

    /// Price deterministically (explicit method for API clarity).
    ///
    /// This is the same as calling `price()` with a deterministic facility,
    /// but provides an explicit API for callers who know they have a deterministic spec.
    pub(crate) fn price_deterministic(
        facility: &RevolvingCredit,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        let fixings = resolve_fixings(facility, market);
        let engine = CashflowEngine::new(facility, Some(market), as_of, fixings)?;
        let schedule = engine.generate_deterministic()?;
        let result = Self::price_single_path(facility, market, as_of, &schedule)?;
        Ok(result.pv)
    }

    /// Price with full MC path capture for analysis.
    ///
    /// Returns detailed results including all individual path PVs, cashflows,
    /// and trajectories for distribution analysis.
    ///
    /// # Arguments
    ///
    /// * `facility` - The revolving credit facility (must have stochastic spec)
    /// * `market` - Market context with curves
    /// * `as_of` - Valuation date
    ///
    /// # Returns
    ///
    /// Enhanced Monte Carlo result with full path details
    pub fn price_with_paths(
        facility: &RevolvingCredit,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<EnhancedMonteCarloResult> {
        match &facility.draw_repay_spec {
            DrawRepaySpec::Stochastic(_) => Self::price_monte_carlo(facility, market, as_of),
            DrawRepaySpec::Deterministic(_) => Err(finstack_quant_core::Error::Validation(
                "Path capture requires stochastic spec".into(),
            )),
        }
    }

    /// Internal MC pricing with 3-factor path generation and aggregation.
    ///
    /// This method:
    /// 1. Generates 3-factor MC paths (utilization, rate, spread)
    /// 2. Generates cashflows for each path
    /// 3. Prices each path deterministically
    /// 4. Computes MC statistics across all paths
    fn price_monte_carlo(
        facility: &RevolvingCredit,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<EnhancedMonteCarloResult> {
        // Extract stochastic spec
        let stoch_spec = match &facility.draw_repay_spec {
            DrawRepaySpec::Stochastic(spec) => spec.as_ref(),
            DrawRepaySpec::Deterministic(_) => {
                return Err(finstack_quant_core::Error::Validation(
                    "Stochastic spec required for MC pricing".to_string(),
                ))
            }
        };

        // Get or synthesize MC config
        use super::super::types::{CreditSpreadProcessSpec, McConfig};
        let mc_config_to_use;
        let mc_config = if let Some(ref mc_config) = stoch_spec.mc_config {
            mc_config.validate()?;
            mc_config
        } else {
            // Synthesize minimal McConfig
            // If facility has hazard curve, use market-anchored process; otherwise constant zero
            let credit_process = if let Some(ref hazard_id) = facility.credit_curve_id {
                CreditSpreadProcessSpec::MarketAnchored {
                    hazard_curve_id: hazard_id.clone(),
                    kappa: 0.1,
                    implied_vol: 1e-10, // Minimal volatility for deterministic behavior
                    tenor_years: None,
                }
            } else {
                CreditSpreadProcessSpec::Constant(0.0)
            };

            mc_config_to_use = McConfig {
                correlation_matrix: None,
                recovery_rate: facility.recovery_rate,
                credit_spread_process: credit_process,
                interest_rate_process: None,
                util_credit_corr: None,
            };
            mc_config_to_use.validate()?;
            &mc_config_to_use
        };

        // Historical fixings remain contractual in stochastic valuation. The
        // short-rate process drives only reset dates that have not fixed yet.
        let fixings = resolve_fixings(facility, market);
        let engine = CashflowEngine::new(facility, Some(market), as_of, fixings)?;
        let payment_dates = super::super::utils::build_payment_dates(facility, false)?;

        // Generate 3-factor paths (simulation starts at as_of for seasoned facilities)
        let paths = generate_three_factor_paths(
            stoch_spec,
            mc_config,
            facility,
            market,
            &payment_dates,
            as_of,
        )?;

        // Price each path. Paths carry their own pre-generated randomness and
        // `generate_stochastic_path` / `price_single_path` are pure functions of
        // `path_data` plus the shared (immutable) engine/facility/market, so the
        // valuation is parallelised. `into_par_iter().collect()` preserves path
        // order, keeping the antithetic pairing and the PV statistics identical
        // to the serial implementation.
        let path_results: Vec<_> = paths
            .into_par_iter()
            .map(|path_data| {
                let schedule = engine.generate_stochastic_path(path_data)?;
                Self::price_single_path(facility, market, as_of, &schedule)
            })
            .collect::<Result<Vec<_>>>()?;

        // Compute MC statistics using Bessel-corrected variance (N-1 denominator)
        // for unbiased standard error estimation.
        //
        // Antithetic paths are NOT i.i.d. — each (z, −z) pair is negatively
        // correlated by construction. Treating the 2N pathwise PVs as
        // independent overstates the effective sample size and misstates the
        // standard error. The correct estimator averages each antithetic
        // pair into ONE i.i.d. sample first (pairs are adjacent in path
        // order), then applies the usual sample statistics.
        let pvs: Vec<f64> = path_results.iter().map(|r| r.pv.amount()).collect();
        let use_antithetic = stoch_spec.antithetic && !stoch_spec.use_sobol_qmc;
        let samples: Vec<f64> = if use_antithetic {
            pvs.chunks(2)
                .map(|pair| pair.iter().sum::<f64>() / pair.len() as f64)
                .collect()
        } else {
            pvs.clone()
        };
        let n = samples.len() as f64;
        let mean = samples.iter().sum::<f64>() / n;

        // Use N-1 for unbiased variance estimation (Bessel's correction)
        let variance = if samples.len() > 1 {
            samples.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)
        } else {
            0.0 // Single pair/path case
        };
        let stderr = (variance / n).sqrt();

        // Compute 95% confidence interval (assuming asymptotic normality via CLT)
        let z_95 = 1.96;
        let ci_low = mean - z_95 * stderr;
        let ci_high = mean + z_95 * stderr;

        let estimate = MoneyEstimate::from_estimate(
            Estimate::new(mean, stderr, (ci_low, ci_high), pvs.len()),
            facility.commitment_amount.currency(),
        );

        let result = EnhancedMonteCarloResult {
            mc_result: MonteCarloResult {
                estimate,
                paths: None,
                run: None,
            },
            path_results,
        };

        // Touch exported details so they are live under `-D dead-code`.
        let _ = result.mc_result.estimate.num_paths;
        let _ = result.path_results.len();

        Ok(result)
    }

    /// Compute dynamic survival probabilities at arbitrary cashflow dates.
    ///
    /// Interpolates survival from the credit spread path to match cashflow dates.
    ///
    /// Uses the relation: hazard_rate = credit_spread / (1 - recovery_rate)
    ///
    /// # Arguments
    ///
    /// * `credit_spreads` - Credit spread values at each time point
    /// * `time_points` - Time grid in years from commitment date
    /// * `cashflow_dates` - Dates at which to compute survival probabilities
    /// * `recovery_rate` - Recovery rate for hazard-to-spread mapping
    /// * `commitment_date` - Facility commitment date
    /// * `day_count` - Optional day count convention (defaults to Act365F if None)
    fn compute_dynamic_survival_at_dates(
        credit_spreads: &[f64],
        time_points: &[f64],
        cashflow_dates: &[Date],
        recovery_rate: f64,
        commitment_date: Date,
        day_count: DayCount,
    ) -> Result<Vec<f64>> {
        use finstack_quant_core::dates::DayCountContext;
        // Use facility day count for consistency with path generation
        let dc = day_count;

        // First, compute cumulative hazard at each payment date
        let mut cumulative_hazards = Vec::with_capacity(time_points.len());
        let mut cumulative_hazard = 0.0;
        cumulative_hazards.push(0.0); // At commitment date

        for i in 0..(credit_spreads.len() - 1) {
            let dt = time_points[i + 1] - time_points[i];
            let hazard_rate = credit_spreads[i] / (1.0 - recovery_rate).max(1e-6);
            cumulative_hazard += hazard_rate * dt;
            cumulative_hazards.push(cumulative_hazard);
        }

        // Now interpolate survival for each cashflow date
        let mut survival_probs = Vec::with_capacity(cashflow_dates.len());
        for &cf_date in cashflow_dates {
            // Find the interval containing cf_date
            let t_cf = dc.year_fraction(commitment_date, cf_date, DayCountContext::default())?;

            // Find the bracketing payment dates
            let hazard_at_cf = if let Some(idx) = time_points.iter().position(|&t| t >= t_cf) {
                if idx == 0
                    || (time_points[idx] - t_cf).abs() < super::super::INTERPOLATION_TOLERANCE
                {
                    // At or before first point
                    cumulative_hazards[idx.min(cumulative_hazards.len() - 1)]
                } else {
                    // Interpolate between idx-1 and idx
                    let t0 = time_points[idx - 1];
                    let t1 = time_points[idx];
                    let h0 = cumulative_hazards[idx - 1];
                    let h1 = cumulative_hazards[idx];

                    let alpha = (t_cf - t0) / (t1 - t0).max(super::super::INTERPOLATION_TOLERANCE);
                    h0 + alpha * (h1 - h0)
                }
            } else {
                // After last point - use last cumulative hazard
                cumulative_hazards.last().copied().unwrap_or(0.0)
            };

            survival_probs.push((-hazard_at_cf).exp());
        }

        Ok(survival_probs)
    }

    /// Build a monthly-or-finer grid for recovery leg integration.
    ///
    /// Merges monthly dates with payment dates and deterministic draw/repay event
    /// dates, then filters to `(as_of, maturity]`. This gives much better accuracy
    /// than relying solely on the (potentially quarterly/annual) payment schedule.
    fn build_recovery_grid(
        facility: &RevolvingCredit,
        as_of: Date,
        path_schedule: &PathAwareCashflowSchedule,
    ) -> Result<Vec<Date>> {
        use std::collections::BTreeSet;
        let mut dates = BTreeSet::new();

        // Seed with payment dates
        if let Some(ref path_data) = path_schedule.path_data {
            dates.extend(path_data.payment_dates.iter().copied());
        } else {
            dates.extend(super::super::utils::build_payment_dates(facility, false)?);
        }

        // Seed with deterministic draw/repay event dates (exposure jumps)
        if let DrawRepaySpec::Deterministic(ref events) = facility.draw_repay_spec {
            dates.extend(events.iter().map(|e| e.date));
        }

        // Fill in monthly dates from as_of to maturity
        let mut d = as_of.add_months(1);
        while d < facility.maturity {
            dates.insert(d);
            d = d.add_months(1);
        }
        dates.insert(facility.maturity);

        Ok(dates.into_iter().filter(|&d| d > as_of).collect())
    }

    /// Compute exposure (drawn balance) at each grid date.
    ///
    /// For stochastic paths, linearly interpolates utilization between the path's
    /// payment-date observations. For deterministic, uses balance evolution.
    fn exposure_at_grid(
        facility: &RevolvingCredit,
        _as_of: Date,
        grid: &[Date],
        path_schedule: &PathAwareCashflowSchedule,
    ) -> Result<Vec<f64>> {
        if let Some(ref path_data) = path_schedule.path_data {
            let commitment = facility.commitment_amount.amount();
            grid.iter()
                .map(|&date| {
                    let util = Self::interpolate_utilization_at_date(
                        date,
                        facility.commitment_date,
                        facility.day_count,
                        &path_data.time_points,
                        &path_data.utilization_path,
                    )?;
                    Ok(util * commitment)
                })
                .collect()
        } else {
            grid.iter()
                .map(|&date| {
                    Ok(
                        super::super::cashflow_engine::calculate_drawn_balance_at_date(
                            facility, date,
                        )?
                        .amount(),
                    )
                })
                .collect()
        }
    }

    /// Pathwise bank-account discount factor `exp(−∫_{t_a}^{t_b} r ds)` along
    /// the simulated short-rate path.
    ///
    /// The short rate is linearly interpolated between the recorded path
    /// points (flat extrapolation beyond the grid) and the integral is taken
    /// exactly on the resulting piecewise-linear rate (trapezoidal on each
    /// sub-interval). Returns 1.0 when `t_b <= t_a`.
    fn pathwise_bank_account_df(
        time_points: &[f64],
        short_rates: &[f64],
        t_a: f64,
        t_b: f64,
    ) -> f64 {
        if t_b <= t_a || time_points.is_empty() || short_rates.len() != time_points.len() {
            return 1.0;
        }
        let rate_at = |t: f64| -> f64 {
            let n = time_points.len();
            if t <= time_points[0] {
                return short_rates[0];
            }
            if t >= time_points[n - 1] {
                return short_rates[n - 1];
            }
            let idx = time_points.partition_point(|&tp| tp <= t);
            let i = idx.saturating_sub(1);
            let alpha = (t - time_points[i]) / (time_points[i + 1] - time_points[i]).max(1e-12);
            short_rates[i] + alpha * (short_rates[i + 1] - short_rates[i])
        };

        // Integration breakpoints: t_a, every interior grid point, t_b.
        let mut integral = 0.0;
        let mut prev_t = t_a;
        let mut prev_r = rate_at(t_a);
        for &tp in time_points.iter().filter(|&&tp| tp > t_a && tp < t_b) {
            let r = rate_at(tp);
            integral += 0.5 * (prev_r + r) * (tp - prev_t);
            prev_t = tp;
            prev_r = r;
        }
        let r_b = rate_at(t_b);
        integral += 0.5 * (prev_r + r_b) * (t_b - prev_t);

        (-integral).exp()
    }

    /// Linearly interpolate utilization from the MC path at a given calendar date.
    fn interpolate_utilization_at_date(
        date: Date,
        commitment_date: Date,
        day_count: DayCount,
        time_points: &[f64],
        utilization_path: &[f64],
    ) -> Result<f64> {
        if time_points.len() != utilization_path.len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "utilization path length {} does not match time-grid length {}",
                utilization_path.len(),
                time_points.len()
            )));
        }

        if time_points.is_empty() || utilization_path.is_empty() {
            return Ok(0.0);
        }
        let t = day_count.year_fraction(
            commitment_date,
            date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= time_points[0] {
            return Ok(utilization_path[0].clamp(0.0, 1.0));
        }
        let n = time_points.len();
        if t >= time_points[n - 1] {
            return Ok(utilization_path[n - 1].clamp(0.0, 1.0));
        }
        let idx = time_points.partition_point(|&tp| tp <= t);
        let i = idx.saturating_sub(1);
        let alpha = (t - time_points[i]) / (time_points[i + 1] - time_points[i]).max(1e-12);
        let util = utilization_path[i] + alpha * (utilization_path[i + 1] - utilization_path[i]);
        Ok(util.clamp(0.0, 1.0))
    }
}

impl Pricer for RevolvingCreditPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::RevolvingCredit, self.model)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        use crate::pricer::expect_inst;

        let facility: &RevolvingCredit = expect_inst(instrument, InstrumentType::RevolvingCredit)?;

        let ctx = PricingErrorContext::new()
            .instrument_id(facility.id.as_str())
            .instrument_type(InstrumentType::RevolvingCredit)
            .model(self.model);

        // Route to appropriate pricing method based on model
        let result_pv = match self.model {
            ModelKey::Discounting => {
                // For discounting, we use the unified price method which handles
                // deterministic specs (and errs on stochastic if MC not enabled/used)
                Self::price(facility, market, as_of)
                    .map_err(|e| PricingError::from_core(e, ctx.clone()))?
            }

            ModelKey::MonteCarloGBM => {
                // For MC, we ensure we're using the MC path
                let enhanced = Self::price_with_paths(facility, market, as_of)
                    .map_err(|e| PricingError::from_core(e, ctx.clone()))?;
                enhanced.mc_result.estimate.mean
            }
            _ => {
                return Err(PricingError::model_failure_with_context(
                    format!("Unsupported model for RevolvingCredit: {}", self.model),
                    ctx,
                ));
            }
        };

        // Wrap in ValuationResult
        let mut result = ValuationResult::stamped(facility.id.as_str(), as_of, result_pv);
        result.measures.insert(
            crate::metrics::MetricId::custom("model"),
            self.model.to_string().parse().unwrap_or(0.0),
        ); // Just tagging
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::instruments::fixed_income::revolving_credit::{
        BaseRateSpec, CreditSpreadProcessSpec, DrawRepaySpec, McConfig, RevolvingCredit,
        RevolvingCreditFees, StochasticUtilizationSpec, UtilizationProcess,
    };
    use finstack_quant_core::dates::DayCount;

    use finstack_quant_core::market_data::context::MarketContext;

    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    use finstack_quant_core::money::Money;

    use finstack_quant_core::{currency::Currency, dates::Tenor};
    use time::Month;

    /// Item 6 verification: the contractual leg (survival-weighted no-default
    /// cashflows) plus the recovery leg must NOT double-count LGD on
    /// principal.
    ///
    /// The audit flagged a suspected double-count (PV overstated ≈ R·drawn·PD).
    /// This test pins the exact decomposition: for a zero-coupon facility
    /// (principal only) with flat DF = 1, flat hazard, and recovery R, the
    /// correct risky PV of principal is `D·SP + R·D·(1-SP)` — full repayment
    /// if the borrower survives, recovery if it defaults. The priced PV must
    /// equal that exactly, NOT the double-counted `D·SP + 2·R·D·(1-SP)`.
    ///
    /// Conclusion (recorded as a regression guard): the current implementation
    /// is correct — the survival-weighted contractual leg represents the
    /// no-default state and the recovery leg adds the disjoint default-state
    /// value, exactly as a risky-cashflow decomposition should.
    #[test]
    fn recovery_leg_does_not_double_count_lgd_on_principal() {
        use crate::instruments::fixed_income::revolving_credit::RevolvingCreditFees;
        use finstack_quant_core::market_data::term_structures::HazardCurve;

        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("date");

        let facility = RevolvingCredit::builder()
            .id("RC-RECOVERY-NODBL".into())
            .commitment_amount(Money::new(1_000_000.0, Currency::USD))
            .drawn_amount(Money::new(1_000_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            // Zero coupon isolates the principal leg.
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.0 })
            .day_count(DayCount::Act365F)
            .frequency(Tenor::annual())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id("USD-HZ".into())
            .recovery_rate(0.4)
            .build()
            .expect("facility");

        // Flat DF = 1 everywhere → arithmetic is exact (no discounting noise).
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 1.0), (5.0, 1.0)])
            .build()
            .expect("curve");
        // Flat hazard 20% → SP(1y) = exp(-0.2).
        let hz = HazardCurve::builder("USD-HZ")
            .base_date(start)
            .knots([(1.0, 0.20), (5.0, 0.20)])
            .build()
            .expect("hazard");
        let market = MarketContext::new().insert(disc).insert(hz);

        let pv = RevolvingCreditPricer::price(&facility, &market, start)
            .expect("price")
            .amount();

        let d = 1_000_000.0_f64;
        let sp = (-0.20_f64).exp();
        let r = 0.4_f64;
        // Correct: full repayment on survival + recovery on default.
        let correct = d * sp + r * d * (1.0 - sp);
        // Double-counted: an extra R·D·(1-SP) on top.
        let double_counted = correct + r * d * (1.0 - sp);

        assert!(
            (pv - correct).abs() < 1.0,
            "risky PV {pv} should equal D·SP + R·D·(1-SP) = {correct}, not the \
             double-counted {double_counted}"
        );
        assert!(
            (pv - double_counted).abs() > 1.0,
            "risky PV {pv} must NOT equal the double-counted value {double_counted}"
        );
    }

    /// M2.10: survival weighting must be conditioned on survival to `as_of`.
    ///
    /// A seasoned zero-coupon facility (flat DF = 1, flat hazard λ = 20%)
    /// priced one year into a two-year life must be worth
    /// `D·SP(as_of→T) + R·D·(1−SP(as_of→T))` with `SP(as_of→T) = e^{-0.2}`,
    /// NOT the unconditional `D·SP(0→T)/1 + …` which understates PV by the
    /// factor S(0→as_of) = e^{-0.2}.
    #[test]
    fn seasoned_facility_survival_is_conditioned_on_as_of() {
        use finstack_quant_core::market_data::term_structures::HazardCurve;

        let start = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("date");

        let facility = RevolvingCredit::builder()
            .id("RC-SEASONED".into())
            .commitment_amount(Money::new(1_000_000.0, Currency::USD))
            .drawn_amount(Money::new(1_000_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.0 })
            .day_count(DayCount::Act365F)
            .frequency(Tenor::annual())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id("USD-HZ".into())
            .recovery_rate(0.4)
            .build()
            .expect("facility");

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 1.0), (5.0, 1.0)])
            .build()
            .expect("curve");
        let hz = HazardCurve::builder("USD-HZ")
            .base_date(start)
            .knots([(1.0, 0.20), (5.0, 0.20)])
            .build()
            .expect("hazard");
        let market = MarketContext::new().insert(disc).insert(hz);

        let pv = RevolvingCreditPricer::price(&facility, &market, as_of)
            .expect("price")
            .amount();

        let d = 1_000_000.0_f64;
        let r = 0.4_f64;
        // One year remains at λ = 20%, conditional on survival to as_of.
        let sp_cond = (-0.20_f64).exp();
        let correct = d * sp_cond + r * d * (1.0 - sp_cond);
        // Unconditional weighting multiplies the survival leg by an extra
        // S(0→as_of) = e^{-0.2} and scales the default leg the same way.
        let sp_uncond_t = (-0.40_f64).exp();
        let unconditional = d * sp_uncond_t + r * d * ((-0.20_f64).exp() - sp_uncond_t);

        assert!(
            (pv - correct).abs() < 1.0,
            "seasoned PV {pv} should equal conditional value {correct} \
             (unconditional would be {unconditional})"
        );
        assert!(
            (pv - unconditional).abs() > 1.0,
            "seasoned PV {pv} must NOT equal the unconditional value {unconditional}"
        );
    }

    /// M2.8: a deterministic draw/repay event dated on the commitment date is
    /// rejected — the position at commitment is defined by `drawn_amount`
    /// and a commitment-date event double-counted principal (interest on 2X,
    /// 2X terminal repayment).
    #[test]
    fn commitment_date_event_is_rejected() {
        use crate::instruments::fixed_income::revolving_credit::DrawRepayEvent;

        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("date");

        let facility = RevolvingCredit::builder()
            .id("RC-COMMIT-EVENT".into())
            .commitment_amount(Money::new(1_000_000.0, Currency::USD))
            .drawn_amount(Money::new(400_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act365F)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![DrawRepayEvent {
                date: start,
                amount: Money::new(400_000.0, Currency::USD),
                is_draw: true,
            }]))
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.4)
            .build()
            .expect("facility");

        // Instrument-level validation rejects it…
        assert!(
            facility.validate().is_err(),
            "validate() must reject a commitment-date event"
        );

        // …and the pricing path rejects it even if validate() is skipped.
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, 1.0)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(disc);
        assert!(
            RevolvingCreditPricer::price(&facility, &market, start).is_err(),
            "pricing must reject a commitment-date event"
        );
    }

    /// M2.9: Sobol QMC requires one coordinate per (step, factor). The
    /// weekly-refined grid of a one-year facility needs ~52×3 dimensions —
    /// far beyond the supported Sobol table — so `use_sobol_qmc` must be
    /// rejected rather than silently consuming a 3-dimensional sequence
    /// once per time step (van-der-Corput anti-correlated, biased paths).
    #[test]
    fn sobol_qmc_with_underdimensioned_schedule_is_rejected() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("date");

        let facility = RevolvingCredit::builder()
            .id("RC-SOBOL".into())
            .commitment_amount(Money::new(1_000_000.0, Currency::USD))
            .drawn_amount(Money::new(400_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                StochasticUtilizationSpec {
                    utilization_process: UtilizationProcess::MeanReverting {
                        target_rate: 0.5,
                        speed: 0.75,
                        volatility: 0.05,
                    },
                    num_paths: 8,
                    seed: Some(7),
                    antithetic: false,
                    use_sobol_qmc: true,
                    mc_config: Some(McConfig {
                        recovery_rate: 0.4,
                        credit_spread_process: CreditSpreadProcessSpec::Constant(0.0),
                        interest_rate_process: None,
                        correlation_matrix: None,
                        util_credit_corr: None,
                    }),
                },
            )))
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.4)
            .build()
            .expect("facility");

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, 1.0)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(disc);

        let err = RevolvingCreditPricer::price_with_paths(&facility, &market, start)
            .expect_err("Sobol with num_steps×num_factors > MAX_SOBOL_DIMENSION must error");
        assert!(
            err.to_string().contains("use_sobol_qmc"),
            "error should explain the Sobol dimension contract, got: {err}"
        );
    }

    #[test]
    fn test_compute_dynamic_survival() {
        let spreads = vec![0.01, 0.02, 0.015, 0.018];
        let times = vec![0.0, 0.25, 0.5, 0.75];
        let recovery = 0.4;
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let cashflow_dates = vec![
            start,
            Date::from_calendar_date(2025, Month::April, 1).expect("valid date"),
            Date::from_calendar_date(2025, Month::July, 1).expect("valid date"),
            Date::from_calendar_date(2025, Month::October, 1).expect("valid date"),
        ];

        let survivals = RevolvingCreditPricer::compute_dynamic_survival_at_dates(
            &spreads,
            &times,
            &cashflow_dates,
            recovery,
            start,
            DayCount::Act365F,
        )
        .expect("should succeed");

        assert_eq!(survivals.len(), 4);
        // Survival at t=0 should be 1.0
        assert!((survivals[0] - 1.0).abs() < 1e-10);
        // Survival should generally decrease over time (with positive spreads)
        // All survivals should be in (0, 1]
        for &s in &survivals {
            assert!(s > 0.0 && s <= 1.0);
        }
    }

    #[test]
    fn test_day_count_consistency() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let dc_act360 = DayCount::Act360;

        // Create time points using Act360 (approx 1.0139 for 1 year)
        let t_end_act360 = dc_act360
            .year_fraction(start, end, Default::default())
            .expect("valid date range for year fraction");
        let time_points = vec![0.0, t_end_act360];

        // Spread path: 100bps constant
        let spreads = vec![0.01, 0.01];
        let recovery = 0.0; // Simple hazard = spread

        // We want to look up survival at 'end' date
        let cashflow_dates = vec![end];

        // 1. Correct: Pass Act360
        let survivals_correct = RevolvingCreditPricer::compute_dynamic_survival_at_dates(
            &spreads,
            &time_points,
            &cashflow_dates,
            recovery,
            start,
            dc_act360,
        )
        .expect("should succeed");

        // Should match exact calculation: exp(-hazard * t)
        // hazard = 0.01
        // t = t_end_act360
        let expected = (-0.01 * t_end_act360).exp();
        assert!(
            (survivals_correct[0] - expected).abs() < 1e-10,
            "Correct day count should yield exact match. Got {}, expected {}",
            survivals_correct[0],
            expected
        );

        // 2. Incorrect: Pass Act365F (simulating the bug)
        let survivals_mismatch = RevolvingCreditPricer::compute_dynamic_survival_at_dates(
            &spreads,
            &time_points,
            &cashflow_dates,
            recovery,
            start,
            DayCount::Act365F,
        )
        .expect("should succeed");

        assert!(
            (survivals_mismatch[0] - survivals_correct[0]).abs() > 1e-5,
            "Mismatching day counts should yield different results"
        );
    }

    #[test]
    fn test_price_with_paths_uses_moneyestimate_defaults() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

        let facility = RevolvingCredit::builder()
            .id("RC-UNIFIED-PATHS".into())
            .commitment_amount(Money::new(1_000_000.0, Currency::USD))
            .drawn_amount(Money::new(400_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                StochasticUtilizationSpec {
                    utilization_process: UtilizationProcess::MeanReverting {
                        target_rate: 0.5,
                        speed: 0.75,
                        volatility: 0.05,
                    },
                    num_paths: 8,
                    seed: Some(7),
                    antithetic: false,
                    use_sobol_qmc: false,
                    mc_config: Some(McConfig {
                        recovery_rate: 0.4,
                        credit_spread_process: CreditSpreadProcessSpec::Constant(0.0),
                        interest_rate_process: None,
                        correlation_matrix: None,
                        util_credit_corr: None,
                    }),
                },
            )))
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.4)
            .build()
            .expect("facility should build");

        let disc_curve = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.03f64).exp()),
                (5.0, (-0.03f64 * 5.0).exp()),
            ])
            .build()
            .expect("curve should build");
        let market = MarketContext::new().insert(disc_curve);

        let result = RevolvingCreditPricer::price_with_paths(&facility, &market, start)
            .expect("should price");

        assert_eq!(result.mc_result.estimate.num_paths, 8);
        assert_eq!(result.path_results.len(), 8);
        assert!(result.mc_result.estimate.std_dev.is_none());
        assert!(result.mc_result.estimate.median.is_none());
        assert!(result.mc_result.estimate.percentile_25.is_none());
        assert!(result.mc_result.estimate.percentile_75.is_none());
        assert!(result.mc_result.estimate.min.is_none());
        assert!(result.mc_result.estimate.max.is_none());
    }

    /// `num_paths < 2` must be rejected: a single path has no variance
    /// estimate (previously produced NaN std error downstream).
    #[test]
    fn single_path_mc_is_rejected() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

        let facility = RevolvingCredit::builder()
            .id("RC-ONE-PATH".into())
            .commitment_amount(Money::new(1_000_000.0, Currency::USD))
            .drawn_amount(Money::new(400_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                StochasticUtilizationSpec {
                    utilization_process: UtilizationProcess::MeanReverting {
                        target_rate: 0.5,
                        speed: 0.75,
                        volatility: 0.05,
                    },
                    num_paths: 1,
                    seed: Some(7),
                    antithetic: false,
                    use_sobol_qmc: false,
                    mc_config: None,
                },
            )))
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.4)
            .build()
            .expect("facility");

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, 1.0)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(disc);

        let err = RevolvingCreditPricer::price_with_paths(&facility, &market, start)
            .expect_err("num_paths = 1 must be rejected");
        assert!(
            err.to_string().contains("num_paths"),
            "error should mention num_paths, got: {err}"
        );
    }

    /// Zero utilization volatility must freeze ONLY the utilization factor:
    /// the credit-spread (and rate) factors keep their own dynamics. The
    /// previous behavior skipped the discretization step entirely, silently
    /// freezing all three factors.
    #[test]
    fn zero_util_vol_freezes_only_the_utilization_factor() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

        let facility = RevolvingCredit::builder()
            .id("RC-ZEROVOL".into())
            .commitment_amount(Money::new(1_000_000.0, Currency::USD))
            .drawn_amount(Money::new(400_000.0, Currency::USD))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                StochasticUtilizationSpec {
                    utilization_process: UtilizationProcess::MeanReverting {
                        target_rate: 0.5,
                        speed: 0.75,
                        volatility: 0.0, // zero utilization vol
                    },
                    num_paths: 4,
                    seed: Some(11),
                    antithetic: false,
                    use_sobol_qmc: false,
                    mc_config: Some(McConfig {
                        recovery_rate: 0.4,
                        // Genuinely stochastic credit spread.
                        credit_spread_process: CreditSpreadProcessSpec::Cir {
                            kappa: 0.5,
                            theta: 0.02,
                            sigma: 0.05,
                            initial: 0.02,
                        },
                        interest_rate_process: None,
                        correlation_matrix: None,
                        util_credit_corr: None,
                    }),
                },
            )))
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.4)
            .build()
            .expect("facility");

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, 1.0)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(disc);

        let result = RevolvingCreditPricer::price_with_paths(&facility, &market, start)
            .expect("should price");
        let path = result.path_results[0]
            .path_data
            .as_ref()
            .expect("path data");

        // Utilization frozen at its initial value across the whole path…
        let u0 = path.utilization_path[0];
        assert!(
            path.utilization_path
                .iter()
                .all(|&u| (u - u0).abs() < 1e-12),
            "utilization must be frozen with zero vol: {:?}",
            path.utilization_path
        );
        // …while the credit spread still diffuses.
        let s0 = path.credit_spread_path[0];
        assert!(
            path.credit_spread_path
                .iter()
                .any(|&s| (s - s0).abs() > 1e-6),
            "credit spread must keep stepping with zero util vol: {:?}",
            path.credit_spread_path
        );
    }

    /// `ThreeFactorPathData::validate` must reject length mismatches with an
    /// error instead of letting downstream indexing panic.
    #[test]
    fn path_data_validation_rejects_mismatched_lengths() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let d2 = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");

        let bad = ThreeFactorPathData {
            utilization_path: vec![0.4, 0.5],
            short_rate_path: vec![0.03], // wrong length
            credit_spread_path: vec![0.01, 0.01],
            time_points: vec![0.0, 0.5],
            payment_dates: vec![start, d2],
            stochastic_rates: false,
        };
        let err = bad.validate().expect_err("length mismatch must error");
        assert!(err.to_string().contains("short_rate_path"), "got: {err}");

        let non_monotone = ThreeFactorPathData {
            utilization_path: vec![0.4, 0.5],
            short_rate_path: vec![0.03, 0.03],
            credit_spread_path: vec![0.01, 0.01],
            time_points: vec![0.5, 0.0],
            payment_dates: vec![start, d2],
            stochastic_rates: false,
        };
        assert!(
            non_monotone.validate().is_err(),
            "non-increasing time_points must error"
        );
    }

    /// Regression test for parallel MC determinism.
    ///
    /// After parallelizing the Philox path loop with rayon, two runs with the
    /// same seed must produce bit-identical PVs and per-path values regardless
    /// of how many cores rayon uses to execute. This guards against any RNG
    /// substream initialization regressions or accidental order-dependence in
    /// the parallel `collect()` pipeline.
    #[test]
    fn parallel_mc_is_deterministic_across_runs() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

        let make_facility = || {
            RevolvingCredit::builder()
                .id("RC-DETERMINISM".into())
                .commitment_amount(Money::new(1_000_000.0, Currency::USD))
                .drawn_amount(Money::new(400_000.0, Currency::USD))
                .commitment_date(start)
                .maturity(end)
                .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
                .day_count(DayCount::Act360)
                .frequency(Tenor::quarterly())
                .fees(RevolvingCreditFees::default())
                .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                    StochasticUtilizationSpec {
                        utilization_process: UtilizationProcess::MeanReverting {
                            target_rate: 0.5,
                            speed: 0.75,
                            volatility: 0.05,
                        },
                        num_paths: 64,
                        seed: Some(123_456_789),
                        antithetic: true,
                        use_sobol_qmc: false,
                        mc_config: Some(McConfig {
                            recovery_rate: 0.4,
                            credit_spread_process: CreditSpreadProcessSpec::Constant(0.0),
                            interest_rate_process: None,
                            correlation_matrix: None,
                            util_credit_corr: None,
                        }),
                    },
                )))
                .discount_curve_id("USD-OIS".into())
                .recovery_rate(0.4)
                .build()
                .expect("facility should build")
        };

        let disc_curve = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.03f64).exp()),
                (5.0, (-0.03f64 * 5.0).exp()),
            ])
            .build()
            .expect("curve should build");
        let market = MarketContext::new().insert(disc_curve);

        let r1 = RevolvingCreditPricer::price_with_paths(&make_facility(), &market, start)
            .expect("first run should price");
        let r2 = RevolvingCreditPricer::price_with_paths(&make_facility(), &market, start)
            .expect("second run should price");

        assert_eq!(r1.path_results.len(), r2.path_results.len());
        // Mean PV must be bit-identical (same seed → same paths → same PVs).
        let m1 = r1.mc_result.estimate.mean.amount();
        let m2 = r2.mc_result.estimate.mean.amount();
        assert_eq!(
            m1.to_bits(),
            m2.to_bits(),
            "parallel MC must be deterministic for fixed seed; got mean1={m1} mean2={m2}"
        );
        for (i, (p1, p2)) in r1
            .path_results
            .iter()
            .zip(r2.path_results.iter())
            .enumerate()
        {
            let v1 = p1.pv.amount();
            let v2 = p2.pv.amount();
            assert_eq!(
                v1.to_bits(),
                v2.to_bits(),
                "path {i} PV diverges between runs: {v1} vs {v2}"
            );
        }
    }
}
