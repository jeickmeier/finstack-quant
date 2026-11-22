//! Forward variance projection and Carr-Madan smile replication for FX variance swaps.

use super::{annualization_factor, get_historical_prices_with_dates, observation_dates};
use crate::fx_variance_swap::FxVarianceSwap;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::{
    Result,
    dates::{Date, DayCountContext},
    market_data::{
        context::MarketContext,
        surfaces::{VolQuoteType, VolSurfaceAxis},
    },
};
use finstack_quant_models::closed_form::bs_price_unchecked;
use finstack_quant_models::types::OptionType;
use finstack_quant_models::volatility::variance_replication::carr_madan_forward_variance;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::pricing::discounting::zero_rate_from_df;

pub(super) fn require_close_to_close_forecast(inst: &FxVarianceSwap) -> Result<()> {
    if inst.realized_var_method.requires_ohlc() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "FX variance swap '{}': pre-settlement forecasting is supported only for close_to_close; {} requires an estimator-specific OHLC forecast model. Fully observed settlement remains supported.",
            inst.id, inst.realized_var_method,
        )));
    }
    Ok(())
}

/// Gaussian independent-increment projection calibrated to the smile-replicated
/// cumulative expected quadratic variation. Rates are deterministic domestic
/// and foreign discount curves. For each unsettled return, E[R²] = q + m²,
/// where m includes log FX carry, -q/2, and the observed partial return.
///
/// This is exact for deterministic instantaneous variance and deterministic
/// rates. A vanilla smile alone does not identify forward log-return second
/// moments under general stochastic volatility or stochastic rates; there is
/// no claim that this projection supplies that missing joint distribution.
pub(super) fn remaining_squared_returns(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    obs_dates: &[Date],
    historical_prices: &[f64],
) -> Result<(f64, usize)> {
    require_close_to_close_forecast(inst)?;
    let dom = context.get_discount(inst.domestic_discount_curve_id.as_str())?;
    let foreign = context.get_discount(inst.foreign_discount_curve_id.as_str())?;
    let spot = inst.spot_rate(context, as_of)?;
    if !spot.is_finite() || spot <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "FX variance forecast requires finite positive spot".into(),
        ));
    }
    let mut total = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    let mut count = 0;
    let mut prior_boundary = None;
    for window in obs_dates.windows(2).filter(|window| window[1] > as_of) {
        let start = window[0].max(as_of);
        let end = window[1];
        let start_variation = match prior_boundary {
            Some((date, variation)) if date == start => variation,
            _ => cumulative_variation(inst, context, as_of, start)?,
        };
        let end_variation = cumulative_variation(inst, context, as_of, end)?;
        prior_boundary = Some((end, end_variation));
        let raw_variation = end_variation - start_variation;
        let tolerance = 1e-12 * end_variation.abs().max(start_variation.abs()).max(1.0);
        if raw_variation < -tolerance {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FX variance swap '{}': negative forward variation {raw_variation} over {start} to {end}; check smile calendar consistency and replication grid",
                inst.id,
            )));
        }
        let variation = raw_variation.max(0.0);
        let carry =
            (foreign.df_between_dates(start, end)? / dom.df_between_dates(start, end)?).ln();
        let partial_return = if window[0] <= as_of {
            let last_fixing = historical_prices.last().copied().ok_or_else(|| {
                finstack_quant_core::Error::Validation("FX variance missing latest fixing".into())
            })?;
            (spot / last_fixing).ln()
        } else {
            0.0
        };
        let mean = partial_return + carry - 0.5 * variation;
        total.add(variation + mean * mean)?;
        count += 1;
    }
    Ok((total.get_total()?, count))
}

pub(super) fn cumulative_variation(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    end: Date,
) -> Result<f64> {
    if end <= as_of {
        return Ok(0.0);
    }
    let t = inst
        .day_count
        .year_fraction(as_of, end, DayCountContext::default())?;
    Ok(replicated_variance_to(inst, context, as_of, end)? * t)
}

pub(crate) fn remaining_forward_variance(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    let dates = observation_dates(inst)?;
    if dates.last().is_some_and(|&date| as_of >= date) {
        return Ok(0.0);
    }
    let prices = get_historical_prices_with_dates(inst, context, as_of, &dates)?;
    let (sum, count) = remaining_squared_returns(inst, context, as_of, &dates, &prices)?;
    if count == 0 {
        return Ok(0.0);
    }
    Ok(annualization_factor(inst) * sum / count as f64)
}

/// Smile-replicated annualized quadratic variation from as-of to one boundary.
pub(super) fn replicated_variance_to(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    final_observation_date: Date,
) -> Result<f64> {
    let t =
        inst.day_count
            .year_fraction(as_of, final_observation_date, DayCountContext::default())?;
    if t <= 0.0 {
        return Ok(0.0);
    }

    let spot = inst.spot_rate(context, as_of)?;
    let surface = context.get_surface(inst.vol_surface_id.as_str())?;
    surface.require_quote_type(VolQuoteType::BlackLognormal)?;
    surface.require_secondary_axis(VolSurfaceAxis::Strike)?;
    let dom = context.get_discount(inst.domestic_discount_curve_id.as_str())?;
    let for_curve = context.get_discount(inst.foreign_discount_curve_id.as_str())?;
    // Date-based discount factors: `df_between_dates(as_of, maturity)` resolves the
    // year fraction on the curve's own time axis as a ratio `df(to)/df(from)`, so it
    // correctly represents the forward DF over `[as_of, maturity]` regardless of
    // where `as_of` sits relative to the curve's `base_date`.
    //
    // The previous code called `curve.df(yf(as_of, maturity))`, which looks up the
    // *spot* DF at `yf(as_of, maturity)` years from the curve's `base_date` — i.e.
    // the DF from `base_date` to roughly `base_date + yf(as_of, mat)`, not from
    // `as_of` to `mat`.  For a non-flat term structure (or any `as_of != base_date`)
    // this gives the wrong rate and therefore the wrong GK forward.  The terminal-PV
    // discount in this same function was already corrected to use `df_between_dates`
    // (see lines 66, 83); this aligns the forward-recovery path with that fix.
    let df_dom = dom.df_between_dates(as_of, final_observation_date)?;
    let df_for = for_curve.df_between_dates(as_of, final_observation_date)?;

    let r_d = zero_rate_from_df(df_dom, t, "FxVarianceSwap domestic discount")?;
    let r_f = zero_rate_from_df(df_for, t, "FxVarianceSwap foreign discount")?;
    let fwd = spot * ((r_d - r_f) * t).exp();
    let strikes = surface.secondary_values();
    let vol_fn = |t_exp: f64, k: f64| {
        finstack_quant_models::volatility::get_surface_vol_clamped(&surface, t_exp, k)
    };
    let bs_fn = |k: f64, v: f64, opt: OptionType| -> f64 {
        bs_price_unchecked(spot, k, r_d, r_f, v, t, opt)
    };
    carr_madan_forward_variance(strikes, fwd, r_d, t, vol_fn, bs_fn).ok_or_else(|| {
        finstack_quant_core::Error::Calibration {
            message: format!(
                "FX variance swap '{}': Carr-Madan replication failed. The supplied vol surface \
             must contain valid OTM put and call wings around forward {fwd} at maturity {t:.4}y \
             (received {} strikes); ATM-vol² fallback is disabled because it discards smile risk.",
                inst.id(),
                strikes.len()
            ),
            category: "fx_variance_swap_replication".to_string(),
        }
    })
}
