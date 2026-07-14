//! Gamma calculator for interest rate options (caps/floors/caplets/floorlets).

use crate::instruments::rates::cap_floor::{CapFloor, CapFloorVolType, RateOptionType};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

use super::common::CapletInputs;

/// Gamma calculator (model-consistent forward gamma, aggregated for caps/floors).
///
/// Dispatches to the appropriate model based on `vol_type`:
/// - `Lognormal`: Black-76 gamma = n(d₁) / (F·σ·√T)
/// - `ShiftedLognormal`: Black-76 gamma on shifted rates
/// - `Normal`: Bachelier gamma = n(d) / (σ·√T)
pub(crate) struct GammaCalculator;

impl MetricCalculator for GammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CapFloor = context.instrument_as()?;
        let strike = option.strike_f64()?;
        let vol_type = option.vol_type;
        let vol_shift = option.resolved_vol_shift();
        let is_cap = matches!(
            option.rate_option_type,
            RateOptionType::Caplet | RateOptionType::Cap
        );
        super::common::aggregate_over_caplets(option, context, |c: CapletInputs| {
            caplet_gamma(vol_type, is_cap, strike, vol_shift, c)
        })
    }
}

fn caplet_gamma(
    vol_type: CapFloorVolType,
    is_cap: bool,
    strike: f64,
    vol_shift: f64,
    c: CapletInputs,
) -> f64 {
    use super::common::{lognormal_delta_with_fallback, lognormal_gamma_with_fallback};
    use crate::instruments::rates::cap_floor::pricing::black;
    let (coupon_delta, coupon_gamma) = match vol_type {
        // `Auto` is a lognormal surface; both share the Black-with-Bachelier
        // fallback path so the Greek matches the pricer for any rate sign.
        CapFloorVolType::Lognormal | CapFloorVolType::Auto => (
            lognormal_delta_with_fallback(is_cap, strike, c.forward, c.sigma, c.fixing_t),
            lognormal_gamma_with_fallback(strike, c.forward, c.sigma, c.fixing_t),
        ),
        CapFloorVolType::ShiftedLognormal => (
            black::delta(
                is_cap,
                strike + vol_shift,
                c.forward + vol_shift,
                c.sigma,
                c.fixing_t,
            ),
            black::gamma(
                strike + vol_shift,
                c.forward + vol_shift,
                c.sigma,
                c.fixing_t,
            ),
        ),
        CapFloorVolType::Normal => (
            crate::instruments::rates::cap_floor::pricing::normal::delta(
                is_cap, strike, c.forward, c.sigma, c.fixing_t,
            ),
            crate::instruments::rates::cap_floor::pricing::normal::gamma(
                strike, c.forward, c.sigma, c.fixing_t,
            ),
        ),
    };
    coupon_gamma * c.forward_sensitivity * c.forward_sensitivity
        + coupon_delta * c.forward_second_sensitivity
}
