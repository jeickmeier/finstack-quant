//! Deterministic single-path revolving-credit pricing.

use super::components::compute_upfront_fee_pv;
use super::results::PathResult;
use super::unified::RevolvingCreditPricer;
use crate::instruments::fixed_income::revolving_credit::cashflow_engine::{
    CashflowEngine, PathAwareCashflowSchedule, ThreeFactorPathData,
};
use crate::instruments::fixed_income::revolving_credit::types::{
    BaseRateSpec, DrawRepaySpec, RevolvingCredit,
};
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use rust_decimal::prelude::ToPrimitive;

/// Contractual credit margin of a facility for a draw on `date`: the spread
/// over the index for floating facilities; for fixed-rate facilities the
/// fixed rate less the simple par forward of the discount curve from `date`
/// to maturity (ACT/365F), the spread embedded in the fixed rate.
pub(super) fn contractual_margin(
    facility: &RevolvingCredit,
    disc_curve: &DiscountCurve,
    date: Date,
) -> Result<f64> {
    match &facility.base_rate_spec {
        BaseRateSpec::Floating(spec) => {
            spec.spread_bp
                .to_f64()
                .map(|bp| bp / 10_000.0)
                .ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "spread {} cannot be represented as f64",
                        spec.spread_bp
                    ))
                })
        }
        BaseRateSpec::Fixed { rate } => {
            if facility.maturity <= date {
                return Ok(*rate);
            }
            let df = disc_curve.df_between_dates(date, facility.maturity)?;
            let tau = DayCount::Act365F.year_fraction(
                date,
                facility.maturity,
                DayCountContext::default(),
            )?;
            let par = if df > 0.0 && tau > 0.0 {
                (1.0 / df - 1.0) / tau
            } else {
                0.0
            };
            Ok(rate - par)
        }
    }
}

/// The fair spread of a facility whose spread process is constant: the
/// configured constant, or zero for a facility without a hazard curve and
/// without an explicit Monte Carlo configuration (the pricer synthesizes a
/// zero constant spread for it). `None` when the spread is simulated.
fn constant_fair_spread(facility: &RevolvingCredit) -> Option<f64> {
    use crate::instruments::fixed_income::revolving_credit::types::CreditSpreadProcessSpec;
    let DrawRepaySpec::Stochastic(spec) = &facility.draw_repay_spec else {
        return None;
    };
    match spec.mc_config.as_ref() {
        Some(config) => match &config.credit_spread_process {
            CreditSpreadProcessSpec::Constant(spread) => Some(spread.max(0.0)),
            CreditSpreadProcessSpec::Cir { .. }
            | CreditSpreadProcessSpec::MarketAnchored { .. } => None,
        },
        None => facility.credit_curve_id.is_none().then_some(0.0),
    }
}

