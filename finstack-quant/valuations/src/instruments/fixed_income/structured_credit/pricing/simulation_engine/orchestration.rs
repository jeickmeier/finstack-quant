use super::*;

/// Release the unused spread-account balance to equity at deal end.
///
/// Record a residual principal distribution to a tranche at deal end: appends
/// the flow to the tranche's cashflows and principal flows and bumps its
/// `total_principal`. No-op if the tranche has no results entry. Shared by the
/// excess-spread and controlled-accumulation terminal sweeps.
fn append_residual_principal(
    state: &mut SimulationState<'_>,
    tranche_id: &str,
    amount: Money,
    date: Date,
) -> Result<()> {
    if let Some(res) = state.results.get_mut(tranche_id) {
        res.cashflows.push((date, amount));
        res.principal_flows.push((date, amount));
        res.total_principal = res.total_principal.checked_add(amount)?;
    }
    Ok(())
}

/// At legal termination retained interest pays deferred coupons, then reaches
/// the residual holder. An active cash-trap rule first applies it to debt par.
/// Cash is never extinguished merely because a trap remains breached.
fn release_spread_account(
    state: &mut SimulationState<'_>,
    instrument: &StructuredCredit,
) -> Result<()> {
    let mut remaining = state
        .spread_account
        .checked_add(state.undistributed_interest)?
        .amount();
    let trapped = instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.excess_spread.as_ref())
        .and_then(|spec| spec.trap_loss_pct)
        .is_some_and(|threshold| {
            state.total_pool_balance.amount() > 0.0
                && state.cumulative_realized_loss / state.total_pool_balance.amount() >= threshold
        })
        || state
            .tranche_triggers
            .iter()
            .any(|saved| saved.trigger.breach_date.is_some());
    let date = state.prev_date.unwrap_or(state.closing_date);
    let mut order: Vec<_> = (0..state.tranches.tranches.len()).collect();
    order.sort_by_key(|&i| state.tranches.tranches[i].payment_priority);
    for i in &order {
        let tranche = &state.tranches.tranches[*i];
        if tranche.seniority == TrancheSeniority::Equity {
            continue;
        }
        let id = tranche.id.to_string();
        let deferred = state
            .deferred_interest
            .get(&id)
            .map_or(0.0, |amount| amount.amount());
        let interest = remaining.min(deferred).max(0.0);
        if interest > 0.0 {
            append_terminal_interest(state, &id, Money::new(interest, state.base_currency)?, date)?;
            state.deferred_interest.insert(
                id.clone(),
                Money::new(deferred - interest, state.base_currency)?,
            );
            remaining -= interest;
        }
        if trapped {
            let balance = state.tranche_balances[&id].amount();
            let principal = remaining.min(balance).max(0.0);
            if principal > 0.0 {
                append_residual_principal(
                    state,
                    &id,
                    Money::new(principal, state.base_currency)?,
                    date,
                )?;
                state
                    .tranche_balances
                    .insert(id, Money::new(balance - principal, state.base_currency)?);
                remaining -= principal;
            }
        }
    }
    if remaining > 0.0 {
        if let Some(&i) = order.last() {
            let id = state.tranches.tranches[i].id.to_string();
            append_terminal_interest(
                state,
                &id,
                Money::new(remaining, state.base_currency)?,
                date,
            )?;
        }
    }
    state.spread_account = Money::from((0_i64, state.base_currency));
    state.undistributed_interest = Money::from((0_i64, state.base_currency));
    Ok(())
}

fn append_terminal_interest(
    state: &mut SimulationState<'_>,
    id: &str,
    amount: Money,
    date: Date,
) -> Result<()> {
    if let Some(result) = state.results.get_mut(id) {
        result.cashflows.push((date, amount));
        result.interest_flows.push((date, amount));
        result.total_interest = result.total_interest.checked_add(amount)?;
    }
    Ok(())
}

