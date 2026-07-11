//! Volatility index option pricer implementation.

use crate::instruments::equity::vol_index_option::VolatilityIndexOption;
use crate::instruments::OptionType;
use crate::models::volatility::black::{d1_black76, d2_black76};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::norm_cdf;
use finstack_quant_core::money::Money;

fn resolved_index_forward(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    if as_of >= option.expiry {
        if let Some(fixing) = option.expiry_fixing {
            return Ok(fixing);
        }
        if as_of > option.expiry {
            return Err(finstack_quant_core::Error::Validation(format!(
                "VolatilityIndexOption '{}' requires expiry_fixing between expiry and settlement",
                option.id
            )));
        }
    }
    context
        .get_vol_index_curve(&option.vol_index_curve_id)?
        .forward_level_on_date(option.expiry)
}

pub(crate) fn compute_pv(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    Ok(Money::new(
        compute_pv_raw(option, context, as_of)?,
        option.notional.currency(),
    ))
}

pub(crate) fn compute_pv_raw(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    let settlement_date = option.effective_settlement_date();
    if as_of > settlement_date {
        return Ok(0.0);
    }
    let disc = context.get_discount(&option.discount_curve_id)?;
    let t = option
        .day_count
        .year_fraction(as_of, option.expiry, DayCountContext::default())?
        .max(0.0);

    if t <= 0.0 {
        let forward = resolved_index_forward(option, context, as_of)?;
        let intrinsic = match option.option_type {
            OptionType::Call => (forward - option.strike).max(0.0),
            OptionType::Put => (option.strike - forward).max(0.0),
        };
        let df = disc.df_between_dates(as_of, settlement_date)?;
        return Ok(intrinsic * option.contract_specs.multiplier * option.num_contracts() * df);
    }

    let vol_surface = context.get_surface(&option.vol_of_vol_surface_id)?;
    let forward = resolved_index_forward(option, context, as_of)?;
    let vol_of_vol = vol_surface.value_clamped(t, option.strike);
    let df = disc.df_between_dates(as_of, settlement_date)?;
    // The vol index is √(forward variance), so Black-76 on the index forward is
    // an approximation (see `black_price`). Surface it as a diagnostic so the
    // modelling choice is visible in production logs.
    tracing::debug!(
        instrument_id = %option.id,
        forward = forward,
        vol_of_vol = vol_of_vol,
        "VolatilityIndexOption priced with Black-76 on the index forward — \
         approximation: the index is √(forward variance), not a lognormal asset"
    );
    let black_price = black_price(option, forward, vol_of_vol, t);
    Ok(black_price * option.contract_specs.multiplier * option.num_contracts() * df)
}

/// Black-76 price of a volatility-index option on the index forward.
///
/// # Model approximation (W-38)
///
/// A volatility index (VIX-style) is the **square root of a forward variance**,
/// `VIX = √(forward variance)`. It is therefore *not* itself a lognormal traded
/// asset, and Black-76 — which assumes the underlying forward is lognormal — is
/// a deliberate **approximation** here, not an exact model.
///
/// A fully consistent valuation would model the forward variance (e.g. a
/// vol-of-variance / mean-reverting square-root model) and carry the convexity
/// of the `√(·)` map explicitly. That is out of scope for this discounting
/// pricer; the approximation is the standard dealer practice of quoting a
/// "vol of vol" and running Black-76 on the index forward, with the
/// vol-of-vol smile absorbing the residual model error.
///
/// # Consistency with `vol_index_future`
///
/// The `forward` passed here is `VolatilityIndexCurve::forward_level(t)` — the
/// **same** forward source that `vol_index_future::forward_vol` uses to price
/// the volatility-index future. So a VIX option and a VIX future on the same
/// curve are struck against an identical forward; the option only adds the
/// Black-76 optionality layer on top of that shared forward.
pub(crate) fn black_price(option: &VolatilityIndexOption, forward: f64, sigma: f64, t: f64) -> f64 {
    if t <= 0.0 || sigma <= 0.0 {
        return match option.option_type {
            OptionType::Call => (forward - option.strike).max(0.0),
            OptionType::Put => (option.strike - forward).max(0.0),
        };
    }

    let d1 = d1_black76(forward, option.strike, sigma, t);
    let d2 = d2_black76(forward, option.strike, sigma, t);
    match option.option_type {
        OptionType::Call => forward * norm_cdf(d1) - option.strike * norm_cdf(d2),
        OptionType::Put => option.strike * norm_cdf(-d2) - forward * norm_cdf(-d1),
    }
}

