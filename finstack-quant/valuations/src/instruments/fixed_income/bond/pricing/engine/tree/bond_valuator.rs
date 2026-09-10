//! Pricing-engine components for fixed-income bonds.
//!
use super::super::super::super::types::Bond;
use super::TreePricer;
use crate::instruments::common_impl::pricing::rates_credit::continuous_frp_weight;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Duration};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;
use finstack_quant_models::trees::hull_white_tree::HullWhiteTree;
use finstack_quant_models::trees::two_factor_rates_credit::RatesCreditTree;
use finstack_quant_models::{NodeState, TreeValuator};

/// Bond valuator for tree-based pricing of callable/putable bonds.
///
/// Implements [`TreeValuator`] trait for backward induction pricing with embedded options.
/// Maps bond cashflows and call/put schedules to tree time steps and handles
/// exercise decisions during backward induction.
///
/// # Call/Put Redemption Convention
///
/// Call/put redemption prices are computed as `outstanding_principal × (price_pct_of_par / 100)`,
/// where `outstanding_principal` is the remaining principal at the exercise date after
/// any amortization. This correctly handles amortizing callable bonds.
///
/// # Performance
///
/// Uses `Vec` instead of `HashMap` for step-indexed lookups to eliminate hashing
/// overhead in the backward induction hot path. For a 200-step tree, this provides
/// significant speedup over hash-based lookups.
///
/// # Thread Safety
///
/// `BondValuator` is `Send + Sync` (all fields are owned data or primitives),
/// making it safe to share across threads for parallel portfolio pricing.
pub struct BondValuator {
    bond: Bond,
    /// Holder-view cashflow amounts indexed by time step (dense vector for O(1) access).
    /// Includes coupons, amortization, and final redemption — all positive receipts
    /// from the holder's perspective. Index `i` corresponds to time step `i`.
    /// Default value is 0.0.
    pub(super) cashflow_vec: Vec<f64>,
    /// Base-curve-timing-adjusted cashflow pieces and their event-minus-step
    /// time offsets. The offsets let OAS discounting preserve each original
    /// cashflow date even when a uniform tree grid is used.
    cashflow_components: Vec<Vec<(f64, f64)>>,
    /// Positive notional repayments mapped with the same timing convention as
    /// `cashflow_vec`. At the terminal node these are the contractual
    /// redemption alternative to a maturity call or put, rather than an
    /// unconditional payment in addition to exercise proceeds.
    redemption_vec: Vec<f64>,
    /// OAS timing components for `redemption_vec`.
    redemption_components: Vec<Vec<(f64, f64)>>,
    /// Call prices indexed by time step (sparse via Option for memory efficiency).
    /// `Some(price)` indicates a call option is exercisable at that step.
    /// Price is computed as `outstanding_principal × (price_pct / 100)`.
    pub(super) call_vec: Vec<Option<f64>>,
    /// Put prices indexed by time step (sparse via Option for memory efficiency).
    /// `Some(price)` indicates a put option is exercisable at that step.
    /// Price is computed as `outstanding_principal × (price_pct / 100)`.
    pub(super) put_vec: Vec<Option<f64>>,
    /// Outstanding principal indexed by time step for amortizing bonds.
    /// Used for call/put redemption and recovery calculations.
    pub(super) outstanding_principal_vec: Vec<f64>,
    /// Time steps for tree pricing
    time_steps: Vec<f64>,
    /// Base discount factors at each time step, conditional on `as_of`.
    step_discount_factors: Vec<f64>,
    /// Optional recovery rate sourced from a hazard curve in MarketContext
    recovery_rate: Option<f64>,
    /// Issuer call exercise friction in **cents per 100** of outstanding principal.
    ///
    /// This raises the exercise threshold (issuer calls only when continuation exceeds
    /// `call_price + friction_amount`), but redemption still occurs at `call_price`.
    call_friction_cents: f64,
}

