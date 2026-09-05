use super::*;

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
    // Seasoning for PSA/SDA ramps = collateral age, not deal age: the
    // pool's balance-weighted average loan age at closing (WALA, derived
    // from asset acquisition dates) plus the months elapsed since closing.
    // Seasoned collateral therefore enters the ramp partway up instead of
    // restarting at month zero on the deal closing date.
    let seasoning_months = state.pool_wala_months + state.closing_date.months_until(pay_date);

    // Capture period start before updating prev_date (for accrual calculations)
    let period_start = state.prev_date.unwrap_or(state.closing_date);

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

    // Early amortization (master-trust style): once cumulative losses reach the
    // configured threshold, the revolving period ends immediately and the deal
    // begins amortizing, regardless of the scheduled revolving-period end.
    let early_amortization = instrument
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
            loss_fraction >= spec.max_cumulative_loss_pct
        });

    // Reinvestment/revolving logic -- determined before pool flows so
    // reconciliation can snap pool_outstanding to the correct pre-flow asset
    // balances. The revolving period also ends early on an early-amortization
    // event.
    let is_reinvestment_active = !early_amortization
        && state
            .pool
            .reinvestment_period
            .as_ref()
            .is_some_and(|period| pay_date <= period.end_date);

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
    let principal_diverted = is_reinvestment_active || is_accumulating;

    // Reconciliation: When reinvestment transitions from active → inactive,
    // snap pool_outstanding to the actual sum of asset balances BEFORE this
    // period's flows are applied. During the reinvestment period,
    // pool_outstanding is reduced only by defaults (gross), which can cause
    // it to diverge from the true sum of asset-level balances (e.g. due to
    // matured assets, rounding, or partial defaults). This one-time
    // reconciliation eliminates the phantom balance at the transition point.
    //
    // Must happen before calculate_pool_flows so that Step 4's normal
    // subtraction of this period's flows is applied to the correct base.
    if state.was_reinvestment_active && !is_reinvestment_active {
        let actual_sum: f64 = state.pool_state.balances.iter().sum();
        state.pool_outstanding = Money::new(actual_sum.max(0.0), state.base_currency)?;
    }
    state.was_reinvestment_active = is_reinvestment_active;

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

    state.prev_date = Some(pay_date);

    // ── Reinvestment recycling ───────────────────────────────────────
    // During the reinvestment period, collected principal (scheduled
    // amortization + prepayments) is NOT distributed to the tranches — it is
    // recycled by the manager into new collateral. Without this recycle the
    // asset-level balances shrink every period (calculate_pool_flows debits
    // scheduled principal and prepayments), the collected principal is never
    // distributed (Step 3 excludes it from the waterfall), and at the
    // reinvestment-end reconciliation `pool_outstanding` is snapped DOWN to
    // the shrunken asset sum — so the recycled principal silently vanishes
    // and never generates future cashflows.
    //
    // Recycle by crediting the collected principal back onto the surviving
    // performing assets (pro-rata to their post-flow balances). This holds
    // the pool balance flat net of defaults, so the recycled cash continues
    // to throw off interest, principal and defaults in later periods.
    // Recoveries are CASH and are never recycled — they flow to the waterfall.
    if is_reinvestment_active {
        let recyclable = pool_flows.scheduled_principal.amount().max(0.0)
            + pool_flows.prepayment.amount().max(0.0);
        if recyclable.is_finite() && recyclable > 0.0 {
            // Reinvestment price (`behavior_overrides.reinvestment_price`, % of
            // par): collected principal buys new collateral at this price, so
            // $1 of cash acquires `1 / price_fraction` of par. Defaults to par
            // (100%) when unset, which reproduces the prior 1:1 recycling.
            // Clamped to a sane (1%, 200%] band.
            let price_pct = instrument
                .behavior_overrides
                .reinvestment_price
                .filter(|p| p.is_finite() && *p > 0.0)
                .unwrap_or(100.0)
                .clamp(1.0, 200.0);
            recycle_reinvestment_principal(state, recyclable, price_pct / 100.0);
        }
    }

    // Add new recoveries to the lag queue
    state
        .recovery_queue
        .add_recovery(pay_date, pool_flows.recovery);

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

    if state.cumulative_realized_loss > WRITEDOWN_DE_MINIMIS
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
        let mut remaining_loss = (state.cumulative_realized_loss - already_allocated).max(0.0);
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
                .map(|t| t.original_balance.amount())
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

    // ── Step 3: Prepare waterfall inputs ─────────────────────────────
    // Total principal from pool (scheduled + prepayment)
    let total_principal_from_pool = pool_flows
        .scheduled_principal
        .checked_add(pool_flows.prepayment)?;

    // During reinvestment, principal collections are reinvested into new assets;
    // during controlled accumulation they are held in the funding account. Either
    // way pool principal is withheld from the waterfall (`principal_diverted`).
    // Recoveries are CASH and always flow through the waterfall.
    let mut principal_available_for_waterfall = if principal_diverted {
        released_recoveries
    } else {
        total_principal_from_pool.checked_add(released_recoveries)?
    };

    let mut total_cash_for_waterfall = pool_flows
        .interest
        .checked_add(principal_available_for_waterfall)?;

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

        let interest_avail = pool_flows.interest.amount();
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
        // N4: size the draw against the cash the waterfall will ACTUALLY have,
        // not pool interest alone.
        //
        // This waterfall is fungible: a single sequential sweep applies all
        // available cash — interest, principal collections and released
        // recoveries alike — to the fee, interest and principal tiers in order.
        // Measuring the shortfall against interest only therefore drew the
        // reserve in periods with no genuine funding gap: with interest 80,
        // fees + note interest due 90 and principal collections 200, the old
        // measure drew 10 even though the waterfall would have covered the
        // claim comfortably. The drawn enhancement then entered at the top and
        // the marginal cash landed at the first unsatisfied claim — extra
        // principal paydown, or equity residual once principal was retired.
        //
        // Depleting credit enhancement in unstressed periods understates
        // protection in the stressed ones it was funded for, which is the
        // opposite of what a reserve is for. `total_cash_for_waterfall` already
        // reflects this period's excess-spread capture or draw, so it is the
        // right base.
        let shortfall = (debt_interest_due - total_cash_for_waterfall.amount()).max(0.0);
        let draw = shortfall.min(state.reserve_balance.amount()).max(0.0);
        if draw > 0.0 {
            let draw_money = Money::new(draw, state.base_currency)?;
            state.reserve_balance = state.reserve_balance.checked_sub(draw_money)?;
            total_cash_for_waterfall = total_cash_for_waterfall.checked_add(draw_money)?;
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

    // OC numerator uses end-of-period collateral. `pool_outstanding` is not
    // decremented until Step 6, and coverage tests add `principal_collections`,
    // so BOP would overstate the numerator. Net principal + defaults here to
    // match the balance the tranches are secured by:
    //
    //     N = B_end + cash = B_start − defaults + recoveries
    let coverage_test_pool_balance = state
        .pool_outstanding
        .checked_sub(total_principal_from_pool)?
        .checked_sub(pool_flows.default)?;
    let coverage_test_pool_balance = if coverage_test_pool_balance.amount() < 0.0 {
        Money::from((0_i64, state.base_currency))
    } else {
        coverage_test_pool_balance
    };

    // N2: principal held in the controlled-accumulation funding account is
    // still collateral for the notes and must count toward the OC numerator.
    //
    // During accumulation the collected principal leaves the asset balances
    // (Step 1 amortizes them) and is diverted into the account, so it appears
    // in NEITHER the collateral term NOR the cash term of the test. The OC
    // ratio therefore decayed by exactly the accumulated amount each period
    // while the denominator stayed flat — a pool of 1000 against 900 of rated
    // notes with a 1.05 trigger reads 1.044 after only 60 accumulates, a
    // breach that does not exist.
    //
    // That spurious breach then does real damage: the diverted pass sets
    // principal targets to zero, overriding the accumulation lockout whose
    // whole job is to hold investor balances flat. At the bullet date the
    // account lands in `principal_collections` and the ratio snaps back,
    // giving a sawtooth OC path.
    //
    // Real indentures count principal-collection-account cash in par-value
    // tests, so including it is also the market convention.
    let coverage_test_pool_balance =
        coverage_test_pool_balance.checked_add(state.principal_funding_account)?;

    let waterfall_context =
        crate::instruments::fixed_income::structured_credit::pricing::waterfall::WaterfallContext {
            available_cash: total_cash_for_waterfall,
            interest_collections: pool_flows.interest,
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
            recovery_proceeds: released_recoveries,
            floating_rate_shift: state.floating_rate_shift,
        };

    let waterfall_result =
        crate::instruments::fixed_income::structured_credit::pricing::waterfall::execute_waterfall(
            &period_waterfall,
            state.tranches,
            state.pool,
            waterfall_context,
        )?;

    // Update reserve balance from waterfall distributions to ReserveAccount recipients.
    for (recipient, amount) in &waterfall_result.distributions {
        if let RecipientType::ReserveAccount(_) = recipient {
            state.reserve_balance = state.reserve_balance.checked_add(*amount)?;
        }
    }

    // ── Step 5: Record flows and update balances ─────────────────────
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
        // Equity keeps the legacy metadata-coupon path: equity is paid via
        // `RecipientType::Equity`/`ResidualCash` (never a tranche-keyed
        // interest recipient), and its recorded interest-vs-principal split is
        // a reporting convention, not a waterfall claim.
        let current_interest_due = if tranche.seniority == TrancheSeniority::Equity {
            Money::new(
                tranche_period_interest_due(
                    tranche,
                    current_balance.amount(),
                    TrancheAccrualDates {
                        start: period_start,
                        payment: pay_date,
                        valuation: as_of,
                    },
                    context,
                    0.0,
                    false,
                    state.floating_rate_shift,
                )?,
                state.base_currency,
            )?
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
        let interest_paid = if interest_portion.amount() >= total_interest_claim.amount() {
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
        let current_interest_shortfall = Money::new(
            (current_interest_due.amount() - current_interest_paid.amount()).max(0.0),
            state.base_currency,
        )?;

        // Anything the waterfall did not classify as interest retires notional.
        // (`interest_paid` can be below `interest_portion` when the claim is
        // smaller than what the interest tiers paid — that excess is principal.)
        let principal_payment = payment_received
            .checked_sub(interest_paid)
            .unwrap_or(Money::from((0_i64, state.base_currency)));

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

    // ── Step 6: Update pool balance ──────────────────────────────────
    if is_reinvestment_active {
        // During reinvestment, principal is recycled into new assets.
        // AssetPool balance drops only by defaults (gross).
        state.pool_outstanding = state.pool_outstanding.checked_sub(pool_flows.default)?;
    } else {
        // After reinvestment, all principal reductions hit pool balance.
        state.pool_outstanding = state
            .pool_outstanding
            .checked_sub(total_principal_from_pool)?
            .checked_sub(pool_flows.default)?;
    }

    // Numerical cleanup: avoid tiny negative residual balances like -0.00
    // after repeated principal/default arithmetic.
    if state.pool_outstanding.amount() < 0.0
        && state.pool_outstanding.amount().abs() <= WRITEDOWN_DE_MINIMIS
    {
        state.pool_outstanding = Money::from((0_i64, state.base_currency));
    }

    // Pool cash must equal recipient distributions plus residual cash and net
    // side-account capture. Reserve draws are negative capture; controlled-
    // accumulation releases add cash back to the waterfall.
    let side_net_capture = spread_net_capture + reserve_net_capture - funding_net_release;
    assert_cash_conserved(
        total_cash_for_waterfall,
        &pool_flows,
        released_recoveries,
        principal_diverted,
        &waterfall_result,
        side_net_capture,
    )?;

    Ok(())
}