/// Distribute terminal capital by note priority, then pay residual capital.
/// Principal collections cannot cure a deferred coupon without an explicit transfer.
fn distribute_terminal_principal(
    state: &mut SimulationState<'_>,
    amount: Money,
    date: Date,
) -> Result<()> {
    let mut remaining = amount.amount();
    let mut order: Vec<_> = (0..state.tranches.tranches.len()).collect();
    order.sort_by_key(|&i| state.tranches.tranches[i].payment_priority);
    for &i in &order {
        let id = state.tranches.tranches[i].id.to_string();
        let balance = state.tranche_balances[&id].amount();
        let paid = remaining.min(balance).max(0.0);
        if paid > 0.0 {
            append_residual_principal(state, &id, Money::new(paid, state.base_currency)?, date)?;
            state
                .tranche_balances
                .insert(id, Money::new(balance - paid, state.base_currency)?);
            remaining -= paid;
        }
    }
    if remaining > 0.0 {
        if let Some(&i) = order.last() {
            let id = state.tranches.tranches[i].id.to_string();
            append_residual_principal(
                state,
                &id,
                Money::new(remaining, state.base_currency)?,
                date,
            )?;
        }
    }
    Ok(())
}

/// Release funded reserves into principal at legal termination.
fn release_reserve_account(state: &mut SimulationState<'_>) -> Result<()> {
    distribute_terminal_principal(
        state,
        state.reserve_balance,
        state.prev_date.unwrap_or(state.closing_date),
    )?;
    state.reserve_balance = Money::from((0_i64, state.base_currency));
    Ok(())
}

/// Release undistributed and controlled-accumulation principal at termination.
fn release_principal_funding_account(state: &mut SimulationState<'_>) -> Result<()> {
    let amount = state
        .principal_funding_account
        .checked_add(state.undistributed_principal)?;
    distribute_terminal_principal(state, amount, state.prev_date.unwrap_or(state.closing_date))?;
    state.principal_funding_account = Money::from((0_i64, state.base_currency));
    state.undistributed_principal = Money::from((0_i64, state.base_currency));
    Ok(())
}

/// Loop-invariant deal simulation context built once per pricing run.
///
/// Validation, waterfall construction, calendar resolution, and the full
/// contractual schedule are pure functions of `(instrument, as_of)` — none of
/// them depends on path shocks. Preparing once and sharing read-only across
/// scenario paths is bit-identical and keeps that work out of the per-path
/// hot loop of the MC par-iter.
pub(crate) struct PreparedDealSimulation {
    /// Valuation date the simulation was prepared for (`as_of`).
    pub(crate) valuation_date: Date,
    /// Future-dated contractual payment dates (>= `valuation_date`).
    pub(crate) periods: Vec<crate::cashflow::builder::periods::SchedulePeriod>,
    /// Last contractual boundary before `valuation_date`; anchors first accrual.
    pub(crate) state_anchor: Date,
    /// Concrete base waterfall (available-funds cap layered per period).
    pub(crate) waterfall: Waterfall,
    /// Resolved strict payment calendar.
    pub(crate) calendar: &'static dyn HolidayCalendar,
    /// Payment business-day convention.
    pub(crate) convention: BusinessDayConvention,
    /// Months per payment period.
    pub(crate) months_per_period: f64,
    /// Frozen initial-state pieces shared by every path's [`SimulationState`].
    pub(crate) state_template: StateTemplate,
}

