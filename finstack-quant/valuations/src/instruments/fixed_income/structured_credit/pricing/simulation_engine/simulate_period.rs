use super::*;
use crate::instruments::fixed_income::structured_credit::types::{PaymentCalculation, PaymentType};

/// Interest shortfalls at or below this amount (in currency units) are float
/// residue from reconstructing the paid coupon by subtraction, not deferrals.
const INTEREST_SHORTFALL_FLOOR: f64 = 1e-6;

/// Simulate a single payment period.
///
/// Period execution order matches INTEX/Bloomberg convention:
///   1. Calculate pool cashflows (interest, principal, default, recovery)
///   2. Allocate losses through capital structure (using expected loss at default)
///   3. Execute waterfall on post-loss tranche balances
///   4. Record cashflows and update tranche balances
///   5. Update pool balance
///
/// Loss allocation uses **expected net loss** = default * (1 - recovery_rate),
/// applied at the point of default. This decouples loss recognition from cash
/// timing of recovery receipts (which are lagged). Recoveries still flow through
/// the waterfall as cash when they mature from the recovery queue.
/// Zero out a negative residual within the de-minimis tolerance.
fn snap_de_minimis(amount: Money) -> Money {
    if amount.amount() < 0.0 && amount.amount() >= -WRITEDOWN_DE_MINIMIS {
        Money::from((0_i64, amount.currency()))
    } else {
        amount
    }
}

