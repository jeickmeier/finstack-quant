//! Shared FX finite-difference Greeks, vol scenarios, and settlement helpers.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;

/// `true` once `expiry` is reached on the instrument's `day_count` basis, where
/// option Greeks are reported as zero.
pub(crate) fn greeks_expired(
    day_count: DayCount,
    as_of: Date,
    expiry: Date,
) -> finstack_quant_core::Result<bool> {
    Ok(day_count.year_fraction(as_of, expiry, DayCountContext::default())? <= 0.0)
}

/// One-sided PV change per basis point of a parallel bump of `curve_id`.
pub(crate) fn discount_rho_bp<I>(
    instrument: &I,
    market: &MarketContext,
    as_of: Date,
    curve_id: &CurveId,
    bump_bp: f64,
) -> finstack_quant_core::Result<f64>
where
    I: finstack_quant_valuations_core::instruments::Instrument,
{
    let base_pv = instrument.value(market, as_of)?.amount();
    let bumped = finstack_quant_valuations_core::metrics::bump_discount_curve_parallel(
        market, curve_id, bump_bp,
    )?;
    let pv_bumped = instrument.value(&bumped, as_of)?.amount();
    Ok((pv_bumped - base_pv) / bump_bp)
}

/// Option rho on `curve_id` per basis point, zero once the option has expired.
///
/// # Arguments
///
/// * `instrument` - Option repriced on the bumped market.
/// * `day_count` - Option day count deciding whether `expiry` has been reached.
/// * `expiry` - Option expiry date.
/// * `market` - Unbumped market.
/// * `as_of` - Valuation date.
/// * `curve_id` - Domestic or foreign discount curve to bump in parallel.
/// * `bump_bp` - Bump size in basis points.
pub(crate) fn discount_rho_greek<I>(
    instrument: &I,
    day_count: DayCount,
    expiry: Date,
    market: &MarketContext,
    as_of: Date,
    curve_id: &CurveId,
    bump_bp: f64,
) -> finstack_quant_core::Result<Option<f64>>
where
    I: finstack_quant_valuations_core::instruments::Instrument,
{
    if greeks_expired(day_count, as_of, expiry)? {
        return Ok(Some(0.0));
    }
    discount_rho_bp(instrument, market, as_of, curve_id, bump_bp).map(Some)
}

/// Market with the FX matrix `base/quote` rate bumped relatively by `bump_pct`.
///
/// Uses the matrix's own relative bump, which keeps its `FxConfig` (pivot,
/// triangulation) and pinned quotes and never mutates the shared provider.
pub(crate) fn fx_matrix_bumped_market(
    market: &MarketContext,
    base_currency: Currency,
    quote_currency: Currency,
    as_of: Date,
    bump_pct: f64,
) -> finstack_quant_core::Result<MarketContext> {
    Ok(market
        .clone()
        .insert_fx(market.fx_required()?.with_bumped_rate(
            finstack_quant_core::money::fx::FxQuery::new(
                base_currency,
                quote_currency,
                as_of,
                finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
            ),
            bump_pct,
        )?))
}

/// Central spot delta `(PV(S·(1+h)) − PV(S·(1−h))) / (2·S·h)`.
///
/// `bumped_market(h)` returns the market with the instrument's spot source
/// bumped relatively by `h`.
pub(crate) fn spot_delta<I, F>(
    instrument: &I,
    as_of: Date,
    spot: f64,
    bump_pct: f64,
    bumped_market: F,
) -> finstack_quant_core::Result<f64>
where
    I: finstack_quant_valuations_core::instruments::Instrument,
    F: Fn(f64) -> finstack_quant_core::Result<MarketContext>,
{
    let up = bumped_market(bump_pct)?;
    let down = bumped_market(-bump_pct)?;
    let pv_up = instrument.value(&up, as_of)?.amount();
    let pv_dn = instrument.value(&down, as_of)?.amount();
    finstack_quant_valuations_core::metrics::central_diff_by_width(
        pv_up,
        pv_dn,
        2.0 * spot * bump_pct,
    )
}

/// Central spot gamma `(PV(S·(1+h)) − 2·PV(S) + PV(S·(1−h))) / (S·h)²`.
///
/// Returns `0.0` when the bump width `S·h` is not positive. `bumped_market(h)`
/// returns the market with the instrument's spot source bumped relatively by `h`.
pub(crate) fn spot_gamma<I, F>(
    instrument: &I,
    as_of: Date,
    base_pv: f64,
    spot: f64,
    bump_pct: f64,
    bumped_market: F,
) -> finstack_quant_core::Result<f64>
where
    I: finstack_quant_valuations_core::instruments::Instrument,
    F: Fn(f64) -> finstack_quant_core::Result<MarketContext>,
{
    let bump_size = spot * bump_pct;
    if bump_size <= 0.0 {
        return Ok(0.0);
    }
    let up = bumped_market(bump_pct)?;
    let pv_up = instrument.value(&up, as_of)?.amount();
    let down = bumped_market(-bump_pct)?;
    let pv_dn = instrument.value(&down, as_of)?.amount();
    Ok((pv_up - 2.0 * base_pv + pv_dn) / (bump_size * bump_size))
}