pub(crate) fn forward_vol(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    resolved_index_forward(option, context, as_of)
}

pub(crate) fn delta(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    if as_of > option.expiry {
        return Ok(0.0);
    }
    let vol_surface = context.get_surface(&option.vol_of_vol_surface_id)?;
    let disc = context.get_discount(&option.discount_curve_id)?;
    let t = option
        .day_count
        .year_fraction(as_of, option.expiry, DayCountContext::default())?
        .max(0.0);

    if t <= 0.0 {
        let forward = resolved_index_forward(option, context, as_of)?;
        // Expiry-edge delta per index point: ±1 when ITM (put ITM → −1),
        // zero otherwise — the t → 0 limit of the Black-76 branch below.
        // Scale by multiplier × num_contracts × df exactly like the t > 0
        // branch; returning a bare ±1/0 here would mis-scale the position
        // delta by orders of magnitude and drop the put's sign.
        let delta_per_point = match option.option_type {
            OptionType::Call => {
                if forward > option.strike {
                    1.0
                } else {
                    0.0
                }
            }
            OptionType::Put => {
                if forward < option.strike {
                    -1.0
                } else {
                    0.0
                }
            }
        };
        let df = disc.df_between_dates(as_of, option.effective_settlement_date())?;
        return Ok(delta_per_point
            * option.contract_specs.multiplier
            * option.num_contracts()
            * df);
    }

    let forward = resolved_index_forward(option, context, as_of)?;
    let sigma = vol_surface.value_clamped(t, option.strike);
    let df = disc.df_between_dates(as_of, option.effective_settlement_date())?;
    let d1 = d1_black76(forward, option.strike, sigma, t);
    let delta_per_point = match option.option_type {
        OptionType::Call => norm_cdf(d1),
        OptionType::Put => norm_cdf(d1) - 1.0,
    };
    Ok(delta_per_point * option.contract_specs.multiplier * option.num_contracts() * df)
}

pub(crate) fn gamma(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    if as_of > option.expiry {
        return Ok(0.0);
    }
    let vol_surface = context.get_surface(&option.vol_of_vol_surface_id)?;
    let disc = context.get_discount(&option.discount_curve_id)?;
    let t = option
        .day_count
        .year_fraction(as_of, option.expiry, DayCountContext::default())?
        .max(0.0);
    if t <= 0.0 {
        return Ok(0.0);
    }
    let forward = resolved_index_forward(option, context, as_of)?;
    let sigma = vol_surface.value_clamped(t, option.strike);
    let df = disc.df_between_dates(as_of, option.effective_settlement_date())?;
    let d1 = d1_black76(forward, option.strike, sigma, t);
    let n_prime_d1 = (-0.5 * d1 * d1).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let gamma_per_point = n_prime_d1 / (forward * sigma * t.sqrt());
    Ok(gamma_per_point * option.contract_specs.multiplier * option.num_contracts() * df)
}

pub(crate) fn vega(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    if as_of > option.expiry {
        return Ok(0.0);
    }
    let vol_surface = context.get_surface(&option.vol_of_vol_surface_id)?;
    let disc = context.get_discount(&option.discount_curve_id)?;
    let t = option
        .day_count
        .year_fraction(as_of, option.expiry, DayCountContext::default())?
        .max(0.0);
    if t <= 0.0 {
        return Ok(0.0);
    }
    let forward = resolved_index_forward(option, context, as_of)?;
    let sigma = vol_surface.value_clamped(t, option.strike);
    let df = disc.df_between_dates(as_of, option.effective_settlement_date())?;
    let d1 = d1_black76(forward, option.strike, sigma, t);
    let n_prime_d1 = (-0.5 * d1 * d1).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let vega_per_point = forward * n_prime_d1 * t.sqrt();
    Ok(vega_per_point * option.contract_specs.multiplier * option.num_contracts() * df * 0.01)
}

pub(crate) fn theta(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    if as_of > option.effective_settlement_date() {
        return Ok(0.0);
    }
    let next_day = as_of + time::Duration::days(1);
    Ok(compute_pv_raw(option, context, next_day)? - compute_pv_raw(option, context, as_of)?)
}

