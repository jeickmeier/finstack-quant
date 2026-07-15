//! Vega calculator for interest rate options (caps/floors/caplets/floorlets).

use crate::instruments::rates::cap_floor::hw_pricer::resolve_capfloor_hw1f_model_params;
use crate::instruments::rates::cap_floor::{CapFloor, CapFloorVolType};
use crate::metrics::bump_surface_vol_absolute;
use crate::metrics::{MetricCalculator, MetricContext};
use crate::pricer::ModelKey;
use finstack_quant_core::market_data::surfaces::VolQuoteType;
use finstack_quant_core::Result;

use super::common::CapletInputs;

const DEFAULT_HW_VEGA_BUMP: f64 = 0.0001;

/// Vega calculator (model-consistent vega per 1% vol, aggregated for caps/floors).
///
/// Dispatches to the appropriate model based on `vol_type`:
/// - `Lognormal`: Black-76 vega = F·n(d₁)·√T / 100
/// - `ShiftedLognormal`: Black-76 vega on shifted rates
/// - `Normal`: Bachelier vega = n(d)·√T / 100
///
/// Vega is "per 1% vol", i.e. per a 0.01 change in the model's volatility input.
/// For lognormal vol that is a 1 percentage-point move (e.g. 20% → 21%). For
/// normal vol it is a 0.01 = 100bp move in the absolute-rate vol; scale by 0.01
/// for a per-1bp normal vega if that is the desired desk convention.
///
/// # Timing convention
///
/// The vega formula's `T` argument is the year fraction to the option fixing
/// date (`c.fixing_t`) — the same time the pricer uses for both the vol-surface
/// lookup and the model `T`. Using a single time keeps vega consistent with the
/// reported price and with delta/gamma (a finite-difference vega reconciles with
/// the analytic one).
pub(crate) struct VegaCalculator;

/// Direct Hull-White short-rate σ sensitivity per 0.01 absolute σ.
///
/// This model-parameter sensitivity is intentionally separate from
/// [`VegaCalculator`], whose Hull-White implementation bumps market normal-vol
/// quotes and recalibrates σ.
pub(crate) struct HwSigmaVegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CapFloor = context.instrument_as()?;
        if matches!(
            context.clone_pricer_dispatch().0,
            Some(ModelKey::HullWhite1F)
        ) {
            return hull_white_tree_vega_per_pct(option, context);
        }
        let strike = option.strike_f64()?;
        let vol_type = option.vol_type;
        let vol_shift = option.resolved_vol_shift();
        super::common::aggregate_over_caplets(option, context, |c: CapletInputs| {
            caplet_vega(vol_type, strike, vol_shift, c)
        })
    }
}

impl MetricCalculator for HwSigmaVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CapFloor = context.instrument_as()?;
        if !matches!(
            context.clone_pricer_dispatch().0,
            Some(ModelKey::HullWhite1F)
        ) {
            return Err(finstack_quant_core::Error::Validation(
                "hw_sigma_vega requires HullWhite1F pricing".to_owned(),
            ));
        }
        hull_white_sigma_vega_per_pct(option, context)
    }
}

fn caplet_vega(vol_type: CapFloorVolType, strike: f64, vol_shift: f64, c: CapletInputs) -> f64 {
    use super::common::lognormal_vega_with_fallback;
    use crate::instruments::rates::cap_floor::pricing::black;
    let t = c.fixing_t;
    match vol_type {
        // `Auto` is a lognormal surface; both share the Black-with-Bachelier
        // fallback path so the Greek matches the pricer for any rate sign.
        CapFloorVolType::Lognormal | CapFloorVolType::Auto => {
            lognormal_vega_with_fallback(strike, c.forward, c.sigma, t)
        }
        CapFloorVolType::ShiftedLognormal => {
            black::vega_per_pct(strike + vol_shift, c.forward + vol_shift, c.sigma, t)
        }
        CapFloorVolType::Normal => {
            crate::instruments::rates::cap_floor::pricing::normal::vega_per_pct(
                strike, c.forward, c.sigma, t,
            )
        }
    }
}

