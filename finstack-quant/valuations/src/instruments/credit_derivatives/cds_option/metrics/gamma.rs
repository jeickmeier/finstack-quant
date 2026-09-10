//! Gamma for [`CDSOption`], branched by strike kind.
//!
//! - **Spread strikes**: Bloomberg's CDSO terminal reports Γ as the
//!   (±5 bp) finite difference of the Black-76 N(d₁) delta in the
//!   displayed ATM forward spread.
//! - **Clean-price strikes**: a clean price is never fed into the Black
//!   spread `d₁`; Γ is the change in the curve-reprice price-strike delta
//!   under the same ±5 bp screen spread bump, applied as a par-quote
//!   hazard bump with rebootstrap and sticky-strike/sticky-surface rules.
//!
//! This module is the single source of truth; [`CDSOption::gamma`] is a
//! thin pass-through to [`gamma`].

use super::delta::{black_delta_ratio, price_strike_delta};
use crate::instruments::credit_derivatives::cds_option::bloomberg_quadrature::ForwardCdsContext;
use crate::instruments::credit_derivatives::cds_option::pricer::{
    resolve_sigma, synthetic_underlying_cds,
};
use crate::instruments::credit_derivatives::cds_option::{CDSOption, CDSOptionStrikeKind};
use crate::metrics::{MetricCalculator, MetricContext};
use crate::recalibration::{
    HazardRecalibrationAction, HazardRecalibrationRequest, QuoteBump, RecalibrationProvider,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;

/// Screen gamma bump in forward-spread units (5 bp), for the Black `d₁`
/// spread-strike branch.
const GAMMA_SPREAD_BUMP: f64 = 0.0005;
/// Screen gamma bump in basis points, for the price-strike curve-reprice
/// branch (the same ±5 bp move expressed as a par-quote bump).
const PRICE_GAMMA_SPREAD_BUMP_BP: f64 = 5.0;

/// Bloomberg-screen `Gamma (+/-5bps)` calculator for credit options on CDS spreads.
pub(crate) struct GammaCalculator;

impl MetricCalculator for GammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CDSOption = context.instrument_as()?;
        let provider = if option.strike.kind() == CDSOptionStrikeKind::CleanPricePct {
            Some(context.recalibration_provider("cds_option_gamma")?)
        } else {
            None
        };
        gamma_with_provider(option, &context.curves, context.as_of, provider.as_deref())
    }
}

/// CDS option Γ, branched by strike kind (see the module docs).
pub(crate) fn gamma(
    option: &CDSOption,
    curves: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<f64> {
    gamma_with_provider(option, curves, as_of, None)
}

fn gamma_with_provider(
    option: &CDSOption,
    curves: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
    provider: Option<&dyn RecalibrationProvider>,
) -> Result<f64> {
    option.validate_supported_configuration()?;
    let t = option.time_to_expiry(as_of)?;
    if t <= 0.0 {
        return Ok(0.0);
    }
    match option.strike.kind() {
        CDSOptionStrikeKind::Spread => spread_black_gamma(option, curves, as_of, t),
        CDSOptionStrikeKind::CleanPricePct => {
            let provider = provider
                .ok_or_else(|| crate::recalibration::provider_missing("cds_option_gamma"))?;
            price_strike_gamma(option, curves, as_of, provider)
        }
    }
}

/// Bloomberg CDSO Γ — central difference of the Black-76 N(d₁) delta
/// across a ±5 bp move in the displayed ATM forward (spread strikes).
fn spread_black_gamma(
    option: &CDSOption,
    curves: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
    t: f64,
) -> Result<f64> {
    let sigma = resolve_sigma(option, curves, as_of)?;
    let cds = synthetic_underlying_cds(option, as_of)?;
    let disc = curves.get_discount(&option.discount_curve_id)?;
    let surv = curves.get_hazard(&option.credit_curve_id)?;
    let ctx = ForwardCdsContext::build(option, disc.as_ref(), surv.as_ref(), &cds, as_of, sigma)?;
    let clean_forward = ctx.forward_par_spread;
    let up = black_delta_ratio(
        option.option_type,
        clean_forward + GAMMA_SPREAD_BUMP,
        ctx.spread_strike()?,
        sigma,
        t,
    );
    let down = black_delta_ratio(
        option.option_type,
        (clean_forward - GAMMA_SPREAD_BUMP).max(1e-12),
        ctx.spread_strike()?,
        sigma,
        t,
    );
    Ok(up - down)
}

/// Price-strike Γ — change in the curve-reprice hedge-ratio delta across
/// a ±5 bp par-quote spread bump with rebootstrap. The surface is not
/// bumped, so each nested delta resolves the same sticky volatility.
fn price_strike_gamma(
    option: &CDSOption,
    curves: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
    provider: &dyn RecalibrationProvider,
) -> Result<f64> {
    let hazard = curves.get_hazard(&option.credit_curve_id)?;
    crate::metrics::sensitivities::cs01::require_hazard_replay(
        hazard.as_ref(),
        "CDS option price-strike gamma",
    )?;
    let bumped_market = |bump_bp: f64| -> Result<MarketContext> {
        let bumped = provider.rebuild_hazard_curve(&HazardRecalibrationRequest {
            hazard: std::sync::Arc::clone(&hazard),
            source_market: std::sync::Arc::new(curves.clone()),
            target_market: std::sync::Arc::new(curves.clone()),
            discount_curve_id: option.discount_curve_id.clone(),
            doc_clause: None,
            cds_valuation_convention: None,
            deal_quote_override: None,
            action: HazardRecalibrationAction::SpreadBump(QuoteBump::ParallelBp(bump_bp)),
        })?;
        Ok(curves.clone().insert(bumped.as_ref().clone()))
    };
    let delta_up = price_strike_delta(
        option,
        &bumped_market(PRICE_GAMMA_SPREAD_BUMP_BP)?,
        as_of,
        provider,
    )?;
    let delta_down = price_strike_delta(
        option,
        &bumped_market(-PRICE_GAMMA_SPREAD_BUMP_BP)?,
        as_of,
        provider,
    )?;
    Ok(delta_up - delta_down)
}
