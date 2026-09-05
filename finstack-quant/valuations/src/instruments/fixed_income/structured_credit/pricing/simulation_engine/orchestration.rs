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

/// If the deal configures `excess_spread` and the account holds a positive
/// balance after the final period, that balance is distributed to the equity
/// tranche as a residual flow — unless a cumulative-loss trap trigger
/// (`trap_loss_pct`) is breached, in which case the balance is retained in the
/// deal (consumed enhancement) and permanently reduces equity. No-op when no
/// `excess_spread` rule is configured (identity).
///
/// The release is booked via [`append_residual_principal`] (i.e. as a principal
/// flow), matching the engine-wide convention that equity/residual distributions
/// are recorded as principal: the equity tranche carries no coupon, so its Step-5
/// interest claim is zero and every residual it receives is already classified as
/// principal. Routing the spread release the same way keeps equity's
/// interest/principal split internally consistent; the choice is PV-neutral
/// (NPV, yield and IRR use the total cashflow, not the interest/principal split).
fn release_spread_account(
    state: &mut SimulationState<'_>,
    instrument: &StructuredCredit,
) -> Result<()> {
    let Some(es) = instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.excess_spread.as_ref())
    else {
        return Ok(());
    };

    let balance = state.spread_account;
    if balance.amount() <= 0.0 {
        return Ok(());
    }

    // Retain (do not release) if the cumulative-loss trap trigger is breached.
    if let Some(trap) = es.trap_loss_pct {
        let denom = state.total_pool_balance.amount();
        let loss_fraction = if denom > 0.0 {
            state.cumulative_realized_loss / denom
        } else {
            0.0
        };
        if loss_fraction >= trap {
            state.spread_account = Money::from((0_i64, state.base_currency));
            return Ok(());
        }
    }

    // Release to the residual holder as a residual principal flow.
    //
    // N7: the release must not require a tranche with
    // `TrancheSeniority::Equity` — silently zeroing the account when none
    // exists destroys cash (the same sink class as the reserve-account defect
    // SC-C07), and a deal whose most junior class is Subordinated rather than
    // Equity is a perfectly ordinary structure.
    //
    // Falls back to the most-junior tranche by payment priority, matching
    // `release_reserve_account` and `release_principal_funding_account` so all
    // three terminal sweeps behave identically and none can strand cash.
    let residual_id = state
        .tranches
        .tranches
        .iter()
        .find(|t| t.seniority == TrancheSeniority::Equity)
        .or_else(|| {
            state
                .tranches
                .tranches
                .iter()
                .max_by_key(|t| t.payment_priority)
        })
        .map(|t| t.id.as_str().to_string());
    if let Some(id) = residual_id {
        let release_date = state.prev_date.unwrap_or(state.closing_date);
        append_residual_principal(state, &id, balance, release_date)?;
    }
    state.spread_account = Money::from((0_i64, state.base_currency));
    Ok(())
}

/// Release any residual reserve-account balance at deal end, paying it down the
/// capital structure senior-first with the remainder to the residual holder.
///
/// A reserve account is credit enhancement funded at closing and topped up by
/// `ReserveAccount` waterfall recipients. It is drawn during the deal to cover
/// debt-interest shortfalls (see the draw in `simulate_period`); whatever
/// survives to deal end belongs to the transaction, not to the void.
///
/// Without this sweep the account was a pure sink: the only mutation anywhere
/// was the `checked_add` booking deposits, so every deposited dollar was
/// destroyed and lifetime distributions were understated by the full balance.
///
/// Pays outstanding notes most-senior-first (a released reserve is available to
/// redeem the notes), then routes any excess over the remaining capital
/// structure to the most-junior / equity tranche — mirroring
/// [`release_principal_funding_account`], so the two terminal sweeps behave
/// identically and neither can strand cash.
fn release_reserve_account(state: &mut SimulationState<'_>) -> Result<()> {
    let mut remaining = state.reserve_balance.amount();
    if remaining <= 0.0 {
        return Ok(());
    }

    let release_date = state.prev_date.unwrap_or(state.closing_date);
    let mut order: Vec<usize> = (0..state.tranches.tranches.len()).collect();
    order.sort_by_key(|&i| state.tranches.tranches[i].payment_priority);

    for i in order {
        if remaining <= 0.0 {
            break;
        }
        let id = state.tranches.tranches[i].id.as_str().to_string();
        let balance = state.tranche_balances.get(&id).map_or(0.0, |m| m.amount());
        if balance <= 0.0 {
            continue;
        }
        let pay = remaining.min(balance);
        state
            .tranche_balances
            .insert(id.clone(), Money::new(balance - pay, state.base_currency)?);
        append_residual_principal(
            state,
            &id,
            Money::new(pay, state.base_currency)?,
            release_date,
        )?;
        remaining -= pay;
    }

    if remaining > WRITEDOWN_DE_MINIMIS {
        let residual_id = state
            .tranches
            .tranches
            .iter()
            .max_by_key(|t| t.payment_priority)
            .map(|t| t.id.as_str().to_string());
        if let Some(id) = residual_id {
            append_residual_principal(
                state,
                &id,
                Money::new(remaining, state.base_currency)?,
                release_date,
            )?;
        }
    }
    state.reserve_balance = Money::from((0_i64, state.base_currency));
    Ok(())
}

