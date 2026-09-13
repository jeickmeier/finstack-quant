//! Path replay: `ReplayCursor` stepping and `ReplayTemplate` construction and evaluation.

use super::*;

impl ReplayCursor {
    pub(super) fn new(template: &ReplayTemplate) -> Result<Self> {
        let mut floating = Vec::with_capacity(template.floating.len());
        for coupon in &template.floating {
            let mut state = coupon.compiled.replay_state();
            if let Some(rate) = coupon.initial_term_rate {
                coupon.compiled.observe_term(&mut state, rate)?;
            }
            if let Some(notional) = coupon.initial_notional {
                coupon.compiled.capture_notional(&mut state, notional)?;
            }
            floating.push(state);
        }
        Ok(Self {
            outstanding: template.initial_outstanding,
            cumulative_distribution_cash: template.historical_distribution_cash,
            cumulative_distribution_target_pv: template.historical_distribution_target_pv,
            floating,
        })
    }

    pub(super) fn from_checkpoint(
        template: &ReplayTemplate,
        checkpoint: &ProductCheckpoint,
    ) -> Result<Self> {
        let mut floating = vec![FloatingRuntimeState::default(); template.floating.len()];
        for live in &checkpoint.live_floating {
            let slot = floating.get_mut(live.coupon_id).ok_or_else(|| {
                Error::internal("bond hazard LSMC checkpoint has an invalid floating coupon id")
            })?;
            *slot = live.state.clone();
        }
        Ok(Self {
            outstanding: checkpoint.outstanding,
            cumulative_distribution_cash: checkpoint.cumulative_distribution_cash,
            cumulative_distribution_target_pv: checkpoint.cumulative_distribution_target_pv,
            floating,
        })
    }

    pub(super) fn checkpoint(&self, template: &ReplayTemplate) -> ProductCheckpoint {
        let live_floating = self
            .floating
            .iter()
            .enumerate()
            .filter(|(coupon_id, state)| template.floating[*coupon_id].compiled.is_live(state))
            .map(|(coupon_id, state)| LiveFloatingCheckpoint {
                coupon_id,
                state: state.clone(),
            })
            .collect::<SmallVec<_>>();
        ProductCheckpoint {
            outstanding: self.outstanding,
            cumulative_distribution_cash: self.cumulative_distribution_cash,
            cumulative_distribution_target_pv: self.cumulative_distribution_target_pv,
            live_floating,
        }
    }

    pub(super) fn advance_overnight(
        &mut self,
        template: &ReplayTemplate,
        path: &[RatesCreditPathState],
        coupon_id: usize,
        end: Date,
    ) -> Result<()> {
        let coupon = template.floating.get(coupon_id).ok_or_else(|| {
            Error::internal("bond hazard LSMC overnight coupon id is outside the replay template")
        })?;
        let FloatingRateModel::Overnight(overnight) = &coupon.rate_model else {
            return Ok(());
        };
        let state = self.floating.get_mut(coupon_id).ok_or_else(|| {
            Error::internal("bond hazard LSMC overnight coupon state is outside the replay cursor")
        })?;
        coupon.compiled.advance_overnight(state, end, |slice| {
            overnight
                .sources
                .get(&(slice.observation_date, slice.rate_tenor_days))
                .ok_or_else(|| {
                    Error::internal(format!(
                        "bond hazard LSMC has no overnight source for {} / {} day(s)",
                        slice.observation_date, slice.rate_tenor_days
                    ))
                })?
                .rate(path)
        })?;
        Ok(())
    }

