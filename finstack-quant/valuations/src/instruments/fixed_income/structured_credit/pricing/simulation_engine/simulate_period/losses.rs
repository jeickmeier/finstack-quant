//! Realized-loss allocation and the loss-driven period tests.

use super::*;

/// The loss-allocation policy decides whether realized loss reaches the
/// note balances now (`WriteDown`: RMBS/CMBS realized-loss allocation) or
/// only as unpaid principal at legal final (`ParPreserving`: CLO/ABS notes
/// carry par, keep accruing their full coupon ahead of the residual, and
/// the OC tests do the de-levering). The cumulative realized loss still drives
/// every loss-based trigger under both policies.
///
/// Under `WriteDown` the cumulative realized loss not yet allocated is
/// written off the notes bottom-up (each capped at its balance, the excess
/// cascading upward) before the waterfall runs.
///
/// # Arguments
///
/// * `state` - Simulation state whose note balances and results are written down.
/// * `instrument` - Deal whose loss-allocation policy applies.
/// * `pay_date` - Payment date the write-downs are dated.
pub(super) fn allocate_realized_loss(
    state: &mut SimulationState,
    instrument: &StructuredCredit,
    pay_date: Date,
) -> Result<()> {
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
    Ok(())
}

/// Recovery value of defaulted collateral whose cash has not yet arrived:
/// the OC tests carry it as collateral (CLO convention: defaulted
/// obligations at their assumed recovery), so a default costs the test its
/// expected loss rather than the whole defaulted par.
/// Defaulted collateral is carried at its modeled recovery value by
/// default; a `MarketValue` rule carries the defaulted par at that price.
///
/// # Arguments
///
/// * `state` - Simulation state holding the recovery queue.
/// * `waterfall` - Base waterfall whose coverage rules set the valuation.
pub(super) fn defaulted_collateral_value(
    state: &SimulationState,
    waterfall: &Waterfall,
) -> Result<Money> {
    Ok(match waterfall
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
    })
}

/// Early amortization (master-trust style): once cumulative losses reach the
/// configured threshold, the revolving period ends immediately and the deal
/// begins amortizing, regardless of the scheduled revolving-period end.
/// Either test is an event: once it fires the revolving period stays
/// closed. The excess-spread test reads the three-period trailing average
/// of the annualized excess spread realized so far.
///
/// # Arguments
///
/// * `state` - Simulation state with the cumulative loss and excess-spread history.
/// * `instrument` - Deal whose early-amortization rule is tested.
pub(super) fn early_amortization_event(
    state: &SimulationState,
    instrument: &StructuredCredit,
) -> bool {
    instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.early_amortization.as_ref())
        .is_some_and(|spec| {
            let denom = state.original_pool_balance.amount();
            let loss_fraction = if denom > 0.0 {
                state.cumulative_realized_loss / denom
            } else {
                0.0
            };
            let spread_breached = spec.min_excess_spread_3m.is_some_and(|floor| {
                let history = &state.excess_spread_history;
                history.len() >= 3 && history[history.len() - 3..].iter().sum::<f64>() / 3.0 < floor
            });
            spec.max_cumulative_loss
                .is_some_and(|max_loss| loss_fraction >= max_loss)
                || spread_breached
        })
}