impl BondValuator {
    /// Nearest grid step to a time (year fraction), clamped to the grid.
    fn nearest_step(time_steps: &[f64], t: f64) -> usize {
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
    fn fractional_step(time_steps: &[f64], t: f64) -> f64 {
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
        Self::mandatory_grid_times_with_day_count(
            bond,
            market_context,
            as_of,
            discount_curve.day_count(),
        )
    }

    /// Collect mandatory grid times on an explicitly selected model time
    /// basis. The rates-credit lattice uses this with ACT/365F, while the
    /// risk-free trees retain the discount curve's day count through
    /// [`Self::mandatory_grid_times`].
    pub(crate) fn mandatory_grid_times_with_day_count(
        bond: &Bond,
        market_context: &MarketContext,
        as_of: Date,
        grid_day_count: DayCount,
    ) -> Result<Vec<f64>> {
        let flows = bond.pricing_dated_cashflows(market_context, as_of)?;
        let cashflow_dates: Vec<Date> = flows.iter().map(|(date, _)| *date).collect();
        let mut dates: Vec<Date> = cashflow_dates
            .iter()
            .copied()
            .filter(|d| *d > as_of)
            .collect();
        if let Some(ref call_put) = bond.call_put {
            for opt in call_put.calls.iter().chain(call_put.puts.iter()) {
                dates.extend(Self::exercise_dates_for_period(
                    opt.start_date,
                    opt.end_date,
                    as_of,
                    bond.maturity,
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

    fn exercise_dates_for_period(
        start_date: Date,
        end_date: Date,
        as_of: Date,
        maturity: Date,
    ) -> Vec<Date> {
        // A call/put period is an exercise *window*: the option is exercisable
        // on every calendar date in `[start_date, end_date]`, including the
        // valuation date when the right is already active. Restricting the
        // window to endpoints and coupon dates misses economically valid
        // between-coupon exercise.
        let first = start_date.max(as_of);
        let last = end_date.min(maturity);
        if first > last {
            return Vec::new();
        }

        let mut dates = Vec::with_capacity((last - first).whole_days().max(0) as usize + 1);
        let mut date = first;
        loop {
            dates.push(date);
            if date == last {
                break;
            }
            let Some(next) = date.next_day() else {
                break;
            };
            date = next;
        }
        dates
    }

    pub(crate) fn make_whole_call_price(
        spec: &crate::instruments::fixed_income::bond::MakeWholeSpec,
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

    /// Create a new bond valuator for tree pricing.
    ///
    /// Builds maps of coupons, call prices, and put prices indexed by tree step.
    /// Cashflows and option exercise dates are mapped to the nearest tree step
    /// using the discount curve's day-count convention.
    ///
    /// # Arguments
    ///
    /// * `bond` - The bond to value
    /// * `market_context` - Market data including curves
    /// * `as_of` - Valuation date (time origin for the tree)
    /// * `time_to_maturity` - Time from `as_of` to maturity in years
    /// * `tree_steps` - Number of tree steps
    ///
    /// # Returns
    ///
    /// A `BondValuator` instance ready for tree-based pricing.
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - Discount curve is not found
    /// - Cashflow schedule building fails
    /// - Time fraction calculations fail
    ///
    /// # Time Axis Consistency
    ///
    /// The `as_of` date defines the time origin (t=0) for the tree. All cashflow
    /// times and option exercise times are measured from `as_of` using the discount
    /// curve's day-count convention to ensure consistency with tree calibration.
    pub fn new(
        bond: Bond,
        market_context: &MarketContext,
        as_of: Date,
        time_to_maturity: f64,
        tree_steps: usize,
    ) -> Result<Self> {
        let dt = time_to_maturity / tree_steps as f64;
        let time_steps: Vec<f64> = (0..=tree_steps).map(|i| i as f64 * dt).collect();
        Self::new_with_time_steps(bond, market_context, as_of, time_steps)
    }

    /// Create a bond valuator on an explicit (possibly non-uniform) time
    /// grid, e.g. the grid of a [`HullWhiteTree`] calibrated through
    /// mandatory call/coupon dates.
    ///
    /// `time_steps` must be a strictly increasing grid starting at `0.0`
    /// whose last entry is the time to maturity. Cashflows and exercise
    /// dates are mapped onto this grid by nearest-point lookup, with
    /// discount-factor timing corrections preserving each flow's PV.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the grid is invalid, curves are missing, or
    /// schedule/day-count computation fails.
    pub fn new_with_time_steps(
        bond: Bond,
        market_context: &MarketContext,
        as_of: Date,
        time_steps: Vec<f64>,
    ) -> Result<Self> {
        let grid_day_count = market_context
            .get_discount(&bond.discount_curve_id)?
            .day_count();
        Self::new_with_time_steps_and_day_count(
            bond,
            market_context,
            as_of,
            time_steps,
            grid_day_count,
        )
    }

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
        use crate::cashflow::primitives::CFKind;

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
        let mut exercise_dates = std::collections::HashSet::new();
        if let Some(ref call_put) = bond.call_put {
            for option in call_put.calls.iter().chain(&call_put.puts) {
                exercise_dates.extend(Self::exercise_dates_for_period(
                    option.start_date,
                    option.end_date,
                    as_of,
                    bond.maturity,
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
            crate::instruments::fixed_income::bond::pricing::principal::recovery_principal_path(
                &full_schedule,
            )?;
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
            (flow.date > as_of || (flow.date == as_of && exercise_dates.contains(&as_of)))
                && flow.kind == CFKind::Notional
                && flow.amount.amount() > 0.0
        }) {
            let event_time =
                grid_day_count.year_fraction(as_of, flow.date, DayCountContext::default())?;
            let raw_clamped = Self::fractional_step(&time_steps, event_time);
            if exercise_dates.contains(&flow.date) {
                let step = Self::nearest_step(&time_steps, event_time);
                let adjusted = Self::value_at_step_time(
                    flow.amount.amount(),
                    flow.date,
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
                        flow.amount.amount() * (1.0 - upper_weight),
                        flow.date,
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
                        flow.amount.amount() * upper_weight,
                        flow.date,
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
                            flow.get_balance_date() == date
                                && flow.kind == CFKind::Notional
                                && flow.amount.amount() > 0.0
                        })
                        .map(|flow| flow.amount.amount())
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
            let accrual_index =
                crate::cashflow::accrual::AccrualIndex::build(&full_schedule, &accrual_cfg)?;
            for call in &call_put.calls {
                for exercise_date in Self::exercise_dates_for_period(
                    call.start_date,
                    call.end_date,
                    as_of,
                    bond.maturity,
                ) {
                    let exercise_time = grid_day_count.year_fraction(
                        as_of,
                        exercise_date,
                        DayCountContext::default(),
                    )?;
                    let step = Self::nearest_step(&time_steps, exercise_time);
                    let outstanding = exercise_outstanding(exercise_date);
                    let floor_price = outstanding * (call.price_pct_of_par / 100.0);
                    let accrued_on_call = accrual_index.accrued_at(exercise_date)?;
                    let clean_call_price = if let Some(spec) = &call.make_whole {
                        let reference_curve =
                            market_context.get_discount(&spec.reference_curve_id)?;
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
                    call_vec[step] = Some(
                        call_vec[step].map_or(call_price, |existing| existing.min(call_price)),
                    );
                }
            }
            for put in &call_put.puts {
                for exercise_date in Self::exercise_dates_for_period(
                    put.start_date,
                    put.end_date,
                    as_of,
                    bond.maturity,
                ) {
                    let exercise_time = grid_day_count.year_fraction(
                        as_of,
                        exercise_date,
                        DayCountContext::default(),
                    )?;
                    let step = Self::nearest_step(&time_steps, exercise_time);
                    // Use outstanding principal at exercise step, not original notional
                    let outstanding = exercise_outstanding(exercise_date);
                    let clean_put_price = outstanding * (put.price_pct_of_par / 100.0);
                    let accrued_on_put = accrual_index.accrued_at(exercise_date)?;
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
                if let Some(call_price) = call_barriers_by_date.get(date) {
                    if *put_price > call_price + 1e-10 {
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
        }

        // Source recovery rate from the bond's explicit credit_curve_id,
        // consistent with HazardBondEngine and TreePricer::calculate_oas.
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

    /// Get the total holder-view cashflow amount at this time step.
    ///
    /// This includes coupons, amortization, and final redemption — all positive
    /// receipts from the holder's perspective.
    #[inline]
    fn cashflow_at(&self, step: usize) -> f64 {
        self.cashflow_vec.get(step).copied().unwrap_or(0.0)
    }

    /// Cashflow at a tree step with continuous OAS timing preserved at each
    /// original event date.
    #[inline]
    fn cashflow_at_oas(&self, step: usize, oas_rate: f64) -> f64 {
        if oas_rate.abs() <= f64::EPSILON {
            return self.cashflow_at(step);
        }
        self.cashflow_components
            .get(step)
            .map(|components| {
                components
                    .iter()
                    .map(|(amount, event_minus_step)| amount * (-oas_rate * event_minus_step).exp())
                    .sum()
            })
            .unwrap_or(0.0)
    }

    /// Positive contractual notional repayment at a tree step, preserving the
    /// original event date under OAS discounting.
    #[inline]
    fn redemption_at_oas(&self, step: usize, oas_rate: f64) -> f64 {
        if oas_rate.abs() <= f64::EPSILON {
            return self.redemption_vec.get(step).copied().unwrap_or(0.0);
        }
        self.redemption_components
            .get(step)
            .map(|components| {
                components
                    .iter()
                    .map(|(amount, event_minus_step)| amount * (-oas_rate * event_minus_step).exp())
                    .sum()
            })
            .unwrap_or(0.0)
    }

    /// Check if there's a call option at this time step.
    #[inline]
    fn call_at(&self, step: usize) -> Option<f64> {
        self.call_vec.get(step).copied().flatten()
    }

    /// Conditional discount factors from `as_of` to each model slice,
    /// obtained only through calendar-date curve queries. For a slice between
    /// two calendar dates, interpolate log discount factors on the model
    /// day-count coordinate. This mirrors the rates-credit target builder and
    /// avoids passing ACT/365F coordinates to a curve whose own basis differs.
    fn conditional_discount_factors_on_grid(
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

    fn value_at_step_time(
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

    /// Check if there's a put option at this time step.
    #[inline]
    fn put_at(&self, step: usize) -> Option<f64> {
        self.put_vec.get(step).copied().flatten()
    }

    /// Get outstanding principal at this time step.
    ///
    /// For bullet bonds, this returns the original notional.
    /// For amortizing bonds, this returns the remaining principal after amortization.
    #[inline]
    fn outstanding_principal_at(&self, step: usize) -> f64 {
        self.outstanding_principal_vec
            .get(step)
            .copied()
            .unwrap_or(self.bond.notional.amount())
    }

    /// Apply holder put and issuer call rights to a value that excludes current
    /// contractual payments. Call friction raises only the issuer's exercise
    /// threshold; it never changes holder proceeds.
    #[inline]
    fn apply_exercise(&self, step: usize, risky_continuation: f64) -> f64 {
        let mut exercised_or_held = risky_continuation;
        if let Some(put_price) = self.put_at(step) {
            exercised_or_held = exercised_or_held.max(put_price);
        }
        if let Some(call_price) = self.call_at(step) {
            let outstanding = self.outstanding_principal_at(step);
            let friction_amount = outstanding * (self.call_friction_cents / 10_000.0);
            if exercised_or_held > call_price + friction_amount {
                exercised_or_held = call_price;
            }
        }
        exercised_or_held
    }

    /// Terminal contractual payment with maturity exercise applied to the
    /// redemption component only. Coupons and scheduled amortization remain
    /// payable; the final notional repayment is the hold alternative.
    #[inline]
    fn terminal_value(&self, step: usize, oas_rate: f64) -> f64 {
        let current_payments = self.cashflow_at_oas(step, oas_rate);
        let contractual_redemption = self.redemption_at_oas(step, oas_rate);
        let other_payments = current_payments - contractual_redemption;
        other_payments + self.apply_exercise(step, contractual_redemption)
    }

    /// Price the bond using a calibrated Hull-White trinomial tree with OAS.
    ///
    /// Uses `HullWhiteTree::backward_induction` with the bond's cashflow and
    /// call/put schedules applied at each node. The OAS is applied as an
    /// additional parallel shift to the short rate when discounting.
    ///
    /// # Arguments
    ///
    /// * `hw_tree` - Calibrated Hull-White tree
    /// * `oas_bp` - Option-adjusted spread in basis points
    ///
    /// # Returns
    ///
    /// Model dirty price of the bond.
    ///
    /// # Errors
    ///
    /// Propagates tree backward-induction validation failures.
    pub(crate) fn price_with_hw_tree(&self, hw_tree: &HullWhiteTree, oas_bp: f64) -> Result<f64> {
        let final_step = hw_tree.num_steps();
        let comp = hw_tree.config().compounding;
        let oas_rate = oas_bp / 10_000.0;

        let terminal_cf = self.terminal_value(final_step, oas_rate);
        let terminal_values = vec![terminal_cf; hw_tree.num_nodes(final_step)];

        hw_tree.backward_induction(&terminal_values, |step, _node_idx, continuation| {
            // The HW tree's backward_induction already discounts by the short
            // rate r(step, node). Apply the OAS as additional discounting
            // over this step's (possibly non-uniform) interval.
            let oas_adjusted = continuation * comp.df(oas_rate, hw_tree.dt_at_step(step));

            self.cashflow_at_oas(step, oas_rate) + self.apply_exercise(step, oas_adjusted)
        })
    }

    /// Price a calibrated rates-credit tree whose rate and hazard factors are
    /// both deterministic. The collapsed lattice has one economically unique
    /// state per slice, so a scalar reverse pass avoids the cubic joint-node
    /// rollback required for stochastic factors while preserving the same
    /// risky-continuation, FRP recovery, exercise, and payment ordering.
    ///
    /// `oas_bp` is a continuously compounded spread in basis points, matching
    /// the internal quote conversion used by [`TreePricer`].
    pub(crate) fn price_deterministic_rates_credit(
        &self,
        tree: &RatesCreditTree,
        oas_bp: f64,
    ) -> Result<f64> {
        if tree.config.rate_vol != 0.0 || tree.config.hazard_vol != 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "deterministic rates-credit bond rollback requires zero rate_vol and hazard_vol, got {} and {}",
                tree.config.rate_vol, tree.config.hazard_vol
            )));
        }
        if !oas_bp.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "deterministic rates-credit bond OAS must be finite, got {oas_bp}"
            )));
        }

        let tree_times = tree.time_grid()?;
        if tree_times.len() != self.time_steps.len()
            || tree_times
                .iter()
                .zip(&self.time_steps)
                .any(|(tree_time, value_time)| {
                    let tolerance = 1.0e-12_f64.max(tree_time.abs() * 1.0e-10);
                    (tree_time - value_time).abs() > tolerance
                })
        {
            return Err(finstack_quant_core::Error::Validation(
                "deterministic rates-credit tree grid does not match BondValuator grid".to_string(),
            ));
        }

        let last = tree_times.len() - 1;
        let oas_rate = oas_bp / 10_000.0;
        let recovery_rate = tree.recovery_rate().clamp(0.0, 1.0);
        let mut value = self.terminal_value(last, oas_rate);

        for step in (0..last).rev() {
            let dt = tree_times[step + 1] - tree_times[step];
            let short_rate = tree.rate_at_node(step, 0)?;
            let hazard = tree.hazard_at_node(step, 0)?.max(0.0);
            let interval_discount = (-(short_rate + oas_rate) * dt).exp();
            let survival = (-hazard * dt).exp();
            let recovery_weight = continuous_frp_weight(interval_discount, survival)?;
            let recovery = recovery_rate * self.outstanding_principal_at(step) * recovery_weight;
            let risky_continuation = survival * interval_discount * value + recovery;
            value = self.cashflow_at_oas(step, oas_rate)
                + self.apply_exercise(step, risky_continuation);
        }

        Ok(value)
    }

    /// Price embedded options against the deterministic discount curve used to
    /// build this valuator. This is the zero-short-rate-volatility limit of the
    /// risk-free tree and avoids inventing volatility merely to satisfy a
    /// stochastic tree constructor.
    pub(crate) fn price_deterministic_discount_curve(&self, oas_bp: f64) -> Result<f64> {
        if !oas_bp.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "deterministic bond OAS must be finite, got {oas_bp}"
            )));
        }
        if self.step_discount_factors.len() != self.time_steps.len() {
            return Err(finstack_quant_core::Error::internal(
                "BondValuator discount-factor and time grids differ in length",
            ));
        }

        let last = self.time_steps.len() - 1;
        let oas_rate = oas_bp / 10_000.0;
        let mut value = self.terminal_value(last, oas_rate);
        for step in (0..last).rev() {
            let current_df = self.step_discount_factors[step];
            let next_df = self.step_discount_factors[step + 1];
            if !current_df.is_finite()
                || !next_df.is_finite()
                || current_df <= 0.0
                || next_df <= 0.0
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "BondValuator deterministic discount factors must be positive and finite at step {step}"
                )));
            }
            let dt = self.time_steps[step + 1] - self.time_steps[step];
            let interval_df = next_df / current_df * (-oas_rate * dt).exp();
            value = self.cashflow_at_oas(step, oas_rate)
                + self.apply_exercise(step, interval_df * value);
        }
        Ok(value)
    }