/// Prepare everything about a deal simulation that does not depend on path
/// shocks. Returns `None` when the pool is exhausted (zero balance), which
/// [`run_simulation_with_source`] maps to an empty result.
///
/// # Errors
///
/// Propagates invalid waterfall rules, invalid custom waterfalls, missing
/// payment calendars, and schedule-construction failures, in that order.
pub(crate) fn prepare_deal_simulation(
    instrument: &StructuredCredit,
    as_of: Date,
) -> Result<Option<PreparedDealSimulation>> {
    let pool = &instrument.pool;
    let tranches = &instrument.tranches;

    if pool.total_balance()?.amount() <= 0.0
        && pool.collection_account.amount() <= 0.0
        && pool.reserve_account.amount() <= 0.0
        && pool.excess_spread_account.amount() <= 0.0
        && !pool.assets.iter().any(|asset| {
            asset
                .recovery_amount
                .is_some_and(|amount| amount.amount() > 0.0)
        })
    {
        return Ok(None);
    }

    if pool
        .assets
        .iter()
        .any(|asset| asset.default_date.is_some_and(|date| date > as_of))
    {
        return Err(finstack_quant_core::Error::Validation(
            "defaulted collateral has a future default date".into(),
        ));
    }

    // Reject malformed declarative rules up front: an unresolved tranche-id
    // reference or an out-of-range trigger fraction would otherwise misprice
    // the deal *silently* (the cap/lock-out becomes a no-op). See
    // `WaterfallRules::validate`.
    if let Some(rules) = instrument.waterfall_rules.as_ref() {
        rules.validate(tranches)?;
    }

    // A custom waterfall must be structurally valid, reference only real
    // (non-equity) tranches, and not conflict with deal-level `fees`. Deals
    // arriving via JSON never pass through `with_waterfall`, so re-validate
    // here rather than pricing a silently-broken structure.
    instrument.validate_custom_waterfall()?;

    let months_per_period = match instrument.frequency.months() {
        Some(m) => m as f64,
        None => {
            return Err(finstack_quant_core::Error::Validation(
                "Structured credit instruments require month-based payment frequencies".to_string(),
            ));
        }
    };

    // Concrete base waterfall. The available-funds cap is *not* baked here:
    // it is layered onto the per-period waterfall inside `simulate_period` using
    // the live collateral WAC (`live_afc_cap_rate`), so the cap tracks pool
    // amortization/prepayment/defaults rather than being frozen at closing.
    let waterfall = instrument.create_waterfall()?;
    for tranche in &tranches.tranches {
        if waterfall.coverage_triggers.iter().any(|trigger| {
            trigger.tranche_id == tranche.id.as_str()
                && ((trigger.oc_trigger.is_some() && tranche.oc_trigger.is_some())
                    || (trigger.ic_trigger.is_some() && tranche.ic_trigger.is_some()))
        }) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "duplicate tranche/waterfall coverage configuration for {}",
                tranche.id
            )));
        }
    }

    // Resolve payment calendar - required for structured credit deals.
    // Silent fallback to weekends-only would shift coupons around holidays,
    // breaking WAC/WAL and OC tests.
    let calendar_id = instrument.payment_calendar_id.as_deref().ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "Structured credit instruments require a payment_calendar_id for accurate \
             schedule generation. Specify a valid calendar ID (e.g., 'nyse', 'target2') \
             to ensure payment dates are adjusted correctly for business days."
                .to_string(),
        )
    })?;
    let calendar = crate::cashflow::builder::calendar::resolve_calendar_strict(calendar_id)?;

    let convention = instrument
        .payment_business_day_convention
        .unwrap_or(BusinessDayConvention::ModifiedFollowing);

    // Generate the full contractual payment schedule, then filter future dates.
    // Re-anchoring the schedule at `as_of` would shift legal coupon dates for
    // seasoned deals valued between payment dates.
    use crate::cashflow::builder::periods::{
        build_periods, build_single_period, BuildPeriodsParams,
    };
    let schedule_params = crate::cashflow::builder::ScheduleParams {
        frequency: instrument.frequency,
        day_count: DayCount::Act360,
        business_day_convention: convention,
        calendar_id: calendar_id.to_string(),
        stub: StubKind::ShortBack,
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
        roll_rule: crate::cashflow::builder::specs::RollRule::None,
    };
    let first_period = build_single_period(BuildPeriodsParams::from_schedule(
        &schedule_params,
        instrument.closing_date,
        instrument.first_payment_date,
        None,
    ))?;
    let remaining_periods = if instrument.first_payment_date < instrument.maturity {
        build_periods(BuildPeriodsParams::from_schedule(
            &schedule_params,
            instrument.first_payment_date,
            instrument.maturity,
            None,
        ))?
    } else {
        // A single contractual coupon has no schedule after its first payment.
        Vec::new()
    };
    let all_periods: Vec<_> = std::iter::once(first_period)
        .chain(remaining_periods)
        .collect();
    let state_anchor = all_periods
        .iter()
        .map(|period| period.payment_date)
        .filter(|date| *date < as_of)
        .max()
        .unwrap_or(instrument.closing_date);
    let periods = all_periods
        .into_iter()
        .filter(|period| period.payment_date >= as_of)
        .collect();

    // Freeze the initial-state computation once; each path clones it.
    let state_template = StateTemplate::new(pool, tranches, instrument.closing_date)?;

    Ok(Some(PreparedDealSimulation {
        valuation_date: as_of,
        periods,
        state_anchor,
        waterfall,
        calendar,
        convention,
        months_per_period,
        state_template,
    }))
}

