//! `ReplayTemplate` assembly: initial balance, exercise schedule, and static grid events.

use super::super::*;

/// Inputs shared by the [`ReplayTemplate`] builders.
#[derive(Clone, Copy)]
pub(super) struct TemplateInputs<'a> {
    pub(super) tree: &'a RatesCreditTree,
    pub(super) bond: &'a Bond,
    pub(super) market: &'a MarketContext,
    pub(super) as_of: Date,
    pub(super) times: &'a [f64],
    pub(super) step_dates: &'a [Option<Date>],
    pub(super) schedule: &'a finstack_quant_cashflows::builder::CashFlowSchedule,
    pub(super) discount:
        &'a std::sync::Arc<finstack_quant_core::market_data::term_structures::DiscountCurve>,
    pub(super) final_redemption_date: Option<Date>,
}

/// Exercise decisions built by [`ReplayTemplate::build_exercise`].
struct ExerciseEvents {
    by_date: BTreeMap<Date, ExerciseDate>,
    make_whole_claims: Vec<MakeWholeClaim>,
    make_whole_bases: Vec<MakeWholeBasis>,
}

/// Static grid events built by [`ReplayTemplate::build_static_events`].
struct StaticEvents {
    static_cash: Vec<Vec<CashEvent>>,
    balance_events: Vec<Vec<BalanceEvent>>,
    static_accruals: Vec<AccrualClaim>,
    static_distributions: Vec<Vec<DistributionEvent>>,
    static_redemption_pik: f64,
}

