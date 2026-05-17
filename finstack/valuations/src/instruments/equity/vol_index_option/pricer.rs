//! Volatility index option pricer implementation.

use crate::instruments::common_impl::models::volatility::black::{d1_black76, d2_black76};
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::vol_index_option::VolatilityIndexOption;
use crate::instruments::OptionType;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, PricingResult,
};
use crate::results::ValuationResult;
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::math::norm_cdf;
use finstack_core::money::Money;

/// Registry-facing pricer for volatility index options.
pub struct VolIndexOptionDiscountingPricer;

pub(crate) fn compute_pv(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_core::Result<Money> {
    Ok(Money::new(
        compute_pv_raw(option, context, as_of)?,
        option.notional.currency(),
    ))
}

pub(crate) fn compute_pv_raw(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_core::Result<f64> {
    let disc = context.get_discount(&option.discount_curve_id)?;
    let vol_curve = context.get_vol_index_curve(&option.vol_index_curve_id)?;
    let vol_surface = context.get_surface(&option.vol_of_vol_surface_id)?;
    let t = option
        .day_count
        .year_fraction(as_of, option.expiry, DayCountContext::default())?
        .max(0.0);

    if t <= 0.0 {
        let forward = vol_curve.spot_level();
        let intrinsic = match option.option_type {
            OptionType::Call => (forward - option.strike).max(0.0),
            OptionType::Put => (option.strike - forward).max(0.0),
        };
        return Ok(intrinsic * option.contract_specs.multiplier * option.num_contracts());
    }

    let forward = vol_curve.forward_level(t);
    let vol_of_vol = vol_surface.value_clamped(t, option.strike);
    let df = disc.df_between_dates(as_of, option.expiry)?;
    let black_price = black_price(option, forward, vol_of_vol, t);
    Ok(black_price * option.contract_specs.multiplier * option.num_contracts() * df)
}

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
) -> finstack_core::Result<f64> {
    let vol_curve = context.get_vol_index_curve(&option.vol_index_curve_id)?;
    let t = option
        .day_count
        .year_fraction(as_of, option.expiry, DayCountContext::default())?
        .max(0.0);
    Ok(vol_curve.forward_level(t))
}

pub(crate) fn delta(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_core::Result<f64> {
    let vol_curve = context.get_vol_index_curve(&option.vol_index_curve_id)?;
    let vol_surface = context.get_surface(&option.vol_of_vol_surface_id)?;
    let disc = context.get_discount(&option.discount_curve_id)?;
    let t = option
        .day_count
        .year_fraction(as_of, option.expiry, DayCountContext::default())?
        .max(0.0);

    if t <= 0.0 {
        let forward = vol_curve.spot_level();
        let itm = match option.option_type {
            OptionType::Call => forward > option.strike,
            OptionType::Put => forward < option.strike,
        };
        return Ok(if itm { 1.0 } else { 0.0 });
    }

    let forward = vol_curve.forward_level(t);
    let sigma = vol_surface.value_clamped(t, option.strike);
    let df = disc.df_between_dates(as_of, option.expiry)?;
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
) -> finstack_core::Result<f64> {
    let vol_curve = context.get_vol_index_curve(&option.vol_index_curve_id)?;
    let vol_surface = context.get_surface(&option.vol_of_vol_surface_id)?;
    let disc = context.get_discount(&option.discount_curve_id)?;
    let t = option
        .day_count
        .year_fraction(as_of, option.expiry, DayCountContext::default())?
        .max(0.0);
    if t <= 0.0 {
        return Ok(0.0);
    }
    let forward = vol_curve.forward_level(t);
    let sigma = vol_surface.value_clamped(t, option.strike);
    let df = disc.df_between_dates(as_of, option.expiry)?;
    let d1 = d1_black76(forward, option.strike, sigma, t);
    let n_prime_d1 = (-0.5 * d1 * d1).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let gamma_per_point = n_prime_d1 / (forward * sigma * t.sqrt());
    Ok(gamma_per_point * option.contract_specs.multiplier * option.num_contracts() * df)
}

pub(crate) fn vega(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_core::Result<f64> {
    let vol_curve = context.get_vol_index_curve(&option.vol_index_curve_id)?;
    let vol_surface = context.get_surface(&option.vol_of_vol_surface_id)?;
    let disc = context.get_discount(&option.discount_curve_id)?;
    let t = option
        .day_count
        .year_fraction(as_of, option.expiry, DayCountContext::default())?
        .max(0.0);
    if t <= 0.0 {
        return Ok(0.0);
    }
    let forward = vol_curve.forward_level(t);
    let sigma = vol_surface.value_clamped(t, option.strike);
    let df = disc.df_between_dates(as_of, option.expiry)?;
    let d1 = d1_black76(forward, option.strike, sigma, t);
    let n_prime_d1 = (-0.5 * d1 * d1).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let vega_per_point = forward * n_prime_d1 * t.sqrt();
    Ok(vega_per_point * option.contract_specs.multiplier * option.num_contracts() * df * 0.01)
}

pub(crate) fn theta(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_core::Result<f64> {
    let vol_curve = context.get_vol_index_curve(&option.vol_index_curve_id)?;
    let vol_surface = context.get_surface(&option.vol_of_vol_surface_id)?;
    let disc = context.get_discount(&option.discount_curve_id)?;
    let t = option
        .day_count
        .year_fraction(as_of, option.expiry, DayCountContext::default())?
        .max(0.0);
    if t <= 0.0 {
        return Ok(0.0);
    }
    let forward = vol_curve.forward_level(t);
    let sigma = vol_surface.value_clamped(t, option.strike);
    let df = disc.df_between_dates(as_of, option.expiry)?;
    let r = -df.ln() / t;
    let d1 = d1_black76(forward, option.strike, sigma, t);
    let n_prime_d1 = (-0.5 * d1 * d1).exp() / (2.0 * std::f64::consts::PI).sqrt();

    // PV(t) = df(t)·black(t), with df = e^{-r·t}. The (negative) time
    // derivative — i.e. theta as time-to-expiry t shrinks — is
    //   -∂PV/∂t = df·(-∂black/∂t) + r·df·black.
    // `time_decay` is the discounted Black decay term df·(-∂black/∂t); the
    // carry term must be built from the *discounted* PV (r·df·black), NOT the
    // undiscounted Black forward price. Each term carries exactly one `df`.
    let time_decay = -forward * n_prime_d1 * sigma / (2.0 * t.sqrt()) * df;
    let discounted_pv = black_price(option, forward, sigma, t) * df;
    let carry_cost = r * discounted_pv;
    let theta_per_day = (time_decay + carry_cost) / 365.0;
    Ok(theta_per_day * option.contract_specs.multiplier * option.num_contracts())
}

pub(crate) fn intrinsic_value(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_core::Result<f64> {
    let forward = forward_vol(option, context, as_of)?;
    let intrinsic = match option.option_type {
        OptionType::Call => (forward - option.strike).max(0.0),
        OptionType::Put => (option.strike - forward).max(0.0),
    };
    Ok(intrinsic * option.contract_specs.multiplier * option.num_contracts())
}

pub(crate) fn time_value(
    option: &VolatilityIndexOption,
    context: &MarketContext,
    as_of: Date,
) -> finstack_core::Result<f64> {
    Ok(compute_pv_raw(option, context, as_of)? - intrinsic_value(option, context, as_of)?)
}

impl Default for VolIndexOptionDiscountingPricer {
    fn default() -> Self {
        Self
    }
}

impl Pricer for VolIndexOptionDiscountingPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::VolatilityIndexOption, ModelKey::Discounting)
    }

    #[tracing::instrument(
        name = "vol_index_option.discounting.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> PricingResult<ValuationResult> {
        let option = instrument
            .as_any()
            .downcast_ref::<VolatilityIndexOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::VolatilityIndexOption, instrument.key())
            })?;
        let pv = compute_pv(option, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(option).model(ModelKey::Discounting),
            )
        })?;
        Ok(ValuationResult::stamped(option.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Attributes;
    use crate::instruments::{ExerciseStyle, OptionType};
    use finstack_core::currency::Currency;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::{DiscountCurve, VolatilityIndexCurve};
    use finstack_core::types::{CurveId, InstrumentId};
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
            .day_count(finstack_core::dates::DayCount::Act365F)
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
        let pv_of_t = |t: f64| -> f64 {
            black_price(&option, forward, sigma, t) * (-r * t).exp()
        };
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
