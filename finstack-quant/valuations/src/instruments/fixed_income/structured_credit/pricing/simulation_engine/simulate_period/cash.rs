//! The period's cash before the waterfall: opening accruals, the recovery
//! queue, the debt-interest claim and the side accounts (excess spread,
//! reserve, controlled-accumulation funding) that add to or take from it.

use super::*;

/// Preserve the opening claim before this period's projected writedowns or
/// distributions. Paid coupons can include prior deferrals and cannot be
/// used to reconstruct a buyer's current-period accrued interest.
///
/// # Arguments
///
/// * `state` - Simulation state whose tranche results record the accrual periods.
/// * `inputs` - The period's deal, market, dates and interest-claim caps.
pub(super) fn record_opening_accruals(
    state: &mut SimulationState,
    inputs: &PeriodInputs<'_>,
) -> Result<()> {
    let (period, context, claim_caps) = (inputs.period, inputs.context, inputs.claim_caps);
    let (pay_date, as_of) = (period.payment, period.valuation);
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
    Ok(())
}

/// Queue this period's recoveries: balloon workouts settle on their own
/// dates, the rest of the period's defaults after the deal's recovery lag.
///
/// # Arguments
///
/// * `state` - Simulation state holding the recovery queue.
/// * `pool_flows` - The period's pool flows (defaults, recoveries, workouts).
/// * `pay_date` - Payment date the lagged recoveries are queued from.
pub(super) fn queue_recoveries(
    state: &mut SimulationState,
    pool_flows: &PoolFlows,
    pay_date: Date,
) -> Result<()> {
    let mut workout_recovery = Money::from((0_i64, state.base_currency));
    let mut workout_par = Money::from((0_i64, state.base_currency));
    for (date, recovery, par) in &pool_flows.workout_claims {
        state.recovery_queue.add_recovery(*date, *recovery, *par);
        workout_recovery = workout_recovery.checked_add(*recovery)?;
        workout_par = workout_par.checked_add(*par)?;
    }
    state.recovery_queue.add_recovery(
        pay_date,
        pool_flows.recovery.checked_sub(workout_recovery)?,
        pool_flows.default.checked_sub(workout_par)?,
    );
    Ok(())
}

/// Cash entering the waterfall, split into its interest and principal
/// accounts; `total` is their sum.
pub(super) struct WaterfallCash {
    /// Interest proceeds.
    pub(super) interest: Money,
    /// Principal proceeds.
    pub(super) principal: Money,
    /// Total available cash.
    pub(super) total: Money,
}

