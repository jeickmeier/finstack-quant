use super::*;

/// Per-period cash-conservation invariant (debug/test builds only).
///
/// Verifies two identities for one payment period:
///
/// 1. **Input identity** — the cash handed to the waterfall equals the pool
///    cash that is actually distributable this period:
///    `total_cash_for_waterfall = interest + released_recoveries`
///    (`+ scheduled_principal + prepayment` when reinvestment is inactive;
///    during reinvestment that principal is recycled into collateral, not
///    distributed).
///
/// 2. **Output identity** — the waterfall conserves cash:
///    `Σ distributions + remaining_cash = total_available`.
///
/// Compiled out entirely in release builds (`debug_assert!`), so there is no
/// hot-path cost; it exists to fail loudly in tests and debug runs if a future
/// change breaks the engine's cash accounting.
#[inline]
pub(super) fn assert_cash_conserved(
    total_cash_for_waterfall: Money,
    pool_flows: &PoolFlows,
    released_recoveries: Money,
    principal_diverted: bool,
    waterfall_result: &WaterfallDistribution,
    side_net_capture: f64,
) -> Result<()> {
    // SC-m13: this runs in RELEASE builds, not only under `debug_assertions`.
    //
    // Compiling it out of production runs would leave the one invariant that
    // catches a cash-accounting regression — the waterfall neither creating
    // nor destroying cash — checked only in tests, and cash has vanished
    // through at least three sinks before (the reserve sink SC-C07, the
    // cleanup-call excess SC-M22, and the spread-account sink N7); a
    // silently-violated conservation identity corrupts every tranche
    // cashflow and PV downstream with no diagnostic.
    //
    // The cost is a handful of float operations per payment period against a
    // full waterfall execution, which is not measurable. A violation is now a
    // hard error naming the discrepancy rather than a wrong number.

    // Tolerance scales with deal size: penny-safe pro-rata allocation in the
    // waterfall rounds to the currency's smallest unit per recipient.
    let tol = (total_cash_for_waterfall.amount().abs() * 1e-9).max(1.0);

    // Identity 1: input to the waterfall == distributable pool cash, net of any
    // cash diverted into (or supplied from) the side accounts (excess-spread and
    // controlled-accumulation funding). When pool principal is diverted (an
    // active revolving period recycles it, or controlled accumulation holds it)
    // it is not part of this period's distributable cash.
    let expected_input = if principal_diverted {
        pool_flows.interest.amount() + released_recoveries.amount()
    } else {
        pool_flows.interest.amount()
            + pool_flows.scheduled_principal.amount()
            + pool_flows.prepayment.amount()
            + released_recoveries.amount()
    } - side_net_capture;
    if (total_cash_for_waterfall.amount() - expected_input).abs() > tol {
        return Err(finstack_quant_core::Error::Validation(format!(
            "cash-conservation (input): waterfall received {} but distributable \
         pool cash is {} (interest={}, scheduled={}, prepay={}, recoveries={}, \
         principal_diverted={})",
            total_cash_for_waterfall.amount(),
            expected_input,
            pool_flows.interest.amount(),
            pool_flows.scheduled_principal.amount(),
            pool_flows.prepayment.amount(),
            released_recoveries.amount(),
            principal_diverted,
        )));
    }

    // Identity 2: the waterfall neither creates nor destroys cash.
    let distributed: f64 = waterfall_result
        .distributions
        .values()
        .map(|m| m.amount())
        .sum();
    let accounted = distributed + waterfall_result.remaining_cash.amount();
    if (accounted - waterfall_result.total_available.amount()).abs() > tol {
        return Err(finstack_quant_core::Error::Validation(format!(
            "cash-conservation (output): waterfall distributed {} + residual {} = \
         {} but had {} available",
            distributed,
            waterfall_result.remaining_cash.amount(),
            accounted,
            waterfall_result.total_available.amount(),
        )));
    }

    Ok(())
}

