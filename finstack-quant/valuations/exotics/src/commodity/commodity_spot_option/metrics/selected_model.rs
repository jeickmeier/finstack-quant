//! Commodity spot-option greeks under a selected (non-native) pricing model.
//!
//! Delta and gamma under the Schwartz-Smith Monte Carlo model are central
//! differences in the quoted forward through the context's repricing
//! dispatch; volatility risk needs explicit model-parameter bumps and is
//! rejected.

use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::common_impl::traits::OptionGreekKind;
use finstack_quant_valuations_core::metrics::MetricContext;

/// Commodity delta/gamma retain their published forward-price coordinate.
/// MC parameters and random numbers remain owned by the selected registry.
pub(crate) fn selected_commodity_greek(
    kind: OptionGreekKind,
    context: &MetricContext,
) -> Result<f64> {
    use crate::commodity::commodity_spot_option::CommoditySpotOption;
    if !matches!(kind, OptionGreekKind::Delta | OptionGreekKind::Gamma) {
        return Err(finstack_quant_core::Error::Validation(
            "Schwartz-Smith volatility risk requires explicit sigma_x/sigma_y parameter bumps in the registered model; Black surface vega, vanna and volga are not supported".into(),
        ));
    }
    let option: &CommoditySpotOption = context.instrument_as()?;
    let forward = option.forward_price(&context.curves, context.as_of)?;
    let config = finstack_quant_valuations_core::metrics::sensitivities::config::resolve(context)?;
    let h = (forward * config.spot_bump_decimal)
        .max(1e-6)
        .min(forward * 0.5);
    let evaluate = |shift: f64| {
        let mut bumped = option.clone();
        bumped.quoted_forward = Some(forward + shift);
        context.reprice_instrument_raw(&bumped, &context.curves, context.as_of)
    };
    let up = evaluate(h)?;
    let down = evaluate(-h)?;
    if kind == OptionGreekKind::Delta {
        Ok((up - down) / (2.0 * h))
    } else {
        Ok((up - 2.0 * evaluate(0.0)? + down) / (h * h))
    }
}