pub(super) fn resolve_fixings<'a>(
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
    /// Price one deterministic or MC path.
    ///
    /// Discounts scheduled cashflows with survival weights (static hazard or
    /// pathwise credit) and adds the upfront-fee PV.
    ///
    /// # Arguments
    ///
    /// * `facility` - Revolving credit facility being valued.
    /// * `market` - Curves and optional hazard used for discounting and survival.
    /// * `as_of` - Valuation date; survival is conditioned on this date.
    /// * `path_schedule` - Contractual or path-generated cashflows plus optional
    ///   3-factor path data.
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
                .get_flows()
                .iter()
                .map(|cf| cf.date)
                .collect();
            Self::compute_dynamic_survival_at_dates(
                &path_data.credit_spread_path,
                &path_data.time_points,
                &cashflow_dates,
                facility.recovery_rate,
                facility.commitment_date,
                super::super::MC_CLOCK_DAY_COUNT,
            )?
        } else if let Some(ref hazard_id) = facility.credit_curve_id {
            // Static survival from hazard curve
            let hazard = market.get_hazard(hazard_id.as_str())?;
            hazard.survival_at_dates(
                &path_schedule
                    .schedule
                    .get_flows()
                    .iter()
                    .map(|cf| cf.date)
                    .collect::<Vec<_>>(),
            )?
        } else {
            // No credit risk
            vec![1.0; path_schedule.schedule.get_flows().len()]
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
                super::super::MC_CLOCK_DAY_COUNT,
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
        let t_asof_path = super::super::MC_CLOCK_DAY_COUNT.signed_year_fraction(
            facility.commitment_date,
            as_of,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        let df_asof_to = |date: Date| -> Result<f64> {
            if let Some(p) = pathwise_rates {
                let t = super::super::MC_CLOCK_DAY_COUNT.signed_year_fraction(
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
        if survival_probs.len() != path_schedule.schedule.get_flows().len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "survival probability count {} does not match cashflow count {}",
                survival_probs.len(),
                path_schedule.schedule.get_flows().len()
            )));
        }

        let mut total_pv = 0.0;
        for (cf, survival_uncond) in path_schedule
            .schedule
            .get_flows()
            .iter()
            .zip(&survival_probs)
        {
            if cf.date < as_of {
                continue;
            }
            let df = df_asof_to(cf.date)?;
            let survival = *survival_uncond / sp_as_of;
            total_pv += cf.amount.amount() * df * survival;
        }

        // Default leg PV — trapezoidal integration on a monthly-or-finer grid.
        //
        //   PV_def = Σ PD(t-1, t) · DF(t) · [ R · E_drawn(t) + LEQ · U(t) · (R − 1) ]
        //
        // `E_drawn` is the drawn balance and `U` the undrawn commitment at the
        // grid date. The borrower is assumed to draw `leq` of the undrawn
        // commitment at default (loan-equivalent exposure, Basel CCF); the
        // lender funds that draw at par and recovers it at `R`, a net (R − 1)
        // per unit. With `leq = 0` this is the plain recovery leg.
        let commitment = facility.commitment_amount.amount();
        if facility.recovery_rate > 0.0 || facility.leq > 0.0 {
            let future_grid = Self::build_recovery_grid(facility, as_of, path_schedule)?;

            if !future_grid.is_empty() {
                let survival_at_grid = if let Some(ref path_data) = path_schedule.path_data {
                    Self::compute_dynamic_survival_at_dates(
                        &path_data.credit_spread_path,
                        &path_data.time_points,
                        &future_grid,
                        facility.recovery_rate,
                        facility.commitment_date,
                        super::super::MC_CLOCK_DAY_COUNT,
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
                    let undrawn_avg = (commitment - exposure_avg).max(0.0);

                    total_pv += df_avg
                        * prob_default
                        * (exposure_avg * facility.recovery_rate
                            + facility.leq * undrawn_avg * (facility.recovery_rate - 1.0));

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

        let draw_option_cost = match path_schedule.path_data.as_ref() {
            Some(path_data) if facility.is_stochastic() => Self::path_draw_option_cost(
                facility,
                as_of,
                path_data,
                disc_curve.as_ref(),
                &df_asof_to,
                sp_as_of,
            )?,
            _ => 0.0,
        };

        let result = PathResult {
            pv: Money::new(total_pv, facility.commitment_amount.currency())?,
            path_data: path_schedule.path_data.clone(),
            cashflows: path_schedule.schedule.clone(),
            draw_option_cost: Money::new(draw_option_cost, facility.commitment_amount.currency())?,
        };

        // Keep optional payloads live under `-D dead-code`:
        // callers expect to inspect cashflows and paths, and we also touch them here.
        let _ = result.cashflows.get_flows().len();
        let _ = result.path_data.is_some();

        Ok(result)
    }

    /// Option cost of the path's draws.
    ///
    /// Each draw `ΔD_j = C · (u_j − u_{j−1})⁺` at observation `j` is a
    /// forward loan to maturity at the contractual margin `s_K` against the
    /// path's fair spread `s(t_j)`, worth
    /// `ΔD_j · (s_K − s(t_j)) · A_j` to the lender, where
    /// `A_j = Σ_{m > j} DF(t_m) · SP(t_m) / SP(as_of) · dt_m` is the risky
    /// annuity of the remaining accrual periods on the path (facility day
    /// count, pathwise or curve discounting, pathwise survival). Negative
    /// when the path's spread sits above the margin.
    ///
    /// # Arguments
    ///
    /// * `facility` - Facility supplying commitment, margin, day count and maturity.
    /// * `as_of` - Valuation date; only draws after it count.
    /// * `path_data` - Simulated utilization and spread on the observation grid.
    /// * `disc_curve` - Deal discount curve, used for the fixed-rate margin's
    ///   par forward.
    /// * `df_asof_to` - Discount factor from `as_of` used for the path's cashflows.
    /// * `sp_as_of` - Survival to `as_of` the path survivals are conditioned on.
    fn path_draw_option_cost(
        facility: &RevolvingCredit,
        as_of: Date,
        path_data: &ThreeFactorPathData,
        disc_curve: &DiscountCurve,
        df_asof_to: &dyn Fn(Date) -> Result<f64>,
        sp_as_of: f64,
    ) -> Result<f64> {
        let dates = &path_data.payment_dates;
        let n = dates.len();
        if n < 2 || path_data.utilization_path.len() != n || path_data.credit_spread_path.len() != n
        {
            return Ok(0.0);
        }
        let survival = Self::compute_dynamic_survival_at_dates(
            &path_data.credit_spread_path,
            &path_data.time_points,
            dates,
            facility.recovery_rate,
            facility.commitment_date,
            super::super::MC_CLOCK_DAY_COUNT,
        )?;

        // Risky annuity of the accrual periods after each observation.
        let mut annuity_after = vec![0.0_f64; n];
        let mut running = 0.0;
        for m in (1..n).rev() {
            let (start, end) = (dates[m - 1], dates[m]);
            if start >= as_of && end <= facility.maturity && end > start {
                let dt =
                    facility
                        .day_count
                        .year_fraction(start, end, DayCountContext::default())?;
                running += df_asof_to(end)? * (survival[m] / sp_as_of) * dt;
            }
            annuity_after[m - 1] = running;
        }

        // A constant spread process is simulated with minimal CIR dynamics
        // for numerical stability; the fair spread it stands for is the
        // constant itself.
        let constant_spread = constant_fair_spread(facility);
        let commitment = facility.commitment_amount.amount();
        let mut cost = 0.0;
        for j in 1..n {
            if dates[j] <= as_of {
                continue;
            }
            let draw = commitment
                * (path_data.utilization_path[j] - path_data.utilization_path[j - 1]).max(0.0);
            if draw <= 0.0 {
                continue;
            }
            let margin = contractual_margin(facility, disc_curve, dates[j])?;
            let fair = constant_spread.unwrap_or_else(|| path_data.credit_spread_path[j].max(0.0));
            cost += draw * (margin - fair) * annuity_after[j];
        }
        Ok(cost)
    }

    /// Price a deterministic facility by generating its contractual schedule.
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

    pub(crate) fn compute_dynamic_survival_at_dates(
        credit_spreads: &[f64],
        time_points: &[f64],
        cashflow_dates: &[Date],
        recovery_rate: f64,
        commitment_date: Date,
        day_count: DayCount,
    ) -> Result<Vec<f64>> {
        use finstack_quant_core::dates::DayCountContext;

        // First, compute cumulative hazard at each payment date.
        //
        // Trapezoidal integration of the hazard path: the left-Riemann sum
        // used previously carried an O(dt) bias systematically correlated
        // with the direction of spread moves along the path (survival
        // overstated when spreads rise). Trapezoidal matches the
        // second-order accuracy of the recovery-leg integration.
        let mut cumulative_hazards = Vec::with_capacity(time_points.len());
        let mut cumulative_hazard = 0.0;
        cumulative_hazards.push(0.0); // At commitment date

        let loss_given_default = (1.0 - recovery_rate).max(1e-6);
        for i in 0..(credit_spreads.len() - 1) {
            let dt = time_points[i + 1] - time_points[i];
            let hazard_avg = 0.5 * (credit_spreads[i] + credit_spreads[i + 1]) / loss_given_default;
            cumulative_hazard += hazard_avg * dt;
            cumulative_hazards.push(cumulative_hazard);
        }

        // Now interpolate survival for each cashflow date
        let mut survival_probs = Vec::with_capacity(cashflow_dates.len());
        for &cf_date in cashflow_dates {
            let t_cf =
                day_count.year_fraction(commitment_date, cf_date, DayCountContext::default())?;

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
                        super::super::MC_CLOCK_DAY_COUNT,
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