/// Tranche cashflows plus the deal-level accounting of one simulation run.
#[derive(Debug, Clone)]
pub struct SimulationRun {
    /// Detailed cashflow results per tranche id.
    pub tranches: HashMap<String, TrancheCashflows>,
    /// Reserve, draw-funding and reserve-interest accounting.
    pub diagnostics: SimulationDiagnostics,
}

/// Execute the prepared period loop and return tranche results with the
/// deal-level diagnostics.
pub(crate) fn simulate_prepared<S: PoolFlowSource + ?Sized>(
    instrument: &StructuredCredit,
    context: &MarketContext,
    prepared: &PreparedDealSimulation,
    source: &mut S,
) -> Result<SimulationRun> {
    let pool = &instrument.pool;
    let tranches = &instrument.tranches;
    let as_of = prepared.valuation_date;

    // Balances supplied on the instrument are current-state balances. Anchor
    // the first projected accrual at the last contractual boundary, rather
    // than replaying pool dynamics from closing on those current balances.
    let mut state = SimulationState::from_template(
        &prepared.state_template,
        pool,
        tranches,
        instrument.closing_date,
        prepared.state_anchor,
        instrument.credit_model.recovery_spec.recovery_lag,
    );

    // Simulate period-by-period
    for contractual_period in &prepared.periods {
        let pay_date = contractual_period.payment_date;
        if state.is_pool_exhausted() {
            state.prev_date = Some(state.prev_date.unwrap_or(as_of).max(as_of));
            break;
        }

        // Clean-up call: if pool factor drops below threshold, redeem tranches.
        //
        // INTEX/Bloomberg convention: when pool factor (current / original total
        // balance) drops below the cleanup threshold (typically 10%), the equity
        // holder may exercise an optional redemption. Redemption pays tranches
        // in seniority order (senior first), bounded by the remaining pool value.
        //
        // Cleanup-call redemption settles at full claim: par + accrued stub
        // interest + deferred (PIK) interest (+ optional call premium). Interest
        // is recorded as an interest flow and does not retire notional.
        if let Some(cleanup_threshold) = instrument.cleanup_call_pct {
            let pool_factor = if state.total_pool_balance.amount() > 0.0 {
                state.pool_outstanding.amount() / state.total_pool_balance.amount()
            } else {
                0.0
            };
            if pool_factor < cleanup_threshold && pool_factor > 0.0 {
                // Cleanup redemption realizes the remaining pool and its
                // pending recovery rights. The lag queue therefore contributes
                // cash in addition to current pool outstanding.
                let pending_recoveries = state.recovery_queue.pending_amount(state.base_currency);
                // The cleanup call realizes the pending recoveries immediately
                // (they fund the redemption below); drain the queue so the
                // end-of-simulation drain cannot release them a second time.
                let _ = state.recovery_queue.drain_pending();
                // Any controlled-accumulation funding account holds collected
                // pool principal not yet released as a bullet. At the cleanup call
                // it is real cash available to redeem the notes; fold it into the
                // redemption and zero the account so the post-loop terminal sweep
                // (`release_principal_funding_account`) cannot double-count or
                // mis-allocate it across the (now-redeemed) balances.
                let funding_balance = state.principal_funding_account.amount();
                state.principal_funding_account = Money::from((0_i64, state.base_currency));
                let mut available_for_redemption =
                    state.pool_outstanding.amount() + pending_recoveries.amount() + funding_balance;

                // Stub-period start for accrued-interest calculation: the last
                // payment date (or closing) up to this cleanup-call date.
                let cleanup_period_start = state.prev_date.unwrap_or(state.closing_date);

                // Pay tranches in seniority order (Senior=0 first, Equity=3 last)
                let mut redemption_order: Vec<usize> = (0..state.tranches.tranches.len()).collect();
                redemption_order.sort_by_key(|&i| state.tranches.tranches[i].payment_priority);

                for &idx in &redemption_order {
                    if available_for_redemption <= WRITEDOWN_DE_MINIMIS {
                        break;
                    }
                    let tranche = &state.tranches.tranches[idx];
                    let tranche_id_str = tranche.id.as_str();
                    let balance = state
                        .tranche_balances
                        .get(tranche_id_str)
                        .copied()
                        .unwrap_or(Money::from((0_i64, state.base_currency)));

                    if balance.amount() <= WRITEDOWN_DE_MINIMIS {
                        continue;
                    }

                    // Accrued interest for the stub period on the current
                    // (post-writedown) balance, using the tranche coupon and
                    // its own day-count convention.
                    let coupon_rate =
                        tranche
                            .coupon
                            .try_rate_for_period(cleanup_period_start, as_of, context)?;
                    let accrual_factor = tranche.day_count.year_fraction(
                        cleanup_period_start,
                        pay_date,
                        DayCountContext::default(),
                    )?;
                    let accrued = balance.amount() * coupon_rate * accrual_factor;

                    // Deferred / PIK interest carried forward into this period.
                    let deferred = state
                        .deferred_interest
                        .get(tranche_id_str)
                        .map(|m| m.amount())
                        .unwrap_or(0.0);

                    // Cleanup calls currently redeem at par with no premium.
                    let premium = cleanup_call_premium(instrument, balance.amount());

                    // Full redemption claim, bounded by available cash.
                    let total_claim = balance.amount() + accrued + deferred + premium;
                    let redemption_amt = total_claim.min(available_for_redemption);
                    available_for_redemption -= redemption_amt;

                    // Split the bounded redemption: interest (accrued +
                    // deferred + premium) is satisfied first as the senior
                    // claim within the redemption, the remainder retires
                    // principal. Only the principal portion reduces notional.
                    let interest_claim = accrued + deferred + premium;
                    let interest_paid = redemption_amt.min(interest_claim).max(0.0);
                    let principal_paid = (redemption_amt - interest_paid).max(0.0);

                    let redemption = Money::new(redemption_amt, state.base_currency)?;
                    let interest_money = Money::new(interest_paid, state.base_currency)?;
                    let principal_money = Money::new(principal_paid, state.base_currency)?;

                    if let Some(res) = state.results.get_mut(tranche_id_str) {
                        if tranche.seniority != TrancheSeniority::Equity {
                            res.accrual_periods.push(crate::instruments::fixed_income::structured_credit::TrancheAccrualPeriod {
                                start: contractual_period.accrual_start,
                                end: contractual_period.accrual_end,
                                payment_date: pay_date,
                                opening_balance: balance,
                                coupon_rate,
                                day_count: tranche.day_count,
                            });
                        }
                        res.cashflows.push((pay_date, redemption));
                        if interest_paid > 0.0 {
                            res.interest_flows.push((pay_date, interest_money));
                            res.total_interest = res.total_interest.checked_add(interest_money)?;
                        }
                        if principal_paid > 0.0 {
                            res.principal_flows.push((pay_date, principal_money));
                            res.total_principal =
                                res.total_principal.checked_add(principal_money)?;
                        }
                    }
                    // Deferred interest cured by this redemption is cleared.
                    if let Some(def) = state.deferred_interest.get_mut(tranche_id_str) {
                        let cured = interest_paid.min(deferred).max(0.0);
                        *def = def
                            .checked_sub(Money::new(cured, state.base_currency)?)
                            .unwrap_or(Money::from((0_i64, state.base_currency)));
                    }
                    // Only the principal portion retires notional.
                    if let Some(bal) = state.tranche_balances.get_mut(tranche_id_str) {
                        *bal = bal
                            .checked_sub(principal_money)
                            .unwrap_or(Money::from((0_i64, state.base_currency)));
                    }
                }

                // Residual after full redemption goes to equity / most-junior
                // (cleanup calls normally leave pool value above note claim).
                // Mirrors `drain_pending_recoveries_at_end` and terminal sweeps.
                if available_for_redemption > WRITEDOWN_DE_MINIMIS {
                    let residual_id = state
                        .tranches
                        .tranches
                        .iter()
                        .max_by_key(|t| t.payment_priority)
                        .map(|t| t.id.as_str().to_string());
                    if let Some(id) = residual_id {
                        let residual = Money::new(available_for_redemption, state.base_currency)?;
                        append_residual_principal(&mut state, &id, residual, pay_date)?;
                    }
                }
                break; // Terminate simulation after cleanup call
            }
        }

        simulate_period(
            &mut state,
            instrument,
            &prepared.waterfall,
            SimulationPeriod {
                accrual_start: contractual_period.accrual_start,
                accrual_end: contractual_period.accrual_end,
                payment: pay_date,
                valuation: as_of,
            },
            context,
            prepared.months_per_period,
            source,
        )?;
    }

    // Current terminal accounts settle before later, lagged recoveries.
    // The loss trap can transfer retained interest to outstanding debt par.
    release_spread_account(&mut state, instrument)?;

    // Release any controlled-accumulation funding-account balance that was never
    // paid out as a bullet — e.g. the deal hit a cleanup call or its scheduled
    // end before the bullet date. Prevents the accumulated principal from being
    // silently stranded.
    release_principal_funding_account(&mut state)?;

    // Release any reserve-account balance that survived to deal end. The
    // reserve is credit enhancement, drawn during the deal to cover interest
    // shortfalls; whatever remains belongs to the transaction. Without this the
    // account was a pure sink and every deposited dollar was destroyed.
    release_reserve_account(&mut state)?;

    // Each remaining recovery claim settles at its contractual lagged date,
    // after current terminal distributions have updated outstanding note par.
    drain_pending_recoveries_at_end(&mut state, prepared.calendar, prepared.convention)?;

    let (tranches, diagnostics) = state.finalize_with_diagnostics();
    Ok(SimulationRun {
        tranches,
        diagnostics,
    })
}

