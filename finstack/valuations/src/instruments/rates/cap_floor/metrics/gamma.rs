//! Gamma calculator for interest rate options (caps/floors/caplets/floorlets).

use crate::instruments::rates::cap_floor::{CapFloor, CapFloorVolType};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

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
        super::common::aggregate_over_caplets(option, context, |c: CapletInputs| {
            caplet_gamma(vol_type, strike, vol_shift, c)
        })
    }
}

fn caplet_gamma(vol_type: CapFloorVolType, strike: f64, vol_shift: f64, c: CapletInputs) -> f64 {
    use crate::instruments::rates::cap_floor::pricing::{black, normal};
    match vol_type {
        CapFloorVolType::Lognormal => black::gamma(strike, c.forward, c.sigma, c.fixing_t),
        CapFloorVolType::ShiftedLognormal => black::gamma(
            strike + vol_shift,
            c.forward + vol_shift,
            c.sigma,
            c.fixing_t,
        ),
        CapFloorVolType::Normal => normal::gamma(strike, c.forward, c.sigma, c.fixing_t),
        CapFloorVolType::Auto => {
            if c.forward > 0.0 && strike > 0.0 {
                black::gamma(strike, c.forward, c.sigma, c.fixing_t)
            } else {
                normal::gamma(strike, c.forward, c.sigma, c.fixing_t)
            }
        }
    }
}
