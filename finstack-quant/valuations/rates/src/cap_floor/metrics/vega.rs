//! Vega calculator for interest rate options (caps/floors/caplets/floorlets).

use crate::cap_floor::CapFloor;
use crate::cap_floor::hw_pricer::resolve_capfloor_hw1f_model_params;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use finstack_quant_valuations_core::pricer::ModelKey;

use super::caplet::CapletInputs;

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
/// date (`c.time_to_fixing`) — the same time the pricer uses for both the vol-surface
/// lookup and the model `T`. Using a single time keeps vega consistent with the
/// reported price and with delta/gamma (a finite-difference vega reconciles with
/// the analytic one).
pub(crate) struct VegaCalculator;

/// Direct Hull-White short-rate σ sensitivity per 0.01 absolute σ.
///
/// Use this metric under [`ModelKey::HullWhite1F`] instead of [`VegaCalculator`],
/// because Hull-White cap/floor pricing consumes explicit short-rate parameters
/// rather than the market cap/floor volatility surface.
pub(crate) struct HwSigmaVegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CapFloor = context.instrument_as()?;
        if matches!(
            context.clone_pricer_dispatch().model(),
            Some(ModelKey::HullWhite1F)
        ) {
            return Err(finstack_quant_core::Error::Validation(
                "HullWhite1F cap/floor pricing consumes explicit short-rate parameters; use hw_sigma_vega, not surface vega".into(),
            ));
        }
        super::caplet::aggregate_over_caplets(option, context, |c: CapletInputs| caplet_vega(c))
    }
}

impl MetricCalculator for HwSigmaVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CapFloor = context.instrument_as()?;
        if !matches!(
            context.clone_pricer_dispatch().model(),
            Some(ModelKey::HullWhite1F)
        ) {
            return Err(finstack_quant_core::Error::Validation(
                "hw_sigma_vega requires HullWhite1F pricing".to_owned(),
            ));
        }
        hull_white_sigma_vega_per_pct(option, context)
    }
}

/// Caplet vega per 1% (absolute) vol, non-negative for long options.
///
/// At `sigma <= 0` the true vega is zero away from the money and only finite
/// exactly at-the-money; the closed forms report zero vega for a degenerate
/// (extrapolated) zero vol rather than the `n(0)` value an ATM-equivalent `d`
/// would give, which would overstate vega for ITM/OTM strikes.
fn caplet_vega(c: CapletInputs) -> f64 {
    use finstack_quant_models::closed_form::{bachelier_vega, black_vega};
    use finstack_quant_models::volatility::VolatilityConvention;
    match c.convention {
        VolatilityConvention::Normal => {
            bachelier_vega(c.forward, c.strike, c.sigma, c.time_to_fixing) / 100.0
        }
        _ => black_vega(c.forward, c.strike, c.sigma, c.time_to_fixing) / 100.0,
    }
}

fn hull_white_sigma_vega_per_pct(option: &CapFloor, context: &MetricContext) -> Result<f64> {
    let market = context.curves.as_ref();
    let bump = crate::HW_SIGMA_BUMP;
    let base = resolve_capfloor_hw1f_model_params(option, market, context.as_of)?;
    let use_central = base.get_values().iter().all(|sigma| *sigma > bump);
    crate::vol_model::hw_sigma_vega_fd(bump, use_central, |shift| {
        if shift == 0.0 {
            return context.reprice_instrument_raw(option, market, context.as_of);
        }
        let mut bumped = option.clone();
        bumped
            .instrument_pricing_overrides
            .model_config
            .hw1f_mean_reversion = Some(base.kappa);
        if !base.get_times().is_empty() {
            let values = base
                .get_values()
                .iter()
                .map(|sigma| sigma + shift)
                .collect();
            bumped.instrument_pricing_overrides.model_config.hw1f_sigma = None;
            bumped
                .instrument_pricing_overrides
                .model_config
                .hw1f_sigma_schedule = Some(
                finstack_quant_core::math::piecewise::PiecewiseConstantCurve::new(
                    base.get_times().to_vec(),
                    values,
                )?,
            );
        } else {
            bumped.instrument_pricing_overrides.model_config.hw1f_sigma =
                Some(base.get_values()[0] + shift);
        }
        context.reprice_instrument_raw(&bumped, market, context.as_of)
    })
}