/// Quote-per-base FX policy stamp: whether the matrix triangulated the pair.
///
/// `spot_overridden` marks a spot taken from outside the matrix (a quoted
/// spot or an explicit spot scalar), where triangulation does not apply.
pub(crate) fn fx_valuation_details(
    market: &MarketContext,
    base_currency: Currency,
    quote_currency: Currency,
    as_of: Date,
    spot_overridden: bool,
) -> finstack_quant_valuations_core::results::ValuationDetails {
    let fx_triangulated = if spot_overridden {
        None
    } else {
        market
            .fx()
            .and_then(|fx| {
                fx.rate(FxQuery::new(
                    base_currency,
                    quote_currency,
                    as_of,
                    finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
                ))
                .ok()
            })
            .map(|q| q.triangulated)
    };
    finstack_quant_valuations_core::results::ValuationDetails::Fx(
        finstack_quant_valuations_core::results::FxValuationDetails { fx_triangulated },
    )
}

/// Empty schedule of a contract whose cashflows have all settled.
pub(crate) fn settled_schedule(
    notional_hint: Option<finstack_quant_core::money::Money>,
) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
    Ok(finstack_quant_cashflows::schedule_from_classified_flows(
        Vec::new(),
        DayCount::Act365F,
        finstack_quant_cashflows::ScheduleBuildOpts {
            notional_hint,
            meta: finstack_quant_cashflows::builder::CashFlowMeta {
                representation:
                    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual,
                ..Default::default()
            },
        },
    ))
}

/// One volatility scenario: the instrument and market to reprice.
pub(crate) type VolScenario<I> = (I, MarketContext);

/// Applied half-width with the up and down volatility scenarios.
pub(crate) type VolScenarios<I> = (f64, VolScenario<I>, VolScenario<I>);

/// Central volatility bump on the source pricing reads.
///
/// An instrument with an `implied_volatility` override is bumped on that
/// quote (the market is left unchanged); otherwise the named surface is bumped
/// in parallel. With an override the half-width is capped at half the quoted
/// vol so the down scenario stays positive.
///
/// `overrides` must be the instrument's own pricing overrides; they decide
/// whether the quote or the surface is the active source.
///
/// Returns `(bump_abs, up, down)`, where `bump_abs` is the half-width actually
/// applied (absolute vol, `0.01` = one vol point).
pub(crate) fn active_vol_scenarios<I>(
    instrument: &I,
    overrides: &InstrumentPricingOverrides,
    market: &MarketContext,
    surface_id: &str,
    requested_bump_abs: f64,
) -> finstack_quant_core::Result<VolScenarios<I>>
where
    I: finstack_quant_valuations_core::instruments::Instrument + Clone,
{
    use finstack_quant_valuations_core::metrics::bump_active_volatility;

    let bump_abs = overrides
        .market_quotes
        .implied_volatility
        .map_or(requested_bump_abs, |vol| requested_bump_abs.min(vol * 0.5));
    let up = bump_active_volatility(instrument, market, surface_id, bump_abs)?;
    let down = bump_active_volatility(instrument, market, surface_id, -bump_abs)?;
    Ok((bump_abs, up, down))
}

/// Spot date of an FX trade under the product's explicit rule:
/// `trade_date` plus `settlement_days` business days on `calendar`, which
/// must be open on the result (record CORE-CONVENTIONS-011 #6).
///
/// The caller selects the lag and the calendar (the strict composite of
/// every member the trade's convention needs, including any USD gate);
/// nothing is inferred from the currencies.
pub(crate) fn spot_date_from_lag(
    trade_date: finstack_quant_core::dates::Date,
    settlement_days: u32,
    calendar: &dyn finstack_quant_core::dates::HolidayCalendar,
) -> finstack_quant_core::Result<finstack_quant_core::dates::Date> {
    use finstack_quant_core::dates::DateExt;
    let lag = i32::try_from(settlement_days).map_err(|_| {
        finstack_quant_core::Error::Validation(format!(
            "FX spot lag {settlement_days} is out of range"
        ))
    })?;
    let spot = trade_date.add_business_days(lag, calendar)?;
    if !calendar.is_business_day(spot)? {
        return Err(finstack_quant_core::Error::Validation(format!(
            "FX spot date {spot} is not a business day of the settlement calendar"
        )));
    }
    Ok(spot)
}
