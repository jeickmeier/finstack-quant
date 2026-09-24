use super::*;

pub(crate) fn term_rate_for_period(
    fwd: &ForwardCurve,
    context: &MarketContext,
    accrual_start: Date,
) -> Result<f64> {
    let calendar = crate::cashflow::builder::calendar::resolve_calendar_strict("weekends_only")?;
    let fixing_date = accrual_start.add_business_days(-fwd.reset_lag(), calendar)?;
    if fixing_date < fwd.base_date() {
        let series = fixings::get_fixing_series(context, fwd.id().as_str())?;
        return fixings::require_fixing_value_exact(
            Some(series),
            fwd.id().as_str(),
            fixing_date,
            fwd.base_date(),
        );
    }
    let reset_end =
        crate::instruments::fixed_income::structured_credit::utils::rate_helpers::try_tenor_to_period_end(
            fixing_date,
            fwd.tenor(),
            fwd.day_count(),
        )?;
    crate::instruments::common_impl::pricing::time::rate_between_on_dates(
        fwd,
        fixing_date,
        reset_end,
    )
}

/// Resolve an asset's all-in coupon without re-projecting an already-reset
/// period. The pool's stored `rate` is the contractual current coupon and is
/// therefore the authoritative fallback when no historical fixing series is
/// supplied for a reset before the curve base date.
pub(crate) fn collateral_asset_rate_for_period(
    fwd: &ForwardCurve,
    context: &MarketContext,
    accrual_start: Date,
    fallback_all_in_rate: f64,
    spread_bp: Option<f64>,
    rate_shift: f64,
    index_floor: Option<f64>,
) -> Result<f64> {
    let calendar = crate::cashflow::builder::calendar::resolve_calendar_strict("weekends_only")?;
    let fixing_date = accrual_start.add_business_days(-fwd.reset_lag(), calendar)?;
    let spread = spread_bp.unwrap_or(0.0) / 10_000.0;
    // The floor applies to the index before the spread (`FloatingRateSpec`
    // convention), so a floored loan pays `max(index, floor) + spread`.
    let floored = |index: f64| index_floor.map_or(index, |floor| index.max(floor));

    if fixing_date < fwd.base_date() {
        if let Ok(series) = fixings::get_fixing_series(context, fwd.id().as_str()) {
            let fixing = fixings::require_fixing_value_exact(
                Some(series),
                fwd.id().as_str(),
                fixing_date,
                fwd.base_date(),
            )?;
            // A PAST fixing is observed, not projected: the simulated path
            // cannot retroactively change it, so no shift applies.
            return Ok(floored(fixing) + spread);
        }
        return Ok(fallback_all_in_rate);
    }

    // Shift the PROJECTED forward, so a floating asset's coupon follows
    // the simulated rate path. Floored at zero — a deeply negative shift must
    // not manufacture a negative all-in coupon.
    Ok(
        (floored(term_rate_for_period(fwd, context, accrual_start)? + rate_shift) + spread)
            .max(0.0),
    )
}

/// Live collateral weighted-average coupon from the *current* pool state:
/// balance-weighted `rate` over performing (non-defaulted, positive-balance)
/// assets. Mirrors [`crate::instruments::fixed_income::structured_credit::AssetPool::weighted_avg_coupon`]
/// but on the current balances, so a net-WAC cap tracks collateral that has
/// amortized, prepaid or defaulted heterogeneously instead of being frozen at
/// closing. Returns `0.0` for an empty/exhausted performing pool.
pub(super) fn current_collateral_wac(
    state: &SimulationState,
    context: &MarketContext,
    period_start: Date,
) -> Result<f64> {
    let mut weighted = 0.0_f64;
    let mut balance = 0.0_f64;
    for i in 0..state.pool_state.len() {
        if state.pool_state.is_defaulted[i] {
            continue;
        }
        let b = state.pool_state.balances[i];
        if b <= 0.0 {
            continue;
        }
        let all_in_rate = if let Some(curve_idx) = state.pool_state.curve_indices[i] {
            let curve_id = &state.pool_state.unique_curves[curve_idx];
            let fwd = context.get_forward(curve_id)?;
            collateral_asset_rate_for_period(
                fwd.as_ref(),
                context,
                period_start,
                state.pool_state.rates[i],
                state.pool_state.spread_bp[i],
                state.floating_rate_shift,
                state.pool_state.index_floors[i],
            )?
        } else {
            state.pool_state.rates[i]
        };
        let all_in_rate =
            state.pool_state.rate_floors[i].map_or(all_in_rate, |floor| all_in_rate.max(floor));
        weighted += all_in_rate * b;
        balance += b;
    }
    if balance > 0.0 {
        Ok(weighted / balance)
    } else {
        Ok(0.0)
    }
}

