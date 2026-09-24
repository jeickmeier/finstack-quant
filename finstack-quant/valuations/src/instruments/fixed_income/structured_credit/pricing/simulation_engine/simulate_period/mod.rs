//! One payment period of the deal simulation, in phases: pool collections
//! and the recovery queue ([`cash`]), loss allocation and loss-driven tests
//! ([`losses`]), the side accounts that set the waterfall cash ([`cash`]),
//! the period's resolved waterfall ([`period_waterfall`]), execution, and
//! recording the payments per tranche ([`record`]).

use super::*;
use crate::instruments::fixed_income::structured_credit::types::{PaymentCalculation, PaymentType};

mod cash;
mod losses;
mod period_waterfall;
mod record;

use cash::{AccumulationPhase, WaterfallCash};
use period_waterfall::{insert_hedge_payments, PeriodRuleFlags, PeriodRuleFlows};

/// Deal-level inputs shared by the phases of one period.
pub(super) struct PeriodInputs<'a> {
    /// The deal being simulated.
    pub(super) instrument: &'a StructuredCredit,
    /// The deal's base waterfall.
    pub(super) waterfall: &'a Waterfall,
    /// Market data for floating coupons and fees.
    pub(super) context: &'a MarketContext,
    /// Dates of the period.
    pub(super) period: SimulationPeriod,
    /// Start of the period (the previous payment date, or closing).
    pub(super) period_start: Date,
    /// Interest claim per tranche as the waterfall spec defines it.
    pub(super) claim_caps: &'a HashMap<&'a str, Option<f64>>,
}

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
    // waterfall with the live AFC cap applied exactly as the AFC rule
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

    // Card master trust: the first period outside the revolving period (its
    // scheduled end, an early-amortization event, a trigger that stops
    // reinvestment, or redemption) fixes the investor allocation on the
    // receivables held at the period's opening. When the period opens already
    // outside the window the base is fixed before the pool flows; when this
    // period's own event closes the window the flows have run on the live
    // (equal) opening balance and the base is fixed from it afterwards.
    let card_opening_balances = instrument
        .credit_model
        .card
        .as_ref()
        .filter(|_| state.card_flow_base.is_none())
        .map(|card| -> Result<Vec<f64>> {
            card.investor_flow_base(&state.pool_state.balances, state.base_currency)
        })
        .transpose()?;
    if let Some(base) = card_opening_balances.as_ref() {
        let revolving = !state.early_amortization_triggered
            && !period.redemption
            && state
                .pool
                .reinvestment_period
                .as_ref()
                .is_some_and(|window| window.is_active && pay_date <= window.end_date);
        if !revolving {
            state.card_flow_base = Some(base.clone());
        }
    }

    // Lender draws: scheduled draws due by this payment date and, while the
    // deal revolves, a re-advance up to the note's commitment and the
    // borrowing base. The cash joins principal proceeds (recycled while
    // revolving) and the note's balance rises by the same amount.
    apply_tranche_draws(state, instrument, &period, pay_date)?;

    // ── Step 1: Calculate pool cashflows for the period ──────────────
    // Special-servicing flags at the period's opening: a loan defaulting
    // this period is specially serviced (and pays the fee) from the next.
    let special_serviced_open = state.pool_state.special_serviced.clone();
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

    let inputs = PeriodInputs {
        instrument,
        waterfall,
        context,
        period,
        period_start,
        claim_caps: &claim_caps,
    };
    cash::record_opening_accruals(state, &inputs)?;

    state.prev_date = Some(pay_date);

    cash::queue_recoveries(state, &pool_flows, pay_date)?;

    // Release matured recoveries (these become cash for waterfall distribution)
    let (released_recoveries, released_par) = state.recovery_queue.release_matured(
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
    // Why there is no writedown REVERSAL (writeup) mechanism.
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
    // Under at-liquidation recognition (RMBS/CMBS servicing convention) the
    // loss is booked when the claim settles after the recovery lag instead:
    // the released claims' defaulted par less the recovery cash they return.
    let period_expected_loss = match state.loss_recognition {
        LossRecognition::AtDefault => {
            (pool_flows.default.amount() - pool_flows.recovery.amount()).max(0.0)
        }
        LossRecognition::AtLiquidation => {
            (released_par.amount() - released_recoveries.amount()).max(0.0)
        }
    };
    state.cumulative_realized_loss += period_expected_loss;

    let defaulted_collateral_value = losses::defaulted_collateral_value(state, waterfall)?;
    losses::allocate_realized_loss(state, instrument, pay_date)?;

    // The tests are evaluated on what the executor will see: the waterfall
    // with this period's hedge payments ranked as fees, and interest proceeds
    // including call premia, reserve interest and hedge receipts.
    let mut trigger_waterfall = std::borrow::Cow::Borrowed(waterfall);
    if !hedge_flows.payments.is_empty() {
        insert_hedge_payments(trigger_waterfall.to_mut(), &hedge_flows.payments);
    }
    let trigger_actions = super::triggers::advance(
        state,
        &trigger_waterfall,
        period,
        period_start,
        super::triggers::TriggerCashInputs {
            interest: pool_flows
                .interest
                .checked_add(pool_flows.call_premium)?
                .checked_add(reserve_interest.to_waterfall)?
                .checked_add(hedge_flows.receipts)?
                .checked_add(state.undistributed_interest)?,
            principal: pool_principal
                .checked_add(released_recoveries)?
                .checked_add(state.undistributed_principal)?,
            defaulted_collateral_value,
        },
        context,
    )?;

    // Early amortization (master-trust style): once cumulative losses or the
    // trailing excess spread breach the rule, the revolving period ends and
    // stays closed.
    let early_amortization_event = losses::early_amortization_event(state, instrument);
    if early_amortization_event {
        state.early_amortization_triggered = true;
    }
    let early_amortization = trigger_actions.accelerate || state.early_amortization_triggered;
    if early_amortization && state.early_amortization_date.is_none() {
        state.early_amortization_date = Some(pay_date);
    }

    // Apply current-period trigger outcomes before buying replacement collateral.
    let is_reinvestment_active = !early_amortization
        && !period.redemption
        && !trigger_actions.stop_reinvestment
        && state
            .pool
            .reinvestment_period
            .as_ref()
            .is_some_and(|period| period.is_active && pay_date <= period.end_date);

    if let (Some(base), false, true) = (
        card_opening_balances,
        is_reinvestment_active,
        state.card_flow_base.is_none(),
    ) {
        state.card_flow_base = Some(base);
    }

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
    let interest_available_for_waterfall = pool_flows
        .interest
        .checked_add(pool_flows.call_premium)?
        .checked_add(reserve_interest.to_waterfall)?
        .checked_add(hedge_flows.receipts)?
        .checked_add(state.undistributed_interest)?;
    let mut cash = WaterfallCash {
        total: interest_available_for_waterfall.checked_add(principal_available_for_waterfall)?,
        interest: interest_available_for_waterfall,
        principal: principal_available_for_waterfall,
    };

    // Excess-spread capture/draw and the reserve release/draw size against
    // what the waterfall owes ahead of the residual this period.
    let debt_interest_due =
        cash::debt_interest_due(state, &inputs, pool_flows.interest, &special_serviced_open)?;
    let spread_net_capture =
        cash::apply_excess_spread(state, instrument, &mut cash, debt_interest_due)?;
    let (reserve_target, reserve_net_capture) =
        cash::apply_reserve(state, instrument, &mut cash, debt_interest_due)?;
    let funding_net_release = cash::apply_funding_account(
        state,
        accumulation_spec,
        AccumulationPhase {
            is_accumulating,
            early_amortization,
        },
        pay_date,
        total_principal_from_pool,
        &mut cash,
    )?;

    // ── Step 4: Execute Waterfall on post-loss balances ──────────────
    let afc_carryover = instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.afc.as_ref())
        .is_some_and(|afc| afc.carryover);
    let period_waterfall = period_waterfall::resolve_period_waterfall(
        state,
        &inputs,
        &PeriodRuleFlags {
            is_accumulating,
            is_reinvestment_active,
            live_afc_cap,
            reserve_target,
            trigger_actions: &trigger_actions,
        },
        &PeriodRuleFlows {
            scheduled_principal: pool_flows.scheduled_principal,
            prepayment: pool_flows.prepayment,
            released_recoveries,
            hedge_payments: &hedge_flows.payments,
        },
    )?;

    // Canonical asset balances already reflect amortization, defaults, and
    // any par purchased with reinvested cash. Restricted cash is passed once.
    let coverage_test_pool_balance =
        Money::new(state.pool_state.balances.iter().sum(), state.base_currency)?;

    // Equity's cash to date for the incentive-fee hurdle: capital at closing
    // against every recorded equity distribution (waterfall payments and
    // reserve interest routed straight to equity).
    let equity_history = state.equity_history()?;
    let unresolved_npl = state.unresolved_npl();

    let waterfall_context =
        crate::instruments::fixed_income::structured_credit::pricing::waterfall::WaterfallContext {
            available_cash: cash.total,
            interest_collections: cash.interest,
            principal_collections: cash.principal,
            payment_date: pay_date,
            period_start,
            valuation_date: as_of,
            pool_balance: coverage_test_pool_balance,
            market: context,
            tranche_balances: Some(&state.tranche_balances),
            asset_balances: Some(&state.pool_state.balances),
            live_collateral: Some(state.live_collateral(&unresolved_npl)),
            special_serviced: Some(&special_serviced_open),
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
    let note_interest_due =
        record::record_tranche_flows(state, &inputs, &waterfall_result, afc_carryover)?;

    // Asset state is authoritative, including par bought away from par.
    state.pool_outstanding =
        Money::new(state.pool_state.balances.iter().sum(), state.base_currency)?;

    // Realized excess spread this period, annualized on the opening pool
    // balance: interest collections less the debt coupons due, the fees paid
    // and the net charge-off. Feeds the early-amortization excess-spread test.
    let waterfall_fees: f64 = waterfall_result
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
    // Special-servicing workout and liquidation fees were taken inside the
    // pool flows, so the collections above are already net of them.
    let fees_paid = waterfall_fees + pool_flows.special_servicing_fees.amount();
    let net_charge_off = (pool_flows.default.amount() - pool_flows.recovery.amount()).max(0.0);
    let excess = pool_flows.interest.amount() - note_interest_due - waterfall_fees - net_charge_off;
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
        cash.total,
        &pool_flows,
        released_recoveries,
        principal_diverted,
        &waterfall_result,
        side_net_capture,
    )?;

    // Per-period deal accounting: end-of-period balances, the cash that moved
    // and the coverage tests exactly as the executor evaluated them (single
    // source: the waterfall's own results).
    let pool_factor = if state.original_pool_balance.amount() > 0.0 {
        state.pool_outstanding.amount() / state.original_pool_balance.amount()
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
            servicer_advances_outstanding: Money::new(
                state.servicer_advances_outstanding(),
                state.base_currency,
            )?,
            excess_spread: state.excess_spread_history.last().copied().unwrap_or(0.0),
            coverage_tests,
        });

    state
        .reserve_balance_path
        .push((pay_date, state.reserve_balance));

    Ok(())
}

