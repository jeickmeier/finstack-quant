use super::*;

/// Release the unused spread-account balance to equity at deal end.
///
/// Record a residual principal distribution to a tranche at deal end: appends
/// the flow to the tranche's cashflows and principal flows and bumps its
/// `total_principal`. No-op if the tranche has no results entry. Shared by the
/// excess-spread and controlled-accumulation terminal sweeps and the call
/// residual. Cash reaching an equity tranche first pays the manager's
/// incentive share of the part above the equity hurdle, booked as a fee of
/// the last simulated period.
fn append_residual_principal(
    state: &mut SimulationState<'_>,
    tranche_id: &str,
    amount: Money,
    date: Date,
) -> Result<()> {
    let is_equity = state.tranches.tranches.iter().any(|tranche| {
        tranche.id.as_str() == tranche_id && tranche.seniority == TrancheSeniority::Equity
    });
    let amount = match (is_equity, state.incentive_fee) {
        (true, Some(spec)) => {
            let shortfall = state
                .equity_history()?
                .hurdle_shortfall(date, amount, spec.hurdle_irr)
                .amount();
            let fee = (amount.amount() - shortfall).max(0.0) * spec.share_pct;
            if fee > 0.0 {
                let fee = Money::new(fee, state.base_currency)?;
                if let Some(last) = state.period_diagnostics.last_mut() {
                    last.fees_paid = last.fees_paid.checked_add(fee)?;
                }
                amount.checked_sub(fee)?
            } else {
                amount
            }
        }
        _ => amount,
    };
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
            state.original_pool_balance.amount() > 0.0
                && state.cumulative_realized_loss / state.original_pool_balance.amount()
                    >= threshold
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
    let date = state.prev_date.unwrap_or(state.closing_date);
    if state.reserve_covers_principal_at_final {
        distribute_terminal_principal(state, state.reserve_balance, date)?;
    } else if state.reserve_balance.amount() > 0.0 {
        // The rules release the reserve to the residual holder instead of
        // the notes.
        let residual_id = state
            .tranches
            .tranches
            .iter()
            .max_by_key(|t| t.payment_priority)
            .map(|t| t.id.as_str().to_string());
        if let Some(id) = residual_id {
            append_residual_principal(state, &id, state.reserve_balance, date)?;
        }
    }
    state.reserve_balance = Money::from((0_i64, state.base_currency));
    Ok(())
}