/// Recycle reinvestment-period principal back into the surviving pool.
///
/// During the reinvestment period, collected scheduled principal and
/// prepayments are reinvested by the manager into new collateral rather than
/// distributed to the tranches. This helper models that by crediting the
/// `recyclable` cash onto the still-performing assets (those that are not
/// defaulted and carry a positive balance), pro-rata to their current
/// balances. The net effect is that the pool balance stays flat net of
/// defaults, so the recycled principal continues to generate interest,
/// scheduled principal and defaults in subsequent periods instead of silently
/// vanishing at the reinvestment-end reconciliation.
///
/// If no performing assets remain (the whole pool has defaulted/amortized),
/// the returned cash-spent amount is zero and principal remains available to
/// the waterfall. Successful purchases return the cash actually spent.
///
/// `price_fraction` is the reinvestment price as a fraction of par (e.g. `0.97`
/// for a 97-price). Reinvesting `recyclable` cash at a discount buys
/// `recyclable / price_fraction` of par, so a sub-par price builds par (and the
/// extra interest-earning collateral that benefits the residual/equity);
/// `1.0` reproduces 1:1 par recycling.
pub(super) fn recycle_reinvestment_principal(
    state: &mut SimulationState,
    recyclable: Money,
    price_fraction: f64,
    payment_date: Date,
    market: &MarketContext,
) -> Result<Money> {
    if !price_fraction.is_finite() || price_fraction <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "reinvestment price must be finite and positive".into(),
        ));
    }
    let Some(period) = state.pool.reinvestment_period.as_ref() else {
        return Ok(Money::from((0_i64, state.base_currency)));
    };
    let criteria = &period.criteria;
    if !criteria.max_price.is_finite()
        || criteria.max_price <= 0.0
        || !criteria.min_yield.is_finite()
    {
        return Err(finstack_quant_core::Error::Validation(
            "invalid reinvestment price/yield criteria".into(),
        ));
    }
    if price_fraction * 100.0 > criteria.max_price || recyclable.amount() <= 0.0 {
        return Ok(Money::from((0_i64, state.base_currency)));
    }
    // Replacement collateral keeps the surviving pool's composition and
    // contractual maturities. Requiring every component to be eligible avoids
    // silently changing credit-quality weights or WAL by selective purchases.
    for i in 0..state.pool_state.len() {
        if state.pool_state.is_defaulted[i] || state.pool_state.balances[i] <= 0.0 {
            continue;
        }
        let coupon = if let Some(curve_idx) = state.pool_state.curve_indices[i] {
            collateral_asset_rate_for_period(
                market
                    .get_forward(&state.pool_state.unique_curves[curve_idx])?
                    .as_ref(),
                market,
                payment_date,
                state.pool_state.rates[i],
                state.pool_state.spread_bp[i],
                state.floating_rate_shift,
            )?
        } else {
            state.pool_state.rates[i]
        };
        if state.pool_state.maturities[i] <= payment_date
            || coupon / price_fraction < criteria.min_yield
        {
            return Ok(Money::from((0_i64, state.base_currency)));
        }
    }
    let performing_total: f64 = state
        .pool_state
        .is_defaulted
        .iter()
        .zip(state.pool_state.balances.iter())
        .filter(|(defaulted, balance)| !**defaulted && **balance > 0.0)
        .map(|(_, balance)| *balance)
        .sum();

    if performing_total <= 0.0 {
        // No surviving collateral to reinvest into — recycle is a no-op.
        return Ok(Money::from((0_i64, state.base_currency)));
    }

    // Par acquired by spending `recyclable` cash at the reinvestment price.
    let par_acquired = par_acquired_at_price(recyclable.amount(), price_fraction);

    let n = state.pool_state.len();
    for i in 0..n {
        if state.pool_state.is_defaulted[i] {
            continue;
        }
        let balance = state.pool_state.balances[i];
        if balance <= 0.0 {
            continue;
        }
        let share = balance / performing_total;
        state.pool_state.balances[i] = balance + par_acquired * share;
        if let Some(payment) = &mut state.pool_state.level_payments[i] {
            *payment *= state.pool_state.balances[i] / balance;
        }
    }
    Ok(recyclable)
}

/// Par acquired when reinvesting `cash` at `price_fraction` (a fraction of par).
///
/// Buying at a discount price `p < 1` acquires `cash / p` of par (par build);
/// at par (`p == 1`) it is `cash`. The caller validates a finite positive price.
#[inline]
pub(super) fn par_acquired_at_price(cash: f64, price_fraction: f64) -> f64 {
    cash / price_fraction
}
