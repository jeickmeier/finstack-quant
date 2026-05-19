//! Gamma metric for `CDSOption`.

use super::delta::{black_delta_ratio, delta_display_forward};
use crate::instruments::credit_derivatives::cds_option::bloomberg_quadrature::ForwardCdsContext;
use crate::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds;
use crate::instruments::credit_derivatives::cds_option::CDSOption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;
use rust_decimal::prelude::ToPrimitive;

const GAMMA_SPREAD_BUMP: f64 = 0.0005;

/// Bloomberg-screen `Gamma (+/-5bps)` calculator for credit options on CDS spreads.
pub(crate) struct GammaCalculator;

impl MetricCalculator for GammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CDSOption = context.instrument_as()?;
        let t = option.time_to_expiry(context.as_of)?;
        if t <= 0.0 {
            return Ok(0.0);
        }
        let strike = option.strike.to_f64().unwrap_or(0.0);
        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &option.pricing_overrides.market_quotes,
            &context.curves,
            option.vol_surface_id.as_str(),
            t,
            strike,
        )?;
        let cds = synthetic_underlying_cds(option, context.as_of)?;
        let disc = context.curves.get_discount(&option.discount_curve_id)?;
        let surv = context.curves.get_hazard(&option.credit_curve_id)?;
        let ctx = ForwardCdsContext::build(
            option,
            disc.as_ref(),
            surv.as_ref(),
            &cds,
            context.as_of,
            sigma,
        )?;
        let clean_forward = delta_display_forward(option, &context.curves, &ctx, context.as_of)?;
        let up = black_delta_ratio(
            option.option_type,
            clean_forward + GAMMA_SPREAD_BUMP,
            ctx.strike,
            sigma,
            t,
        );
        let down = black_delta_ratio(
            option.option_type,
            (clean_forward - GAMMA_SPREAD_BUMP).max(1e-12),
            ctx.strike,
            sigma,
            t,
        );
        Ok(up - down)
    }
}
