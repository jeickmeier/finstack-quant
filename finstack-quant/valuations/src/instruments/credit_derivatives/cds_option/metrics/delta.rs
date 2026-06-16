//! Bloomberg-screen delta for [`CDSOption`].
//!
//! Bloomberg's CDSO terminal reports Δ as the Black-76 N(d₁) sensitivity
//! of the option premium to the displayed ATM forward spread (DOCS
//! 2055833 §2.5 in conjunction with the closed-form lognormal-spread
//! identity). This module is the single source of truth for that value;
//! [`CDSOption::delta`] is a thin pass-through to [`delta`].

use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::credit_derivatives::cds::pricer::CDSPricer;
use crate::instruments::credit_derivatives::cds_option::bloomberg_quadrature::ForwardCdsContext;
use crate::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds;
use crate::instruments::credit_derivatives::cds_option::CDSOption;
use crate::instruments::OptionType;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;

/// Bloomberg-screen Delta calculator for credit options on CDS spreads.
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CDSOption = context.instrument_as()?;
        delta(option, &context.curves, context.as_of)
    }
}

/// Bloomberg CDSO Δ — closed-form Black-76 N(d₁) on the displayed ATM
/// forward spread (Bloomberg CDSO methodology / DOCS 2055833).
///
/// Returned as a unit-less ratio (multiply by 100 for the displayed
/// percentage). Calls Δ ≥ 0, puts Δ ≤ 0.
pub(crate) fn delta(
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
    Ok(black_delta_ratio(
        option.option_type,
        clean_forward,
        ctx.strike,
        sigma,
        t,
    ))
}

pub(super) fn delta_display_forward(
    option: &CDSOption,
    curves: &MarketContext,
    ctx: &ForwardCdsContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<f64> {
    let cds = synthetic_underlying_cds(option, as_of)?;
    let disc = curves.get_discount(&option.discount_curve_id)?;
    let surv = curves.get_hazard(&option.credit_curve_id)?;
    // Bloomberg CDSO Default_Leg(0, T_mat): the synthetic CDS carries the
    // `BloombergCdswClean` convention, so protection integrates from the
    // valuation date itself (step-in 0).
    let spot_protection_pv = CDSPricer::new()
        .pv_protection_leg(&cds, disc.as_ref(), surv.as_ref(), as_of)?
        .amount();
    let denom = ctx.df_to_expiry
        * ctx.survival_to_expiry
        * ctx.bootstrapped_l_at_expiry
        * cds.notional.amount();
    if denom <= 1e-12 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "degenerate CDS option delta display-forward denominator: id={}, denom={denom:.6e}",
            option.id,
        )));
    }
    Ok(spot_protection_pv / denom)
}

pub(super) fn black_delta_ratio(
    option_type: OptionType,
    forward: f64,
    strike: f64,
    sigma: f64,
    t: f64,
) -> f64 {
    if forward <= 0.0 || strike <= 0.0 || sigma <= 0.0 || t <= 0.0 {
        return 0.0;
    }
    let sigma_sqrt_t = sigma * t.sqrt();
    if sigma_sqrt_t <= 1e-12 {
        return 0.0;
    }
    let d1 = ((forward / strike).ln() + 0.5 * sigma_sqrt_t * sigma_sqrt_t) / sigma_sqrt_t;

    match option_type {
        OptionType::Call => finstack_quant_core::math::norm_cdf(d1),
        OptionType::Put => -finstack_quant_core::math::norm_cdf(-d1),
    }
}