impl ReplayTemplate {
    pub(in super::super) fn new(
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
            .filter(|flow| is_par_redemption(flow))
            .map(|flow| flow.get_date())
            .max();
        let inputs = TemplateInputs {
            tree,
            bond,
            market,
            as_of,
            times: &times,
            step_dates: &step_dates,
            schedule: &schedule,
            discount: &discount,
            final_redemption_date,
        };
        let return_floor = bond
            .return_floor
            .as_ref()
            .map(|spec| {
                spec.validate()?;
                Ok::<_, Error>(ReturnFloorTemplate {
                    kind: spec.kind,
                    issue_price: spec.issue_price.resolve(bond.notional)?,
                    issue_date: bond.issue_date,
                    // XIRR floors are on the fixed ACT/365F clock (record 017).
                    day_count: DayCount::Act365F,
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

        let initial_outstanding = Self::build_initial_outstanding(&inputs)?;

        let (floating, dynamic_flow_indices) =
            Self::build_floating(&inputs, include_as_of_option_cash)?;
        if matches!(
            &bond.cashflow_spec,
            CashflowSpec::Amortizing { schedule, .. }
                if !matches!(schedule, finstack_quant_cashflows::builder::AmortizationSpec::None)
        ) && floating.iter().any(|coupon| {
            coupon.capitalizes
                && match &coupon.rate_model {
                    FloatingRateModel::Term(ObservedRateSource::Conditional(_)) => true,
                    FloatingRateModel::Overnight(model) => model
                        .sources
                        .values()
                        .any(|source| matches!(source, ObservedRateSource::Conditional(_))),
                    FloatingRateModel::Term(ObservedRateSource::Fixed(_)) => false,
                }
        }) {
            return Err(Error::Validation(format!(
                "Bond '{}' hazard LSMC does not support stochastic floating PIK with \
                 amortization: scheduled repayments require canonical runtime balance \
                 targets rather than deterministic projected PIK amounts",
                bond.id.as_str()
            )));
        }

        let ExerciseEvents {
            by_date: exercise_by_date,
            mut make_whole_claims,
            make_whole_bases,
        } = Self::build_exercise(&inputs)?;
        let include_as_of_cash = exercise_by_date.contains_key(&as_of);
        let StaticEvents {
            static_cash,
            balance_events,
            static_accruals,
            static_distributions,
            static_redemption_pik,
        } = Self::build_static_events(&inputs, &dynamic_flow_indices, include_as_of_cash)?;

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
        if let Some(step) = redemption_step
            && decision_steps.binary_search(&step).is_err()
        {
            decision_steps.push(step);
            decision_steps.sort_unstable();
        }
        for claim in &mut make_whole_claims {
            claim.decision_index =
                decision_steps
                    .binary_search(&claim.exercise_step)
                    .map_err(|_| {
                        Error::Internal(
                            "bond hazard LSMC make-whole claim has no matching decision step"
                                .into(),
                        )
                    })?;
        }

        let mut floating_reset_ids = vec![Vec::new(); times.len()];
        let mut floating_accrual_start_ids = vec![Vec::new(); times.len()];
        let mut floating_payment_ids = vec![Vec::new(); times.len()];
        let mut floating_balance_ids = vec![Vec::new(); times.len()];
        for (id, coupon) in floating.iter().enumerate() {
            if matches!(coupon.rate_model, FloatingRateModel::Term(_))
                && coupon.initial_term_rate.is_none()
            {
                floating_reset_ids[coupon.reset_step].push(id);
            }
            if coupon.initial_notional.is_none() {
                floating_accrual_start_ids[coupon.accrual_start_step].push(id);
            }
            if coupon.pays_cash {
                floating_payment_ids[coupon.payment_step].push(id);
            }
            if coupon.capitalizes && coupon.compiled.period().accrual_end >= as_of {
                floating_balance_ids[coupon.effective_end_step].push(id);
            }
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
            floating_balance_ids,
            static_accruals,
            static_distributions,
            static_redemption_pik,
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

    /// Principal immediately before the origin's contractual events. The
    /// replay then applies same-day PIK/amortization before exercise while
    /// leaving a same-day final redemption as the holder's terminal claim.
    fn build_initial_outstanding(inputs: &TemplateInputs<'_>) -> Result<f64> {
        let TemplateInputs {
            bond,
            as_of,
            schedule,
            final_redemption_date,
            ..
        } = *inputs;
        // Start immediately before the origin's contractual events. The replay
        // then applies same-day PIK/amortization before exercise while leaving
        // a same-day final redemption available as the holder's terminal claim.
        let mut initial_outstanding = bond.notional.amount();
        for flow in schedule.get_flows().iter().filter(|flow| {
            flow.get_balance_date()
                .is_some_and(|balance| balance < as_of)
        }) {
            if is_opening_funding(flow, bond.issue_date) {
                continue;
            }
            let terminal_redemption = is_terminal_redemption(flow, final_redemption_date);
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

        Ok(initial_outstanding)
    }

    /// Exercise decisions by date (calls, puts, return-floor claims) and the
    /// make-whole claims and regression bases of conditional make-whole calls.
    fn build_exercise(inputs: &TemplateInputs<'_>) -> Result<ExerciseEvents> {
        let TemplateInputs {
            tree,
            bond,
            market,
            as_of,
            times,
            schedule,
            discount,
            ..
        } = *inputs;
        let event_dates = BondValuator::exercise_event_dates(schedule);
        let mut exercise_by_date = BTreeMap::<Date, ExerciseDate>::new();
        let mut make_whole_claims = Vec::new();
        let mut make_whole_bases = Vec::<MakeWholeBasis>::new();
        if let Some(call_put) = &bond.call_put {
            for call in &call_put.calls {
                for date in exercise_dates(call, as_of, bond.maturity, &event_dates) {
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
                        .any(|flow| flow.get_date() > date && flow.get_kind().is_cash_settlement());
                    let make_whole = if call.make_whole.is_none() {
                        None
                    } else if date == bond.maturity && !has_later_reference_cash {
                        // Exercise occurs after same-day holder cash. At the
                        // terminal decision there are no later reference-basis
                        // cashflows to regress; the clean contractual call
                        // floor remains authoritative. A rolled payment after
                        // contractual maturity remains a live reference claim.
                        Some(MakeWholeExercise::Deterministic(0.0))
                    } else if tree.get_config().rate_vol > 0.0 {
                        let claim_id = make_whole_claims.len();
                        let (exercise_step, basis) = build_make_whole_claim(
                            call,
                            date,
                            discount.as_ref(),
                            market,
                            as_of,
                            times,
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
                for date in exercise_dates(put, as_of, bond.maturity, &event_dates) {
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
            for date in return_floor_dates(
                spec.window,
                bond.issue_date,
                bond.maturity,
                as_of,
                &event_dates,
            )? {
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
        Ok(ExerciseEvents {
            by_date: exercise_by_date,
            make_whole_claims,
            make_whole_bases,
        })
    }

    /// Static (non-refixing) cash, balance, accrual and distribution events on
    /// the grid.
    fn build_static_events(
        inputs: &TemplateInputs<'_>,
        dynamic_flow_indices: &BTreeSet<usize>,
        include_as_of_cash: bool,
    ) -> Result<StaticEvents> {
        let TemplateInputs {
            bond,
            as_of,
            times,
            schedule,
            final_redemption_date,
            ..
        } = *inputs;
        let grid_day_count = DayCount::Act365F;
        let entitled_cashflows = if include_as_of_cash {
            bond.pricing_dated_cashflows_from_schedule_inclusive(schedule, as_of, as_of)?
        } else {
            bond.pricing_dated_cashflows_from_schedule(schedule, as_of, as_of)?
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
        let mut static_redemption_pik = 0.0;
        for (flow_index, flow) in schedule.get_flows().iter().enumerate() {
            if dynamic_flow_indices.contains(&flow_index) {
                continue;
            }
            if is_opening_funding(flow, bond.issue_date) {
                // The replay starts with issued principal already outstanding;
                // the negative inception exchange is an investor cashflow,
                // not a second draw into the modeled balance.
                continue;
            }
            let event_time = if flow.get_date() >= as_of {
                grid_day_count.year_fraction(as_of, flow.get_date(), DayCountContext::default())?
            } else {
                0.0
            };
            let step = nearest_step(times, event_time);
            let terminal_redemption = is_terminal_redemption(flow, final_redemption_date);
            if flow.get_kind() == CFKind::Pik
                && final_redemption_date.is_some_and(|redemption| {
                    flow.get_date() <= redemption
                        && flow
                            .get_balance_date()
                            .is_some_and(|balance| balance > redemption)
                })
            {
                static_redemption_pik += static_balance_delta(flow, false).unwrap_or(0.0);
            }
            if flow.get_date() >= as_of
                && flow.get_kind().is_cash_settlement()
                && !terminal_redemption
            {
                let entitlement_key = (flow.get_date(), flow.get_amount().amount().to_bits());
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
                            bond.id.as_str(),
                            flow.get_date()
                        )));
                    }
                    static_cash[step].push(CashEvent {
                        amount_at_step: flow.get_amount().amount(),
                        event_minus_step,
                    });
                }
            }
            if let Some(balance_date) = flow.get_balance_date()
                && balance_date >= as_of
                && let Some(delta) = static_balance_delta(flow, terminal_redemption)
            {
                let balance_step = exact_grid_step(times, as_of, balance_date)?;
                balance_events[balance_step].push(BalanceEvent { delta });
            }
            if flow.get_date() >= as_of && is_holder_distribution(flow) {
                static_distributions[step].push(DistributionEvent {
                    date: flow.get_date(),
                    amount: flow.get_amount().amount().max(0.0),
                });
            }
            if let Some(accrual) = flow
                .get_accrual()
                .as_ref()
                .filter(|_| {
                    matches!(
                        flow.get_kind(),
                        CFKind::Fixed | CFKind::Floating | CFKind::Pik
                    )
                })
                .filter(|accrual| flow.get_date() >= as_of || accrual.get_end() >= as_of)
            {
                static_accruals.push(AccrualClaim {
                    start: accrual.get_start(),
                    end: accrual.get_end(),
                    payment: flow.get_date(),
                    day_count: accrual.get_day_count(),
                    day_count_context: DayCountContext {
                        calendar: Some(calendar_by_id_strict(
                            accrual
                                .get_context()
                                .get_calendar_id()
                                .unwrap_or(&bond.cashflow_spec.schedule().calendar_id),
                        )?),
                        frequency: Some(bond.cashflow_spec.schedule().frequency),
                        bus_basis: None,
                        coupon_period: accrual.get_context().get_coupon_period(),
                        end_is_termination_date: accrual
                            .get_context()
                            .get_end_is_termination_date(),
                    },
                    // A PIK claim is its explicit capitalization, not its cash
                    // amount (record CORE-CONVENTIONS-015 #1, #4).
                    amount: if flow.get_kind() == CFKind::Pik {
                        flow.get_principal_delta()
                            .ok_or_else(|| {
                                finstack_quant_core::Error::Validation(format!(
                                    "PIK row on {} has no principal_delta",
                                    flow.get_date()
                                ))
                            })?
                            .amount()
                    } else {
                        flow.get_amount().amount()
                    },
                    pik: flow.get_kind() == CFKind::Pik,
                });
            }
        }

        Ok(StaticEvents {
            static_cash,
            balance_events,
            static_accruals,
            static_distributions,
            static_redemption_pik,
        })
    }
}
