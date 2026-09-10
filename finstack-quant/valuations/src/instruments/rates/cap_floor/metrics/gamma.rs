//! Gamma calculator for interest rate options (caps/floors/caplets/floorlets).

use crate::instruments::rates::cap_floor::{CapFloor, RateOptionType};
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
        let is_cap = matches!(
            option.rate_option_type,
            RateOptionType::Caplet | RateOptionType::Cap
        );
        super::common::aggregate_over_caplets(option, context, |c: CapletInputs| {
            caplet_gamma(is_cap, c)
        })
    }
}

fn caplet_gamma(is_cap: bool, c: CapletInputs) -> f64 {
    use crate::instruments::rates::cap_floor::pricing::{black, normal};
    use finstack_quant_models::volatility::VolatilityConvention;
    let (delta, gamma) = match c.convention {
        VolatilityConvention::Normal => (
            normal::delta(is_cap, c.strike, c.forward, c.sigma, c.fixing_t),
            normal::gamma(c.strike, c.forward, c.sigma, c.fixing_t),
        ),
        _ => (
            black::delta(is_cap, c.strike, c.forward, c.sigma, c.fixing_t),
            black::gamma(c.strike, c.forward, c.sigma, c.fixing_t),
        ),
    };
    gamma * c.forward_sensitivity * c.forward_sensitivity + delta * c.forward_second_sensitivity
}
