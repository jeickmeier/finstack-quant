//! Shared FX finite-difference Greeks, vol scenarios, and settlement helpers.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::ZERO_TOLERANCE;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_core::instruments::{GreekBumps, Instrument};
use finstack_quant_valuations_core::metrics::{
    VOL_POINTS_PER_ABSOLUTE_VOL, bump_active_volatility, central_diff_by_width,
};

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

/// Volatility scenarios around the active vol quote or surface.
pub(crate) enum VolScenarios<I> {
    /// Symmetric ±`half_width` scenarios for O(h²) central finite differences.
    Central {
        half_width: f64,
        up: VolScenario<I>,
        down: VolScenario<I>,
    },
    /// Forward `+step` scenario used when symmetric down-bumping would hit zero vol.
    Forward { step: f64, up: VolScenario<I> },
}

/// Central (or forward at zero vol) bump on the source pricing reads.
///
/// An instrument with an `implied_volatility` override is bumped on that
/// quote (the market is left unchanged); otherwise the named surface is bumped
/// in parallel. With a positive override the half-width is capped at half the
/// quoted vol so the down scenario stays positive; when the override is zero
/// (or nearly zero) a forward bump is returned so FD Greeks never divide by 0.
pub(crate) fn active_vol_scenarios<I>(
    instrument: &I,
    overrides: &InstrumentPricingOverrides,
    market: &MarketContext,
    surface_id: &str,
    requested_bump_abs: f64,
) -> finstack_quant_core::Result<VolScenarios<I>>
where
    I: Instrument + Clone,
{
    if !requested_bump_abs.is_finite() || requested_bump_abs <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "vol bump must be finite and positive, got {requested_bump_abs}"
        )));
    }
    if let Some(vol) = overrides.market_quotes.implied_volatility
        && vol * 0.5 <= ZERO_TOLERANCE
    {
        if !vol.is_finite() || vol < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "implied_volatility override must be finite and non-negative, got {vol}"
            )));
        }
        let up = bump_active_volatility(instrument, market, surface_id, requested_bump_abs)?;
        return Ok(VolScenarios::Forward {
            step: requested_bump_abs,
            up,
        });
    }

    let half_width = overrides
        .market_quotes
        .implied_volatility
        .map_or(requested_bump_abs, |vol| requested_bump_abs.min(vol * 0.5));
    let up = bump_active_volatility(instrument, market, surface_id, half_width)?;
    let down = bump_active_volatility(instrument, market, surface_id, -half_width)?;
    Ok(VolScenarios::Central {
        half_width,
        up,
        down,
    })
}

/// Spot-bump and timing inputs shared by option spot delta and gamma helpers.
#[derive(Clone, Copy)]
pub(crate) struct FxSpotGreekSpec {
    pub(crate) day_count: DayCount,
    pub(crate) expiry: Date,
    pub(crate) bump_pct: f64,
}

/// Timing and volatility-source inputs shared by option vol and cross-Greek helpers.
#[derive(Clone, Copy)]
pub(crate) struct FxVolGreekSpec<'a> {
    pub(crate) day_count: DayCount,
    pub(crate) expiry: Date,
    pub(crate) overrides: &'a InstrumentPricingOverrides,
    pub(crate) surface_id: &'a str,
}

/// Shared option spot delta (zero after expiry) via central bump on the caller's spot source.
pub(crate) fn option_spot_delta<I, S, F>(
    instrument: &I,
    spec: FxSpotGreekSpec,
    as_of: Date,
    resolve_spot: S,
    bumped_market: F,
) -> finstack_quant_core::Result<Option<f64>>
where
    I: Instrument,
    S: FnOnce() -> finstack_quant_core::Result<f64>,
    F: Fn(f64) -> finstack_quant_core::Result<MarketContext>,
{
    if greeks_expired(spec.day_count, as_of, spec.expiry)? {
        return Ok(Some(0.0));
    }
    let spot = resolve_spot()?;
    if spot * spec.bump_pct <= 0.0 {
        return Ok(Some(0.0));
    }
    Ok(Some(spot_delta(
        instrument,
        as_of,
        spot,
        spec.bump_pct,
        bumped_market,
    )?))
}

/// Shared option spot gamma (zero after expiry) via central bump on the caller's spot source.
pub(crate) fn option_spot_gamma<I, S, F>(
    instrument: &I,
    spec: FxSpotGreekSpec,
    market: &MarketContext,
    as_of: Date,
    resolve_spot: S,
    bumped_market: F,
) -> finstack_quant_core::Result<Option<f64>>
where
    I: Instrument,
    S: FnOnce() -> finstack_quant_core::Result<f64>,
    F: Fn(f64) -> finstack_quant_core::Result<MarketContext>,
{
    if greeks_expired(spec.day_count, as_of, spec.expiry)? {
        return Ok(Some(0.0));
    }
    let base_pv = instrument.value(market, as_of)?.amount();
    let spot = resolve_spot()?;
    Ok(Some(spot_gamma(
        instrument,
        as_of,
        base_pv,
        spot,
        spec.bump_pct,
        bumped_market,
    )?))
}

