use super::super::super::super::types::Bond;
use super::TreePricer;
use crate::models::trees::hull_white_tree::HullWhiteTree;
use crate::models::{NodeState, TreeValuator};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;

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
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::BondValuator;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// let valuator = BondValuator::new(bond, &market, as_of, 5.0, 100)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
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
        let dc_curve = discount_curve.day_count();
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
                    &cashflow_dates,
                ));
            }
        }
        dates.sort_unstable();
        dates.dedup();

        dates
            .into_iter()
            .map(|d| {
                dc_curve.year_fraction(
                    as_of,
                    d,
                    finstack_quant_core::dates::DayCountContext::default(),
                )
            })
            .collect()
    }

    fn exercise_dates_for_period(
        start_date: Date,
        end_date: Date,
        as_of: Date,
        maturity: Date,
        cashflow_dates: &[Date],
    ) -> Vec<Date> {
        // A call/put period is an exercise *window*: the option is exercisable
        // throughout `[start_date, end_date]`. Exercise at the window endpoints
        // plus every cashflow (coupon) date inside the window, matching the
        // YTW enumeration in `quote_conversions::solve_ytw_from_flows`.
        // Endpoint-only exercise materially undervalues the issuer option for
        // multi-coupon windows.
        let mut dates: Vec<Date> = [start_date, end_date]
            .into_iter()
            .chain(
                cashflow_dates
                    .iter()
                    .copied()
                    .filter(|d| *d >= start_date && *d <= end_date),
            )
            .filter(|date| *date > as_of && *date <= maturity)
            .collect();
        dates.sort_unstable();
        dates.dedup();
        dates
    }

    fn make_whole_call_price(
        call: &crate::instruments::fixed_income::bond::CallPut,
        reference_curve: &dyn finstack_quant_core::market_data::traits::Discounting,
        time_steps: &[f64],
        cashflow_vec: &[f64],
        step: usize,
        floor_price: f64,
    ) -> f64 {
        let call_time = *time_steps.get(step).unwrap_or(&0.0);
        let spread = call
            .make_whole
            .as_ref()
            .map(|spec| spec.spread_bps / 10_000.0)
            .unwrap_or(0.0);

        let mut pv_remaining = 0.0;
        for (future_step, amount) in cashflow_vec.iter().enumerate().skip(step + 1) {
            let amount = *amount;
            if amount.abs() <= f64::EPSILON {
                continue;
            }
            let future_time = *time_steps.get(future_step).unwrap_or(&call_time);
            if future_time <= call_time {
                continue;
            }

            let tau = future_time - call_time;
            let df_ratio = reference_curve.df(future_time) / reference_curve.df(call_time);
            pv_remaining += amount * df_ratio * (-spread * tau).exp();
        }

        floor_price.max(pv_remaining)
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
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    /// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::BondValuator;
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::dates::Date;
    ///
    /// # let bond = Bond::example().unwrap();
    /// # let market = MarketContext::new();
    /// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
    /// let valuator = BondValuator::new(bond, &market, as_of, 5.0, 100)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
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
        use crate::cashflow::primitives::CFKind;

        if time_steps.len() < 2 || !time_steps.windows(2).all(|w| w[1] > w[0]) {
            return Err(finstack_quant_core::Error::Validation(
                "BondValuator time grid must be strictly increasing with at least 2 points"
                    .to_string(),
            ));
        }
        let num_steps = time_steps.len();

        let curves = market_context;
        let discount_curve = market_context.get_discount(&bond.discount_curve_id)?;
        let dc_curve = discount_curve.day_count();
        let flows = bond.pricing_dated_cashflows(curves, as_of)?;

        // Build outstanding principal schedule from the full cashflow schedule.
        // This tracks notional minus cumulative amortization at each step for
        // correct call/put redemption pricing on amortizing bonds.
        let full_schedule = bond.full_cashflow_schedule(market_context)?;
        let mut outstanding_principal_vec = vec![bond.notional.amount(); num_steps];

        // Collect amortization events sorted by date
        let mut amort_events: Vec<(Date, f64)> = full_schedule
            .get_flows()
            .iter()
            .filter(|cf| matches!(cf.kind, CFKind::Amortization | CFKind::Notional))
            .filter(|cf| cf.date > as_of && cf.amount.amount() > 0.0)
            .map(|cf| (cf.date, cf.amount.amount()))
            .collect();
        amort_events.sort_by_key(|(d, _)| *d);

        // Track cumulative amortization and map to time steps
        let mut cumulative_amort = 0.0;
        let initial_notional = bond.notional.amount();
        let mut amort_idx = 0;

        for step in 0..num_steps {
            let step_time = time_steps[step];
            // Half the distance to the next grid point (or the previous one
            // at the final step): an amortization belongs to this step when
            // it is closer to it than to the next.
            let half_step = if step + 1 < num_steps {
                (time_steps[step + 1] - step_time) / 2.0
            } else {
                (step_time - time_steps[step - 1]) / 2.0
            };

            // Process any amortization events that occur at or before this step time
            while amort_idx < amort_events.len() {
                let (amort_date, amort_amt) = amort_events[amort_idx];
                // Propagate day-count failures: a silent 0.0 would book the
                // amortization at step 0 and misstate outstanding principal.
                let amort_time = dc_curve.year_fraction(
                    as_of,
                    amort_date,
                    finstack_quant_core::dates::DayCountContext::default(),
                )?;

                if amort_time <= step_time + half_step {
                    // This amortization has occurred by this step
                    cumulative_amort += amort_amt;
                    amort_idx += 1;
                } else {
                    break;
                }
            }

            outstanding_principal_vec[step] = (initial_notional - cumulative_amort).max(0.0);
        }

        // Collect exercise dates so we can snap coincident coupons to the same
        // tree step used for the call/put (ceil mapping), preventing timing
        // mismatches between coupon receipt and exercise decision.
        let mut exercise_dates = std::collections::HashSet::new();
        let cashflow_dates: Vec<Date> = flows.iter().map(|(date, _)| *date).collect();
        if let Some(ref call_put) = bond.call_put {
            for call in &call_put.calls {
                exercise_dates.extend(Self::exercise_dates_for_period(
                    call.start_date,
                    call.end_date,
                    as_of,
                    bond.maturity,
                    &cashflow_dates,
                ));
            }
            for put in &call_put.puts {
                exercise_dates.extend(Self::exercise_dates_for_period(
                    put.start_date,
                    put.end_date,
                    as_of,
                    bond.maturity,
                    &cashflow_dates,
                ));
            }
        }

        // Pre-allocate vectors for O(1) access during backward induction
        let mut cashflow_vec = vec![0.0; num_steps];
        let mut cashflow_components = vec![Vec::new(); num_steps];
        for (date, amount) in &flows {
            if *date > as_of {
                let time_frac = dc_curve.year_fraction(
                    as_of,
                    *date,
                    finstack_quant_core::dates::DayCountContext::default(),
                )?;
                // Continuous (fractional) grid position of the cashflow,
                // clamped to the grid.
                let raw_clamped = Self::fractional_step(&time_steps, time_frac);

                // When a cashflow date matches an exercise date, snap to the
                // exercise step to prevent timing mismatches between coupon
                // receipt and exercise decision.
                if exercise_dates.contains(date) {
                    let step = Self::nearest_step(&time_steps, time_frac).clamp(1, num_steps - 1);
                    let adjusted_amount = Self::value_at_step_time(
                        amount.amount(),
                        time_frac,
                        time_steps[step],
                        discount_curve.as_ref(),
                    );
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
                            time_frac,
                            time_steps[step_idx],
                            discount_curve.as_ref(),
                        );
                        cashflow_vec[step_idx] += adjusted_amount;
                        cashflow_components[step_idx]
                            .push((adjusted_amount, time_frac - time_steps[step_idx]));
                    }

                    // Distribute to step_idx + 1 (weight: weight)
                    if step_idx + 1 < num_steps {
                        let adjusted_amount = Self::value_at_step_time(
                            amount.amount() * weight,
                            time_frac,
                            time_steps[step_idx + 1],
                            discount_curve.as_ref(),
                        );
                        cashflow_vec[step_idx + 1] += adjusted_amount;
                        cashflow_components[step_idx + 1]
                            .push((adjusted_amount, time_frac - time_steps[step_idx + 1]));
                    }
                }
            }
        }

        // Sparse vectors for call/put (most steps have no option)
        // Call/put redemption uses outstanding principal at exercise date, not original notional.
        let mut call_vec: Vec<Option<f64>> = vec![None; num_steps];
        let mut put_vec: Vec<Option<f64>> = vec![None; num_steps];
        if let Some(ref call_put) = bond.call_put {
            for call in &call_put.calls {
                for exercise_date in Self::exercise_dates_for_period(
                    call.start_date,
                    call.end_date,
                    as_of,
                    bond.maturity,
                    &cashflow_dates,
                ) {
                    let exercise_time = dc_curve.year_fraction(
                        as_of,
                        exercise_date,
                        finstack_quant_core::dates::DayCountContext::default(),
                    )?;
                    let step =
                        Self::nearest_step(&time_steps, exercise_time).clamp(1, num_steps - 1);
                    let outstanding = outstanding_principal_vec[step];
                    let floor_price = outstanding * (call.price_pct_of_par / 100.0);
                    let clean_call_price = if let Some(spec) = &call.make_whole {
                        let reference_curve =
                            market_context.get_discount(&spec.reference_curve_id)?;
                        Self::make_whole_call_price(
                            call,
                            reference_curve.as_ref(),
                            &time_steps,
                            &cashflow_vec,
                            step,
                            floor_price,
                        )
                    } else {
                        floor_price
                    };
                    let accrued_on_call = crate::cashflow::accrual::accrued_interest_amount(
                        &full_schedule,
                        exercise_date,
                        &bond.accrual_config(),
                    )?;
                    let call_price = Self::value_at_step_time(
                        clean_call_price + accrued_on_call,
                        exercise_time,
                        time_steps[step],
                        discount_curve.as_ref(),
                    );
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
                    &cashflow_dates,
                ) {
                    let exercise_time = dc_curve.year_fraction(
                        as_of,
                        exercise_date,
                        finstack_quant_core::dates::DayCountContext::default(),
                    )?;
                    let step =
                        Self::nearest_step(&time_steps, exercise_time).clamp(1, num_steps - 1);
                    // Use outstanding principal at exercise step, not original notional
                    let outstanding = outstanding_principal_vec[step];
                    let clean_put_price = outstanding * (put.price_pct_of_par / 100.0);
                    let accrued_on_put = crate::cashflow::accrual::accrued_interest_amount(
                        &full_schedule,
                        exercise_date,
                        &bond.accrual_config(),
                    )?;
                    let put_price = Self::value_at_step_time(
                        clean_put_price + accrued_on_put,
                        exercise_time,
                        time_steps[step],
                        discount_curve.as_ref(),
                    );
                    put_vec[step] =
                        Some(put_vec[step].map_or(put_price, |existing| existing.max(put_price)));
                }
            }
        }

        // Source recovery rate from the bond's explicit credit_curve_id,
        // consistent with HazardBondEngine and TreePricer::calculate_oas.
        let recovery_rate = Self::resolve_recovery_rate(&bond, market_context);
        let call_friction_cents = bond
            .pricing_overrides
            .model_config
            .call_friction_cents
            .unwrap_or(0.0);

        Ok(Self {
            bond,
            cashflow_vec,
            cashflow_components,
            call_vec,
            put_vec,
            outstanding_principal_vec,
            time_steps,
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

    /// Check if there's a call option at this time step.
    #[inline]
    fn call_at(&self, step: usize) -> Option<f64> {
        self.call_vec.get(step).copied().flatten()
    }

    fn value_at_step_time(
        cash_value_at_event_time: f64,
        event_time: f64,
        step_time: f64,
        discount_curve: &dyn finstack_quant_core::market_data::traits::Discounting,
    ) -> f64 {
        let step_df = discount_curve.df(step_time);
        if step_df <= f64::EPSILON {
            return cash_value_at_event_time;
        }
        cash_value_at_event_time * discount_curve.df(event_time) / step_df
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

        let terminal_cf = self.cashflow_at_oas(final_step, oas_rate);
        let terminal_values = vec![terminal_cf; hw_tree.num_nodes(final_step)];

        hw_tree.backward_induction(&terminal_values, |step, _node_idx, continuation| {
            // The HW tree's backward_induction already discounts by the short
            // rate r(step, node). Apply the OAS as additional discounting
            // over this step's (possibly non-uniform) interval.
            let oas_adjusted = continuation * comp.df(oas_rate, hw_tree.dt_at_step(step));

            let coupon = self.cashflow_at_oas(step, oas_rate);
            let mut principal_value = oas_adjusted;

            if let Some(put_price) = self.put_at(step) {
                principal_value = principal_value.max(put_price);
            }

            if let Some(call_price) = self.call_at(step) {
                let outstanding = self.outstanding_principal_at(step);
                let friction_amount = outstanding * (self.call_friction_cents / 10_000.0);
                let threshold = call_price + friction_amount;
                if principal_value > threshold {
                    principal_value = principal_value.min(call_price);
                }
            }

            coupon + principal_value
        })
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
        let oas_rate = state.get_var_or(crate::models::short_rate_keys::OAS, 0.0) / 10_000.0;
        let cashflow = self.cashflow_at_oas(final_step, oas_rate);
        Ok(cashflow)
    }

    fn value_at_node(&self, state: &NodeState, continuation_value: f64, dt: f64) -> Result<f64> {
        let step = state.step;
        let oas_rate = state.get_var_or(crate::models::short_rate_keys::OAS, 0.0) / 10_000.0;
        let coupon = self.cashflow_at_oas(step, oas_rate);

        // Call/put exercise logic:
        // - Coupon is ALWAYS paid on coupon dates regardless of exercise decision
        // - Call/put redemption is principal-only (price_pct_of_par × outstanding)
        // - Exercise decision compares continuation vs redemption value
        //
        // Formula: value = coupon + min(max(continuation, put_redemption), call_redemption)
        //
        // This ensures:
        // 1. Coupon is received regardless of exercise
        // 2. Put floor: holder can demand redemption if continuation < put_price
        // 3. Call cap: issuer can redeem if continuation > call_price

        // Start with continuation value (principal path if not exercised)
        let mut principal_value = continuation_value;

        // Put option: holder can exercise if redemption > continuation
        if let Some(put_price) = self.put_at(step) {
            principal_value = principal_value.max(put_price);
        }

        // Call option: issuer can exercise if redemption < continuation, subject to friction.
        //
        // With friction, the issuer only calls when continuation exceeds:
        //   call_price + (outstanding_principal × call_friction_cents / 10_000)
        //
        // (because 1 cent per 100 of par = 0.0001 of notional).
        if let Some(call_price) = self.call_at(step) {
            let outstanding = self.outstanding_principal_at(step);
            let friction_amount = outstanding * (self.call_friction_cents / 10_000.0);
            let threshold = call_price + friction_amount;
            if principal_value > threshold {
                principal_value = principal_value.min(call_price);
            }
        }

        // Coupon is added after exercise decision (coupon is paid regardless)
        let alive_value = coupon + principal_value;

        // Default handling: if hazard rate is present, compute survival/default weighting.
        // Use cached fields instead of hash lookups for performance.
        //
        // Recovery convention: recovery is received at the *current* node upon
        // default (standard Hull/Brigo-Mercurio convention). No additional one-
        // period discounting is applied — `alive_value` and `recovery` are both
        // in PV-at-this-node terms.
        if let Some(hazard) = state.hazard_rate {
            let p_surv = (-hazard.max(0.0) * dt).exp();
            let default_prob = (1.0 - p_surv).clamp(0.0, 1.0);
            // Use outstanding principal at this step for recovery (FRP convention)
            let outstanding = self.outstanding_principal_at(step);
            let recovery = self
                .recovery_rate
                .map(|rr| rr.clamp(0.0, 1.0) * outstanding)
                .unwrap_or(0.0);
            let node_value = p_surv * alive_value + default_prob * recovery;
            Ok(node_value)
        } else {
            // No hazard info at this node; return alive path value
            Ok(alive_value)
        }
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
    use crate::instruments::fixed_income::bond::{Bond, CallPut, CallPutSchedule, CashflowSpec};
    use crate::models::trees::tree_framework::map_date_to_step;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, DayCountContext, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use time::macros::date;

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
            Money::new(1_000.0, Currency::USD),
            0.06,
            as_of,
            maturity,
            "USD-OIS",
        )
        .expect("bond");
        bond.cashflow_spec = CashflowSpec::fixed(0.06, Tenor::annual(), DayCount::Act365F)
            .expect("finite test coupon");
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
        let dc = discount_curve.day_count();
        let time_to_maturity = dc
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
            let tf = dc
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
            let tf = dc
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
            Money::new(1_000.0, Currency::USD),
            0.06,
            as_of,
            maturity,
            "USD-OIS",
        )
        .expect("bond");
        bond.cashflow_spec = CashflowSpec::fixed(0.06, Tenor::annual(), DayCount::Act365F)
            .expect("finite test coupon");
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
        let dc = discount_curve.day_count();
        let time_to_maturity = dc
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
            let tf = dc
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
            let tf = dc
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
            Money::new(1_000.0, Currency::USD),
            0.06,
            as_of,
            maturity,
            "USD-OIS",
        )
        .expect("bond");
        bond.cashflow_spec = CashflowSpec::fixed(0.06, Tenor::annual(), DayCount::Act365F)
            .expect("finite test coupon");
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
        let dc = discount_curve.day_count();
        let time_to_maturity = dc
            .year_fraction(as_of, maturity, DayCountContext::default())
            .expect("time to maturity");
        let event_time = dc
            .year_fraction(as_of, call_date, DayCountContext::default())
            .expect("call time");
        let step = map_date_to_step(
            as_of,
            call_date,
            maturity,
            tree_steps,
            dc,
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
        let expected = BondValuator::value_at_step_time(
            raw_exercise_date_cashflow,
            event_time,
            step_time,
            discount_curve.as_ref(),
        );
        let actual = valuator.cashflow_vec[step];

        assert!(
            (actual - expected).abs() < 1e-10,
            "exercise-date cashflow should be valued consistently with call redemption timing: actual={actual}, expected={expected}, raw={raw_exercise_date_cashflow}"
        );
    }
}