pub(crate) fn intrinsic_value(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    if as_of > option.effective_settlement_date() {
        return Ok(0.0);
    }
    let forward = forward_vol(option, context, as_of)?;
    let intrinsic = match option.option_type {
        OptionType::Call => (forward - option.strike).max(0.0),
        OptionType::Put => (option.strike - forward).max(0.0),
    };
    let df = context
        .get_discount(&option.discount_curve_id)?
        .df_between_dates(as_of, option.effective_settlement_date())?;
    Ok(intrinsic * option.contract_specs.multiplier * option.num_contracts() * df)
}

pub(crate) fn time_value(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    if as_of > option.effective_settlement_date() {
        return Ok(0.0);
    }
    Ok(compute_pv_raw(option, context, as_of)? - intrinsic_value(option, context, as_of)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::instruments::{ExerciseStyle, OptionType};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, VolatilityIndexCurve};
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::macros::date;

    fn setup_market() -> MarketContext {
        let base_date = date!(2025 - 01 - 01);
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (1.0, 0.96)])
            .build()
            .expect("disc");
        let vix = VolatilityIndexCurve::builder("VIX")
            .base_date(base_date)
            .spot_level(18.0)
            .knots([(0.0, 18.0), (0.25, 20.0), (0.5, 21.0), (1.0, 22.0)])
            .build()
            .expect("curve");
        let volvol = VolSurface::builder("VIX-VOLVOL")
            .expiries(&[0.25, 0.5, 1.0])
            .strikes(&[15.0, 20.0, 25.0])
            .row(&[0.8, 0.8, 0.8])
            .row(&[0.8, 0.8, 0.8])
            .row(&[0.8, 0.8, 0.8])
            .build()
            .expect("surface");
        MarketContext::new()
            .insert(disc)
            .insert(vix)
            .insert_surface(volvol)
    }

    fn sample_option() -> VolatilityIndexOption {
        VolatilityIndexOption::builder()
            .id(InstrumentId::new("VIX-CALL"))
            .notional(Money::new(10_000.0, Currency::USD))
            .strike(20.0)
            .option_type(OptionType::Call)
            .exercise_style(ExerciseStyle::European)
            .expiry(date!(2025 - 03 - 19))
            .contract_specs(
                crate::instruments::equity::vol_index_option::VolIndexOptionSpecs::vix(),
            )
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .vol_of_vol_surface_id(CurveId::new("VIX-VOLVOL"))
            .attributes(Attributes::new())
            .build()
            .expect("option")
    }

    #[test]
    fn compute_pv_matches_instrument_value() {
        let market = setup_market();
        let option = sample_option();
        let as_of = date!(2025 - 01 - 01);

        let via_pricer = compute_pv(&option, &market, as_of).expect("pricer pv");
        let via_instrument = option.value(&market, as_of).expect("instrument pv");

        assert_eq!(via_pricer, via_instrument);
    }

    /// W-38: a volatility-index option and a volatility-index future on the
    /// same vol-index curve and the same date must reference an identical
    /// forward. The Black-76 option pricer is an approximation (the index is
    /// √(forward variance), not a lognormal asset), but its forward must be the
    /// exact same `VolatilityIndexCurve::forward_level` the future uses — the
    /// option only adds optionality on top of the shared forward.
    #[test]
    fn vol_index_option_and_future_share_the_same_forward() {
        use crate::instruments::equity::vol_index_future::{
            VolIndexContractSpecs, VolatilityIndexFuture,
        };
        use crate::instruments::rates::ir_future::Position;

        let market = setup_market();
        let as_of = date!(2025 - 01 - 01);
        let expiry = date!(2025 - 03 - 19);

        // Option forward via the option pricer's `forward_vol`.
        let option = sample_option();
        let option_forward = forward_vol(&option, &market, as_of).expect("option forward");

        // Future forward via the future pricer's `forward_vol`, settling on the
        // same date.
        let future = VolatilityIndexFuture::builder()
            .id(InstrumentId::new("VIX-FUT-SHARED"))
            .notional(Money::new(20_000.0, Currency::USD))
            .expiry(expiry)
            .settlement_date(expiry)
            .quoted_price(20.0)
            .position(Position::Long)
            .contract_specs(VolIndexContractSpecs::vix())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .attributes(Attributes::new())
            .build()
            .expect("future");
        let future_forward =
            crate::instruments::equity::vol_index_future::pricer::forward_vol(&future, &market)
                .expect("future forward");

        assert!(
            (option_forward - future_forward).abs() < 1e-10,
            "VIX option and VIX future must price off the same curve forward: \
             option={option_forward} future={future_forward}"
        );
    }

    /// Expiry-edge delta: an ITM put at expiry must report −1 per index
    /// point, scaled by multiplier × num_contracts × df — not a bare +1.
    /// The spot VIX level (18.0) is below the strike (20.0), so the put is
    /// ITM and the call is OTM.
    #[test]
    fn expired_itm_put_delta_has_negative_sign_and_position_scaling() {
        let market = setup_market();
        let expiry = date!(2025 - 03 - 19);

        let mut put = sample_option();
        put.option_type = OptionType::Put;

        put.expiry_fixing = Some(18.0);

        let d = delta(&put, &market, expiry).expect("expired put delta");
        // df(expiry, expiry) = 1, so the expected delta is the full
        // per-point scale with a negative sign.
        let scale = put.contract_specs.multiplier * put.num_contracts();
        assert!(
            (d - (-scale)).abs() < 1e-9,
            "expired ITM put delta must be -multiplier×num_contracts ({}), got {d}",
            -scale
        );

        // The ITM-put scenario makes the call OTM: delta must be exactly 0.
        let mut call = sample_option();
        call.expiry_fixing = Some(18.0);
        let d_call = delta(&call, &market, expiry).expect("expired call delta");
        assert!(
            d_call.abs() < 1e-12,
            "expired OTM call delta must be 0, got {d_call}"
        );
    }

    /// Expiry-edge PV: the intrinsic settlement branch carries the same
    /// multiplier × num_contracts × df scaling as the live branch.
    #[test]
    fn expired_itm_put_pv_is_discounted_intrinsic() {
        let market = setup_market();
        let expiry = date!(2025 - 03 - 19);

        let mut put = sample_option();
        put.option_type = OptionType::Put;
        put.expiry_fixing = Some(18.0);

        let pv = compute_pv_raw(&put, &market, expiry).expect("expired put pv");
        let expected = 2.0 * put.contract_specs.multiplier * put.num_contracts();
        assert!(
            (pv - expected).abs() < 1e-9,
            "expired ITM put PV must be intrinsic × scale ({expected}), got {pv}"
        );
    }

    /// W-34: the analytic theta must equal the finite-difference theta of the
    /// *discounted* PV. The carry term `-r·V` must be consistent with the final
    /// `∂PV/∂t = ∂(df·black)/∂t`.
    ///
    /// This isolates the time-to-expiry component (Black decay + discount roll)
    /// by holding the forward and vol-of-vol fixed and differencing only `t` —
    /// the analytic Black/Greeks formulas likewise hold the forward fixed, so a
    /// curve-roll FD theta would not be a like-for-like comparison.
    #[test]
    fn analytic_theta_matches_fd_theta_of_discounted_pv() {
        let market = setup_market();
        let option = sample_option();
        let as_of = date!(2025 - 01 - 01);

        let analytic = theta(&option, &market, as_of).expect("analytic theta");

        // Reconstruct the discounted PV as a closed-form function of `t` only,
        // with the forward, vol-of-vol and short rate held at their as-of
        // values — exactly the quantities the analytic theta differentiates.
        let vol_curve = market
            .get_vol_index_curve(&option.vol_index_curve_id)
            .expect("vix");
        let vol_surface = market
            .get_surface(&option.vol_of_vol_surface_id)
            .expect("volvol");
        let disc = market
            .get_discount(&option.discount_curve_id)
            .expect("disc");
        let t0 = option
            .day_count
            .year_fraction(as_of, option.expiry, DayCountContext::default())
            .expect("t")
            .max(0.0);
        let forward = vol_curve.forward_level(t0);
        let sigma = vol_surface.value_clamped(t0, option.strike);
        let df0 = disc.df_between_dates(as_of, option.expiry).expect("df");
        let r = -df0.ln() / t0;

        // Discounted PV per contract-point as a function of t.
        let pv_of_t = |t: f64| -> f64 { black_price(&option, forward, sigma, t) * (-r * t).exp() };
        let scale = option.contract_specs.multiplier * option.num_contracts();
        let one_day = 1.0 / 365.0;
        // theta-per-day: PV decays as t (time-to-expiry) falls by one day.
        let fd_theta = (pv_of_t(t0 - one_day) - pv_of_t(t0)) * scale;

        let denom = analytic.abs().max(fd_theta.abs()).max(1e-9);
        assert!(
            (analytic - fd_theta).abs() / denom < 0.02,
            "analytic theta {analytic} disagrees with FD theta of discounted PV {fd_theta}",
        );
    }
}