/// Run full cashflow simulation for a structured credit instrument.
///
/// Prepares the loop-invariant [`PreparedDealSimulation`] then executes the
/// period loop. Single-shot callers (deterministic pricing, OAS scenarios)
/// use this entry point; the stochastic engines prepare once per pricing run
/// and call [`simulate_prepared`] per path.
///
/// Returns detailed cashflow results for each tranche.
pub(crate) fn run_simulation_with_source<S: PoolFlowSource + ?Sized>(
    instrument: &StructuredCredit,
    context: &MarketContext,
    as_of: Date,
    source: &mut S,
) -> Result<HashMap<String, TrancheCashflows>> {
    simulate_with_source(instrument, context, as_of, source).map(|run| run.tranches)
}

/// Resolve, prepare and simulate a deal whose pool holds instrument
/// collateral, driving period flows from the instruments' own schedules.
///
/// # Arguments
///
/// * `instrument` - Deal whose `pool.instruments` is populated.
/// * `context` - Market context used to project every instrument schedule.
/// * `as_of` - Valuation date.
pub(crate) fn simulate_instrument_pool(
    instrument: &StructuredCredit,
    context: &MarketContext,
    as_of: Date,
) -> Result<SimulationRun> {
    let resolved = instrument.resolved_for_pricing()?;
    let instrument = &resolved;
    let currency = instrument.pool.get_base_currency();
    match prepare_deal_simulation(instrument, as_of)? {
        Some(prepared) => {
            let mut source = super::instrument_flows::InstrumentScheduleFlowSource::prepare(
                instrument, context, &prepared,
            )?;
            simulate_prepared(instrument, context, &prepared, &mut source)
        }
        None => Ok(SimulationRun {
            tranches: HashMap::default(),
            diagnostics: SimulationDiagnostics {
                reserve_balance_path: Vec::new(),
                reserve_interest_paid: Vec::new(),
                draws_from_reserve: Money::from((0_i64, currency)),
                draws_from_principal: Money::from((0_i64, currency)),
                unfunded_draws: Money::from((0_i64, currency)),
                reserve_replenished: Money::from((0_i64, currency)),
            },
        }),
    }
}