/// Excess-spread (spread-account) capture/draw, applied to the cash entering
/// the waterfall. Capturing *here* — before the single sequential waterfall
/// can sweep surplus interest into senior principal — is what lets the
/// account fund from excess interest mid-deal and later draw to cover debt
/// interest shortfalls. No-op (identity) when no `excess_spread` is set.
/// `spread_net_capture` is the net cash diverted into the account this period
/// (negative when drawing), reconciled by the cash-conservation check.
/// Total interest the waterfall owes debt (non-equity) tranches this period:
/// the current-period coupon (shared helper, so the surplus measured here
/// matches what Step 5 records, including the live AFC cap) PLUS each
/// tranche's outstanding non-PIK deferred interest — a senior claim the
/// waterfall must also satisfy. Omitting the deferred piece would let the
/// excess-spread account capture interest it should instead leave behind to
/// cure that shortfall.
///
/// Shared by excess-spread capture/draw and reserve-account draw (same
/// shortfall). Computed only when one of those features is live.
///
/// Zero unless an excess-spread rule or a funded reserve needs it.
///
/// # Arguments
///
/// * `state` - Simulation state with the note balances and deferred interest.
/// * `inputs` - The period's deal, base waterfall, market, dates and claim caps.
/// * `pool_interest` - The period's pool interest collections.
/// * `special_serviced_open` - Special-servicing flags at the period's opening.
pub(super) fn debt_interest_due(
    state: &SimulationState,
    inputs: &PeriodInputs<'_>,
    pool_interest: Money,
    special_serviced_open: &[bool],
) -> Result<f64> {
    let (instrument, waterfall, context, claim_caps) = (
        inputs.instrument,
        inputs.waterfall,
        inputs.context,
        inputs.claim_caps,
    );
    let (period_start, pay_date, as_of) = (
        inputs.period_start,
        inputs.period.payment,
        inputs.period.valuation,
    );
    let needs_interest_due = instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.excess_spread.as_ref())
        .is_some()
        || state.reserve_balance.amount() > 0.0;
    // N1: senior fees are part of what the waterfall owes AHEAD of the notes.
    //
    // `debt_interest_due` below was written for the reserve/excess-spread work
    // BEFORE the fee tier existed, and was never revisited
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
            // The BASE waterfall: the period rules only rewrite AFC caps
            // on tranche interest, step-down/shifting weights on principal
            // tiers, and accumulation lockout targets — it never touches fee
            // tiers, so the fee accrual is identical either way.
            waterfall,
            state.tranches,
            &tranche_index,
            crate::instruments::fixed_income::structured_credit::pricing::waterfall::SeniorFeeInputs {
                available: pool_interest,
                tranche_balances: Some(&state.tranche_balances),
                deferred_interest: Some(&state.deferred_interest),
                pool_balance: state.pool_outstanding,
                special_serviced_balance:
                    crate::instruments::fixed_income::structured_credit::pricing::waterfall::special_serviced_balance(
                        &state.pool,
                        Some(&state.pool_state.balances),
                        Some(special_serviced_open),
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
    Ok(debt_interest_due)
}

/// Excess-spread (spread-account) capture or draw on the waterfall cash:
/// surplus interest above `debt_interest_due` is captured up to the target
/// balance; a shortfall is drawn from the account.
///
/// # Arguments
///
/// * `state` - Simulation state holding the spread account.
/// * `instrument` - Deal whose excess-spread rule applies.
/// * `cash` - Waterfall cash adjusted in place.
/// * `debt_interest_due` - Senior fees plus note interest due this period.
///
/// # Returns
///
/// The net cash moved into the account (negative when drawing).
pub(super) fn apply_excess_spread(
    state: &mut SimulationState,
    instrument: &StructuredCredit,
    cash: &mut WaterfallCash,
    debt_interest_due: f64,
) -> Result<f64> {
    let mut spread_net_capture = 0.0_f64;
    if let Some(es) = instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.excess_spread.as_ref())
    {
        // Snapshot the account balance before any capture/draw, to independently
        // reconcile the recorded net capture against the actual balance move.
        let spread_before = state.spread_account.amount();

        let interest_avail = cash.interest.amount();
        if interest_avail > debt_interest_due {
            // Capture surplus interest into the account, up to the target.
            let room = (es.target_balance.amount() - state.spread_account.amount()).max(0.0);
            let capture = (interest_avail - debt_interest_due).min(room).max(0.0);
            // `capture <= interest_avail <= cash.total`, so the
            // withdrawal is non-negative; assert the floor is a no-op so a future
            // divergence between this surplus check and the waterfall cash cannot
            // silently leak cash into (or out of) the account.
            let net_after = cash.total.amount() - capture;
            debug_assert!(
                net_after >= -WRITEDOWN_DE_MINIMIS,
                "excess-spread capture {capture} overdrew waterfall cash {}",
                cash.total.amount()
            );
            state.spread_account = state
                .spread_account
                .checked_add(Money::new(capture, state.base_currency)?)?;
            cash.total = Money::new(net_after.max(0.0), state.base_currency)?;
            spread_net_capture = capture;
        } else {
            // Draw from the account to cover the interest shortfall (bounded by
            // the account balance, so the subtraction stays non-negative).
            let draw = (debt_interest_due - interest_avail)
                .min(state.spread_account.amount())
                .max(0.0);
            let draw_money = Money::new(draw, state.base_currency)?;
            state.spread_account = state.spread_account.checked_sub(draw_money)?;
            cash.total = cash.total.checked_add(draw_money)?;
            spread_net_capture = -draw;
        }
        cash.interest = cash
            .interest
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
    Ok(spread_net_capture)
}

/// Reserve account: release the balance above this period's target into
/// interest proceeds when the rules say so, then draw to cover any remaining
/// debt-interest shortfall.
///
/// # Arguments
///
/// * `state` - Simulation state holding the reserve balance.
/// * `instrument` - Deal whose reserve rules apply.
/// * `cash` - Waterfall cash adjusted in place.
/// * `debt_interest_due` - Senior fees plus note interest due this period.
///
/// # Returns
///
/// The period's reserve target (if configured) and the net cash moved into
/// the reserve (negative on release or draw).
pub(super) fn apply_reserve(
    state: &mut SimulationState,
    instrument: &StructuredCredit,
    cash: &mut WaterfallCash,
    debt_interest_due: f64,
) -> Result<(Option<f64>, f64)> {
    // Reserve rules: this period's target from the live pool; any balance
    // above it is released into interest proceeds when the rules say so.
    let reserve_rules = instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.reserve.as_ref());
    let reserve_target = reserve_rules.map(|spec| {
        spec.target.resolve(
            state.pool_outstanding.amount() + state.principal_funding_account.amount(),
            state.original_pool_balance.amount(),
        )
    });
    let mut reserve_net_capture = 0.0_f64;
    if let (Some(spec), Some(target)) = (reserve_rules, reserve_target) {
        let excess = state.reserve_balance.amount() - target;
        if spec.release_excess && excess > WRITEDOWN_DE_MINIMIS {
            let release = Money::new(excess, state.base_currency)?;
            state.reserve_balance = state.reserve_balance.checked_sub(release)?;
            cash.total = cash.total.checked_add(release)?;
            cash.interest = cash.interest.checked_add(release)?;
            reserve_net_capture -= excess;
        }
    }

    // Reserve draw: credit enhancement drawn after excess-spread capture to
    // cover remaining debt-interest shortfall. Bounded by shortfall and
    // balance; `reserve_net_capture` (negative on draw) feeds cash conservation.
    if state.reserve_balance.amount() > 0.0 {
        // A funded reserve covers the interest account's remaining fee/coupon
        // shortfall. Principal collections remain restricted to capital uses.
        let shortfall = (debt_interest_due - cash.interest.amount()).max(0.0);
        let draw = shortfall.min(state.reserve_balance.amount()).max(0.0);
        if draw > 0.0 {
            let draw_money = Money::new(draw, state.base_currency)?;
            state.reserve_balance = state.reserve_balance.checked_sub(draw_money)?;
            cash.total = cash.total.checked_add(draw_money)?;
            cash.interest = cash.interest.checked_add(draw_money)?;
            reserve_net_capture -= draw;
        }
    }
    Ok((reserve_target, reserve_net_capture))
}