/// Apply the scheduled draws due on `pay_date` and, while revolving, the
/// borrowing-base re-advance, to the note balances and the period's cash.
fn apply_tranche_draws(
    state: &mut SimulationState,
    instrument: &StructuredCredit,
    period: &SimulationPeriod,
    pay_date: Date,
) -> Result<()> {
    let mut draws: Vec<(String, Money)> = Vec::new();
    while let Some(draw) = instrument.tranche_draws.get(state.next_tranche_draw) {
        if draw.date > pay_date {
            break;
        }
        draws.push((draw.tranche_id.clone(), draw.amount));
        state.next_tranche_draw += 1;
    }
    if let Some(readvance) = instrument.tranche_readvance.as_ref() {
        let revolving = !state.early_amortization_triggered
            && !period.redemption
            && state
                .pool
                .reinvestment_period
                .as_ref()
                .is_some_and(|window| window.is_active && pay_date <= window.end_date);
        if revolving {
            if let Some(rules) = instrument
                .coverage_rules
                .as_ref()
                .and_then(|rules| rules.borrowing_base.as_ref())
            {
                let unresolved_npl = state.unresolved_npl();
                let base = rules
                    .evaluate_live(
                        &state.pool,
                        Some(&state.pool_state.balances),
                        Some(state.live_collateral(&unresolved_npl)),
                    )?
                    .borrowing_base
                    .amount();
                let balance = state
                    .tranche_balances
                    .get(readvance.tranche_id.as_str())
                    .map_or(0.0, Money::amount);
                let headroom = readvance.commitment.amount().min(base) - balance;
                if headroom > 1.0 {
                    draws.push((
                        readvance.tranche_id.clone(),
                        Money::new(headroom, state.base_currency)?,
                    ));
                }
            }
        }
    }
    for (tranche_id, amount) in draws {
        if let Some(balance) = state.tranche_balances.get_mut(&tranche_id) {
            *balance = balance.checked_add(amount)?;
        }
        state.undistributed_principal = state.undistributed_principal.checked_add(amount)?;
        state.tranche_draws.push((tranche_id, pay_date, amount));
    }
    Ok(())
}