/// Live available-funds cap rate: the current collateral WAC
/// ([`current_collateral_wac`]) less the AFC spec's net-WAC fee load (servicing
/// plus trustee bp ranking ahead of the capped interest). Returns `0.0` when no
/// AFC rule is configured (cap unused).
pub(super) fn live_afc_cap_rate(
    instrument: &StructuredCredit,
    state: &SimulationState,
    context: &MarketContext,
    period_start: Date,
) -> Result<f64> {
    match instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.afc.as_ref())
    {
        Some(afc) => Ok((current_collateral_wac(state, context, period_start)?
            - afc.net_wac_fee_bp.unwrap_or(0.0) / 10_000.0)
            .max(0.0)),
        None => Ok(0.0),
    }
}

/// Current-period interest due on one tranche: `balance · coupon · accrual`,
/// with the coupon capped at `afc_cap` when `afc_capped` is set (available-funds
/// cap). Single source of truth for tranche interest-due, shared by the
/// excess-spread surplus check and the Step-5 interest recording so the two
/// cannot diverge on day-count, index fixing, or the AFC cap.
pub(super) fn tranche_period_interest_due(
    tranche: &Tranche,
    balance: f64,
    dates: TrancheAccrualDates,
    context: &MarketContext,
    afc_cap: f64,
    afc_capped: bool,
    rate_shift: f64,
) -> Result<f64> {
    let raw = tranche
        .coupon
        .try_rate_for_period(dates.start, dates.valuation, context)?;
    // Shift FLOATING tranche coupons onto the simulated rate path so a
    // floating-rate note's coupon and its discount factors move together. A
    // fixed coupon is contractual and unaffected. Floored at zero so a deeply
    // negative path cannot manufacture a negative coupon.
    let raw = match tranche.coupon {
        crate::instruments::fixed_income::structured_credit::types::TrancheCoupon::Floating(_) => {
            (raw + rate_shift).max(0.0)
        }
        _ => raw,
    };
    // The AFC cap is applied AFTER the shift: the cap tracks the collateral's
    // net WAC, which is itself shifted, so capping the unshifted rate would
    // compare quantities measured on different rate paths.
    let rate = if afc_capped { raw.min(afc_cap) } else { raw };
    let accrual =
        tranche
            .day_count
            .year_fraction(dates.start, dates.payment, DayCountContext::default())?;
    Ok(balance * rate * accrual)
}

#[derive(Clone, Copy)]
pub(super) struct TrancheAccrualDates {
    pub(super) start: Date,
    pub(super) payment: Date,
    pub(super) valuation: Date,
}

#[derive(Clone, Copy)]
pub(super) struct SimulationPeriod {
    pub(super) accrual_start: Date,
    pub(super) accrual_end: Date,
    pub(super) payment: Date,
    pub(super) valuation: Date,
    /// The deal is called or cleaned up on this payment date: reinvestment is
    /// suspended and the equity residual is withheld so it joins the
    /// redemption proceeds.
    pub(super) redemption: bool,
}