    /// Resolve the recovery rate from the bond's explicit `credit_curve_id`
    /// opt-in, consistent with `HazardBondEngine` and `TreePricer`.
    ///
    /// Returns `None` when the bond has no `credit_curve_id` or the named
    /// hazard curve is absent from the market context.
    fn resolve_recovery_rate(bond: &Bond, market: &MarketContext) -> Option<f64> {
        // Recovery comes only from the bond's explicit `credit_curve_id`
        // opt-in; implicit discovery by naming convention is not supported.
        let credit_id = bond.credit_curve_id.as_ref()?;
        market
            .get_hazard(credit_id.as_str())
            .ok()
            .map(|hc| hc.recovery_rate())
    }
}

impl TreeValuator for BondValuator {
    fn value_at_maturity(&self, state: &NodeState) -> Result<f64> {
        let final_step = self.time_steps.len() - 1;
        let oas_rate =
            state.get_var_or(finstack_quant_models::short_rate_keys::OAS, 0.0) / 10_000.0;
        Ok(self.terminal_value(final_step, oas_rate))
    }

    fn value_at_node(&self, state: &NodeState, continuation_value: f64, dt: f64) -> Result<f64> {
        let step = state.step;
        let oas_rate =
            state.get_var_or(finstack_quant_models::short_rate_keys::OAS, 0.0) / 10_000.0;
        // The tree has already rate/OAS-discounted `continuation_value` from
        // the child slice to this node. Apply only survival to that value, and
        // add continuously paid FRP recovery with the same interval discount.
        // Current contractual payments are at this node, so they are neither
        // survival weighted over the next interval nor included in recovery.
        let risky_continuation = if let Some(hazard) = state.hazard_rate {
            let hazard = hazard.max(0.0);
            let survival = (-hazard * dt).exp();
            let interval_df = state.discount_factor().unwrap_or_else(|| {
                let rate = state.interest_rate().unwrap_or(0.0) + oas_rate;
                (-rate * dt).exp()
            });
            let recovery_weight = continuous_frp_weight(interval_df, survival)?;
            let outstanding = self.outstanding_principal_at(step);
            let recovery = self
                .recovery_rate
                .map(|rr| rr.clamp(0.0, 1.0) * outstanding * recovery_weight)
                .unwrap_or(0.0);
            survival * continuation_value + recovery
        } else {
            continuation_value
        };

        let current_payments = self.cashflow_at_oas(step, oas_rate);
        Ok(current_payments + self.apply_exercise(step, risky_continuation))
    }
}