    pub(super) fn advance_step(
        &mut self,
        template: &ReplayTemplate,
        bond: &Bond,
        path: &[RatesCreditPathState],
        config: &BondLsmcConfig,
        step: usize,
    ) -> Result<StepReplay> {
        let oas = config.oas_bp / 10_000.0;
        if let Some(date) = template.step_dates.get(step).copied().flatten() {
            for coupon_id in 0..template.floating.len() {
                let coupon = &template.floating[coupon_id];
                if coupon.compiled.is_live(&self.floating[coupon_id])
                    && matches!(coupon.rate_model, FloatingRateModel::Overnight(_))
                {
                    self.advance_overnight(
                        template,
                        path,
                        coupon_id,
                        date.min(coupon.compiled.period().accrual_end),
                    )?;
                }
            }
        }

        let mut current_cash = 0.0;
        let mut reference_cash = 0.0;
        for event in &template.static_cash[step] {
            current_cash += event.amount_at_step * (-oas * event.event_minus_step).exp();
            reference_cash += event.amount_at_step;
        }
        for distribution in &template.static_distributions[step] {
            self.cumulative_distribution_cash += distribution.amount;
            if let Some(floor) = template.return_floor {
                self.cumulative_distribution_target_pv +=
                    floor.target_pv(distribution.date, distribution.amount)?;
            }
        }
        let mut pik_to_add = 0.0;
        for &coupon_id in &template.floating_payment_ids[step] {
            let coupon = &template.floating[coupon_id];
            let settlement = coupon.compiled.settle(&self.floating[coupon_id])?;
            let cash_coupon = settlement.cash_amount;
            current_cash += cash_coupon;
            reference_cash += cash_coupon;
            self.cumulative_distribution_cash += cash_coupon.max(0.0);
            if let Some(floor) = template.return_floor {
                self.cumulative_distribution_target_pv +=
                    floor.target_pv(coupon.compiled.period().payment_date, cash_coupon.max(0.0))?;
            }
            pik_to_add += settlement.pik_amount;
            self.floating[coupon_id] = FloatingRuntimeState::default();
        }
        for event in &template.balance_events[step] {
            self.outstanding += event.delta;
        }
        // Canonical cashflow emission applies same-day amortization before PIK
        // capitalization.  The next coupon captures this after-event balance.
        self.outstanding += pik_to_add;
        if self.outstanding < -1.0e-8 || !self.outstanding.is_finite() {
            return Err(Error::Validation(format!(
                "Bond '{}' hazard LSMC replay produced invalid outstanding principal {} at step {step}",
                bond.id.as_str(), self.outstanding
            )));
        }
        self.outstanding = self.outstanding.max(0.0);
        for &coupon_id in &template.floating_reset_ids[step] {
            let coupon = &template.floating[coupon_id];
            let state = &mut self.floating[coupon_id];
            match &coupon.rate_model {
                FloatingRateModel::Term(source) => {
                    let index_rate = source.rate(path)?;
                    coupon.compiled.observe_term(state, index_rate)?;
                }
                FloatingRateModel::Overnight(_) => {
                    return Err(Error::internal(
                        "bond hazard LSMC overnight coupon has a term reset event",
                    ));
                }
            }
        }
        for &coupon_id in &template.floating_accrual_start_ids[step] {
            let coupon = &template.floating[coupon_id];
            coupon
                .compiled
                .capture_notional(&mut self.floating[coupon_id], self.outstanding)?;
        }
        if let Some(date) = template.step_dates.get(step).copied().flatten() {
            for &coupon_id in &template.floating_accrual_start_ids[step] {
                let coupon = &template.floating[coupon_id];
                if matches!(coupon.rate_model, FloatingRateModel::Overnight(_)) {
                    self.advance_overnight(
                        template,
                        path,
                        coupon_id,
                        date.min(coupon.compiled.period().accrual_end),
                    )?;
                }
            }
        }
        Ok(StepReplay {
            current_cash,
            reference_cash,
            outstanding: self.outstanding,
            cumulative_distribution_cash: self.cumulative_distribution_cash,
            cumulative_distribution_target_pv: self.cumulative_distribution_target_pv,
        })
    }
}