/// Resolve, prepare and simulate a deal, returning tranche results with the
/// deal-level diagnostics.
pub(crate) fn simulate_with_source<S: PoolFlowSource + ?Sized>(
    instrument: &StructuredCredit,
    context: &MarketContext,
    as_of: Date,
    source: &mut S,
) -> Result<SimulationRun> {
    let resolved = instrument.resolved_for_pricing()?;
    let instrument = &resolved;
    let currency = instrument.pool.get_base_currency();
    match prepare_deal_simulation(instrument, as_of)? {
        Some(prepared) => simulate_prepared(instrument, context, &prepared, source),
        // Exhausted pool: nothing to simulate, empty result.
        None => Ok(SimulationRun {
            tranches: HashMap::default(),
            diagnostics: SimulationDiagnostics {
                reserve_balance_path: Vec::new(),
                reserve_interest_paid: Vec::new(),
                draws_from_reserve: Money::from((0_i64, currency)),
                draws_from_principal: Money::from((0_i64, currency)),
                unfunded_draws: Money::from((0_i64, currency)),
                reserve_replenished: Money::from((0_i64, currency)),
            },
        }),
    }
}

/// Pay each outstanding recovery claim once, after its full contractual lag.
fn drain_pending_recoveries_at_end(
    state: &mut SimulationState,
    calendar: &dyn HolidayCalendar,
    convention: BusinessDayConvention,
) -> Result<()> {
    let mut pending = state.recovery_queue.drain_pending();
    pending.sort_by_key(|(date, _)| *date);
    let last_date = state.prev_date.unwrap_or(state.closing_date);
    let lag_months = i32::try_from(state.recovery_lag_months).map_err(|_| {
        finstack_quant_core::Error::Validation(
            "recovery lag exceeds supported calendar range".into(),
        )
    })?;
    for (default_date, amount) in pending {
        let release_date = adjust(
            default_date.add_months(lag_months).max(last_date),
            convention,
            calendar,
        )?;
        distribute_terminal_principal(state, amount, release_date)?;
    }
    Ok(())
}