/// Release any residual controlled-accumulation funding-account balance at deal
/// end, paying it down the capital structure senior-first.
///
/// During the accumulation period collected pool principal is held in the
/// funding account and normally released as a bullet at the bullet date. If the
/// deal terminates before then (cleanup call, pool exhaustion, or a bullet date
/// beyond the last payment), the in-period bullet never fires; this terminal
/// sweep distributes the residual to the outstanding tranches (most-senior
/// first), so the accumulated principal is never stranded. A no-op when no
/// accumulation rule is configured or the account is already empty (the normal
/// bullet-release path).
fn release_principal_funding_account(
    state: &mut SimulationState<'_>,
    instrument: &StructuredCredit,
) -> Result<()> {
    if instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.controlled_accumulation.as_ref())
        .is_none()
    {
        return Ok(());
    }
    let mut remaining = state.principal_funding_account.amount();
    if remaining <= 0.0 {
        return Ok(());
    }

    let release_date = state.prev_date.unwrap_or(state.closing_date);
    // Outstanding tranches, most-senior first by payment priority.
    let mut order: Vec<usize> = (0..state.tranches.tranches.len()).collect();
    order.sort_by_key(|&i| state.tranches.tranches[i].payment_priority);

    for i in order {
        if remaining <= 0.0 {
            break;
        }
        let id = state.tranches.tranches[i].id.as_str().to_string();
        let balance = state.tranche_balances.get(&id).map_or(0.0, |m| m.amount());
        if balance <= 0.0 {
            continue;
        }
        let pay = remaining.min(balance);
        state
            .tranche_balances
            .insert(id.clone(), Money::new(balance - pay, state.base_currency)?);
        append_residual_principal(
            state,
            &id,
            Money::new(pay, state.base_currency)?,
            release_date,
        )?;
        remaining -= pay;
    }
    // Any balance beyond the outstanding capital structure (accumulated cash
    // exceeding every remaining note balance) is residual value to the
    // most-junior / equity tranche — routed there rather than silently destroyed
    // by zeroing the account, which would break cash conservation.
    if remaining > WRITEDOWN_DE_MINIMIS {
        let residual_id = state
            .tranches
            .tranches
            .iter()
            .max_by_key(|t| t.payment_priority)
            .map(|t| t.id.as_str().to_string());
        if let Some(id) = residual_id {
            append_residual_principal(
                state,
                &id,
                Money::new(remaining, state.base_currency)?,
                release_date,
            )?;
        }
    }
    state.principal_funding_account = Money::from((0_i64, state.base_currency));
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
    pub(crate) schedule_dates: Vec<Date>,
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

    if pool.total_balance()?.amount() <= 0.0 {
        return Ok(None);
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
    let remaining_periods = build_periods(BuildPeriodsParams::from_schedule(
        &schedule_params,
        instrument.first_payment_date,
        instrument.maturity,
        None,
    ))?;
    let payment_dates: Vec<Date> = std::iter::once(first_period.payment_date)
        .chain(
            remaining_periods
                .into_iter()
                .map(|period| period.payment_date),
        )
        .collect();
    let state_anchor = payment_dates
        .iter()
        .copied()
        .filter(|date| *date < as_of)
        .max()
        .unwrap_or(instrument.closing_date);
    let schedule_dates: Vec<Date> = payment_dates
        .into_iter()
        .filter(|date| *date >= as_of)
        .collect();

    // Freeze the initial-state computation once; each path clones it.
    let state_template = StateTemplate::new(pool, tranches, instrument.closing_date)?;

    Ok(Some(PreparedDealSimulation {
        valuation_date: as_of,
        schedule_dates,
        state_anchor,
        waterfall,
        calendar,
        convention,
        months_per_period,
        state_template,
    }))
}