/// Shared option FD vega per vol point (zero after expiry, forward diff at zero vol).
pub(crate) fn option_vega<I>(
    instrument: &I,
    spec: FxVolGreekSpec<'_>,
    market: &MarketContext,
    as_of: Date,
    bumps: GreekBumps,
) -> finstack_quant_core::Result<Option<f64>>
where
    I: Instrument + Clone,
{
    if greeks_expired(spec.day_count, as_of, spec.expiry)? {
        return Ok(Some(0.0));
    }
    match active_vol_scenarios(
        instrument,
        spec.overrides,
        market,
        spec.surface_id,
        bumps.vol_bump_decimal,
    )? {
        VolScenarios::Central {
            half_width,
            up: (up, mkt_up),
            down: (dn, mkt_dn),
        } => {
            let pv_up = up.value(&mkt_up, as_of)?.amount();
            let pv_dn = dn.value(&mkt_dn, as_of)?.amount();
            Ok(Some(
                (pv_up - pv_dn) / (2.0 * half_width * VOL_POINTS_PER_ABSOLUTE_VOL),
            ))
        }
        VolScenarios::Forward {
            step,
            up: (up, mkt_up),
        } => {
            let pv_up = up.value(&mkt_up, as_of)?.amount();
            let pv_base = instrument.value(market, as_of)?.amount();
            Ok(Some(
                (pv_up - pv_base) / (step * VOL_POINTS_PER_ABSOLUTE_VOL),
            ))
        }
    }
}

/// Shared option FD vanna per vol point (zero after expiry, forward diff at zero vol).
pub(crate) fn option_vanna<I, S, D>(
    instrument: &I,
    spec: FxVolGreekSpec<'_>,
    market: &MarketContext,
    as_of: Date,
    bumps: GreekBumps,
    resolve_spot: S,
    delta_in_scenario: D,
) -> finstack_quant_core::Result<Option<f64>>
where
    I: Instrument + Clone,
    S: FnOnce() -> finstack_quant_core::Result<f64>,
    D: Fn(&I, &MarketContext, f64) -> finstack_quant_core::Result<f64>,
{
    if greeks_expired(spec.day_count, as_of, spec.expiry)? {
        return Ok(Some(0.0));
    }
    let spot = resolve_spot()?;
    if spot * bumps.spot_bump_decimal <= 0.0 {
        return Ok(Some(0.0));
    }
    match active_vol_scenarios(
        instrument,
        spec.overrides,
        market,
        spec.surface_id,
        bumps.vol_bump_decimal,
    )? {
        VolScenarios::Central {
            half_width,
            up: (up, mkt_up),
            down: (dn, mkt_dn),
        } => {
            let d_up = delta_in_scenario(&up, &mkt_up, spot)?;
            let d_dn = delta_in_scenario(&dn, &mkt_dn, spot)?;
            Ok(Some(central_diff_by_width(
                d_up,
                d_dn,
                2.0 * half_width * VOL_POINTS_PER_ABSOLUTE_VOL,
            )?))
        }
        VolScenarios::Forward {
            step,
            up: (up, mkt_up),
        } => {
            let d_up = delta_in_scenario(&up, &mkt_up, spot)?;
            let d_base = delta_in_scenario(instrument, market, spot)?;
            Ok(Some((d_up - d_base) / (step * VOL_POINTS_PER_ABSOLUTE_VOL)))
        }
    }
}

/// Shared option FD volga per vol point squared (zero after expiry, forward 2nd diff at zero vol).
pub(crate) fn option_volga<I>(
    instrument: &I,
    spec: FxVolGreekSpec<'_>,
    market: &MarketContext,
    as_of: Date,
    base_pv: f64,
    bumps: GreekBumps,
) -> finstack_quant_core::Result<Option<f64>>
where
    I: Instrument + Clone,
{
    if greeks_expired(spec.day_count, as_of, spec.expiry)? {
        return Ok(Some(0.0));
    }
    match active_vol_scenarios(
        instrument,
        spec.overrides,
        market,
        spec.surface_id,
        bumps.vol_bump_decimal,
    )? {
        VolScenarios::Central {
            half_width,
            up: (up, mkt_up),
            down: (dn, mkt_dn),
        } => {
            let pv_up = up.value(&mkt_up, as_of)?.amount();
            let pv_dn = dn.value(&mkt_dn, as_of)?.amount();
            let width = half_width * VOL_POINTS_PER_ABSOLUTE_VOL;
            Ok(Some((pv_up - 2.0 * base_pv + pv_dn) / (width * width)))
        }
        VolScenarios::Forward {
            step,
            up: (up, mkt_up),
        } => {
            let pv_up = up.value(&mkt_up, as_of)?.amount();
            let (up2, mkt_up2) =
                bump_active_volatility(instrument, market, spec.surface_id, 2.0 * step)?;
            let pv_2up = up2.value(&mkt_up2, as_of)?.amount();
            let width = step * VOL_POINTS_PER_ABSOLUTE_VOL;
            Ok(Some((pv_2up - 2.0 * pv_up + base_pv) / (width * width)))
        }
    }
}
