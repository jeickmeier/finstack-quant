//! Time-grid mapping, exercise-candidate generation, and conditional discount-factor helpers.

use super::BondValuator;
use crate::bond::Bond;
use finstack_quant_core::Result;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use time::Duration;

impl BondValuator {
    /// Nearest grid step to a time (year fraction), clamped to the grid.
    pub(super) fn nearest_step(time_steps: &[f64], t: f64) -> usize {
        let n = time_steps.len() - 1;
        if t <= time_steps[0] {
            return 0;
        }
        if t >= time_steps[n] {
            return n;
        }
        let upper = time_steps.partition_point(|&g| g <= t);
        let lower = upper - 1;
        if (t - time_steps[lower]) <= (time_steps[upper] - t) {
            lower
        } else {
            upper
        }
    }

    /// Continuous (fractional) grid position of a time, clamped to
    /// `[0, n]`. The integer part is the lower bracketing step; the
    /// fractional part is the position within that interval.
    pub(super) fn fractional_step(time_steps: &[f64], t: f64) -> f64 {
        let n = time_steps.len() - 1;
        if t <= time_steps[0] {
            return 0.0;
        }
        if t >= time_steps[n] {
            return n as f64;
        }
        let upper = time_steps.partition_point(|&g| g <= t);
        let lower = upper - 1;
        let segment = time_steps[upper] - time_steps[lower];
        lower as f64
            + if segment > 0.0 {
                (t - time_steps[lower]) / segment
            } else {
                0.0
            }
    }

    /// Collect the bond's mandatory tree-grid times (year fractions from
    /// `as_of`): all future cashflow dates plus every call/put exercise
    /// date. Used to calibrate a Hull-White tree whose grid passes exactly
    /// through coupon and exercise events.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the discount curve is missing or day-count
    /// computation fails.
    pub(crate) fn mandatory_grid_times(
        bond: &Bond,
        market_context: &MarketContext,
        as_of: Date,
    ) -> Result<Vec<f64>> {
        let discount_curve = market_context.get_discount(&bond.discount_curve_id)?;
        // Grid times are measured on the discount curve's own day count.
        let grid_day_count = discount_curve.day_count();
        let schedule = bond.full_cashflow_schedule(market_context)?;
        let flows = bond.pricing_dated_cashflows_from_schedule(&schedule, as_of, as_of)?;
        let event_dates = Self::exercise_event_dates(&schedule);
        let mut dates: Vec<Date> = flows
            .iter()
            .map(|(date, _)| *date)
            .filter(|d| *d > as_of)
            .collect();
        if let Some(ref call_put) = bond.call_put {
            for opt in call_put.calls.iter().chain(call_put.puts.iter()) {
                dates.extend(Self::right_exercise_dates(
                    opt,
                    as_of,
                    bond.maturity,
                    &event_dates,
                ));
            }
        }
        dates.sort_unstable();
        dates.dedup();

        dates
            .into_iter()
            .map(|d| grid_day_count.year_fraction(as_of, d, DayCountContext::default()))
            .collect()
    }

    /// Exercise dates of a right whose window is clipped to `[first, last]`:
    /// the window ends, every event date strictly inside it and every
    /// month-end inside it.
    ///
    /// Callers pass the bond's schedule dates (coupons, amortization) and, for
    /// return floors, the protection and contractual-call breakpoints. This
    /// replaces exercise on every calendar day, which put one tree node per
    /// day of a window. Measured on six callable, puttable and return-floor
    /// fixtures (`fixed-income/tests/bond/exercise_grid.rs`) the candidate set
    /// prices within 0.005 per 100 of daily exercise; window ends and coupon
    /// dates alone moved them by up to 0.094 per 100, so the month-end
    /// anchors stay. A 5-year window drops from ~1,830 exercise dates to ~70.
    ///
    /// Returns an empty set when `first > last`.
    pub(crate) fn exercise_candidates(first: Date, last: Date, event_dates: &[Date]) -> Vec<Date> {
        if first > last {
            return Vec::new();
        }
        let mut dates = Vec::with_capacity(event_dates.len() + 2);
        dates.push(first);
        dates.extend(
            event_dates
                .iter()
                .copied()
                .filter(|date| *date > first && *date < last),
        );
        let mut month_end = first.end_of_month();
        while month_end < last {
            dates.push(month_end);
            month_end = (month_end + Duration::days(1)).end_of_month();
        }
        dates.push(last);
        dates.sort_unstable();
        dates.dedup();
        dates
    }

    /// Sorted, distinct dates of every flow in `schedule`: the event dates
    /// that split an exercise window into candidate decisions.
    pub(crate) fn exercise_event_dates(
        schedule: &finstack_quant_cashflows::builder::CashFlowSchedule,
    ) -> Vec<Date> {
        let mut dates: Vec<Date> = schedule
            .get_flows()
            .iter()
            .map(|flow| flow.get_date())
            .collect();
        dates.sort_unstable();
        dates.dedup();
        dates
    }

    /// Exercise candidates of one call/put right, clipped to `[as_of, maturity]`.
    pub(super) fn right_exercise_dates(
        right: &crate::bond::CallPut,
        as_of: Date,
        maturity: Date,
        event_dates: &[Date],
    ) -> Vec<Date> {
        Self::exercise_candidates(right.start.max(as_of), right.end.min(maturity), event_dates)
    }