/// Aggregate tranche-level cashflows into one dated cashflow vector.
pub(crate) fn aggregate_tranche_cashflows(
    full_results: &HashMap<String, TrancheCashflows>,
) -> Result<DatedFlows> {
    // Aggregate all tranche cashflows into a single schedule
    let estimated_dates = full_results
        .values()
        .next()
        .map(|r| r.cashflows.len())
        .unwrap_or(0);
    let mut flow_map: HashMap<Date, Money> = {
        let mut m = HashMap::default();
        m.reserve(estimated_dates);
        m
    };

    for result in full_results.values() {
        for (date, amount) in &result.cashflows {
            if let Some(existing) = flow_map.get_mut(date) {
                *existing = existing.checked_add(*amount)?;
            } else {
                flow_map.insert(*date, *amount);
            }
        }
    }

    let mut all_flows: DatedFlows = flow_map.into_iter().collect();
    all_flows.sort_by_key(|(d, _)| *d);

    Ok(all_flows)
}

/// Remove one tranche's cashflows from a full simulation result map.
pub(crate) fn take_tranche_cashflows(
    full_results: &mut HashMap<String, TrancheCashflows>,
    tranche_id: &str,
) -> Result<TrancheCashflows> {
    full_results.remove(tranche_id).ok_or_else(|| {
        finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
            id: format!("tranche:{}", tranche_id),
        })
    })
}