/// Realize every note's unpaid principal as a write-down at legal final.
///
/// After the last period and the terminal sweeps nothing further can reach
/// the notes, so any balance still outstanding is a realized loss: under
/// `ParPreserving` this is where collateral losses finally reach the notes;
/// under `WriteDown` it catches the residual timing difference between the
/// at-default allocation and the cash actually collected. Recorded on the
/// final settlement date, junior first, so `total_principal + total_writedown`
/// reconciles to the starting balance of every note.
fn realize_principal_shortfall(state: &mut SimulationState<'_>, date: Date) -> Result<()> {
    let mut order: Vec<_> = (0..state.tranches.tranches.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(state.tranches.tranches[i].payment_priority));
    for i in order {
        let id = state.tranches.tranches[i].id.to_string();
        let balance = state.tranche_balances[&id].amount();
        if balance <= WRITEDOWN_DE_MINIMIS {
            continue;
        }
        let shortfall = Money::new(balance, state.base_currency)?;
        if let Some(res) = state.results.get_mut(&id) {
            res.writedown_flows.push((date, shortfall));
            res.total_writedown = res.total_writedown.checked_add(shortfall)?;
        }
        state
            .tranche_balances
            .insert(id, Money::from((0_i64, state.base_currency)));
    }
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

/// Value of the remaining collateral when the deal is called or cleaned up:
/// performing par at `liquidation_fraction` of par, unresolved NPL rows at
/// their net liquidation proceeds and delinquent balances at `recovery_rate`.
fn liquidation_value(
    state: &SimulationState<'_>,
    liquidation_fraction: f64,
    recovery_rate: f64,
) -> f64 {
    let pool = &state.pool_state;
    (0..pool.len())
        .map(|i| {
            let balance = pool.balances[i].max(0.0);
            if balance <= 0.0 || pool.is_defaulted[i] {
                return 0.0;
            }
            if let Some(spec) = pool.liquidation[i] {
                return balance * spec.net_proceeds_fraction();
            }
            let delinquent =
                super::delinquency::delinquent_balance(&pool.delinquent[i]).min(balance);
            (balance - delinquent) * liquidation_fraction + delinquent * recovery_rate
        })
        .sum()
}

/// Redeem every note in seniority order from `proceeds` at
/// `redemption_price_pct` (equity at par) plus deferred interest, then pay
/// the surplus to the most junior class. Consumes the pending recoveries and
/// every cash account so the terminal sweeps cannot pay them twice.
fn redeem(
    state: &mut SimulationState<'_>,
    proceeds: f64,
    redemption_price_pct: f64,
    pay_date: Date,
) -> Result<()> {
    let zero = Money::from((0_i64, state.base_currency));
    let _ = state.recovery_queue.drain_pending();
    state.principal_funding_account = zero;
    state.reserve_balance = zero;
    state.spread_account = zero;
    state.undistributed_interest = zero;
    state.undistributed_principal = zero;
    let mut available = proceeds;

    let mut redemption_order: Vec<usize> = (0..state.tranches.tranches.len()).collect();
    redemption_order.sort_by_key(|&i| state.tranches.tranches[i].payment_priority);

    for &idx in &redemption_order {
        if available <= WRITEDOWN_DE_MINIMIS {
            break;
        }
        let tranche = &state.tranches.tranches[idx];
        let tranche_id_str = tranche.id.as_str();
        let balance_amt = state
            .tranche_balances
            .get(tranche_id_str)
            .map_or(0.0, |m| m.amount());
        if balance_amt <= WRITEDOWN_DE_MINIMIS {
            continue;
        }
        let deferred = state
            .deferred_interest
            .get(tranche_id_str)
            .map_or(0.0, |m| m.amount());

        // Equity is the residual holder: it is paid its balance at par from
        // whatever is left, then the surplus below.
        let price = if tranche.seniority == TrancheSeniority::Equity {
            100.0
        } else {
            redemption_price_pct
        };
        let premium = balance_amt * (price / 100.0 - 1.0);
        let principal_claim = balance_amt + premium.min(0.0);
        let interest_claim = deferred + premium.max(0.0);
        let redemption_amt = (principal_claim + interest_claim).min(available);
        available -= redemption_amt;

        // Interest (deferred + premium) is the senior claim within the
        // redemption; the remainder retires principal.
        let interest_paid = redemption_amt.min(interest_claim).max(0.0);
        let principal_paid = (redemption_amt - interest_paid).max(0.0);
        let haircut = (balance_amt - principal_claim).max(0.0);

        if let Some(res) = state.results.get_mut(tranche_id_str) {
            res.cashflows
                .push((pay_date, Money::new(redemption_amt, state.base_currency)?));
            if interest_paid > 0.0 {
                let interest_money = Money::new(interest_paid, state.base_currency)?;
                res.interest_flows.push((pay_date, interest_money));
                res.total_interest = res.total_interest.checked_add(interest_money)?;
            }
            if principal_paid > 0.0 {
                let principal_money = Money::new(principal_paid, state.base_currency)?;
                res.principal_flows.push((pay_date, principal_money));
                res.total_principal = res.total_principal.checked_add(principal_money)?;
            }
            if haircut > WRITEDOWN_DE_MINIMIS {
                let haircut_money = Money::new(haircut, state.base_currency)?;
                res.writedown_flows.push((pay_date, haircut_money));
                res.total_writedown = res.total_writedown.checked_add(haircut_money)?;
            }
        }
        // Deferred interest cured by this redemption is cleared.
        if let Some(def) = state.deferred_interest.get_mut(tranche_id_str) {
            let cured = interest_paid.min(deferred).max(0.0);
            *def = def
                .checked_sub(Money::new(cured, state.base_currency)?)
                .unwrap_or(zero);
        }
        // Principal paid and any call discount retire notional.
        if let Some(bal) = state.tranche_balances.get_mut(tranche_id_str) {
            *bal = bal
                .checked_sub(Money::new(principal_paid + haircut, state.base_currency)?)
                .unwrap_or(zero);
        }
    }

    // Residual after full redemption goes to equity / most-junior (calls
    // normally leave pool value above the note claim).
    if available > WRITEDOWN_DE_MINIMIS {
        let residual_id = state
            .tranches
            .tranches
            .iter()
            .max_by_key(|t| t.payment_priority)
            .map(|t| t.id.as_str().to_string());
        if let Some(id) = residual_id {
            let residual = Money::new(available, state.base_currency)?;
            append_residual_principal(state, &id, residual, pay_date)?;
        }
    }
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
        rules.validate(tranches, instrument.pool.get_base_currency())?;
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
        if waterfall.coverage_tests().any(|test| {
            test.tranche_id == tranche.id.as_str()
                && match test.kind {
                    CoverageTestType::Oc => tranche.oc_trigger.is_some(),
                    CoverageTestType::Ic => tranche.ic_trigger.is_some(),
                    CoverageTestType::BorrowingBase => false,
                }
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
    state.incentive_fee = instrument.fees.as_ref().and_then(|fees| fees.incentive_fee);
    state.loss_recognition = instrument.effective_loss_recognition();
    state.reserve_covers_principal_at_final = instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.reserve.as_ref())
        .is_none_or(|spec| spec.covers_principal_at_final);

    // Hedge swaps depend on the market context, not on the path.
    state.hedge_schedules = super::hedges::hedge_schedules(instrument, context, as_of)?;

    // Simulate period-by-period
    for contractual_period in &prepared.periods {
        let pay_date = contractual_period.payment_date;
        if state.is_pool_exhausted() {
            state.prev_date = Some(state.prev_date.unwrap_or(as_of).max(as_of));
            break;
        }

        // Optional redemption: an assumed deal call on its date, or the
        // clean-up call once the pool factor (current / ORIGINAL balance at
        // period start) drops below the threshold. The redemption period is
        // first simulated like any other: the collateral's stub interest and
        // principal are collected, fees and note coupons are paid through the
        // waterfall, reinvestment is suspended and the equity residual is
        // withheld. The remaining collateral is then realized at
        // `liquidation_price_pct` (NPL rows at their net proceeds, delinquent
        // balances at the recovery rate) together with pending recoveries and
        // every cash account, and the notes are redeemed in seniority order at
        // the redemption price plus deferred interest. The clean-up call is
        // optional: it is only exercised when those proceeds cover the debt
        // notes' claims; a call assumption redeems regardless, bounded by the
        // proceeds. A premium over par is interest, a discount is a principal
        // write-down. Only principal retires notional.
        let deal_call = instrument
            .call_assumption
            .as_ref()
            .filter(|call| matches!(call.scope, CallScope::Deal))
            .filter(|call| {
                // The scheduled date, or the term-out clock from an
                // early-amortization event when that comes first.
                let after_event = call
                    .after_early_amortization_months
                    .zip(state.early_amortization_date)
                    .is_some_and(|(months, event)| {
                        pay_date >= event.add_months(i32::try_from(months).unwrap_or(i32::MAX))
                    });
                pay_date >= call.date || after_event
            });
        let cleanup = instrument.cleanup_call_pct.is_some_and(|threshold| {
            let pool_factor = if state.original_pool_balance.amount() > 0.0 {
                state.pool_outstanding.amount() / state.original_pool_balance.amount()
            } else {
                0.0
            };
            pool_factor < threshold && pool_factor > 0.0
        });
        let redemption_due = deal_call.is_some() || cleanup;

        simulate_period(
            &mut state,
            instrument,
            &prepared.waterfall,
            SimulationPeriod {
                accrual_start: contractual_period.accrual_start,
                accrual_end: contractual_period.accrual_end,
                payment: pay_date,
                valuation: as_of,
                redemption: redemption_due,
            },
            context,
            prepared.months_per_period,
            source,
        )?;

        if redemption_due {
            let redemption_price_pct = deal_call.map_or(100.0, |call| call.price_pct);
            let seasoning_months =
                state.pool_wala_months + state.closing_date.months_until(pay_date);
            let proceeds = liquidation_value(
                &state,
                instrument.liquidation_price_pct.unwrap_or(100.0) / 100.0,
                instrument
                    .credit_model
                    .recovery_spec
                    .recovery_rate(seasoning_months),
            ) + state
                .recovery_queue
                .pending_amount(state.base_currency)
                .amount()
                + state.principal_funding_account.amount()
                + state.reserve_balance.amount()
                + state.spread_account.amount()
                + state.undistributed_interest.amount()
                + state.undistributed_principal.amount();

            // The debt notes' claims at the redemption price: the coupon for
            // the redemption period was paid by the waterfall above, so only
            // principal and any deferred interest remain.
            let claims: f64 = state
                .tranches
                .tranches
                .iter()
                .filter(|tranche| tranche.seniority != TrancheSeniority::Equity)
                .map(|tranche| {
                    let balance = state
                        .tranche_balances
                        .get(tranche.id.as_str())
                        .map_or(0.0, |m| m.amount());
                    let deferred = state
                        .deferred_interest
                        .get(tranche.id.as_str())
                        .map_or(0.0, |m| m.amount());
                    balance * redemption_price_pct / 100.0 + deferred
                })
                .sum();

            if deal_call.is_some() || proceeds + WRITEDOWN_DE_MINIMIS >= claims {
                redeem(&mut state, proceeds, redemption_price_pct, pay_date)?;
                break; // Terminate simulation after the redemption
            }
        }
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
    let final_date =
        drain_pending_recoveries_at_end(&mut state, prepared.calendar, prepared.convention)?;

    // Whatever principal the notes never received is a realized loss on the
    // final settlement date (see `realize_principal_shortfall`).
    realize_principal_shortfall(&mut state, final_date)?;

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
                periods: Vec::new(),
                reserve_balance_path: Vec::new(),
                reserve_interest_paid: Vec::new(),
                draws_from_reserve: Money::from((0_i64, currency)),
                draws_from_principal: Money::from((0_i64, currency)),
                unfunded_draws: Money::from((0_i64, currency)),
                reserve_replenished: Money::from((0_i64, currency)),
                early_amortization_date: None,
                tranche_draws: Vec::new(),
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
                periods: Vec::new(),
                reserve_balance_path: Vec::new(),
                reserve_interest_paid: Vec::new(),
                draws_from_reserve: Money::from((0_i64, currency)),
                draws_from_principal: Money::from((0_i64, currency)),
                unfunded_draws: Money::from((0_i64, currency)),
                reserve_replenished: Money::from((0_i64, currency)),
                early_amortization_date: None,
                tranche_draws: Vec::new(),
            },
        }),
    }
}

