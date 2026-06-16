//! Bloomberg-screen gamma for [`CDSOption`].
//!
//! Bloomberg's CDSO terminal reports Γ as the (±5 bp) finite difference
//! of the Black-76 N(d₁) delta in the displayed ATM forward spread. This
//! module is the single source of truth; [`CDSOption::gamma`] is a thin
//! pass-through to [`gamma`].

use super::delta::{black_delta_ratio, delta_display_forward};
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::credit_derivatives::cds_option::bloomberg_quadrature::ForwardCdsContext;
use crate::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds;
use crate::instruments::credit_derivatives::cds_option::CDSOption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;

const GAMMA_SPREAD_BUMP: f64 = 0.0005;

/// Bloomberg-screen `Gamma (+/-5bps)` calculator for credit options on CDS spreads.
pub(crate) struct GammaCalculator;

impl MetricCalculator for GammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CDSOption = context.instrument_as()?;
        gamma(option, &context.curves, context.as_of)
    }
}

/// Bloomberg CDSO Γ — central difference of the Black-76 N(d₁) delta
/// across a ±5 bp move in the displayed ATM forward (DOCS 2055833 §2.5
/// screen-gamma convention).
pub(crate) fn gamma(
    option: &CDSOption,
    curves: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<f64> {
    option.validate_supported_configuration()?;
    let t = option.time_to_expiry(as_of)?;
    if t <= 0.0 {
        return Ok(0.0);
    }
    let strike = decimal_to_f64(option.strike, "strike")?;
    let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
        &option.pricing_overrides.market_quotes,
        curves,
        option.vol_surface_id.as_str(),
        t,
        strike,
    )?;
    let cds = synthetic_underlying_cds(option, as_of)?;
    let disc = curves.get_discount(&option.discount_curve_id)?;
    let surv = curves.get_hazard(&option.credit_curve_id)?;
    let ctx = ForwardCdsContext::build(option, disc.as_ref(), surv.as_ref(), &cds, as_of, sigma)?;
    let clean_forward = delta_display_forward(option, curves, &ctx, as_of)?;
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