/// Run the period-by-period waterfall against one path's flow source, using a
/// prebuilt [`PreparedDealSimulation`]. Called once per scenario path by the
/// stochastic engines; the deterministic paths go through
/// [`run_simulation_with_source`], which prepares then delegates here.
///
/// Returns detailed cashflow results for each tranche.
pub(crate) fn run_prepared_simulation_with_source<S: PoolFlowSource + ?Sized>(
    instrument: &StructuredCredit,
    context: &MarketContext,
    prepared: &PreparedDealSimulation,
    source: &mut S,
) -> Result<HashMap<String, TrancheCashflows>> {
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
    for pay_date in &prepared.schedule_dates {
        let pay_date = *pay_date;
        if state.is_pool_exhausted() {
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
                redemption_order.sort_by_key(|&i| state.tranches.tranches[i].seniority);

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
                payment: pay_date,
                valuation: as_of,
            },
            context,
            prepared.months_per_period,
            source,
        )?;
    }

    // Drain lagged recoveries still pending at simulation end. When the
    // simulation terminates via pool exhaustion or the final scheduled date,
    // recoveries from defaults within `recovery_lag` months of the end have
    // not yet matured out of the queue; without this drain that recovery cash
    // is silently dropped and losses are overstated for any deal with
    // defaults near maturity. Mirrors the cleanup-call branch, which already
    // realizes the pending queue when it terminates the deal. Each entry is
    // released at the later of the final simulated date and its own
    // default-date + recovery lag, business-day adjusted.
    drain_pending_recoveries_at_end(&mut state, prepared.calendar, prepared.convention)?;

    // Release any unused spread-account balance to equity at deal end (unless a
    // cumulative-loss trap trigger retains it as consumed enhancement).
    release_spread_account(&mut state, instrument)?;

    // Release any controlled-accumulation funding-account balance that was never
    // paid out as a bullet — e.g. the deal hit a cleanup call or its scheduled
    // end before the bullet date. Prevents the accumulated principal from being
    // silently stranded.
    release_principal_funding_account(&mut state, instrument)?;

    // Release any reserve-account balance that survived to deal end. The
    // reserve is credit enhancement, drawn during the deal to cover interest
    // shortfalls; whatever remains belongs to the transaction. Without this the
    // account was a pure sink and every deposited dollar was destroyed.
    release_reserve_account(&mut state)?;

    Ok(state.finalize())
}

/// Run full cashflow simulation for a structured credit instrument.
///
/// Prepares the loop-invariant [`PreparedDealSimulation`] then executes the
/// period loop. Single-shot callers (deterministic pricing, OAS scenarios)
/// use this entry point; the stochastic engines prepare once per pricing run
/// and call [`run_prepared_simulation_with_source`] per path.
///
/// Returns detailed cashflow results for each tranche.
pub(crate) fn run_simulation_with_source<S: PoolFlowSource + ?Sized>(
    instrument: &StructuredCredit,
    context: &MarketContext,
    as_of: Date,
    source: &mut S,
) -> Result<HashMap<String, TrancheCashflows>> {
    match prepare_deal_simulation(instrument, as_of)? {
        Some(prepared) => {
            run_prepared_simulation_with_source(instrument, context, &prepared, source)
        }
        // Exhausted pool: nothing to simulate, empty result.
        None => Ok(HashMap::default()),
    }
}