/// Pay each outstanding recovery claim once, after its full contractual lag.
///
/// Returns the final settlement date of the simulation: the last recovery
/// release date, or the last payment date when no claim was pending.
fn drain_pending_recoveries_at_end(
    state: &mut SimulationState,
    calendar: &dyn HolidayCalendar,
    convention: BusinessDayConvention,
) -> Result<Date> {
    let mut pending = state.recovery_queue.drain_pending();
    pending.sort_by_key(|(date, _, _)| *date);
    let last_date = state.prev_date.unwrap_or(state.closing_date);
    let lag_months = i32::try_from(state.recovery_lag_months).map_err(|_| {
        finstack_quant_core::Error::Validation(
            "recovery lag exceeds supported calendar range".into(),
        )
    })?;
    let mut final_date = last_date;
    for (default_date, amount, par) in pending {
        // Claims settling after the last period book their loss now under
        // at-liquidation recognition.
        if state.loss_recognition == LossRecognition::AtLiquidation {
            state.cumulative_realized_loss += (par.amount() - amount.amount()).max(0.0);
        }
        let release_date = adjust(
            default_date.add_months(lag_months).max(last_date),
            convention,
            calendar,
        )?;
        final_date = final_date.max(release_date);
        distribute_terminal_principal(state, amount, release_date)?;
    }
    Ok(final_date)
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