pub(super) fn simulate_period(
    state: &mut SimulationState,
    instrument: &StructuredCredit,
    waterfall: &Waterfall,
    period: SimulationPeriod,
    context: &MarketContext,
    months_per_period: f64,
    source: &mut (impl PoolFlowSource + ?Sized),
) -> Result<()> {
    let pay_date = period.payment;
    let as_of = period.valuation;
    // Opening collateral balance: the base for this period's excess spread.
    let opening_pool_balance = state.pool_outstanding.amount();
    // Seasoning for PSA/SDA ramps = collateral age, not deal age: the
    // pool's balance-weighted average loan age at closing (WALA, derived
    // from asset acquisition dates) plus the months elapsed since closing.
    // Seasoned collateral therefore enters the ramp partway up instead of
    // restarting at month zero on the deal closing date.
    let seasoning_months = state.pool_wala_months + state.closing_date.months_until(pay_date);

    // Capture period start before updating prev_date (for accrual calculations)
    let period_start = state.prev_date.unwrap_or(state.closing_date);

    // Reserve interest accrues on the opening balance, before this period's
    // collateral draws debit the account; it is routed after the pool flows.
    let reserve_interest_amount =
        super::reserve::reserve_interest_amount(state, period_start, pay_date)?;

    // Live available-funds cap for this period: the current collateral WAC (net
    // of the AFC fee load), read from the start-of-period pool state before this
    // period's pool flows amortize it. `0.0` when no AFC rule is configured. Used
    // for both the cash *routed* to capped tranches (the per-period AFC waterfall
    // below) and the interest *recorded* (Step 5), so the two cannot diverge.
    let live_afc_cap = live_afc_cap_rate(instrument, state, context, period_start)?;

    // Per-tranche interest CLAIMS as the waterfall spec defines them (F3): an
    // uncapped recipient owes the full coupon, a capped recipient owes the
    // capped coupon (the capped-off portion never defers), and a debt tranche
    // with no interest recipient owes nothing. Extracted from the base
    // waterfall with the live AFC cap applied exactly as `resolve_waterfall`
    // does, so these claims match what the period waterfall allocates. Shared
    // by the excess-spread/reserve sizing below and the Step-5 recording.
    let claim_caps =
        crate::instruments::fixed_income::structured_credit::pricing::waterfall::interest_claim_caps(
            waterfall,
            instrument
                .waterfall_rules
                .as_ref()
                .and_then(|rules| rules.afc.as_ref()),
            live_afc_cap,
        );

    // ── Step 1: Calculate pool cashflows for the period ──────────────
    let pool_flows = source.calculate_pool_flows(PoolFlowRequest {
        state,
        instrument,
        pay_date,
        prev_date: period_start,
        seasoning_months,
        months_per_period,
        context,
    })?;

    // Route the reserve interest per the pool's destination: waterfall
    // proceeds, a named tranche (outside the waterfall and the IC numerator),
    // or the reserve itself.
    let reserve_interest =
        super::reserve::route_reserve_interest(state, reserve_interest_amount, pay_date)?;

    // Hedge swaps settle through the waterfall: net receipts join interest
    // proceeds below, net payments become fee recipients in the period
    // waterfall.
    let hedge_flows = super::hedges::period_hedge_flows(state, period_start, pay_date)?;

    // Principal consumed by the collateral this period: draws funded from
    // principal collections and revolver repayments diverted to replenish the
    // reserve. Neither reaches the waterfall.
    let principal_to_collateral = pool_flows
        .draw_from_principal
        .checked_add(pool_flows.reserve_replenished)?;
    // Draw funding and replenishment are each bounded by the collections, so
    // a negative remainder can only be float noise; snap it before the
    // waterfall's sign check.
    let pool_principal = snap_de_minimis(
        pool_flows
            .scheduled_principal
            .checked_add(pool_flows.prepayment)?
            .checked_sub(principal_to_collateral)?,
    );
    if pool_principal.amount() < -WRITEDOWN_DE_MINIMIS {
        return Err(finstack_quant_core::Error::Validation(format!(
            "collateral consumed {} of principal but only {} was collected on {}",
            principal_to_collateral.amount(),
            pool_flows
                .scheduled_principal
                .checked_add(pool_flows.prepayment)?
                .amount(),
            pay_date
        )));
    }

    // Preserve the opening claim before this period's projected writedowns or
    // distributions. Paid coupons can include prior deferrals and cannot be
    // used to reconstruct a buyer's current-period accrued interest.
    for tranche in &state.tranches.tranches {
        if tranche.seniority == TrancheSeniority::Equity {
            continue;
        }
        let Some(cap) = claim_caps.get(tranche.id.as_str()) else {
            continue;
        };
        let mut rate = tranche
            .coupon
            .try_rate_for_period(period.accrual_start, as_of, context)?;
        if matches!(
            tranche.coupon,
            crate::instruments::fixed_income::structured_credit::TrancheCoupon::Floating(_)
        ) {
            rate = (rate + state.floating_rate_shift).max(0.0);
        }
        if let Some(cap) = cap {
            rate = rate.min(*cap);
        }
        let opening_balance = state.tranche_balances[tranche.id.as_str()];
        if let Some(result) = state.results.get_mut(tranche.id.as_str()) {
            result.accrual_periods.push(
                crate::instruments::fixed_income::structured_credit::TrancheAccrualPeriod {
                    start: period.accrual_start,
                    end: period.accrual_end,
                    payment_date: pay_date,
                    opening_balance,
                    coupon_rate: rate,
                    day_count: tranche.day_count,
                },
            );
        }
    }

    state.prev_date = Some(pay_date);

    // Add new recoveries to the lag queue
    state
        .recovery_queue
        .add_recovery(pay_date, pool_flows.recovery, pool_flows.default);

    // Release matured recoveries (these become cash for waterfall distribution)
    let released_recoveries = state.recovery_queue.release_matured(
        pay_date,
        state.recovery_lag_months,
        state.base_currency,
    )?;

    // ── Step 2: Loss allocation through capital structure ────────────
    //
    // INTEX/Moody's Analytics convention: allocate expected net loss at the
    // point of default, NOT when lagged recoveries arrive. This ensures:
    //   - Tranche balances reflect economic reality before the waterfall runs
    //   - Interest accrues only on non-impaired notional
    //   - OC/IC coverage tests see correct post-loss balances
    //   - No risk of paying interest on subsequently written-down principal
    //
    // Net loss = defaulted principal − realized recovery. Using the actual
    // recovered amount (rather than `default × (1 − mean_recovery)`) makes the
    // write-down reflect per-name recovery dispersion; it reduces to the old
    // formula when every default recovers at the period systematic rate.
    // This is a permanent, irreversible write-down.
    // SC-m19 — why there is no writedown REVERSAL (writeup) mechanism.
    //
    // The writedown is taken NET of the period's realized recovery, and that
    // recovery is determined at default time (the lag affects only when the
    // CASH arrives, not the amount). So there is no later surprise recovery
    // that a writeup would recognize: adding one would double-count the
    // recovery already netted here.
    //
    // A writeup mechanism would be required under the alternative convention
    // where writedowns are taken GROSS at default and recoveries restore
    // notional as they arrive. This engine does not use that convention.
    let period_expected_loss =
        (pool_flows.default.amount() - pool_flows.recovery.amount()).max(0.0);
    state.cumulative_realized_loss += period_expected_loss;

    // Recovery value of defaulted collateral whose cash has not yet arrived:
    // the OC tests carry it as collateral (CLO convention: defaulted
    // obligations at their assumed recovery), so a default costs the test its
    // expected loss rather than the whole defaulted par.
    // Defaulted collateral is carried at its modeled recovery value by
    // default; a `MarketValue` rule carries the defaulted par at that price.
    let defaulted_collateral_value = match waterfall
        .coverage_rules
        .as_ref()
        .map(|rules| rules.defaulted_valuation)
        .unwrap_or_default()
    {
        crate::instruments::fixed_income::structured_credit::types::DefaultedValuation::Recovery => {
            state.recovery_queue.pending_amount(state.base_currency)
        }
        crate::instruments::fixed_income::structured_credit::types::DefaultedValuation::MarketValue { pct } => Money::new(
            state.recovery_queue.pending_par(state.base_currency).amount() * pct / 100.0,
            state.base_currency,
        )?,
    };

    // The loss-allocation policy decides whether realized loss reaches the
    // note balances now (`WriteDown`: RMBS/CMBS realized-loss allocation) or
    // only as unpaid principal at legal final (`ParPreserving`: CLO/ABS notes
    // carry par, keep accruing their full coupon ahead of the residual, and
    // the OC tests do the de-levering). The cumulative loss above still drives
    // every loss-based trigger under both policies.
    let write_down_at_default = matches!(
        instrument.effective_loss_allocation(),
        crate::instruments::fixed_income::structured_credit::LossAllocationPolicy::WriteDown
    );

    if write_down_at_default
        && state.cumulative_realized_loss > WRITEDOWN_DE_MINIMIS
        && state.performing_pool_balance.amount() > 0.0
    {
        // Allocate incremental net loss bottom-up. Cap each tranche's
        // write-down at current balance so
        // `principal_repaid + write-down ≤ original_balance`; any excess loss
        // cascades to the next-most-senior tranche.
        let already_allocated: f64 = state
            .results
            .values()
            .map(|r| r.total_writedown.amount())
            .sum();
        let mut remaining_loss =
            (state.cumulative_realized_loss - state.initial_realized_loss - already_allocated)
                .max(0.0);
        // Iterate `loss_alloc_order` by index rather than cloning it each
        // period: each `idx` read is a short immutable borrow of `state` that
        // ends before the tranche-balance / results mutations below, so there
        // is no borrow conflict and no per-period allocation.
        for k in 0..state.loss_alloc_order.len() {
            let idx = state.loss_alloc_order[k];
            if remaining_loss <= WRITEDOWN_DE_MINIMIS {
                break;
            }
            let tranche_id_str = state.tranches.tranches[idx].id.as_str();

            // The tranche can absorb at most its current outstanding balance.
            // `tranche_balances` already nets out every prior principal
            // payment AND prior write-down, so capping the incremental
            // write-down here keeps `principal_repaid + write-down ≤ face`.
            let current = state
                .tranche_balances
                .get(tranche_id_str)
                .map(|m| m.amount())
                .unwrap_or(0.0);
            if current <= WRITEDOWN_DE_MINIMIS {
                continue;
            }

            let incremental = remaining_loss.min(current);
            remaining_loss -= incremental;
            if incremental > WRITEDOWN_DE_MINIMIS {
                // Reduce tranche balance BEFORE waterfall execution.
                if let Some(current_balance) = state.tranche_balances.get_mut(tranche_id_str) {
                    let new_balance = (current_balance.amount() - incremental).max(0.0);
                    *current_balance = Money::new(new_balance, state.base_currency)?;
                }

                let writedown = Money::new(incremental, state.base_currency)?;
                if let Some(res) = state.results.get_mut(tranche_id_str) {
                    res.writedown_flows.push((pay_date, writedown));
                    res.total_writedown = res.total_writedown.checked_add(writedown)?;
                }
            }
        }

        // Unallocated loss after every tranche is fully impaired. Assign
        // (do not `+=`): `remaining_loss` is already the cumulative residual.
        if remaining_loss > WRITEDOWN_DE_MINIMIS {
            state.cumulative_loss_unallocated = remaining_loss;
        }

        // Invariant: unallocated loss can only be non-zero once every tranche
        // is fully written down (no notional left to absorb it). Debug-only —
        // compiled out in release builds.
        if cfg!(debug_assertions) && state.cumulative_loss_unallocated > WRITEDOWN_DE_MINIMIS {
            let total_face: f64 = state
                .tranches
                .tranches
                .iter()
                .map(|t| t.current_balance.amount())
                .sum();
            let total_writedown: f64 = state
                .results
                .values()
                .map(|r| r.total_writedown.amount())
                .sum();
            let total_principal: f64 = state
                .results
                .values()
                .map(|r| r.total_principal.amount())
                .sum();
            debug_assert!(
                total_writedown + total_principal >= total_face - WRITEDOWN_DE_MINIMIS,
                "unallocated loss {} surfaced but structure is not fully \
                 retired: face={total_face}, writedown={total_writedown}, \
                 principal={total_principal}",
                state.cumulative_loss_unallocated,
            );
        }
    }

    let trigger_actions = super::triggers::advance(
        state,
        waterfall,
        period,
        period_start,
        super::triggers::TriggerCashInputs {
            interest: pool_flows
                .interest
                .checked_add(state.undistributed_interest)?,
            principal: pool_principal
                .checked_add(released_recoveries)?
                .checked_add(state.undistributed_principal)?,
            defaulted_collateral_value,
        },
        context,
    )?;

    // Early amortization (master-trust style): once cumulative losses reach the
    // configured threshold, the revolving period ends immediately and the deal
    // begins amortizing, regardless of the scheduled revolving-period end.
    // Either test is an event: once it fires the revolving period stays
    // closed. The excess-spread test reads the three-period trailing average
    // of the annualized excess spread realized so far.
    let early_amortization_event = instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.early_amortization.as_ref())
        .is_some_and(|spec| {
            let denom = state.total_pool_balance.amount();
            let loss_fraction = if denom > 0.0 {
                state.cumulative_realized_loss / denom
            } else {
                0.0
            };
            let spread_breached = spec.min_excess_spread_3m.is_some_and(|floor| {
                let history = &state.excess_spread_history;
                history.len() >= 3 && history[history.len() - 3..].iter().sum::<f64>() / 3.0 < floor
            });
            loss_fraction >= spec.max_cumulative_loss_pct || spread_breached
        });
    if early_amortization_event {
        state.early_amortization_triggered = true;
    }
    let early_amortization = trigger_actions.accelerate || state.early_amortization_triggered;

    // Apply current-period trigger outcomes before buying replacement collateral.
    let is_reinvestment_active = !early_amortization
        && !trigger_actions.stop_reinvestment
        && state
            .pool
            .reinvestment_period
            .as_ref()
            .is_some_and(|period| period.is_active && pay_date <= period.end_date);

    // Controlled accumulation (master-trust style): after any revolving period
    // and before the bullet date, collected pool principal is held in a funding
    // account (investor balances flat) and released as a bullet at the
    // accumulation end. Suspended while reinvestment recycles principal and on
    // early amortization (which pays down immediately). `principal_diverted`
    // unifies the two phases where pool principal is withheld from the waterfall.
    let accumulation_spec = instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.controlled_accumulation.as_ref());
    let is_accumulating = accumulation_spec.is_some_and(|spec| {
        !early_amortization
            && !is_reinvestment_active
            && pay_date >= spec.start_date
            && pay_date < spec.bullet_date
    });
    let principal_diverted = is_accumulating;

    let reinvested_cash = if is_reinvestment_active {
        let recyclable = pool_principal.checked_add(state.undistributed_principal)?;
        // Notes listed as amortizing are paid down before anything is
        // recycled: their whole outstanding balance is owed out of principal
        // proceeds, so only the excess over it can buy collateral.
        let (required_paydown, price) = match state.pool.reinvestment_period.as_ref() {
            Some(period) => (
                period
                    .amortizing_tranches
                    .iter()
                    .map(|id| {
                        state
                            .tranche_balances
                            .get(id.as_str())
                            .map_or(0.0, Money::amount)
                            .max(0.0)
                    })
                    .sum::<f64>(),
                instrument
                    .behavior_overrides
                    .reinvestment_price
                    .or_else(|| period.assumptions.as_ref().map(|a| a.price_pct))
                    .unwrap_or(100.0),
            ),
            None => (0.0, 100.0),
        };
        // Preserve the exact decimal cash budget. Converting the whole account
        // to f64 and back can spend a fraction more than the available cash.
        let budget = if required_paydown >= recyclable.amount() {
            Money::from((0_i64, state.base_currency))
        } else {
            recyclable.checked_sub(Money::new(required_paydown, state.base_currency)?)?
        };
        recycle_reinvestment_principal(state, budget, price / 100.0, pay_date, context)?
    } else {
        Money::from((0_i64, state.base_currency))
    };

    // ── Step 3: Prepare waterfall inputs ─────────────────────────────
    // Total principal from pool (scheduled + prepayment, net of principal
    // consumed by collateral draws and reserve replenishment).
    let total_principal_from_pool = pool_principal;

    // During reinvestment, principal collections are reinvested into new assets;
    // during controlled accumulation they are held in the funding account. Either
    // way pool principal is withheld from the waterfall (`principal_diverted`).
    // Recoveries are CASH and always flow through the waterfall.
    let mut principal_available_for_waterfall = if principal_diverted {
        released_recoveries
    } else {
        total_principal_from_pool
            .checked_sub(reinvested_cash)?
            .checked_add(released_recoveries)?
    };

    let carried_cash = state
        .undistributed_interest
        .checked_add(state.undistributed_principal)?;
    principal_available_for_waterfall =
        principal_available_for_waterfall.checked_add(state.undistributed_principal)?;
    // Interest proceeds: pool interest, call/put premia above par, reserve
    // interest routed to the waterfall, plus interest carried from prior periods.
    let mut interest_available_for_waterfall = pool_flows
        .interest
        .checked_add(pool_flows.call_premium)?
        .checked_add(reserve_interest.to_waterfall)?
        .checked_add(hedge_flows.receipts)?
        .checked_add(state.undistributed_interest)?;

    let mut total_cash_for_waterfall =
        interest_available_for_waterfall.checked_add(principal_available_for_waterfall)?;

    // Excess-spread (spread-account) capture/draw, applied to the cash entering
    // the waterfall. Capturing *here* — before the single sequential waterfall
    // can sweep surplus interest into senior principal — is what lets the
    // account fund from excess interest mid-deal and later draw to cover debt
    // interest shortfalls. No-op (identity) when no `excess_spread` is set.
    // `spread_net_capture` is the net cash diverted into the account this period
    // (negative when drawing), reconciled by the cash-conservation check.
    // Total interest the waterfall owes debt (non-equity) tranches this period:
    // the current-period coupon (shared helper, so the surplus measured here
    // matches what Step 5 records, including the live AFC cap) PLUS each
    // tranche's outstanding non-PIK deferred interest — a senior claim the
    // waterfall must also satisfy. Omitting the deferred piece would let the
    // excess-spread account capture interest it should instead leave behind to
    // cure that shortfall.
    //
    // Shared by excess-spread capture/draw and reserve-account draw (same
    // shortfall). Computed only when one of those features is live.
    let needs_interest_due = instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.excess_spread.as_ref())
        .is_some()
        || state.reserve_balance.amount() > 0.0;
    // N1: senior fees are part of what the waterfall owes AHEAD of the notes.
    //
    // `debt_interest_due` below was written for the reserve/excess-spread work
    // (SC-C07) BEFORE the fee tier existed (SC-M03), and was never revisited
    // when it did. It summed note coupon + deferred interest only, so the
    // excess-spread account measured "surplus" against a claim that omitted
    // every fee the waterfall pays first.
    //
    // Concretely, on a revolving deal with pool interest 100, fees 5 and note
    // interest due 90: capture skimmed 100 − 90 = 10, the waterfall received
    // 90, the fee tier took its 5 first, and the notes were left 5 short —
    // deferring interest (or CAPITALIZING it for PIK tranches, compounding the
    // error into later interest due and OC denominators) while the fee-netted
    // IC test read (100 − 5)/90 = 1.056 and reported healthy. The capture and
    // the coverage test disagreed about the senior claim.
    //
    // Netting fees here makes the two agree, and uses the same
    // `senior_fee_accrual` kernel the waterfall pays with, so the measured and
    // paid amounts cannot drift.
    let senior_fee_accrual_amount = if needs_interest_due {
        let mut tranche_index = finstack_quant_core::HashMap::default();
        for (i, tr) in state.tranches.tranches.iter().enumerate() {
            tranche_index.insert(tr.id.as_str(), i);
        }
        crate::instruments::fixed_income::structured_credit::pricing::waterfall::senior_fee_accrual(
            // The BASE waterfall: `resolve_waterfall` only rewrites AFC caps
            // on tranche interest, step-down/shifting weights on principal
            // tiers, and accumulation lockout targets — it never touches fee
            // tiers, so the fee accrual is identical either way.
            waterfall,
            state.tranches,
            &tranche_index,
            crate::instruments::fixed_income::structured_credit::pricing::waterfall::SeniorFeeInputs {
                available: pool_flows.interest,
                tranche_balances: Some(&state.tranche_balances),
                deferred_interest: Some(&state.deferred_interest),
                pool_balance: state.pool_outstanding,
                special_serviced_balance:
                    crate::instruments::fixed_income::structured_credit::pricing::waterfall::special_serviced_balance(
                        &state.pool,
                        Some(&state.pool_state.balances),
                        state.base_currency,
                    )?,
                period_start,
                payment_date: pay_date,
                valuation_date: as_of,
                market: context,
                reserve_balance: state.reserve_balance,
                floating_rate_shift: state.floating_rate_shift,
            },
        )?
        .amount()
    } else {
        0.0
    };

    let mut debt_interest_due = senior_fee_accrual_amount;
    if needs_interest_due {
        for tranche in &state.tranches.tranches {
            if tranche.seniority == TrancheSeniority::Equity {
                continue;
            }
            // The spec defines the claim: absent = no interest owed (and no
            // deferred claim the waterfall could ever service).
            let Some(cap) = claim_caps.get(tranche.id.as_str()) else {
                continue;
            };
            let bal = state
                .tranche_balances
                .get(tranche.id.as_str())
                .map_or(0.0, Money::amount);
            debt_interest_due += tranche_period_interest_due(
                tranche,
                bal,
                TrancheAccrualDates {
                    start: period_start,
                    payment: pay_date,
                    valuation: as_of,
                },
                context,
                cap.unwrap_or(0.0),
                cap.is_some(),
                state.floating_rate_shift,
            )?;
            if !tranche.pik_enabled {
                debt_interest_due += state
                    .deferred_interest
                    .get(tranche.id.as_str())
                    .map_or(0.0, Money::amount);
            }
        }
    }

    let mut spread_net_capture = 0.0_f64;
    if let Some(es) = instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.excess_spread.as_ref())
    {
        // Snapshot the account balance before any capture/draw, to independently
        // reconcile the recorded net capture against the actual balance move.
        let spread_before = state.spread_account.amount();

        let interest_avail = interest_available_for_waterfall.amount();
        if interest_avail > debt_interest_due {
            // Capture surplus interest into the account, up to the target.
            let room = (es.target_balance.amount() - state.spread_account.amount()).max(0.0);
            let capture = (interest_avail - debt_interest_due).min(room).max(0.0);
            // `capture <= interest_avail <= total_cash_for_waterfall`, so the
            // withdrawal is non-negative; assert the floor is a no-op so a future
            // divergence between this surplus check and the waterfall cash cannot
            // silently leak cash into (or out of) the account.
            let net_after = total_cash_for_waterfall.amount() - capture;
            debug_assert!(
                net_after >= -WRITEDOWN_DE_MINIMIS,
                "excess-spread capture {capture} overdrew waterfall cash {}",
                total_cash_for_waterfall.amount()
            );
            state.spread_account = state
                .spread_account
                .checked_add(Money::new(capture, state.base_currency)?)?;
            total_cash_for_waterfall = Money::new(net_after.max(0.0), state.base_currency)?;
            spread_net_capture = capture;
        } else {
            // Draw from the account to cover the interest shortfall (bounded by
            // the account balance, so the subtraction stays non-negative).
            let draw = (debt_interest_due - interest_avail)
                .min(state.spread_account.amount())
                .max(0.0);
            let draw_money = Money::new(draw, state.base_currency)?;
            state.spread_account = state.spread_account.checked_sub(draw_money)?;
            total_cash_for_waterfall = total_cash_for_waterfall.checked_add(draw_money)?;
            spread_net_capture = -draw;
        }
        interest_available_for_waterfall = interest_available_for_waterfall
            .checked_sub(Money::new(spread_net_capture, state.base_currency)?)?;

        // Independent reconciliation: the account balance actually moved by
        // exactly the recorded net capture (catches a future edit that updates
        // one but not the other). `spread_before` is read only here, so it is
        // unused in release builds where `debug_assert!` is compiled out.
        let _ = spread_before;
        debug_assert!(
            ((state.spread_account.amount() - spread_before) - spread_net_capture).abs()
                <= WRITEDOWN_DE_MINIMIS,
            "spread-account delta {} != recorded net capture {spread_net_capture}",
            state.spread_account.amount() - spread_before
        );
    }

    // Reserve draw: credit enhancement drawn after excess-spread capture to
    // cover remaining debt-interest shortfall. Bounded by shortfall and
    // balance; `reserve_net_capture` (negative on draw) feeds cash conservation.
    let mut reserve_net_capture = 0.0_f64;
    if state.reserve_balance.amount() > 0.0 {
        // A funded reserve covers the interest account's remaining fee/coupon
        // shortfall. Principal collections remain restricted to capital uses.
        let shortfall = (debt_interest_due - interest_available_for_waterfall.amount()).max(0.0);
        let draw = shortfall.min(state.reserve_balance.amount()).max(0.0);
        if draw > 0.0 {
            let draw_money = Money::new(draw, state.base_currency)?;
            state.reserve_balance = state.reserve_balance.checked_sub(draw_money)?;
            total_cash_for_waterfall = total_cash_for_waterfall.checked_add(draw_money)?;
            interest_available_for_waterfall =
                interest_available_for_waterfall.checked_add(draw_money)?;
            reserve_net_capture = -draw;
        }
    }

    // Controlled-accumulation funding account. During accumulation, divert this
    // period's pool principal into the account (kept out of the waterfall above
    // via `principal_diverted`, so investor balances stay flat). At the bullet
    // date, release the whole account into the waterfall as principal. Applied
    // after the excess-spread block so the spread account never captures the
    // bullet principal as if it were surplus interest. `funding_net_release`
    // (cash added back from the account) reconciles the cash-conservation check.
    let mut funding_net_release = 0.0_f64;
    if let Some(spec) = accumulation_spec {
        if is_accumulating {
            let captured = total_principal_from_pool.amount().max(0.0);
            state.principal_funding_account = Money::new(
                state.principal_funding_account.amount() + captured,
                state.base_currency,
            )?;
        } else if (early_amortization || pay_date >= spec.bullet_date)
            && state.principal_funding_account.amount() > 0.0
        {
            // N3: early amortization RELEASES the account, it does not strand
            // it. Gating the release on `!early_amortization` would make a
            // deal that breached into early am with a funded account only see
            // that cash at the terminal sweep — dated at the final simulated
            // period. Early amortization exists precisely to accelerate
            // principal to investors, so withholding already-collected
            // principal until deal end inverts the trigger's purpose and
            // mis-states senior WAL, duration and price in exactly the stress
            // scenario the feature models (cash conserved, timing wrong).
            funding_net_release = state.principal_funding_account.amount();
            state.principal_funding_account = Money::from((0_i64, state.base_currency));
            let release = Money::new(funding_net_release, state.base_currency)?;
            principal_available_for_waterfall =
                principal_available_for_waterfall.checked_add(release)?;
            total_cash_for_waterfall = total_cash_for_waterfall.checked_add(release)?;
        }
    }

    // ── Step 4: Execute Waterfall on post-loss balances ──────────────
    // Per-period step-down: switch principal to pro-rata once the deal has
    // seasoned past the step-down date with cumulative losses below the trigger.
    // Borrowed (zero-cost) when no step-down rule applies.
    let rules = instrument.waterfall_rules.as_ref();
    // Controlled accumulation locks out investor principal (held flat) and takes
    // precedence; otherwise shifting interest and step-down govern principal
    // allocation, with shifting interest winning when both are configured.
    let period_waterfall = if is_accumulating {
        crate::instruments::fixed_income::structured_credit::pricing::resolve::apply_accumulation_lockout(
            waterfall,
            &state.tranche_balances,
        )
    } else if let Some(si) = rules.and_then(|r| r.shifting_interest.as_ref()) {
        let months_from_closing = state.closing_date.months_until(pay_date);
        // Senior's pro-rata share (by current balance) governs scheduled
        // principal; the schedule lock-out governs unscheduled principal.
        let senior_bal = state
            .tranche_balances
            .get(si.senior_id.as_str())
            .map_or(0.0, |m| m.amount());
        let total_debt: f64 = state
            .tranches
            .tranches
            .iter()
            .filter(|t| t.seniority != TrancheSeniority::Equity)
            .map(|t| {
                state
                    .tranche_balances
                    .get(t.id.as_str())
                    .map_or(0.0, |m| m.amount())
            })
            .sum();
        let senior_prorata_share = if total_debt > 0.0 {
            senior_bal / total_debt
        } else {
            0.0
        };
        // Unscheduled (prepayment + recovery) fraction of distributable principal.
        let scheduled = pool_flows.scheduled_principal.amount().max(0.0);
        let unscheduled =
            pool_flows.prepayment.amount().max(0.0) + released_recoveries.amount().max(0.0);
        let unscheduled_fraction = if scheduled + unscheduled > 0.0 {
            unscheduled / (scheduled + unscheduled)
        } else {
            1.0
        };
        crate::instruments::fixed_income::structured_credit::pricing::resolve::apply_shifting_interest(
            waterfall,
            rules,
            months_from_closing,
            senior_prorata_share,
            unscheduled_fraction,
            &state.tranche_balances,
        )
    } else {
        let metrics = step_down_metrics(state);
        crate::instruments::fixed_income::structured_credit::pricing::resolve::apply_step_down(
            waterfall,
            rules,
            pay_date,
            &metrics,
            &state.tranche_balances,
        )
    };

    // Layer the available-funds cap onto the per-period waterfall using the live
    // cap rate, so the cash *routed* to capped tranches' interest matches the
    // interest *recorded* in Step 5 (both keyed on `live_afc_cap`). Identity (no
    // clone) when no AFC rule is configured.
    let period_waterfall = if rules.and_then(|r| r.afc.as_ref()).is_some() {
        std::borrow::Cow::Owned(
            crate::instruments::fixed_income::structured_credit::pricing::resolve::resolve_waterfall(
                &period_waterfall,
                rules,
                live_afc_cap,
            ),
        )
    } else {
        period_waterfall
    };

    let period_waterfall = if is_reinvestment_active {
        let mut resolved = period_waterfall.into_owned();
        let amortizing: &[String] = state
            .pool
            .reinvestment_period
            .as_ref()
            .map_or(&[], |period| period.amortizing_tranches.as_slice());
        for tier in &mut resolved.tiers {
            if tier.payment_type != PaymentType::Principal {
                continue;
            }
            // Principal that cannot be placed remains in the capital account
            // during revolving periods instead of leaking to equity.
            tier.recipients
                .retain(|recipient| recipient.recipient_type != RecipientType::Equity);
            // Every note not listed as amortizing is held flat: its regular
            // principal target is its current balance for this period.
            for recipient in &mut tier.recipients {
                if let PaymentCalculation::TranchePrincipal {
                    tranche_id,
                    target_balance,
                    ..
                } = &mut recipient.calculation
                {
                    if !amortizing.iter().any(|id| id == tranche_id) {
                        *target_balance = state.tranche_balances.get(tranche_id).copied();
                    }
                }
            }
        }
        std::borrow::Cow::Owned(resolved)
    } else {
        period_waterfall
    };

    let period_waterfall = if state.tranche_triggers.is_empty() {
        period_waterfall
    } else {
        let mut resolved = period_waterfall.into_owned();
        trigger_actions.apply(&mut resolved);
        std::borrow::Cow::Owned(resolved)
    };

    let period_waterfall = if hedge_flows.payments.is_empty() {
        period_waterfall
    } else {
        let mut resolved = period_waterfall.into_owned();
        for (id, amount, priority) in &hedge_flows.payments {
            resolved.insert_hedge_payment(
                crate::instruments::fixed_income::structured_credit::types::Recipient::fixed_fee(
                    id.clone(),
                    "SwapCounterparty",
                    *amount,
                ),
                *priority,
            );
        }
        std::borrow::Cow::Owned(resolved)
    };

    // Canonical asset balances already reflect amortization, defaults, and
    // any par purchased with reinvested cash. Restricted cash is passed once.
    let coverage_test_pool_balance =
        Money::new(state.pool_state.balances.iter().sum(), state.base_currency)?;

    // Equity's cash to date for the incentive-fee hurdle: capital at closing
    // against every recorded equity distribution (waterfall payments and
    // reserve interest routed straight to equity).
    let equity_history = equity_history(state)?;

    let waterfall_context =
        crate::instruments::fixed_income::structured_credit::pricing::waterfall::WaterfallContext {
            available_cash: total_cash_for_waterfall,
            interest_collections: interest_available_for_waterfall,
            principal_collections: principal_available_for_waterfall,
            payment_date: pay_date,
            period_start,
            valuation_date: as_of,
            pool_balance: coverage_test_pool_balance,
            market: context,
            tranche_balances: Some(&state.tranche_balances),
            asset_balances: Some(&state.pool_state.balances),
            deferred_interest: Some(&state.deferred_interest),
            reserve_balance: state.reserve_balance,
            // N2: funding-account principal is still collateral for the notes.
            restricted_cash: state.principal_funding_account,
            defaulted_collateral_value,
            recovery_proceeds: released_recoveries,
            floating_rate_shift: state.floating_rate_shift,
            equity_history: Some(&equity_history),
        };

    let waterfall_result =
        crate::instruments::fixed_income::structured_credit::pricing::waterfall::execute_waterfall(
            &period_waterfall,
            state.tranches,
            &state.pool,
            waterfall_context,
        )?;
    // Carry residual cash forward. Tier payments are rounded to the currency's
    // smallest unit, so the residual can sit a few ulps below zero; snapping
    // sub-cent negatives keeps a period with no other interest (an instrument
    // pool between coupon dates) from failing the waterfall's sign check.
    state.undistributed_interest = snap_de_minimis(waterfall_result.remaining_interest);
    state.undistributed_principal = snap_de_minimis(waterfall_result.remaining_principal);

    // Update reserve balance from waterfall distributions to ReserveAccount recipients.
    for (recipient, amount) in &waterfall_result.distributions {
        if let RecipientType::ReserveAccount(_) = recipient {
            state.reserve_balance = state.reserve_balance.checked_add(*amount)?;
        }
    }

    // ── Step 5: Record flows and update balances ─────────────────────
    let mut debt_interest_due = 0.0_f64;
    for (idx, tranche) in state.tranches.tranches.iter().enumerate() {
        let recipient_key = &state.tranche_recipient_keys[idx];
        let tranche_id_str = tranche.id.as_str();

        let current_balance = state
            .tranche_balances
            .get(tranche_id_str)
            .copied()
            .unwrap_or(Money::from((0_i64, state.base_currency)));

        let existing_deferred = state
            .deferred_interest
            .get(tranche_id_str)
            .copied()
            .unwrap_or(Money::from((0_i64, state.base_currency)));

        // Current-period interest due on post-writedown balance, as the
        // waterfall spec defines the claim (`claim_caps`, F3): uncapped
        // recipients owe the full coupon, capped recipients owe the capped
        // coupon (AFC live cap or a custom static cap — the capped-off
        // portion is never owed, so it never defers), and a debt tranche with
        // no interest recipient owes nothing. The same map sized the
        // excess-spread/reserve draw above, so the two cannot diverge.
        //
        // Equity receives residual interest; its metadata coupon creates no
        // separate debt claim or deferred-interest balance.
        let current_interest_due = if tranche.seniority == TrancheSeniority::Equity {
            Money::from((0_i64, state.base_currency))
        } else {
            match claim_caps.get(tranche_id_str) {
                None => Money::from((0_i64, state.base_currency)),
                Some(cap) => Money::new(
                    tranche_period_interest_due(
                        tranche,
                        current_balance.amount(),
                        TrancheAccrualDates {
                            start: period_start,
                            payment: pay_date,
                            valuation: as_of,
                        },
                        context,
                        cap.unwrap_or(0.0),
                        cap.is_some(),
                        state.floating_rate_shift,
                    )?,
                    state.base_currency,
                )?,
            }
        };
        debt_interest_due += current_interest_due.amount();
        let total_interest_claim = if tranche.pik_enabled {
            current_interest_due
        } else {
            existing_deferred.checked_add(current_interest_due)?
        };

        let payment_received = waterfall_result
            .distributions
            .get(recipient_key)
            .copied()
            .unwrap_or(Money::from((0_i64, state.base_currency)));

        // SC-M28: take the waterfall's OWN interest/principal classification
        // rather than re-deriving it from the aggregate.
        //
        // `distributions` keys a tranche's interest and principal under the
        // same `RecipientType::Tranche(id)`, so this used to reconstruct the
        // split by assuming interest is satisfied FIRST:
        //     interest_paid = min(payment_received, total_interest_claim)
        //     principal     = remainder
        //
        // That silently reclassified principal as interest whenever a tranche
        // carried a shortfall — which is exactly the state an OC cure exists to
        // address. A cure diverted to senior PRINCIPAL was booked as interest,
        // the balance was never retired, and the next period's OC denominator
        // was unchanged: the cure could not de-lever the ratio it was sized to
        // fix. The defect bound precisely in the stress scenarios the cure
        // mechanics (SC-M07/M08/M09) were built for.
        //
        // The waterfall already knows which payments were `TranchePrincipal`;
        // `principal_distributions` reports it. Interest is then the remainder,
        // capped by the claim so a residual/equity distribution against a zero
        // interest claim cannot be misbooked as a coupon.
        let principal_from_waterfall = waterfall_result
            .principal_distributions
            .get(recipient_key)
            .copied()
            .unwrap_or(Money::from((0_i64, state.base_currency)));
        let principal_classified = Money::new(
            principal_from_waterfall
                .amount()
                .min(payment_received.amount())
                .max(0.0),
            state.base_currency,
        )?;
        let interest_portion = payment_received
            .checked_sub(principal_classified)
            .unwrap_or(Money::from((0_i64, state.base_currency)));
        let interest_paid = if tranche.seniority == TrancheSeniority::Equity {
            interest_portion
        } else if interest_portion.amount() >= total_interest_claim.amount() {
            total_interest_claim
        } else {
            interest_portion
        };
        let deferred_repaid = Money::new(
            interest_paid
                .amount()
                .min(existing_deferred.amount())
                .max(0.0),
            state.base_currency,
        )?;
        let current_interest_paid = interest_paid
            .checked_sub(deferred_repaid)
            .unwrap_or(Money::from((0_i64, state.base_currency)));
        // `current_interest_paid` is reconstructed by subtraction, so a coupon
        // paid in full can leave a sub-microcent residue; that is float noise,
        // not a deferral, and must not be booked as one.
        let raw_shortfall = current_interest_due.amount() - current_interest_paid.amount();
        let current_interest_shortfall = Money::new(
            if raw_shortfall > INTEREST_SHORTFALL_FLOOR {
                raw_shortfall
            } else {
                0.0
            },
            state.base_currency,
        )?;

        // Only explicitly classified principal retires loss-absorbing capital.
        let principal_payment = principal_classified;

        if let Some(res) = state.results.get_mut(tranche_id_str) {
            if payment_received.amount() > 0.0 {
                res.cashflows.push((pay_date, payment_received));
            }
            if interest_paid.amount() > 0.0 {
                res.interest_flows.push((pay_date, interest_paid));
                res.total_interest = res.total_interest.checked_add(interest_paid)?;
            }
            if principal_payment.amount() > 0.0 {
                res.principal_flows.push((pay_date, principal_payment));
                res.total_principal = res.total_principal.checked_add(principal_payment)?;
            }
            // SC-m11: PIK and DEFERRED interest are different things and are
            // now recorded separately. PIK capitalizes the shortfall into the
            // tranche balance (it accrues thereafter and enlarges the OC
            // denominator); a non-PIK deferral is a separate senior claim that
            // leaves notional untouched. Booking both under `pik_flows` made
            // `total_pik` unusable as a measure of capitalized balance.
            if current_interest_shortfall.amount() > 0.0 {
                if tranche.pik_enabled {
                    res.pik_flows.push((pay_date, current_interest_shortfall));
                    res.total_pik = res.total_pik.checked_add(current_interest_shortfall)?;
                } else {
                    res.deferred_flows
                        .push((pay_date, current_interest_shortfall));
                    res.total_deferred =
                        res.total_deferred.checked_add(current_interest_shortfall)?;
                }
            }
        }

        let remaining_deferred = if tranche.pik_enabled {
            Money::from((0_i64, state.base_currency))
        } else {
            existing_deferred
                .checked_sub(deferred_repaid)
                .unwrap_or(Money::from((0_i64, state.base_currency)))
                .checked_add(current_interest_shortfall)?
        };
        state
            .deferred_interest
            .insert(tranche_id_str.to_string(), remaining_deferred);

        // Update tranche balance:
        // - Always reduce by principal payment
        // - Only accrete shortfall if PIK is explicitly enabled for this tranche
        //
        // Standard CLO/ABS indenture: shortfalls are tracked as deferred interest
        // and paid from future interest collections, NOT capitalized into balance.
        // Non-PIK deferred balances do not compound (no interest-on-interest);
        // only an explicit `pik_enabled` tranche accretes the shortfall so it
        // earns the note rate thereafter.
        if let Some(current) = state.tranche_balances.get_mut(tranche_id_str) {
            let after_principal = current.checked_sub(principal_payment).unwrap_or(*current);
            // The waterfall nets in-period principal against the period-start
            // balance snapshot, so TranchePrincipal payments cannot exceed the
            // remaining balance. Residual/equity distributions, however, are
            // booked here as "principal" against a zero balance — floor at
            // zero so a negative balance never propagates into later periods'
            // interest accrual and coverage tests.
            let after_principal = if after_principal.amount() < 0.0 {
                Money::from((0_i64, state.base_currency))
            } else {
                after_principal
            };
            if tranche.pik_enabled && current_interest_shortfall.amount() > 0.0 {
                *current = after_principal.checked_add(current_interest_shortfall)?;
            } else {
                *current = after_principal;
            }
        }
    }

    // Asset state is authoritative, including par bought away from par.
    state.pool_outstanding =
        Money::new(state.pool_state.balances.iter().sum(), state.base_currency)?;

    // Realized excess spread this period, annualized on the opening pool
    // balance: interest collections less the debt coupons due, the fees paid
    // and the net charge-off. Feeds the early-amortization excess-spread test.
    let fees_paid: f64 = waterfall_result
        .distributions
        .iter()
        .filter(|(recipient, _)| {
            matches!(
                recipient,
                RecipientType::ServiceProvider(_) | RecipientType::ManagerFee(_)
            )
        })
        .map(|(_, amount)| amount.amount())
        .sum();
    let net_charge_off = (pool_flows.default.amount() - pool_flows.recovery.amount()).max(0.0);
    let excess = pool_flows.interest.amount() - debt_interest_due - fees_paid - net_charge_off;
    state
        .excess_spread_history
        .push(if opening_pool_balance > 0.0 {
            excess / opening_pool_balance * 12.0 / months_per_period.max(1e-9)
        } else {
            0.0
        });

    // Pool cash must equal recipient distributions plus residual cash and net
    // side-account capture. Reserve draws are negative capture; controlled-
    // accumulation releases add cash back to the waterfall.
    // Principal consumed by collateral draws and reserve replenishment never
    // reached the waterfall (positive capture); call premia and reserve
    // interest routed to the waterfall are cash added on top of pool flows
    // (negative capture).
    let side_net_capture =
        spread_net_capture + reserve_net_capture - funding_net_release - carried_cash.amount()
            + reinvested_cash.amount()
            + principal_to_collateral.amount()
            - pool_flows.call_premium.amount()
            - reserve_interest.to_waterfall.amount()
            - hedge_flows.receipts.amount();
    assert_cash_conserved(
        total_cash_for_waterfall,
        &pool_flows,
        released_recoveries,
        principal_diverted,
        &waterfall_result,
        side_net_capture,
    )?;

    // Per-period deal accounting: end-of-period balances, the cash that moved
    // and the coverage tests exactly as the executor evaluated them (single
    // source: the waterfall's own results).
    let pool_factor = if state.total_pool_balance.amount() > 0.0 {
        state.pool_outstanding.amount() / state.total_pool_balance.amount()
    } else {
        0.0
    };
    let trigger_levels: HashMap<&str, f64> = period_waterfall
        .coverage_tests()
        .map(|spec| (spec.id.as_str(), spec.trigger_level))
        .collect();
    let coverage_tests = waterfall_result
        .coverage_tests
        .iter()
        .map(|(test_id, ratio, passing)| {
            let trigger_level = trigger_levels
                .get(test_id.as_str())
                .copied()
                .unwrap_or(f64::NAN);
            super::state::CoverageTestDiagnostic {
                test_id: test_id.clone(),
                ratio: *ratio,
                trigger_level,
                cushion: ratio - trigger_level,
                passing: *passing,
            }
        })
        .collect();
    let (weighted_avg_coupon, weighted_avg_spread_bp, warf) =
        super::state::pool_composition(state)?;
    let delinquent: f64 = state
        .pool_state
        .delinquent
        .iter()
        .map(|buckets| super::delinquency::delinquent_balance(buckets))
        .sum();
    state
        .period_diagnostics
        .push(super::state::PeriodDiagnostics {
            payment_date: pay_date,
            pool_balance: state.pool_outstanding,
            pool_factor,
            weighted_avg_coupon,
            weighted_avg_spread_bp,
            warf,
            interest_collections: pool_flows.interest,
            principal_collections: pool_flows
                .scheduled_principal
                .checked_add(pool_flows.prepayment)?,
            defaults: pool_flows.default,
            recoveries: released_recoveries,
            reinvested_par: reinvested_cash,
            fees_paid: Money::new(fees_paid, state.base_currency)?,
            reserve_balance: state.reserve_balance,
            spread_account: state.spread_account,
            funding_account: state.principal_funding_account,
            delinquent_balance: Money::new(delinquent, state.base_currency)?,
            excess_spread: state.excess_spread_history.last().copied().unwrap_or(0.0),
            coverage_tests,
        });

    state
        .reserve_balance_path
        .push((pay_date, state.reserve_balance));

    Ok(())
}

/// Equity's cash to date: the equity notes' original par invested at closing
/// against every distribution recorded for them so far (waterfall interest
/// and principal, plus reserve interest routed straight to equity).
fn equity_history(
    state: &SimulationState,
) -> Result<crate::instruments::fixed_income::structured_credit::types::EquityHistory> {
    let mut invested = Money::from((0_i64, state.base_currency));
    let mut distributions: Vec<(Date, Money)> = Vec::new();
    for tranche in state
        .tranches
        .tranches
        .iter()
        .filter(|tranche| tranche.seniority == TrancheSeniority::Equity)
    {
        invested = invested.checked_add(tranche.original_balance)?;
        if let Some(result) = state.results.get(tranche.id.as_str()) {
            distributions.extend(
                result
                    .interest_flows
                    .iter()
                    .chain(result.principal_flows.iter())
                    .copied(),
            );
        }
    }
    distributions.sort_by_key(|(date, _)| *date);
    Ok(
        crate::instruments::fixed_income::structured_credit::types::EquityHistory {
            invested_on: state.closing_date,
            invested,
            distributions,
        },
    )
}
