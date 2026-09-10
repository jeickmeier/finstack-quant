//! Delta calculator for interest rate options (caps/floors/caplets/floorlets).

use crate::instruments::rates::cap_floor::{CapFloor, RateOptionType};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

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
        let is_cap = matches!(
            option.rate_option_type,
            RateOptionType::Caplet | RateOptionType::Cap
        );
        super::common::aggregate_over_caplets(option, context, |c: CapletInputs| {
            caplet_delta(is_cap, c)
        })
    }
}

fn caplet_delta(is_cap: bool, c: CapletInputs) -> f64 {
    use crate::instruments::rates::cap_floor::pricing::{black, normal};
    use finstack_quant_models::volatility::VolatilityConvention;
    let coupon_delta = match c.convention {
        VolatilityConvention::Normal => {
            normal::delta(is_cap, c.strike, c.forward, c.sigma, c.fixing_t)
        }
        _ => black::delta(is_cap, c.strike, c.forward, c.sigma, c.fixing_t),
    };
    coupon_delta * c.forward_sensitivity
}
