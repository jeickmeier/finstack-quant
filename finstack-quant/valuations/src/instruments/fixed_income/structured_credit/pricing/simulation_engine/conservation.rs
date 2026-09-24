use super::*;
use crate::instruments::fixed_income::structured_credit::types::{
    AssetType, PoolAsset, ReinvestmentAssumptions,
};
use finstack_quant_core::dates::DayCount;

/// Per-period cash-conservation invariant, checked in every build.
///
/// Verifies two identities for one payment period:
///
/// 1. **Input identity** — the cash handed to the waterfall equals the pool
///    cash that is distributable this period, net of the side accounts:
///    `total_cash_for_waterfall = interest + released_recoveries
///    (+ scheduled_principal + prepayment unless principal is diverted into
///    reinvestment or accumulation) − side_net_capture`, where
///    `side_net_capture` is the net cash moved into the excess-spread,
///    reserve, funding and carried-cash accounts (negative when they supply
///    cash, as call premia, reserve interest and hedge receipts do).
///
/// 2. **Output identity** — the waterfall conserves cash:
///    `Σ distributions + remaining_cash = total_available`.
///
/// Cash has vanished through side-account sinks before, and a violated
/// identity corrupts every tranche cashflow downstream, so a violation is a
/// hard error naming the discrepancy. The tolerance scales with deal size
/// (`max(1e-9 · cash, 1.0)`) for penny rounding in pro-rata allocation.
///
/// # Errors
///
/// Returns `Error::Validation` when either identity fails by more than the
/// tolerance.
#[inline]
pub(super) fn assert_cash_conserved(
    total_cash_for_waterfall: Money,
    pool_flows: &PoolFlows,
    released_recoveries: Money,
    principal_diverted: bool,
    waterfall_result: &WaterfallDistribution,
    side_net_capture: f64,
) -> Result<()> {
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
    if let Some(assumptions) = period.assumptions.clone() {
        let min_yield = criteria.min_yield;
        return purchase_replacement_collateral(
            state,
            recyclable,
            price_fraction,
            min_yield,
            payment_date,
            market,
            &assumptions,
        );
    }
    // Replacement collateral replicates the surviving pool's composition and
    // contractual maturities, restricted to the assets that satisfy the
    // eligibility criteria: matured rows and rows whose current yield is
    // below `min_yield` are skipped, the purchase spreads pro rata over the
    // rest, and nothing is bought when no row is eligible.
    let mut eligible = vec![false; state.pool_state.len()];
    #[allow(clippy::needless_range_loop)] // parallel per-asset vectors are indexed together
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
                state.pool_state.index_floors[i],
            )?
        } else {
            state.pool_state.rates[i]
        };
        let coupon = state.pool_state.rate_floors[i].map_or(coupon, |floor| coupon.max(floor));
        eligible[i] = state.pool_state.maturities[i] > payment_date
            && coupon / price_fraction >= criteria.min_yield;
    }
    let performing_total: f64 = eligible
        .iter()
        .zip(state.pool_state.balances.iter())
        .filter(|(eligible, _)| **eligible)
        .map(|(_, balance)| *balance)
        .sum();

    if performing_total <= 0.0 {
        // No surviving collateral to reinvest into — recycle is a no-op.
        return Ok(Money::from((0_i64, state.base_currency)));
    }

    // Par acquired by spending `recyclable` cash at the reinvestment price.
    let par_acquired = par_acquired_at_price(recyclable.amount(), price_fraction);

    let n = state.pool_state.len();
    #[allow(clippy::needless_range_loop)] // parallel per-asset vectors are indexed together
    for i in 0..n {
        if !eligible[i] {
            continue;
        }
        let balance = state.pool_state.balances[i];
        let share = balance / performing_total;
        state.pool_state.balances[i] = balance + par_acquired * share;
        if let Some(payment) = &mut state.pool_state.level_payments[i] {
            *payment *= state.pool_state.balances[i] / balance;
        }
    }
    Ok(recyclable)
}

/// Book a reinvestment purchase as a synthetic first-lien bullet row with the
/// deal's `ReinvestmentAssumptions` instead of cloning the surviving pool.
///
/// The row `REINVEST-{n}` matures `maturity_months` after `payment_date`,
/// capped at the notes' legal final, accrues ACT/360 at the index plus spread
/// (floored at `coupon_floor`) or at the spread as a fixed coupon, and is
/// bought at `price_fraction` of par. A purchase whose current yield is below
/// `min_yield` is ineligible and leaves the cash in the principal account.
fn purchase_replacement_collateral(
    state: &mut SimulationState,
    recyclable: Money,
    price_fraction: f64,
    min_yield: f64,
    payment_date: Date,
    market: &MarketContext,
    assumptions: &ReinvestmentAssumptions,
) -> Result<Money> {
    use finstack_quant_core::dates::DateExt;
    let legal_final = state
        .tranches
        .tranches
        .iter()
        .map(|tranche| tranche.maturity)
        .max()
        .unwrap_or(payment_date);
    let months = i32::try_from(assumptions.maturity_months).map_err(|_| {
        finstack_quant_core::Error::Validation(format!(
            "reinvestment maturity of {} months is out of range",
            assumptions.maturity_months
        ))
    })?;
    let maturity = payment_date.add_months(months).min(legal_final);
    if maturity <= payment_date {
        return Ok(Money::from((0_i64, state.base_currency)));
    }
    let spread = assumptions.spread_bp / 10_000.0;
    let coupon = match &assumptions.index_id {
        Some(index) => collateral_asset_rate_for_period(
            market.get_forward(index)?.as_ref(),
            market,
            payment_date,
            spread,
            Some(assumptions.spread_bp),
            state.floating_rate_shift,
            None,
        )?,
        None => spread,
    };
    let coupon = assumptions
        .coupon_floor
        .map_or(coupon, |floor| coupon.max(floor));
    if coupon / price_fraction < min_yield {
        return Ok(Money::from((0_i64, state.base_currency)));
    }
    let par = Money::new(
        par_acquired_at_price(recyclable.amount(), price_fraction),
        state.base_currency,
    )?;
    let purchases = state
        .pool_state
        .ids
        .iter()
        .filter(|id| id.starts_with("REINVEST-"))
        .count();
    let id = format!("REINVEST-{}", purchases + 1);
    let mut asset = match &assumptions.index_id {
        Some(index) => PoolAsset::floating_rate_loan(
            id,
            par,
            index.clone(),
            assumptions.spread_bp,
            maturity,
            DayCount::Act360,
        ),
        None => PoolAsset::fixed_rate_bond(id, par, spread, maturity, DayCount::Act360),
    };
    asset.asset_type = AssetType::FirstLienLoan { industry: None };
    asset.purchase_price = Some(recyclable);
    asset.acquisition_date = Some(payment_date);
    state
        .pool_state
        .push_asset(&asset, assumptions.coupon_floor);
    state.pool.to_mut().assets.push(asset);
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