fn hull_white_tree_vega_per_pct(option: &CapFloor, context: &MetricContext) -> Result<f64> {
    hull_white_surface_vega_per_pct(option, context)
}

fn hull_white_surface_vega_per_pct(option: &CapFloor, context: &MetricContext) -> Result<f64> {
    let bump = DEFAULT_HW_VEGA_BUMP;
    let market = context.curves.as_ref();
    market
        .get_surface(option.vol_surface_id.as_str())?
        .require_quote_type(VolQuoteType::Normal)?;
    let up_market = bump_surface_vol_absolute(market, option.vol_surface_id.as_str(), bump)?;
    let pv_up = context.reprice_instrument_raw(option, &up_market, context.as_of)?;
    let down_market = bump_surface_vol_absolute(market, option.vol_surface_id.as_str(), -bump)?;
    let pv_down = context.reprice_instrument_raw(option, &down_market, context.as_of)?;
    Ok((pv_up - pv_down) / (2.0 * bump) * 0.01)
}

fn hull_white_sigma_vega_per_pct(option: &CapFloor, context: &MetricContext) -> Result<f64> {
    let market = context.curves.as_ref();
    let bump = DEFAULT_HW_VEGA_BUMP;
    let base = resolve_capfloor_hw1f_model_params(option, market, context.as_of)?;
    let with_sigma = |shift: f64| -> Result<CapFloor> {
        let mut bumped = option.clone();
        bumped.pricing_overrides.model_config.hw1f_mean_reversion = Some(base.kappa);
        if option
            .pricing_overrides
            .model_config
            .hw1f_sigma_schedule
            .is_some()
        {
            let values = base
                .volatility
                .values()
                .iter()
                .map(|sigma| sigma + shift)
                .collect();
            bumped.pricing_overrides.model_config.hw1f_sigma = None;
            bumped.pricing_overrides.model_config.hw1f_sigma_schedule = Some(
                finstack_quant_core::math::piecewise::PiecewiseConstantCurve::new(
                    base.volatility.times().to_vec(),
                    values,
                )?,
            );
        } else {
            bumped.pricing_overrides.model_config.hw1f_sigma =
                Some(base.volatility.values()[0] + shift);
        }
        Ok(bumped)
    };

    let up = with_sigma(bump)?;
    let pv_up = context.reprice_instrument_raw(&up, market, context.as_of)?;

    if base.volatility.values().iter().all(|sigma| *sigma > bump) {
        let down = with_sigma(-bump)?;
        let pv_down = context.reprice_instrument_raw(&down, market, context.as_of)?;
        Ok((pv_up - pv_down) / (2.0 * bump) * 0.01)
    } else {
        let pv_base = context.reprice_instrument_raw(option, market, context.as_of)?;
        Ok((pv_up - pv_base) / bump * 0.01)
    }
}

#[cfg(test)]
mod tests {
    use crate::instruments::rates::cap_floor::pricing::{black, normal};
    use crate::instruments::rates::swaption::types::lognormal_to_normal_vol;

    /// A lognormal cap whose forward turns negative must still report a finite
    /// vega: the metric falls back to Bachelier with a converted normal vol,
    /// matching the pricer rather than producing a NaN.
    #[test]
    fn lognormal_vega_falls_back_to_bachelier_on_negative_forward() {
        let strike = 0.0;
        let forward = -0.005;
        let sigma = 0.20; // lognormal vol on the surface
        let t = 0.5;

        let vega = super::super::common::lognormal_vega_with_fallback(strike, forward, sigma, t);
        assert!(vega.is_finite(), "vega must be finite, got {vega}");

        // Equals the Bachelier vega computed with the converted normal vol.
        let normal_vol = lognormal_to_normal_vol(sigma, forward, strike, t, None);
        let expected = normal::vega_per_pct(strike, forward, normal_vol, t);
        assert!((vega - expected).abs() < 1e-15);

        // Positive forward stays on the Black path.
        let black_vega = super::super::common::lognormal_vega_with_fallback(0.03, 0.04, sigma, t);
        assert!((black_vega - black::vega_per_pct(0.03, 0.04, sigma, t)).abs() < 1e-15);
    }
}