    pub(crate) fn make_whole_call_price(
        spec: &crate::bond::MakeWholeSpec,
        reference_curve: &dyn finstack_quant_core::market_data::traits::Discounting,
        exercise_date: Date,
        flows: &[(Date, finstack_quant_core::money::Money)],
        floor_price: f64,
        accrued_cash: f64,
    ) -> Result<f64> {
        let spread = spec.spread_bp / 10_000.0;

        let mut pv_remaining = 0.0;
        for (payment_date, amount) in flows {
            if *payment_date <= exercise_date {
                continue;
            }
            let amount = amount.amount();
            if amount.abs() <= f64::EPSILON {
                continue;
            }
            let tau = reference_curve.day_count().year_fraction(
                exercise_date,
                *payment_date,
                DayCountContext::default(),
            )?;
            let df_ratio = reference_curve.df_between_dates(exercise_date, *payment_date)?;
            pv_remaining += amount * df_ratio * (-spread * tau).exp();
        }

        // `pv_remaining` is the dirty reference value at the exercise date:
        // the next full coupon includes interest accrued through that date.
        // Compare like with like by removing accrued before applying the clean
        // contractual floor. The caller adds accrued exactly once afterward.
        Ok(floor_price.max(pv_remaining - accrued_cash))
    }

    /// Conditional discount factors from `as_of` to each model slice,
    /// obtained only through calendar-date curve queries. For a slice between
    /// two calendar dates, interpolate log discount factors on the model
    /// day-count coordinate. This mirrors the rates-credit target builder and
    /// avoids passing ACT/365F coordinates to a curve whose own basis differs.
    pub(super) fn conditional_discount_factors_on_grid(
        as_of: Date,
        maturity: Date,
        grid_day_count: DayCount,
        time_steps: &[f64],
        discount_curve: &dyn finstack_quant_core::market_data::traits::Discounting,
    ) -> Result<Vec<f64>> {
        let span_days = (maturity - as_of).whole_days();
        if span_days <= 0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BondValuator grid maturity {maturity} must be after origin {as_of}"
            )));
        }

        let context = DayCountContext::default();
        let grid_horizon = grid_day_count.year_fraction(as_of, maturity, context)?;
        let maturity_df = discount_curve.df_between_dates(as_of, maturity)?;
        if !grid_horizon.is_finite() || grid_horizon <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BondValuator grid horizon must be positive and finite, got {grid_horizon}"
            )));
        }
        if !maturity_df.is_finite() || maturity_df <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BondValuator conditional discount factor at maturity must be positive and finite, got {maturity_df}"
            )));
        }
        let mut factors = Vec::with_capacity(time_steps.len());

        for &step_time in time_steps {
            if step_time <= 0.0 {
                factors.push(1.0);
                continue;
            }
            if step_time >= grid_horizon {
                factors.push(maturity_df);
                continue;
            }

            let mut lower_days = 0_i64;
            let mut upper_days = span_days;
            while lower_days + 1 < upper_days {
                let mid_days = lower_days + (upper_days - lower_days) / 2;
                let mid_date = as_of + Duration::days(mid_days);
                let mid_time = grid_day_count.year_fraction(as_of, mid_date, context)?;
                if mid_time <= step_time {
                    lower_days = mid_days;
                } else {
                    upper_days = mid_days;
                }
            }

            let lower_date = as_of + Duration::days(lower_days);
            let upper_date = as_of + Duration::days(upper_days);
            let lower_time = grid_day_count.year_fraction(as_of, lower_date, context)?;
            let upper_time = grid_day_count.year_fraction(as_of, upper_date, context)?;
            let lower_df = discount_curve.df_between_dates(as_of, lower_date)?;
            let upper_df = discount_curve.df_between_dates(as_of, upper_date)?;
            if !lower_df.is_finite() || !upper_df.is_finite() || lower_df <= 0.0 || upper_df <= 0.0
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "BondValuator requires positive finite conditional discount factors, got {lower_df} and {upper_df}"
                )));
            }
            let denominator = upper_time - lower_time;
            let weight = if denominator.abs() <= f64::EPSILON {
                1.0
            } else {
                ((step_time - lower_time) / denominator).clamp(0.0, 1.0)
            };
            factors.push(((1.0 - weight) * lower_df.ln() + weight * upper_df.ln()).exp());
        }

        Ok(factors)
    }

    pub(super) fn value_at_step_time(
        cash_value_at_event_time: f64,
        event_date: Date,
        step: usize,
        step_discount_factors: &[f64],
        discount_curve: &dyn finstack_quant_core::market_data::traits::Discounting,
        as_of: Date,
    ) -> Result<f64> {
        let step_df = step_discount_factors.get(step).copied().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "BondValuator step {step} is outside its discount-factor grid"
            ))
        })?;
        if step_df <= f64::EPSILON {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BondValuator conditional discount factor at step {step} must be positive, got {step_df}"
            )));
        }
        let event_df = discount_curve.df_between_dates(as_of, event_date)?;
        if !event_df.is_finite() || event_df <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BondValuator conditional discount factor for event date {event_date} must be positive and finite, got {event_df}"
            )));
        }
        Ok(cash_value_at_event_time * event_df / step_df)
    }
}