/// Controlled-accumulation funding account. During accumulation, divert this
/// period's pool principal into the account (kept out of the waterfall above
/// via `principal_diverted`, so investor balances stay flat). At the bullet
/// date, release the whole account into the waterfall as principal. Applied
/// after the excess-spread block so the spread account never captures the
/// bullet principal as if it were surplus interest. `funding_net_release`
/// (cash added back from the account) reconciles the cash-conservation check.
///
/// # Arguments
///
/// * `state` - Simulation state holding the funding account.
/// * `spec` - Controlled-accumulation rule, if configured.
/// * `phase` - Whether the period accumulates and whether early amortization is on.
/// * `pay_date` - Payment date compared with the bullet date.
/// * `pool_principal` - The period's pool principal (captured while accumulating).
/// * `cash` - Waterfall cash adjusted in place.
///
/// # Returns
///
/// The cash released from the account into the waterfall.
pub(super) fn apply_funding_account(
    state: &mut SimulationState,
    spec: Option<
        &crate::instruments::fixed_income::structured_credit::types::ControlledAccumulationSpec,
    >,
    phase: AccumulationPhase,
    pay_date: Date,
    pool_principal: Money,
    cash: &mut WaterfallCash,
) -> Result<f64> {
    let AccumulationPhase {
        is_accumulating,
        early_amortization,
    } = phase;
    let mut funding_net_release = 0.0_f64;
    if let Some(spec) = spec {
        if is_accumulating {
            let captured = pool_principal.amount().max(0.0);
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
            cash.principal = cash.principal.checked_add(release)?;
            cash.total = cash.total.checked_add(release)?;
        }
    }
    Ok(funding_net_release)
}

/// Accumulation flags of the period.
#[derive(Clone, Copy)]
pub(super) struct AccumulationPhase {
    /// Pool principal is being captured into the funding account.
    pub(super) is_accumulating: bool,
    /// Early amortization is in effect.
    pub(super) early_amortization: bool,
}