impl ReplayTemplate {
    pub(super) fn new(
        tree: &RatesCreditTree,
        bond: &Bond,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Self> {
        let times = tree.time_grid()?.to_vec();
        if times.len() < 2 || !times.windows(2).all(|window| window[1] > window[0]) {
            return Err(Error::Validation(
                "bond hazard LSMC requires a strictly increasing calibrated tree grid".to_string(),
            ));
        }
        let terminal_step = times.len() - 1;
        let step_dates = times
            .iter()
            .map(|time| {
                let day_position = time * 365.0;
                let rounded = day_position.round();
                if (day_position - rounded).abs() <= 1.0e-10 {
                    as_of
                        .checked_add(Duration::days(rounded as i64))
                        .map(Some)
                        .ok_or_else(|| {
                            Error::Validation(
                                "bond hazard LSMC grid date exceeds the supported range"
                                    .to_string(),
                            )
                        })
                } else {
                    Ok(None)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let discount = market.get_discount(&bond.discount_curve_id)?;
        let schedule = bond.full_cashflow_schedule(market)?;
        let grid_day_count = DayCount::Act365F;
        let final_redemption_date = schedule
            .get_flows()
            .iter()
            .filter(|flow| flow.kind == CFKind::Notional)
            .map(|flow| flow.date)
            .max();
        let floating_spec = floating_coupon_spec(bond);
        let return_floor = bond
            .return_floor
            .as_ref()
            .map(|spec| {
                spec.validate()?;
                Ok::<_, Error>(ReturnFloorTemplate {
                    kind: spec.kind,
                    issue_price: spec.issue_price.resolve(bond.notional)?,
                    issue_date: bond.issue_date,
                    day_count: spec.day_count.unwrap_or(DayCount::Act365F),
                })
            })
            .transpose()?;
        let include_as_of_option_cash = has_exercise_claim_on_date(bond, as_of)?;
        let mut historical_distribution_cash = 0.0;
        let mut historical_distribution_target_pv = 0.0;
        if let Some(floor) = return_floor {
            for point in realized_distributions(bond, market, bond.issue_date)? {
                if point.date >= as_of {
                    break;
                }
                if !point.outstanding.is_finite() || point.outstanding < 0.0 {
                    return Err(Error::Validation(format!(
                        "Bond '{}' has invalid historical outstanding {} at {}",
                        bond.id.as_str(),
                        point.outstanding,
                        point.date
                    )));
                }
                historical_distribution_cash = point.cum_before + point.coupon;
                historical_distribution_target_pv += floor.target_pv(point.date, point.coupon)?;
            }
        }

        // Start immediately before the origin's contractual events. The replay
        // then applies same-day PIK/amortization before exercise while leaving
        // a same-day final redemption available as the holder's terminal claim.
        let mut initial_outstanding = bond.notional.amount();
        for flow in schedule.get_flows().iter().filter(|flow| flow.date < as_of) {
            if flow.kind == CFKind::Notional
                && flow.date == bond.issue_date
                && flow.amount.amount() < 0.0
            {
                continue;
            }
            let terminal_redemption = flow.kind == CFKind::Notional
                && final_redemption_date.is_some_and(|date| flow.date == date);
            if let Some(delta) = static_balance_delta(flow, terminal_redemption) {
                initial_outstanding += delta;
            }
        }
        if !initial_outstanding.is_finite() || initial_outstanding < -1.0e-8 {
            return Err(Error::Validation(format!(
                "Bond '{}' has invalid principal immediately before {as_of}: {initial_outstanding}",
                bond.id.as_str()
            )));
        }
        initial_outstanding = initial_outstanding.max(0.0);

        let mut dynamic_groups = BTreeMap::<(Date, Date, Date), FloatingBuild>::new();
        let mut dynamic_flow_indices = BTreeSet::new();
        if let Some(spec) = floating_spec {
            for (flow_index, flow) in schedule.get_flows().iter().enumerate() {
                if !matches!(flow.kind, CFKind::FloatReset | CFKind::Pik) || flow.date < as_of {
                    continue;
                }
                let Some(accrual) = &flow.accrual else {
                    continue;
                };
                if accrual.projected_index_rate.is_none() {
                    continue;
                }
                let reset = flow
                    .reset_date
                    .unwrap_or(contractual_reset_date(accrual.start, spec)?);
                let dynamic = flow.date > as_of || include_as_of_option_cash;
                if !dynamic {
                    continue;
                }
                let key = (accrual.start, accrual.end, flow.date);
                let entry = dynamic_groups.entry(key).or_default();
                entry.reset = Some(reset);
                entry.start = Some(accrual.start);
                entry.end = Some(accrual.end);
                entry.payment = Some(flow.date);
                entry.day_count = Some(accrual.day_count);
                entry.accrual = flow.accrual_factor;
                entry.base_index_rate = accrual.projected_index_rate;
                dynamic_flow_indices.insert(flow_index);
            }
        }

        let (cash_fraction, pik_fraction) = floating_spec
            .map(|spec| coupon_fractions(spec.coupon_type))
            .transpose()?
            .unwrap_or((0.0, 0.0));
        let mut params = floating_spec
            .map(|spec| params_from_spec(&spec.rate_spec))
            .unwrap_or_default();
        if floating_spec.is_some_and(|spec| spec.rate_spec.overnight_compounding.is_some()) {
            params.index_floor_bp = None;
            params.index_cap_bp = None;
        }
        params.validate()?;

        let mut floating = Vec::with_capacity(dynamic_groups.len());
        for group in dynamic_groups.into_values() {
            let reset = required_date(group.reset, "reset")?;
            let start = required_date(group.start, "accrual start")?;
            let end = required_date(group.end, "accrual end")?;
            let payment = required_date(group.payment, "payment")?;
            let day_count = group.day_count.ok_or_else(|| {
                Error::Validation("floating coupon is missing day-count metadata".to_string())
            })?;
            if !group.accrual.is_finite() || group.accrual <= 0.0 {
                return Err(Error::Validation(format!(
                    "Bond '{}' floating coupon {} to {} has invalid accrual factor {}",
                    bond.id.as_str(),
                    start,
                    end,
                    group.accrual
                )));
            }
            let reset_time = if reset <= as_of {
                0.0
            } else {
                grid_day_count.year_fraction(as_of, reset, DayCountContext::default())?
            };
            let start_time = if start <= as_of {
                0.0
            } else {
                grid_day_count.year_fraction(as_of, start, DayCountContext::default())?
            };
            let reset_step = nearest_step(&times, reset_time);
            let accrual_start_step = nearest_step(&times, start_time);
            let payment_step = exact_grid_step(&times, as_of, payment)?;
            if accrual_start_step >= payment_step {
                return Err(Error::Validation(format!(
                    "Bond '{}' floating coupon accruing {} to {} (paid {}) maps accrual start and payment to step {}; use a daily rates-credit grid through the adjusted payment date",
                    bond.id.as_str(), start, end, payment, accrual_start_step
                )));
            }
            let spec = floating_spec.ok_or_else(|| {
                Error::internal("bond hazard LSMC dynamic floating coupon has no specification")
            })?;
            let base_index_rate = group.base_index_rate.ok_or_else(|| {
                Error::Validation(format!(
                    "Bond '{}' floating coupon {} to {} is missing projected_index_rate metadata required by hazard LSMC",
                    bond.id.as_str(), start, end
                ))
            })?;
            let (rate_model, observation) = if spec.rate_spec.overnight_compounding.is_some() {
                let (overnight_model, overnight_observation) = build_overnight_coupon(
                    tree,
                    market,
                    discount.as_ref(),
                    as_of,
                    &times,
                    start,
                    end,
                    spec,
                )?;
                (
                    FloatingRateModel::Overnight(overnight_model),
                    overnight_observation,
                )
            } else {
                let forward = market.get_forward(&spec.rate_spec.index_id).ok();
                let tenor_years = forward.as_ref().map_or_else(
                    || {
                        spec.rate_spec
                            .index_tenor
                            .unwrap_or(spec.rate_spec.reset_frequency)
                            .to_years()
                    },
                    |curve| curve.tenor(),
                );
                let fixing_id = fixing_series_id(spec.rate_spec.index_id.as_str());
                let published_same_day = reset == as_of
                    && market
                        .get_series(&fixing_id)
                        .ok()
                        .is_some_and(|series| series.value_on_exact(reset).is_ok());
                let known_reset = reset < as_of || published_same_day;
                let source = if known_reset || forward.is_none() {
                    ObservedRateSource::Fixed(base_index_rate)
                } else {
                    let observation_end_time = reset_time + tenor_years;
                    if observation_end_time > times[terminal_step] + 1.0e-12 {
                        return Err(Error::Validation(format!(
                                "Bond '{}' term-index observation at {} with tenor {:.12}y ends beyond the calibrated rates-credit horizon {:.12}y",
                                bond.id.as_str(), reset, tenor_years, times[terminal_step]
                            )));
                    }
                    let observation_end_step = nearest_step(&times, observation_end_time);
                    if observation_end_step <= reset_step {
                        return Err(Error::Validation(format!(
                                "Bond '{}' term-index observation at {} with tenor {:.12}y maps to no positive rates-credit interval",
                                bond.id.as_str(), reset, tenor_years
                            )));
                    }
                    let index_accrual = times[observation_end_step] - times[reset_step];
                    let reset_grid_date = step_dates[reset_step].ok_or_else(|| {
                        Error::Validation(format!(
                            "Bond '{}' term-index reset step {} has no calendar date",
                            bond.id.as_str(),
                            reset_step
                        ))
                    })?;
                    let observation_end_date =
                        step_dates[observation_end_step].ok_or_else(|| {
                            Error::Validation(format!(
                                "Bond '{}' term-index observation-end step {} has no calendar date",
                                bond.id.as_str(),
                                observation_end_step
                            ))
                        })?;
                    let base_df =
                        discount.df_between_dates(reset_grid_date, observation_end_date)?;
                    if !base_df.is_finite() || base_df <= 0.0 {
                        return Err(Error::Validation(format!(
                                "Bond '{}' has an invalid reference discount factor over the term-index observation [{:.12}, {:.12}]",
                                bond.id.as_str(), times[reset_step], times[observation_end_step]
                            )));
                    }
                    ObservedRateSource::Conditional(ConditionalRate {
                        observation_step: reset_step,
                        accrual: index_accrual,
                        base_index_rate,
                        base_discount_forward: (1.0 / base_df - 1.0) / index_accrual,
                        conditional_discount_factors: tree.conditional_discount_factors(
                            reset_step,
                            observation_end_step,
                            times[terminal_step],
                        )?,
                    })
                };
                (
                    FloatingRateModel::Term(source),
                    FloatingRateObservation::Term {
                        reset_date: reset,
                        tenor_years,
                    },
                )
            };
            let compiled = CompiledFloatingCoupon::compile(
                FloatingCouponPeriod {
                    accrual_start: start,
                    accrual_end: end,
                    payment_date: payment,
                    day_count,
                    accrual_factor: group.accrual,
                },
                FloatingCouponEconomics {
                    cash_fraction,
                    pik_fraction,
                    rate_params: params.clone(),
                },
                observation,
            )?;
            let initial_notional = (start < as_of)
                .then(|| {
                    scheduled_outstanding_after(
                        bond,
                        schedule.get_flows(),
                        start,
                        final_redemption_date,
                    )
                })
                .transpose()?;
            let initial_term_rate = match &rate_model {
                FloatingRateModel::Term(ObservedRateSource::Fixed(rate)) if reset <= as_of => {
                    Some(*rate)
                }
                _ => None,
            };
            floating.push(FloatingCoupon {
                reset_step,
                accrual_start_step,
                payment_step,
                compiled,
                rate_model,
                initial_notional,
                initial_term_rate,
            });
        }

        let mut exercise_by_date = BTreeMap::<Date, ExerciseDate>::new();
        let mut make_whole_claims = Vec::new();
        let mut make_whole_bases = Vec::<MakeWholeBasis>::new();
        if let Some(call_put) = &bond.call_put {
            for call in &call_put.calls {
                for date in exercise_dates(call, as_of, bond.maturity) {
                    let entry = exercise_by_date
                        .entry(date)
                        .or_insert_with(|| ExerciseDate {
                            date,
                            calls: Vec::new(),
                            puts: Vec::new(),
                            return_floor: false,
                        });
                    let has_later_reference_cash = schedule
                        .get_flows()
                        .iter()
                        .any(|flow| flow.date > date && is_cash_settlement_kind(flow.kind));
                    let make_whole = if call.make_whole.is_none() {
                        None
                    } else if date == bond.maturity && !has_later_reference_cash {
                        // Exercise occurs after same-day holder cash. At the
                        // terminal decision there are no later reference-basis
                        // cashflows to regress; the clean contractual call
                        // floor remains authoritative. A rolled payment after
                        // contractual maturity remains a live reference claim.
                        Some(MakeWholeExercise::Deterministic(0.0))
                    } else if tree.config.rate_vol > 0.0 {
                        let claim_id = make_whole_claims.len();
                        let (exercise_step, basis) = build_make_whole_claim(
                            call,
                            date,
                            discount.as_ref(),
                            market,
                            as_of,
                            &times,
                        )?;
                        let basis_index = make_whole_bases
                            .iter()
                            .position(|existing| {
                                existing.interval_adjustments == basis.interval_adjustments
                            })
                            .unwrap_or_else(|| {
                                let index = make_whole_bases.len();
                                make_whole_bases.push(basis);
                                index
                            });
                        make_whole_claims.push(MakeWholeClaim {
                            exercise_step,
                            decision_index: usize::MAX,
                            basis_index,
                        });
                        Some(MakeWholeExercise::Conditional(claim_id))
                    } else {
                        make_whole_value(call, date, schedule.get_flows(), market)?
                            .map(MakeWholeExercise::Deterministic)
                    };
                    entry.calls.push(ExerciseCall {
                        price_pct_of_par: call.price_pct_of_par,
                        make_whole,
                    });
                }
            }
            for put in &call_put.puts {
                for date in exercise_dates(put, as_of, bond.maturity) {
                    let entry = exercise_by_date
                        .entry(date)
                        .or_insert_with(|| ExerciseDate {
                            date,
                            calls: Vec::new(),
                            puts: Vec::new(),
                            return_floor: false,
                        });
                    entry.puts.push(put.clone());
                }
            }
        }
        if let Some(spec) = bond.return_floor.as_ref() {
            for date in return_floor_dates(spec.window, bond.issue_date, bond.maturity, as_of)? {
                exercise_by_date
                    .entry(date)
                    .or_insert_with(|| ExerciseDate {
                        date,
                        calls: Vec::new(),
                        puts: Vec::new(),
                        return_floor: false,
                    })
                    .return_floor = true;
            }
        }
        let include_as_of_cash = exercise_by_date.contains_key(&as_of);
        let entitled_cashflows = if include_as_of_cash {
            bond.pricing_dated_cashflows_from_schedule_inclusive(&schedule, as_of, as_of)?
        } else {
            bond.pricing_dated_cashflows_from_schedule(&schedule, as_of, as_of)?
        };
        let mut entitled_counts = BTreeMap::<(Date, u64), usize>::new();
        for (date, amount) in entitled_cashflows {
            *entitled_counts
                .entry((date, amount.amount().to_bits()))
                .or_default() += 1;
        }

        let mut static_cash = vec![Vec::new(); times.len()];
        let mut balance_events = vec![Vec::new(); times.len()];
        let mut static_accruals = Vec::new();
        let mut static_distributions = vec![Vec::new(); times.len()];
        for (flow_index, flow) in schedule.get_flows().iter().enumerate() {
            if flow.date < as_of || dynamic_flow_indices.contains(&flow_index) {
                continue;
            }
            if flow.kind == CFKind::Notional
                && flow.date == bond.issue_date
                && flow.amount.amount() < 0.0
            {
                // The replay starts with issued principal already outstanding;
                // the negative inception exchange is an investor cashflow,
                // not a second draw into the modeled balance.
                continue;
            }
            let event_time =
                grid_day_count.year_fraction(as_of, flow.date, DayCountContext::default())?;
            let step = nearest_step(&times, event_time);
            let terminal_redemption = flow.kind == CFKind::Notional
                && final_redemption_date.is_some_and(|date| flow.date == date);
            if is_cash_settlement_kind(flow.kind) && !terminal_redemption {
                let entitlement_key = (flow.date, flow.amount.amount().to_bits());
                let entitled = entitled_counts
                    .get_mut(&entitlement_key)
                    .is_some_and(|count| {
                        if *count == 0 {
                            false
                        } else {
                            *count -= 1;
                            true
                        }
                    });
                if entitled {
                    let event_minus_step = event_time - times[step];
                    if event_minus_step.abs() > 1.0e-10 {
                        return Err(Error::Validation(format!(
                            "Bond '{}' cashflow date {} is not an exact node on the daily ACT/365F rates-credit grid (offset {event_minus_step}); rebuild calibration with build_daily_bond_rates_credit_targets",
                            bond.id.as_str(), flow.date
                        )));
                    }
                    static_cash[step].push(CashEvent {
                        amount_at_step: flow.amount.amount(),
                        event_minus_step,
                    });
                }
            }
            if flow.date >= as_of {
                if let Some(delta) = static_balance_delta(flow, terminal_redemption) {
                    balance_events[step].push(BalanceEvent { delta });
                }
            }
            if is_holder_distribution(flow) {
                static_distributions[step].push(DistributionEvent {
                    date: flow.date,
                    amount: flow.amount.amount().max(0.0),
                });
            }
            if let Some(accrual) = flow
                .accrual
                .as_ref()
                .filter(|_| matches!(flow.kind, CFKind::Fixed | CFKind::FloatReset | CFKind::Pik))
            {
                static_accruals.push(AccrualClaim {
                    start: accrual.start,
                    end: accrual.end,
                    payment: flow.date,
                    day_count: accrual.day_count,
                    amount: flow.amount.amount(),
                    pik: flow.kind == CFKind::Pik,
                });
            }
        }

        let mut exercise = vec![Vec::new(); times.len()];
        for item in exercise_by_date.into_values() {
            let event_time =
                grid_day_count.year_fraction(as_of, item.date, DayCountContext::default())?;
            exercise[nearest_step(&times, event_time)].push(item);
        }
        let mut decision_steps = vec![0, terminal_step];
        decision_steps.extend(
            exercise
                .iter()
                .enumerate()
                .filter(|(_, entries)| !entries.is_empty())
                .map(|(step, _)| step),
        );
        decision_steps.sort_unstable();
        decision_steps.dedup();
        let redemption_step = final_redemption_date
            .filter(|date| *date > as_of || (*date == as_of && include_as_of_cash))
            .map(|date| exact_grid_step(&times, as_of, date))
            .transpose()?;
        if let Some(step) = redemption_step {
            if decision_steps.binary_search(&step).is_err() {
                decision_steps.push(step);
                decision_steps.sort_unstable();
            }
        }
        for claim in &mut make_whole_claims {
            claim.decision_index =
                decision_steps
                    .binary_search(&claim.exercise_step)
                    .map_err(|_| {
                        Error::internal(
                            "bond hazard LSMC make-whole claim has no matching decision step",
                        )
                    })?;
        }

        let mut floating_reset_ids = vec![Vec::new(); times.len()];
        let mut floating_accrual_start_ids = vec![Vec::new(); times.len()];
        let mut floating_payment_ids = vec![Vec::new(); times.len()];
        for (id, coupon) in floating.iter().enumerate() {
            if matches!(coupon.rate_model, FloatingRateModel::Term(_))
                && coupon.initial_term_rate.is_none()
            {
                floating_reset_ids[coupon.reset_step].push(id);
            }
            if coupon.initial_notional.is_none() {
                floating_accrual_start_ids[coupon.accrual_start_step].push(id);
            }
            floating_payment_ids[coupon.payment_step].push(id);
        }

        let max_rate_history_steps = floating
            .iter()
            .filter_map(|coupon| match &coupon.rate_model {
                FloatingRateModel::Overnight(overnight) => Some(overnight.max_history_steps),
                FloatingRateModel::Term(_) => None,
            })
            .max()
            .unwrap_or(0);

        Ok(Self {
            times,
            step_dates,
            static_cash,
            balance_events,
            floating,
            floating_reset_ids,
            floating_accrual_start_ids,
            floating_payment_ids,
            static_accruals,
            static_distributions,
            exercise,
            decision_steps,
            initial_outstanding,
            redemption_step,
            call_friction_cents: bond
                .instrument_pricing_overrides
                .model_config
                .call_friction_cents
                .unwrap_or(0.0),
            recovery_rate: tree.recovery_rate(),
            return_floor,
            historical_distribution_cash,
            historical_distribution_target_pv,
            make_whole_claims,
            make_whole_bases,
            max_rate_history_steps,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn decision_snapshot(
        &self,
        bond: &Bond,
        step: usize,
        path: &[RatesCreditPathState],
        state: StepReplay,
        floating: &[FloatingRuntimeState],
        exercise_provider: Option<&dyn BondLsmcExerciseProvider>,
        make_whole_policies: Option<&[RegressionPolicy]>,
    ) -> Result<DecisionSnapshot> {
        let representative_date = self.exercise[step]
            .first()
            .map(|entry| entry.date)
            .or_else(|| self.step_dates.get(step).copied().flatten())
            .unwrap_or(bond.maturity);
        let accrued = self.accrued_state(representative_date, floating)?;
        let mut locked_coupon = 0.0;
        for (id, coupon) in self.floating.iter().enumerate() {
            if matches!(&coupon.rate_model, FloatingRateModel::Term(_))
                && coupon.reset_step <= step
                && step < coupon.payment_step
                && coupon.compiled.captured_notional(&floating[id]).is_some()
                && coupon
                    .compiled
                    .locked_term_index_rate(&floating[id])
                    .is_some()
            {
                locked_coupon += coupon.compiled.settle(&floating[id])?.total_amount;
            }
        }
        let factor = path_state(path, step)?;
        let (call, put) = self.exercise_amounts(ExerciseInputs {
            bond,
            step,
            path,
            outstanding: state.outstanding,
            locked_coupon,
            coupon_states: floating,
            cumulative_distribution_cash: state.cumulative_distribution_cash,
            cumulative_distribution_target_pv: state.cumulative_distribution_target_pv,
            provider: exercise_provider,
            make_whole_policies,
        })?;
        Ok(DecisionSnapshot {
            step,
            features: [
                factor.short_rate,
                factor.hazard_rate,
                state.outstanding,
                locked_coupon,
                accrued.0,
                accrued.1,
                state.cumulative_distribution_cash,
                state.cumulative_distribution_target_pv,
            ],
            current_cash: state.current_cash,
            hold_redemption: if self.redemption_step == Some(step) {
                state.outstanding
            } else {
                0.0
            },
            call,
            put,
            friction: state.outstanding * (self.call_friction_cents / 10_000.0),
            a_to_next: 0.0,
            b_to_next: 0.0,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn replay_block(
        &self,
        tree: &RatesCreditTree,
        bond: &Bond,
        config: &BondLsmcConfig,
        seed: u64,
        path_index: u64,
        antithetic: bool,
        checkpoint: &TrainingCheckpoint,
        low_decision: usize,
        high_decision: usize,
        exercise_provider: Option<&dyn BondLsmcExerciseProvider>,
        make_whole_policies: Option<&[RegressionPolicy]>,
        path: &mut Vec<RatesCreditPathState>,
    ) -> Result<BlockPathRecord> {
        let low_step = self.decision_steps[low_decision];
        let high_step = self.decision_steps[high_decision];
        tree.sample_path_segment_into(
            seed,
            path_index,
            antithetic,
            checkpoint.factor,
            high_step,
            path,
        )?;
        let mut cursor = ReplayCursor::from_checkpoint(self, &checkpoint.product)?;
        let mut step_states = Vec::with_capacity(high_step - low_step + 1);
        let mut decision_snapshots = Vec::with_capacity(high_decision - low_decision + 1);
        let mut next_decision = low_decision;
        for step in low_step..=high_step {
            let state = cursor.advance_step(self, bond, path, config, step)?;
            step_states.push(state);
            if next_decision <= high_decision && self.decision_steps[next_decision] == step {
                decision_snapshots.push(self.decision_snapshot(
                    bond,
                    step,
                    path,
                    state,
                    &cursor.floating,
                    exercise_provider,
                    make_whole_policies,
                )?);
                next_decision += 1;
            }
        }
        if decision_snapshots.len() != high_decision - low_decision + 1 {
            return Err(Error::internal(
                "bond hazard LSMC block replay missed a decision snapshot",
            ));
        }
        let oas = config.oas_bp / 10_000.0;
        for (local, snapshot) in decision_snapshots
            .iter_mut()
            .take(high_decision - low_decision)
            .enumerate()
        {
            let from = self.decision_steps[low_decision + local];
            let to = self.decision_steps[low_decision + local + 1];
            let mut a = 1.0;
            let mut b = 0.0;
            for step in from..to {
                let factor = path_state(path, step)?;
                let state = step_states[step - low_step];
                let dt = self.times[step + 1] - self.times[step];
                let interval_df = factor.discount_to_next * (-oas * dt).exp();
                let recovery = self.recovery_rate
                    * state.outstanding
                    * continuous_frp_weight(interval_df, factor.survival_to_next)?;
                let cash = if step > from { state.current_cash } else { 0.0 };
                b += a * (recovery + cash);
                a *= interval_df * factor.survival_to_next;
            }
            snapshot.a_to_next = a;
            snapshot.b_to_next = b;
        }
        let terminal = (high_decision + 1 == self.decision_steps.len())
            .then(|| decision_snapshots.pop())
            .flatten();
        if terminal.is_none() {
            decision_snapshots.pop();
        }
        Ok(BlockPathRecord {
            snapshots: decision_snapshots,
            terminal,
            step_states,
        })
    }

    pub(super) fn replay(
        &self,
        _tree: &RatesCreditTree,
        bond: &Bond,
        path: &[RatesCreditPathState],
        config: &BondLsmcConfig,
        exercise_provider: Option<&dyn BondLsmcExerciseProvider>,
        make_whole_policies: Option<&[RegressionPolicy]>,
    ) -> Result<PathRecord> {
        if path.len() != self.times.len() {
            return Err(Error::Validation(format!(
                "bond hazard LSMC path has {} states but calibrated grid has {}",
                path.len(),
                self.times.len()
            )));
        }
        let oas = config.oas_bp / 10_000.0;
        let mut cursor = ReplayCursor::new(self)?;
        let mut step_cash = vec![0.0; self.times.len()];
        let mut outstanding_after_events = vec![0.0; self.times.len()];
        let mut snapshots = Vec::with_capacity(self.decision_steps.len());
        let mut next_decision = 0_usize;

        for step in 0..self.times.len() {
            let state = cursor.advance_step(self, bond, path, config, step)?;
            step_cash[step] = state.current_cash;
            outstanding_after_events[step] = state.outstanding;
            if next_decision < self.decision_steps.len()
                && self.decision_steps[next_decision] == step
            {
                snapshots.push(self.decision_snapshot(
                    bond,
                    step,
                    path,
                    state,
                    &cursor.floating,
                    exercise_provider,
                    make_whole_policies,
                )?);
                next_decision += 1;
            }
        }

        if snapshots.len() != self.decision_steps.len() {
            return Err(Error::internal(
                "bond hazard LSMC full replay missed a decision snapshot",
            ));
        }

        for decision in 0..snapshots.len().saturating_sub(1) {
            let from = snapshots[decision].step;
            let to = snapshots[decision + 1].step;
            let mut a = 1.0;
            let mut b = 0.0;
            for step in (from..to).rev() {
                let dt = self.times[step + 1] - self.times[step];
                let interval_df = path[step].discount_to_next * (-oas * dt).exp();
                let q = interval_df * path[step].survival_to_next;
                let recovery = self.recovery_rate
                    * outstanding_after_events[step]
                    * continuous_frp_weight(interval_df, path[step].survival_to_next)?;
                a *= q;
                b = q * b + recovery;
                if step > from {
                    b += step_cash[step];
                }
            }
            snapshots[decision].a_to_next = a;
            snapshots[decision].b_to_next = b;
        }
        Ok(PathRecord { snapshots })
    }

    pub(super) fn accrued_state(
        &self,
        date: Date,
        coupon_states: &[FloatingRuntimeState],
    ) -> Result<(f64, f64)> {
        let mut cash = 0.0;
        let mut pik = 0.0;
        for claim in &self.static_accruals {
            if date <= claim.start || date >= claim.payment {
                continue;
            }
            let elapsed_end = date.min(claim.end);
            let elapsed = claim.day_count.year_fraction(
                claim.start,
                elapsed_end,
                DayCountContext::default(),
            )?;
            let total = claim.day_count.year_fraction(
                claim.start,
                claim.end,
                DayCountContext::default(),
            )?;
            let amount = claim.amount * safe_accrual_ratio(elapsed, total);
            if claim.pik {
                pik += amount;
            } else {
                cash += amount;
            }
        }
        for (id, coupon) in self.floating.iter().enumerate() {
            let period = coupon.compiled.period();
            if date <= period.accrual_start || date >= period.payment_date {
                continue;
            }
            let accrued_amount = coupon.compiled.accrued_amount(&coupon_states[id], date)?;
            cash += accrued_amount * coupon.compiled.economics().cash_fraction;
            pik += accrued_amount * coupon.compiled.economics().pik_fraction;
        }
        Ok((cash, pik))
    }

    pub(super) fn exercise_amounts(
        &self,
        inputs: ExerciseInputs<'_>,
    ) -> Result<(Option<f64>, Option<f64>)> {
        let ExerciseInputs {
            bond,
            step,
            path,
            outstanding,
            locked_coupon,
            coupon_states,
            cumulative_distribution_cash,
            cumulative_distribution_target_pv,
            provider,
            make_whole_policies,
        } = inputs;
        let mut call: Option<f64> = None;
        let mut put: Option<f64> = None;
        let factor = path_state(path, step)?;
        for entry in &self.exercise[step] {
            let (accrued_cash, accrued_pik) = self.accrued_state(entry.date, coupon_states)?;
            let accrued = accrued_cash + accrued_pik;
            let return_floor = if entry.return_floor {
                Some(
                    self.return_floor
                        .ok_or_else(|| {
                            Error::internal(
                                "bond hazard LSMC return-floor date has no floor specification",
                            )
                        })?
                        .redemption(
                            entry.date,
                            outstanding,
                            cumulative_distribution_cash,
                            cumulative_distribution_target_pv,
                            accrued,
                        )?,
                )
            } else {
                None
            };
            let default_call = if entry.calls.is_empty() {
                return_floor.map(|floor| floor + accrued)
            } else {
                entry
                    .calls
                    .iter()
                    .map(|option| {
                        let contractual = outstanding * option.price_pct_of_par / 100.0;
                        let make_whole_dirty = match &option.make_whole {
                            None => contractual,
                            Some(MakeWholeExercise::Deterministic(value)) => *value,
                            Some(MakeWholeExercise::Conditional(claim_id)) => {
                                let Some(policies) = make_whole_policies else {
                                    return Ok(contractual);
                                };
                                let policy = policies.get(*claim_id).ok_or_else(|| {
                                    Error::internal(format!(
                                        "bond hazard LSMC has no make-whole policy for claim {claim_id}"
                                    ))
                                })?;
                                policy.predict(&[
                                    factor.short_rate,
                                    0.0,
                                    outstanding,
                                    locked_coupon,
                                    accrued_cash,
                                    accrued_pik,
                                    cumulative_distribution_cash,
                                    cumulative_distribution_target_pv,
                                ])?
                            }
                        };
                        Ok(contractual
                            .max((make_whole_dirty - accrued).max(0.0))
                            .max(return_floor.unwrap_or(0.0))
                            + accrued)
                    })
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .reduce(f64::min)
            };
            let default_put = entry
                .puts
                .iter()
                .map(|option| outstanding * option.price_pct_of_par / 100.0 + accrued)
                .reduce(f64::max);
            let amounts = if let Some(provider) = provider {
                let state = BondLsmcExerciseState {
                    bond,
                    date: entry.date,
                    step,
                    short_rate: factor.short_rate,
                    hazard_rate: factor.hazard_rate,
                    outstanding,
                    locked_coupon,
                    accrued_cash,
                    accrued_pik,
                    cumulative_distribution_cash,
                    cumulative_distribution_target_pv,
                    default_call,
                    default_put,
                };
                state.validate()?;
                provider.exercise_amounts(&state)?
            } else {
                BondLsmcExerciseAmounts {
                    call: default_call,
                    put: default_put,
                }
            };
            validate_exercise_amount("call", amounts.call)?;
            validate_exercise_amount("put", amounts.put)?;
            if let Some(value) = amounts.call {
                call = Some(call.map_or(value, |existing| existing.min(value)));
            }
            if let Some(value) = amounts.put {
                put = Some(put.map_or(value, |existing| existing.max(value)));
            }
        }
        if let (Some(call), Some(put)) = (call, put) {
            if put > call + 1.0e-10 {
                return Err(Error::Validation(format!(
                    "Bond '{}' has incompatible path-dependent barriers at step {step}: holder put {put} exceeds issuer call {call}",
                    bond.id.as_str()
                )));
            }
        }
        Ok((call, put))
    }
}