const _: () = {
    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}
    fn _assertions() {
        _assert_send::<BondValuator>();
        _assert_sync::<BondValuator>();
        _assert_send::<TreePricer>();
        _assert_sync::<TreePricer>();
    }
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::{CashFlowMeta, CashFlowSchedule, Notional};
    use crate::cashflow::primitives::{CFKind, CashFlow};
    use crate::instruments::fixed_income::bond::{
        Bond, CallPut, CallPutSchedule, CashflowSpec, MakeWholeSpec,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, DayCountContext, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::HashMap;
    use finstack_quant_models::trees::tree_framework::map_date_to_step;
    use finstack_quant_models::trees::two_factor_rates_credit::{
        RatesCreditCalibrationTargets, RatesCreditConfig,
    };
    use finstack_quant_models::{state_keys, NodeState};
    use time::macros::date;

    fn node_test_valuator(
        current_payment: f64,
        terminal_redemption: f64,
        call: [Option<f64>; 2],
        put: [Option<f64>; 2],
        outstanding: f64,
        recovery_rate: Option<f64>,
        call_friction_cents: f64,
    ) -> BondValuator {
        let as_of = date!(2025 - 01 - 01);
        let maturity = date!(2026 - 01 - 01);
        let bond = Bond::fixed(
            "NODE-TEST",
            Money::from((100_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.0).expect("valid rate fixture"),
            as_of,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        BondValuator {
            bond,
            cashflow_vec: vec![current_payment, current_payment + terminal_redemption],
            cashflow_components: vec![Vec::new(), Vec::new()],
            redemption_vec: vec![0.0, terminal_redemption],
            redemption_components: vec![Vec::new(), Vec::new()],
            call_vec: call.to_vec(),
            put_vec: put.to_vec(),
            outstanding_principal_vec: vec![outstanding, 0.0],
            time_steps: vec![0.0, 1.0],
            step_discount_factors: vec![1.0, 1.0],
            recovery_rate,
            call_friction_cents,
        }
    }

    #[test]
    fn risky_hold_is_survival_weighted_before_exercise_and_current_payment_is_not() {
        let valuator = node_test_valuator(5.0, 100.0, [None; 2], [None; 2], 100.0, Some(0.4), 0.0);
        let market = MarketContext::new();
        let rate: f64 = 0.03;
        let hazard: f64 = 0.10;
        let dt: f64 = 1.0;
        let interval_df = (-rate * dt).exp();
        let mut vars = HashMap::default();
        vars.insert(state_keys::INTEREST_RATE, rate);
        vars.insert(state_keys::HAZARD_RATE, hazard);
        vars.insert(state_keys::DF, interval_df);
        let state = NodeState::new(0, 0.0, &vars, &market);
        let discounted_continuation = 100.0 * interval_df;

        let actual = valuator
            .value_at_node(&state, discounted_continuation, dt)
            .expect("node value");
        let survival = (-hazard * dt).exp();
        let recovery =
            0.4 * 100.0 * hazard * (1.0 - (-(rate + hazard) * dt).exp()) / (rate + hazard);
        let expected = 5.0 + survival * discounted_continuation + recovery;
        assert!(
            (actual - expected).abs() < 1e-12,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn immediate_put_is_not_survival_discounted_and_ends_recovery_exposure() {
        let valuator = node_test_valuator(
            5.0,
            100.0,
            [None; 2],
            [Some(100.0), None],
            100.0,
            Some(0.4),
            0.0,
        );
        let market = MarketContext::new();
        let mut vars = HashMap::default();
        vars.insert(state_keys::INTEREST_RATE, 0.0);
        vars.insert(state_keys::HAZARD_RATE, 1.0);
        vars.insert(state_keys::DF, 1.0);
        let state = NodeState::new(0, 0.0, &vars, &market);

        let actual = valuator
            .value_at_node(&state, 0.0, 1.0)
            .expect("node value");
        assert_eq!(actual, 105.0);
    }

    #[test]
    fn within_step_recovery_has_stable_zero_denominator_limit() {
        let survival = (-0.10_f64).exp();
        let weight = continuous_frp_weight(0.10_f64.exp(), survival).expect("recovery weight");
        assert!((weight - 0.10).abs() < 1e-12, "weight={weight}");
    }

    #[test]
    fn deterministic_rates_credit_scalar_pass_matches_one_step_frp() {
        let valuator = node_test_valuator(0.0, 100.0, [None; 2], [None; 2], 100.0, Some(0.4), 0.0);
        let rate = 0.03_f64;
        let hazard = 0.10_f64;
        let discount = (-rate).exp();
        let survival = (-hazard).exp();
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps: 1,
            ..RatesCreditConfig::default()
        });
        tree.calibrate(&RatesCreditCalibrationTargets {
            times: vec![0.0, 1.0],
            discount_factors: vec![1.0, discount],
            survival_probabilities: vec![1.0, survival],
            recovery_rate: 0.4,
        })
        .expect("calibration");

        let actual = valuator
            .price_deterministic_rates_credit(&tree, 0.0)
            .expect("scalar price");
        let recovery =
            0.4 * 100.0 * continuous_frp_weight(discount, survival).expect("recovery weight");
        let expected = discount * survival * 100.0 + recovery;
        assert!(
            (actual - expected).abs() < 1.0e-12,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn terminal_put_replaces_redemption_and_preserves_coupon() {
        let valuator = node_test_valuator(
            50.0,
            1_000.0,
            [None; 2],
            [None, Some(1_100.0)],
            1_000.0,
            None,
            0.0,
        );
        let market = MarketContext::new();
        let vars = HashMap::default();
        let state = NodeState::new(1, 1.0, &vars, &market);

        let actual = valuator.value_at_maturity(&state).expect("terminal value");
        assert_eq!(actual, 1_150.0);
    }

    #[test]
    fn call_friction_changes_threshold_but_not_holder_proceeds() {
        let valuator = node_test_valuator(
            0.0,
            100.0,
            [Some(100.0), None],
            [None; 2],
            100.0,
            None,
            100.0,
        );
        assert_eq!(valuator.apply_exercise(0, 100.5), 100.5);
        assert_eq!(valuator.apply_exercise(0, 101.5), 100.0);
    }

    #[test]
    fn exercise_window_includes_every_calendar_date_and_as_of() {
        let dates = BondValuator::exercise_dates_for_period(
            date!(2025 - 01 - 01),
            date!(2025 - 01 - 03),
            date!(2025 - 01 - 01),
            date!(2026 - 01 - 01),
        );
        assert_eq!(
            dates,
            vec![
                date!(2025 - 01 - 01),
                date!(2025 - 01 - 02),
                date!(2025 - 01 - 03)
            ]
        );
    }

    #[test]
    fn explicit_grid_day_count_controls_event_mapping_when_curve_basis_differs() {
        let as_of = date!(2024 - 01 - 01);
        let exercise = date!(2024 - 06 - 29);
        let maturity = date!(2025 - 01 - 01);
        let mut bond = Bond::fixed(
            "ACT365F-GRID",
            Money::from((100_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.0).expect("valid rate fixture"),
            as_of,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: exercise,
                end_date: exercise,
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: vec![],
        });
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 1.0), (1.1, 0.94)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);
        let horizon = DayCount::Act365F
            .year_fraction(as_of, maturity, DayCountContext::default())
            .expect("ACT/365F horizon");
        // The exercise time is 180/365 = 0.49315. On the curve's ACT/360
        // basis it would be 0.5, which would snap to step 2 on this grid.
        let grid = vec![0.0, 0.49, 0.497, horizon];

        let valuator = BondValuator::new_with_time_steps_and_day_count(
            bond,
            &market,
            as_of,
            grid,
            DayCount::Act365F,
        )
        .expect("valuator");

        assert!(valuator.call_vec[1].is_some());
        assert!(valuator.call_vec[2].is_none());
    }

    #[test]
    fn canonical_balance_replay_carries_historical_amortization_and_pik() {
        let issue = date!(2024 - 01 - 01);
        let as_of = date!(2025 - 01 - 01);
        let maturity = date!(2026 - 01 - 01);
        let money = |amount| Money::new(amount, Currency::USD).expect("valid money fixture");
        let schedule = CashFlowSchedule::from_parts(
            vec![
                CashFlow::new(issue, None, money(-1_000.0), CFKind::Notional, 0.0, None),
                CashFlow::new(
                    date!(2024 - 06 - 01),
                    None,
                    money(200.0),
                    CFKind::Amortization,
                    0.0,
                    None,
                ),
                CashFlow::new(
                    date!(2024 - 12 - 01),
                    None,
                    money(50.0),
                    CFKind::Pik,
                    0.0,
                    None,
                ),
                CashFlow::new(maturity, None, money(850.0), CFKind::Notional, 0.0, None),
            ],
            Notional::par(1_000.0, Currency::USD).expect("valid notional fixture"),
            DayCount::Act365F,
            CashFlowMeta {
                issue_date: Some(issue),
                maturity_date: Some(maturity),
                ..CashFlowMeta::default()
            },
        );
        let mut bond =
            Bond::from_cashflows("SEASONED-PIK", schedule, "USD-OIS", None).expect("custom bond");
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: as_of,
                end_date: as_of,
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: vec![CallPut {
                start_date: maturity,
                end_date: maturity,
                price_pct_of_par: 110.0,
                make_whole: None,
            }],
        });
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);

        let valuator = BondValuator::new(bond, &market, as_of, 1.0, 4).expect("valuator");
        assert!((valuator.outstanding_principal_vec[0] - 850.0).abs() < 1e-12);
        assert_eq!(valuator.call_vec[0], Some(850.0));
        assert!((valuator.put_vec[4].expect("terminal put") - 935.0).abs() < 1e-12);
    }

    #[test]
    fn as_of_coupon_and_amortization_are_paid_before_immediate_call() {
        let issue = date!(2024 - 01 - 01);
        let as_of = date!(2025 - 01 - 01);
        let maturity = date!(2026 - 01 - 01);
        let money = |amount| Money::new(amount, Currency::USD).expect("valid money fixture");
        let schedule = CashFlowSchedule::from_parts(
            vec![
                CashFlow::new(issue, None, money(-1_000.0), CFKind::Notional, 0.0, None),
                CashFlow::new(as_of, None, money(50.0), CFKind::Fixed, 0.0, None),
                CashFlow::new(as_of, None, money(200.0), CFKind::Amortization, 0.0, None),
                CashFlow::new(maturity, None, money(800.0), CFKind::Notional, 0.0, None),
            ],
            Notional::par(1_000.0, Currency::USD).expect("valid notional fixture"),
            DayCount::Act365F,
            CashFlowMeta {
                issue_date: Some(issue),
                maturity_date: Some(maturity),
                ..CashFlowMeta::default()
            },
        );
        let mut bond =
            Bond::from_cashflows("SAME-DAY-EXERCISE", schedule, "USD-OIS", None).expect("bond");
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: as_of,
                end_date: as_of,
                price_pct_of_par: 90.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, 1.0)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);

        let valuator = BondValuator::new(bond, &market, as_of, 1.0, 1).expect("valuator");
        assert_eq!(valuator.cashflow_vec[0], 250.0);
        assert_eq!(valuator.outstanding_principal_vec[0], 800.0);
        assert_eq!(valuator.call_vec[0], Some(720.0));
        let price = valuator
            .price_deterministic_discount_curve(0.0)
            .expect("price");
        assert_eq!(price, 970.0);
    }

    #[test]
    fn make_whole_reference_value_adds_near_coupon_accrued_once() {
        let issue = date!(2025 - 01 - 01);
        let exercise = date!(2025 - 06 - 30);
        let maturity = date!(2026 - 01 - 01);
        let mut bond = Bond::fixed(
            "MAKE-WHOLE-ACCRUED",
            Money::from((1_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.10).expect("valid rate fixture"),
            issue,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: exercise,
                end_date: exercise,
                price_pct_of_par: 50.0,
                make_whole: Some(MakeWholeSpec {
                    reference_curve_id: "USD-OIS".into(),
                    spread_bp: 0.0,
                }),
            }],
            puts: Vec::new(),
        });
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([(0.0, 1.0), (1.0, 1.0)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);
        let flows = bond
            .pricing_dated_cashflows(&market, issue)
            .expect("cashflows");
        let first_future_date = flows
            .iter()
            .find_map(|(date, _)| (*date > exercise).then_some(*date))
            .expect("future coupon");
        assert_eq!(first_future_date, date!(2025 - 07 - 01));
        let reference_dirty = flows
            .iter()
            .filter(|(date, _)| *date > exercise)
            .map(|(_, amount)| amount.amount())
            .sum::<f64>();
        let day_count = market
            .get_discount("USD-OIS")
            .expect("discount curve")
            .day_count();
        let exercise_time = day_count
            .year_fraction(issue, exercise, DayCountContext::default())
            .expect("exercise time");
        let maturity_time = day_count
            .year_fraction(issue, maturity, DayCountContext::default())
            .expect("maturity time");

        let valuator = BondValuator::new_with_time_steps(
            bond,
            &market,
            issue,
            vec![0.0, exercise_time, maturity_time],
        )
        .expect("valuator");
        let dirty_call = valuator.call_vec[1].expect("make-whole call");

        assert!(reference_dirty > 1_000.0);
        assert!(
            (dirty_call - reference_dirty).abs() < 1.0e-10,
            "the dirty reference PV already contains accrued cash: call={dirty_call}, reference={reference_dirty}"
        );
    }

    #[test]
    fn incompatible_same_date_put_above_call_is_rejected() {
        let as_of = date!(2025 - 01 - 01);
        let maturity = date!(2026 - 01 - 01);
        let exercise = date!(2025 - 07 - 01);
        let mut bond = Bond::fixed(
            "BAD-BARRIERS",
            Money::from((100_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.0).expect("valid rate fixture"),
            as_of,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: exercise,
                end_date: exercise,
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: vec![CallPut {
                start_date: exercise,
                end_date: exercise,
                price_pct_of_par: 101.0,
                make_whole: None,
            }],
        });
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);

        let error = BondValuator::new(bond, &market, as_of, 1.0, 12)
            .err()
            .expect("crossed barriers must fail");
        assert!(error.to_string().contains("incompatible call/put barriers"));
    }

    /// Item 8 regression: off-grid coupons on the **non-exercise** path are
    /// distributed across the two bracketing tree steps, and each distributed
    /// piece must carry the discount-factor correction that moves it from the
    /// coupon's true time to its destination step time — exactly as the
    /// exercise-coincident path does via `value_at_step_time`.
    ///
    /// Aggregate PV-preservation invariant: discounting every step's mapped
    /// cashflow at that step's time must reproduce the total present value of
    /// the bond's raw cashflows discounted at their true times:
    /// `Σ_step cashflow_vec[s] · DF(step_time_s) == Σ_flow amount · DF(true_t)`.
    /// Without the DF correction on the distributed path the raw linear split
    /// mis-times discounting and this identity fails.
    #[test]
    fn off_grid_nonexercise_cashflows_carry_df_correction() {
        let as_of = date!(2025 - 01 - 01);
        // Call snapped to its own step; the annual coupons below land off-grid
        // and take the distributed (non-exercise) mapping path.
        let call_date = date!(2031 - 06 - 15);
        let maturity = date!(2032 - 01 - 01);
        let tree_steps = 9;
        let mut bond = Bond::fixed(
            "OFF-GRID-NONEX-CF",
            Money::from((1_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.06).expect("valid rate fixture"),
            as_of,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bond.cashflow_spec = CashflowSpec::fixed(0.06, Tenor::annual(), DayCount::Act365F)
            .expect("finite test coupon");
        bond.instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.01);
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: call_date,
                end_date: call_date,
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: vec![],
        });

        // Strongly sloped curve so a DF-correction error would be material.
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (7.0, 0.50)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);
        let discount_curve = market.get_discount("USD-OIS").expect("discount curve");
        let day_count = discount_curve.day_count();
        let time_to_maturity = day_count
            .year_fraction(as_of, maturity, DayCountContext::default())
            .expect("time to maturity");
        let dt = time_to_maturity / tree_steps as f64;

        let flows = bond
            .pricing_dated_cashflows(&market, as_of)
            .expect("cashflows");
        // Confirm the test premise: at least one coupon is genuinely off-grid.
        let has_off_grid = flows.iter().any(|(date, _)| {
            if *date <= as_of {
                return false;
            }
            let tf = day_count
                .year_fraction(as_of, *date, DayCountContext::default())
                .unwrap_or(0.0);
            let raw = (tf / time_to_maturity) * tree_steps as f64;
            let frac = raw - raw.floor();
            frac > 1e-6 && frac < 1.0 - 1e-6
        });
        assert!(has_off_grid, "test premise: an off-grid coupon must exist");

        let valuator =
            BondValuator::new(bond, &market, as_of, time_to_maturity, tree_steps).expect("tree");

        // PV of every step's mapped cashflow, discounted at its step time.
        let mut pv_mapped = 0.0;
        for (step, amount) in valuator.cashflow_vec.iter().enumerate() {
            pv_mapped += amount * discount_curve.df(step as f64 * dt);
        }
        // PV of the raw cashflows at their true times.
        let mut pv_true = 0.0;
        for (date, amount) in &flows {
            if *date <= as_of {
                continue;
            }
            let tf = day_count
                .year_fraction(as_of, *date, DayCountContext::default())
                .expect("year fraction");
            pv_true += amount.amount() * discount_curve.df(tf);
        }

        assert!(
            (pv_mapped - pv_true).abs() < 1e-9,
            "tree cashflow mapping must preserve present value under distributed \
             (non-exercise) off-grid coupons: pv_mapped={pv_mapped}, pv_true={pv_true}, \
             diff={}",
            (pv_mapped - pv_true).abs()
        );
    }

    /// Regression: a coupon whose raw step index lands in (0, 1)
    /// (i.e. inside the first time step) must keep its `(1 - weight)` share at
    /// step 0. The old guard `step_idx > 0` silently dropped that share,
    /// leaking up to a full coupon of PV for any callable bond valued within
    /// one tree step of a coupon date.
    #[test]
    fn coupon_inside_first_time_step_books_share_at_step_zero() {
        let as_of = date!(2025 - 01 - 01);
        let call_date = date!(2031 - 06 - 15);
        let maturity = date!(2035 - 01 - 01);
        // 10y with 9 steps -> dt ≈ 1.11y, so the first annual coupon (t ≈ 1.0)
        // has raw index ≈ 0.9 ∈ (0, 1) and splits across steps 0 and 1.
        let tree_steps = 9;
        let mut bond = Bond::fixed(
            "FIRST-STEP-COUPON",
            Money::from((1_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.06).expect("valid rate fixture"),
            as_of,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bond.cashflow_spec = CashflowSpec::fixed(0.06, Tenor::annual(), DayCount::Act365F)
            .expect("finite test coupon");
        bond.instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.01);
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: call_date,
                end_date: call_date,
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: vec![],
        });

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, 0.60)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);
        let discount_curve = market.get_discount("USD-OIS").expect("discount curve");
        let day_count = discount_curve.day_count();
        let time_to_maturity = day_count
            .year_fraction(as_of, maturity, DayCountContext::default())
            .expect("time to maturity");
        let dt = time_to_maturity / tree_steps as f64;

        let flows = bond
            .pricing_dated_cashflows(&market, as_of)
            .expect("cashflows");
        // Test premise: at least one cashflow lands strictly inside the first step.
        let has_first_step_coupon = flows.iter().any(|(date, _)| {
            if *date <= as_of {
                return false;
            }
            let tf = day_count
                .year_fraction(as_of, *date, DayCountContext::default())
                .unwrap_or(0.0);
            let raw = (tf / time_to_maturity) * tree_steps as f64;
            raw > 1e-6 && raw < 1.0 - 1e-6
        });
        assert!(
            has_first_step_coupon,
            "test premise: a coupon must fall inside the first time step"
        );

        let valuator =
            BondValuator::new(bond, &market, as_of, time_to_maturity, tree_steps).expect("tree");

        // The (1 - weight) share of the first coupon must be booked at step 0.
        assert!(
            valuator.cashflow_vec[0] > 0.0,
            "step 0 must receive the lower-step share of a first-step coupon, got {}",
            valuator.cashflow_vec[0]
        );

        // PV-preservation identity: mapped cashflows discounted at step times
        // must reproduce the raw cashflows discounted at their true times.
        // Under the old `step_idx > 0` guard the first coupon's step-0 share
        // vanished and this identity failed by ~0.1 coupon.
        let mut pv_mapped = 0.0;
        for (step, amount) in valuator.cashflow_vec.iter().enumerate() {
            pv_mapped += amount * discount_curve.df(step as f64 * dt);
        }
        let mut pv_true = 0.0;
        for (date, amount) in &flows {
            if *date <= as_of {
                continue;
            }
            let tf = day_count
                .year_fraction(as_of, *date, DayCountContext::default())
                .expect("year fraction");
            pv_true += amount.amount() * discount_curve.df(tf);
        }
        assert!(
            (pv_mapped - pv_true).abs() < 1e-9,
            "PV must be preserved when a coupon falls inside the first time step: \
             pv_mapped={pv_mapped}, pv_true={pv_true}, diff={}",
            (pv_mapped - pv_true).abs()
        );
    }

    #[test]
    fn exercise_date_cashflows_are_adjusted_to_snapped_step_time() {
        let as_of = date!(2025 - 01 - 01);
        let call_date = date!(2027 - 01 - 01);
        let maturity = date!(2030 - 01 - 01);
        let tree_steps = 7;
        let mut bond = Bond::fixed(
            "OFF-GRID-CALL-CF",
            Money::from((1_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.06).expect("valid rate fixture"),
            as_of,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bond.cashflow_spec = CashflowSpec::fixed(0.06, Tenor::annual(), DayCount::Act365F)
            .expect("finite test coupon");
        bond.instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.01);
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: call_date,
                end_date: call_date,
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: vec![],
        });

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.55)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);
        let discount_curve = market.get_discount("USD-OIS").expect("discount curve");
        let day_count = discount_curve.day_count();
        let time_to_maturity = day_count
            .year_fraction(as_of, maturity, DayCountContext::default())
            .expect("time to maturity");
        let event_time = day_count
            .year_fraction(as_of, call_date, DayCountContext::default())
            .expect("call time");
        let step = map_date_to_step(
            as_of,
            call_date,
            maturity,
            tree_steps,
            day_count,
            DayCountContext::default(),
        )
        .expect("map call date")
        .clamp(1, tree_steps);
        let step_time = time_to_maturity / tree_steps as f64 * step as f64;
        assert!(
            (event_time - step_time).abs() > 1e-4,
            "test requires an off-grid exercise date"
        );

        let raw_exercise_date_cashflow = bond
            .pricing_dated_cashflows(&market, as_of)
            .expect("cashflows")
            .into_iter()
            .filter(|(date, _)| *date == call_date)
            .map(|(_, amount)| amount.amount())
            .sum::<f64>();
        assert!(
            raw_exercise_date_cashflow > 0.0,
            "test requires a coupon on the exercise date"
        );

        let valuator =
            BondValuator::new(bond, &market, as_of, time_to_maturity, tree_steps).expect("tree");
        let grid = (0..=tree_steps)
            .map(|index| index as f64 * time_to_maturity / tree_steps as f64)
            .collect::<Vec<_>>();
        let step_dfs = BondValuator::conditional_discount_factors_on_grid(
            as_of,
            maturity,
            day_count,
            &grid,
            discount_curve.as_ref(),
        )
        .expect("step discount factors");
        let expected = BondValuator::value_at_step_time(
            raw_exercise_date_cashflow,
            call_date,
            step,
            &step_dfs,
            discount_curve.as_ref(),
            as_of,
        )
        .expect("timing-adjusted cashflow");
        let actual = valuator.cashflow_vec[step];

        assert!(
            (actual - expected).abs() < 1e-10,
            "exercise-date cashflow should be valued consistently with call redemption timing: actual={actual}, expected={expected}, raw={raw_exercise_date_cashflow}"
        );
    }
}