/// Drain and distribute recoveries still pending in the lag queue when the
/// simulation terminates (pool exhaustion or final scheduled payment date).
///
/// Each pending entry is released on the later of the last simulated payment
/// date and its natural maturity (`default_date + recovery_lag`), adjusted to
/// a business day, and distributed through the same terminal path the
/// cleanup-call branch uses: tranches in seniority order, each tranche's
/// claim being its deferred interest (senior portion) plus its remaining
/// principal balance. No new coupon accrues after the final simulated date,
/// so unlike the mid-life cleanup call there is no stub accrued-interest leg.
fn drain_pending_recoveries_at_end(
    state: &mut SimulationState,
    calendar: &dyn HolidayCalendar,
    convention: BusinessDayConvention,
) -> Result<()> {
    let pending = state.recovery_queue.drain_pending();
    if pending.is_empty() {
        return Ok(());
    }

    let last_date = state.prev_date.unwrap_or(state.closing_date);
    let lag_months = i32::try_from(state.recovery_lag_months).unwrap_or(i32::MAX);

    // Seniority order: Senior=0 first, Equity=3 last (same as cleanup call).
    let mut order: Vec<usize> = (0..state.tranches.tranches.len()).collect();
    order.sort_by_key(|&i| state.tranches.tranches[i].seniority);

    for (default_date, amount) in pending {
        let natural_release = default_date.add_months(lag_months);
        let release_date = adjust(natural_release.max(last_date), convention, calendar)?;

        let mut available = amount.amount();
        for &idx in &order {
            if available <= WRITEDOWN_DE_MINIMIS {
                break;
            }
            let tranche_id_str = state.tranches.tranches[idx].id.as_str();
            let balance = state
                .tranche_balances
                .get(tranche_id_str)
                .map(|m| m.amount())
                .unwrap_or(0.0);
            let deferred = state
                .deferred_interest
                .get(tranche_id_str)
                .map(|m| m.amount())
                .unwrap_or(0.0);
            let claim = balance + deferred;
            if claim <= WRITEDOWN_DE_MINIMIS {
                continue;
            }

            let paid = claim.min(available);
            available -= paid;

            // Deferred interest is the senior claim within the distribution;
            // the remainder retires principal (same split as the cleanup
            // call's terminal redemption).
            let interest_paid = paid.min(deferred).max(0.0);
            let principal_paid = (paid - interest_paid).max(0.0);

            let payment = Money::new(paid, state.base_currency)?;
            let interest_money = Money::new(interest_paid, state.base_currency)?;
            let principal_money = Money::new(principal_paid, state.base_currency)?;

            if let Some(res) = state.results.get_mut(tranche_id_str) {
                res.cashflows.push((release_date, payment));
                if interest_paid > 0.0 {
                    res.interest_flows.push((release_date, interest_money));
                    res.total_interest = res.total_interest.checked_add(interest_money)?;
                }
                if principal_paid > 0.0 {
                    res.principal_flows.push((release_date, principal_money));
                    res.total_principal = res.total_principal.checked_add(principal_money)?;
                }
            }
            if interest_paid > 0.0 {
                if let Some(def) = state.deferred_interest.get_mut(tranche_id_str) {
                    *def = def
                        .checked_sub(interest_money)
                        .unwrap_or(Money::from((0_i64, state.base_currency)));
                }
            }
            if principal_paid > 0.0 {
                if let Some(bal) = state.tranche_balances.get_mut(tranche_id_str) {
                    *bal = bal
                        .checked_sub(principal_money)
                        .unwrap_or(Money::from((0_i64, state.base_currency)));
                }
            }
        }

        // Any remainder beyond all note claims is the equity holder's
        // residual — book it to the equity tranche (mirroring the standard
        // waterfall's Equity residual recipient and step-5's booking of
        // residual cash as a principal flow), so recovery cash is never
        // silently dropped.
        if available > WRITEDOWN_DE_MINIMIS {
            let equity_idx = order
                .iter()
                .rev()
                .copied()
                .find(|&i| state.tranches.tranches[i].seniority == TrancheSeniority::Equity);
            if let Some(idx) = equity_idx {
                let tranche_id_str = state.tranches.tranches[idx].id.as_str();
                let residual = Money::new(available, state.base_currency)?;
                if let Some(res) = state.results.get_mut(tranche_id_str) {
                    res.cashflows.push((release_date, residual));
                    res.principal_flows.push((release_date, residual));
                    res.total_principal = res.total_principal.checked_add(residual)?;
                }
            }
        }
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
