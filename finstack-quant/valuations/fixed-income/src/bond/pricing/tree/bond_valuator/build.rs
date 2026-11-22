//! Step-table construction for [`BondValuator`]: principal path, cashflows, redemptions, and exercise barriers.

use super::BondValuator;
use crate::bond::Bond;
use finstack_quant_core::Result;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;

impl BondValuator {
    /// Create a bond valuator on an explicit time grid and model day-count
    /// basis. This is the rates-credit entry point: callers pass
    /// [`DayCount::Act365F`] so every balance, cashflow, reset, and exercise
    /// date is mapped on the same axis used by that lattice.
    ///
    /// Timing corrections use conditional discount factors evaluated by
    /// calendar date. They therefore remain valid when `grid_day_count`
    /// differs from the discount curve's own day-count convention.
    pub(crate) fn new_with_time_steps_and_day_count(
        bond: Bond,
        market_context: &MarketContext,
        as_of: Date,
        time_steps: Vec<f64>,
        grid_day_count: DayCount,
    ) -> Result<Self> {
        use finstack_quant_core::cashflow::CFKind;

        if time_steps.len() < 2
            || time_steps[0].abs() > 1e-12
            || !time_steps.windows(2).all(|w| w[1] > w[0])
        {
            return Err(finstack_quant_core::Error::Validation(
                "BondValuator time grid must start at zero and be strictly increasing with at least 2 points"
                    .to_string(),
            ));
        }
        let num_steps = time_steps.len();

        let discount_curve = market_context.get_discount(&bond.discount_curve_id)?;
        let full_schedule = bond.full_cashflow_schedule(market_context)?;

        // Exercise windows are inclusive of the valuation date. Contractual
        // holder cash on that date is paid before the exercise decision, while
        // ordinary settlement pricing continues to exclude `as_of` cashflows.
        let event_dates = Self::exercise_event_dates(&full_schedule);
        let mut exercise_dates = std::collections::HashSet::new();
        if let Some(ref call_put) = bond.call_put {
            for option in call_put.calls.iter().chain(&call_put.puts) {
                exercise_dates.extend(Self::right_exercise_dates(
                    option,
                    as_of,
                    bond.maturity,
                    &event_dates,
                ));
            }
        }
        let flows = if exercise_dates.contains(&as_of) {
            bond.pricing_dated_cashflows_from_schedule_inclusive(&full_schedule, as_of, as_of)?
        } else {
            bond.pricing_dated_cashflows_from_schedule(&full_schedule, as_of, as_of)?
        };
        // A business-day adjustment can move the final contractual payment
        // beyond the unadjusted bond maturity. Use the actual final payment
        // as the date-axis bound; a risk-free tree that still ends at
        // contractual maturity receives the exact DF(payment)/DF(slice)
        // correction, while the daily rates-credit grid can extend through
        // the adjusted payment itself.
        let grid_horizon_date = flows
            .iter()
            .map(|(date, _)| *date)
            .max()
            .unwrap_or(bond.maturity)
            .max(bond.maturity);
        let step_discount_factors = Self::conditional_discount_factors_on_grid(
            as_of,
            grid_horizon_date,
            grid_day_count,
            &time_steps,
            discount_curve.as_ref(),
        )?;

        // Build the outstanding principal schedule from the canonical replay.
        // This captures historical and future amortization, PIK capitalization,
        // prepayments, and notional events instead of reconstructing only future
        // positive amortization from original par.
        let outstanding_by_date = full_schedule.outstanding_by_date()?;
        let mut outstanding_principal_vec = vec![bond.notional.amount(); num_steps];
        let recovery_path =
            crate::bond::pricing::settlement::recovery_principal_path(&full_schedule)?;
        let mut balance_events = Vec::with_capacity(recovery_path.len());
        for (date, balance) in &recovery_path {
            // Historical events all belong to the initial node. Some day-count
            // implementations reject reversed date ranges, so do not ask them
            // to manufacture negative model times.
            let event_time = if *date <= as_of {
                0.0
            } else {
                grid_day_count.year_fraction(as_of, *date, DayCountContext::default())?
            };
            balance_events.push((event_time, balance.amount()));
        }
        let mut event_idx = 0;
        let mut outstanding = bond.notional.amount();
        for (step, step_time) in time_steps.iter().copied().enumerate() {
            while event_idx < balance_events.len()
                && balance_events[event_idx].0 <= step_time + f64::EPSILON
            {
                outstanding = balance_events[event_idx].1;
                event_idx += 1;
            }
            outstanding_principal_vec[step] = outstanding.max(0.0);
        }

        // Pre-allocate vectors for O(1) access during backward induction
        let mut cashflow_vec = vec![0.0; num_steps];
        let mut cashflow_components = vec![Vec::new(); num_steps];
        for (date, amount) in &flows {
            if *date >= as_of {
                let time_frac =
                    grid_day_count.year_fraction(as_of, *date, DayCountContext::default())?;
                // Continuous (fractional) grid position of the cashflow,
                // clamped to the grid.
                let raw_clamped = Self::fractional_step(&time_steps, time_frac);

                // When a cashflow date matches an exercise date, snap to the
                // exercise step to prevent timing mismatches between coupon
                // receipt and exercise decision.
                if exercise_dates.contains(date) {
                    let step = Self::nearest_step(&time_steps, time_frac);
                    let adjusted_amount = Self::value_at_step_time(
                        amount.amount(),
                        *date,
                        step,
                        &step_discount_factors,
                        discount_curve.as_ref(),
                        as_of,
                    )?;
                    cashflow_vec[step] += adjusted_amount;
                    cashflow_components[step].push((adjusted_amount, time_frac - time_steps[step]));
                } else {
                    // Distributed mapping: spread cashflow between the two
                    // nearest time steps to reduce discretization error and
                    // improve convergence.
                    //
                    // Each distributed piece lands at a *step time* that
                    // differs from the coupon's true `time_frac`. Apply the
                    // same discount-factor correction the exercise-coincident
                    // path uses (`value_at_step_time`): a piece destined for
                    // `step_time` is scaled by `DF(time_frac) / DF(step_time)`
                    // so that, once the tree discounts it from `step_time`, its
                    // present value equals the coupon's PV at its true time.
                    // Without this correction the linear split silently
                    // mis-times discounting.

                    // Lower step index
                    let step_idx = raw_clamped.floor() as usize;

                    // Weight for the upper step (fractional part)
                    let weight = raw_clamped - step_idx as f64;

                    // Distribute to step_idx (weight: 1.0 - weight). Step 0 is
                    // included: backward induction reads cashflow_vec[0], and
                    // value_at_step_time corrects the DF timing for a piece
                    // booked at t=0, so a coupon inside the first time step
                    // keeps its full (1 - weight) share.
                    if step_idx < num_steps {
                        let adjusted_amount = Self::value_at_step_time(
                            amount.amount() * (1.0 - weight),
                            *date,
                            step_idx,
                            &step_discount_factors,
                            discount_curve.as_ref(),
                            as_of,
                        )?;
                        cashflow_vec[step_idx] += adjusted_amount;
                        cashflow_components[step_idx]
                            .push((adjusted_amount, time_frac - time_steps[step_idx]));
                    }

                    // Distribute to step_idx + 1 (weight: weight)
                    if step_idx + 1 < num_steps {
                        let adjusted_amount = Self::value_at_step_time(
                            amount.amount() * weight,
                            *date,
                            step_idx + 1,
                            &step_discount_factors,
                            discount_curve.as_ref(),
                            as_of,
                        )?;
                        cashflow_vec[step_idx + 1] += adjusted_amount;
                        cashflow_components[step_idx + 1]
                            .push((adjusted_amount, time_frac - time_steps[step_idx + 1]));
                    }
                }
            }
        }

        // Keep positive notional settlements separate so terminal exercise can
        // replace contractual redemption while leaving coupons and scheduled
        // amortization payable exactly once.
        let mut redemption_vec = vec![0.0; num_steps];
        let mut redemption_components = vec![Vec::new(); num_steps];
        for flow in full_schedule.get_flows().iter().filter(|flow| {
            (flow.get_date() > as_of
                || (flow.get_date() == as_of && exercise_dates.contains(&as_of)))
                && flow.get_kind() == CFKind::Notional
                && flow.get_amount().amount() > 0.0
        }) {
            let event_time =
                grid_day_count.year_fraction(as_of, flow.get_date(), DayCountContext::default())?;
            let raw_clamped = Self::fractional_step(&time_steps, event_time);
            if exercise_dates.contains(&flow.get_date()) {
                let step = Self::nearest_step(&time_steps, event_time);
                let adjusted = Self::value_at_step_time(
                    flow.get_amount().amount(),
                    flow.get_date(),
                    step,
                    &step_discount_factors,
                    discount_curve.as_ref(),
                    as_of,
                )?;
                redemption_vec[step] += adjusted;
                redemption_components[step].push((adjusted, event_time - time_steps[step]));
            } else {
                let lower = raw_clamped.floor() as usize;
                let upper_weight = raw_clamped - lower as f64;
                if lower < num_steps {
                    let adjusted = Self::value_at_step_time(
                        flow.get_amount().amount() * (1.0 - upper_weight),
                        flow.get_date(),
                        lower,
                        &step_discount_factors,
                        discount_curve.as_ref(),
                        as_of,
                    )?;
                    redemption_vec[lower] += adjusted;
                    redemption_components[lower].push((adjusted, event_time - time_steps[lower]));
                }
                if lower + 1 < num_steps {
                    let adjusted = Self::value_at_step_time(
                        flow.get_amount().amount() * upper_weight,
                        flow.get_date(),
                        lower + 1,
                        &step_discount_factors,
                        discount_curve.as_ref(),
                        as_of,
                    )?;
                    redemption_vec[lower + 1] += adjusted;
                    redemption_components[lower + 1]
                        .push((adjusted, event_time - time_steps[lower + 1]));
                }
            }
        }

        // Sparse vectors for call/put (most steps have no option)
        // Call/put redemption uses outstanding principal at exercise date, not original notional.
        let mut call_vec: Vec<Option<f64>> = vec![None; num_steps];
        let mut put_vec: Vec<Option<f64>> = vec![None; num_steps];
        let mut call_barriers_by_date = std::collections::BTreeMap::<Date, f64>::new();
        let mut put_barriers_by_date = std::collections::BTreeMap::<Date, f64>::new();
        let exercise_outstanding = |date: Date| {
            let path_idx =
                outstanding_by_date.partition_point(|(event_date, _)| *event_date <= date);
            let after_events = if path_idx == 0 {
                bond.notional.amount()
            } else {
                outstanding_by_date[path_idx - 1].1.amount()
            };
            if date == bond.maturity {
                // The final positive notional flow zeroes the replayed balance,
                // but a maturity option is quoted on principal immediately
                // before that contractual redemption. Same-day amortization has
                // already reduced the balance at this point.
                after_events
                    + full_schedule
                        .get_flows()
                        .iter()
                        .filter(|flow| {
                            flow.get_balance_date() == Some(date)
                                && flow.get_kind() == CFKind::Notional
                                && flow.get_amount().amount() > 0.0
                        })
                        .map(|flow| flow.get_amount().amount())
                        .sum::<f64>()
            } else {
                after_events
            }
            .max(0.0)
        };
        // No recovery interval follows the terminal node. Keep its principal
        // state immediately before final redemption so maturity exercise and
        // call friction use the contractual balance rather than the zero
        // post-redemption replay value.
        if let Some(last) = outstanding_principal_vec.last_mut() {
            *last = exercise_outstanding(bond.maturity);
        }
        if let Some(ref call_put) = bond.call_put {
            let accrual_cfg = bond.accrual_config();
            let accrual_index = finstack_quant_cashflows::accrual::AccrualIndex::build(
                &full_schedule,
                &accrual_cfg,
            )?;
            for call in &call_put.calls {
                let make_whole_reference = call
                    .make_whole
                    .as_ref()
                    .map(|spec| market_context.get_discount(&spec.reference_curve_id))
                    .transpose()?;
                for exercise_date in
                    Self::right_exercise_dates(call, as_of, bond.maturity, &event_dates)
                {
                    let exercise_time = grid_day_count.year_fraction(
                        as_of,
                        exercise_date,
                        DayCountContext::default(),
                    )?;
                    let step = Self::nearest_step(&time_steps, exercise_time);
                    let outstanding = exercise_outstanding(exercise_date);
                    let floor_price = outstanding * (call.price_pct_of_par / 100.0);
                    let accrued_on_call = accrual_index.accrued_at(exercise_date)?.amount();
                    let clean_call_price = if let (Some(spec), Some(reference_curve)) =
                        (&call.make_whole, &make_whole_reference)
                    {
                        Self::make_whole_call_price(
                            spec,
                            reference_curve.as_ref(),
                            exercise_date,
                            &flows,
                            floor_price,
                            accrued_on_call,
                        )?
                    } else {
                        floor_price
                    };
                    let dirty_call_price = clean_call_price + accrued_on_call;
                    call_barriers_by_date
                        .entry(exercise_date)
                        .and_modify(|existing| *existing = existing.min(dirty_call_price))
                        .or_insert(dirty_call_price);
                    let call_price = Self::value_at_step_time(
                        dirty_call_price,
                        exercise_date,
                        step,
                        &step_discount_factors,
                        discount_curve.as_ref(),
                        as_of,
                    )?;
                    // Several dates can map to one step of a uniform (BDT /
                    // Ho-Lee) grid; the node keeps the cheapest call among
                    // them. Measured against pricing each step at its nearest
                    // date (5y 5% bond, 4y American call at par, BDT 20%):
                    // 0.045 per 100 at 25 steps, 0.007 at 50, 0.003 at 100,
                    // 0.002 at 400. Hull-White grids pass through every
                    // exercise date, so no bucket is shared there.
                    call_vec[step] = Some(
                        call_vec[step].map_or(call_price, |existing| existing.min(call_price)),
                    );
                }
            }
            for put in &call_put.puts {
                for exercise_date in
                    Self::right_exercise_dates(put, as_of, bond.maturity, &event_dates)
                {
                    let exercise_time = grid_day_count.year_fraction(
                        as_of,
                        exercise_date,
                        DayCountContext::default(),
                    )?;
                    let step = Self::nearest_step(&time_steps, exercise_time);
                    // Use outstanding principal at exercise step, not original notional
                    let outstanding = exercise_outstanding(exercise_date);
                    let clean_put_price = outstanding * (put.price_pct_of_par / 100.0);
                    let accrued_on_put = accrual_index.accrued_at(exercise_date)?.amount();
                    let dirty_put_price = clean_put_price + accrued_on_put;
                    put_barriers_by_date
                        .entry(exercise_date)
                        .and_modify(|existing| *existing = existing.max(dirty_put_price))
                        .or_insert(dirty_put_price);
                    let put_price = Self::value_at_step_time(
                        dirty_put_price,
                        exercise_date,
                        step,
                        &step_discount_factors,
                        discount_curve.as_ref(),
                        as_of,
                    )?;
                    put_vec[step] =
                        Some(put_vec[step].map_or(put_price, |existing| existing.max(put_price)));
                }
            }

            for (date, put_price) in &put_barriers_by_date {
                if let Some(call_price) = call_barriers_by_date.get(date)
                    && *put_price > call_price + 1e-10
                {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Bond '{}' has incompatible call/put barriers on {}: holder put payoff {} exceeds issuer call payoff {}",
                        bond.id.as_str(),
                        date,
                        put_price,
                        call_price
                    )));
                }
            }
        }

        // Source recovery rate from the bond's explicit credit_curve_id,
        // consistent with the hazard-rate pricing kernel.
        let recovery_rate = Self::resolve_recovery_rate(&bond, market_context);
        let call_friction_cents = bond
            .instrument_pricing_overrides
            .model_config
            .call_friction_cents
            .unwrap_or(0.0);

        Ok(Self {
            bond,
            cashflow_vec,
            cashflow_components,
            redemption_vec,
            redemption_components,
            call_vec,
            put_vec,
            outstanding_principal_vec,
            time_steps,
            step_discount_factors,
            recovery_rate,
            call_friction_cents,
        })
    }
}
