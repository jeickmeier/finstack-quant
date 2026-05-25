//! Delta calculator for interest rate options (caps/floors/caplets/floorlets).

use crate::instruments::rates::cap_floor::{CapFloor, CapFloorVolType, RateOptionType};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

use super::common::CapletInputs;

/// Delta calculator (model-consistent forward delta, aggregated for caps/floors).
///
/// Dispatches to the appropriate model based on `vol_type`:
/// - `Lognormal`: Black-76 delta = N(d₁)
/// - `ShiftedLognormal`: Black-76 delta on shifted rates
/// - `Normal`: Bachelier delta = N(d)
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
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
            caplet_delta(vol_type, is_cap, strike, vol_shift, c)
        })
    }
}

fn caplet_delta(
    vol_type: CapFloorVolType,
    is_cap: bool,
    strike: f64,
    vol_shift: f64,
    c: CapletInputs,
) -> f64 {
    use crate::instruments::rates::cap_floor::pricing::{black, normal};
    match vol_type {
        CapFloorVolType::Lognormal => black::delta(is_cap, strike, c.forward, c.sigma, c.fixing_t),
        CapFloorVolType::ShiftedLognormal => black::delta(
            is_cap,
            strike + vol_shift,
            c.forward + vol_shift,
            c.sigma,
            c.fixing_t,
        ),
        CapFloorVolType::Normal => normal::delta(is_cap, strike, c.forward, c.sigma, c.fixing_t),
        CapFloorVolType::Auto => {
            if c.forward > 0.0 && strike > 0.0 {
                black::delta(is_cap, strike, c.forward, c.sigma, c.fixing_t)
            } else {
                normal::delta(is_cap, strike, c.forward, c.sigma, c.fixing_t)
            }
        }
    }
}
