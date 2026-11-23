//! Rates-specific metric helpers for quote-recalibrated rate DV01 and HW1F rate Greeks.

use finstack_quant_core::Result;
use finstack_quant_core::market_data::bumps::BumpSpec;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::sensitivities::bump_config;
use finstack_quant_valuations_core::pricer::ModelKey;
use finstack_quant_valuations_core::recalibration::{QuoteBump, RateMarketRecalibrationRequest};
use std::sync::Arc;

/// Hull-White 1F rate delta or gamma, when the request selected that model.
///
/// Returns `None` unless the metric request prices with
/// [`ModelKey::HullWhite1F`], so the caller falls through to its closed-form
/// greek. Otherwise returns the central finite-difference derivative of the
/// selected model's raw PV to a parallel shift of `curve_id`, in PV per unit
/// decimal rate (first order) or per unit decimal rate squared (second order).
///
/// # Arguments
///
/// * `context` - Metric context whose pricing dispatch, market and bump
///   configuration (`rate_bump_bp`, in basis points) drive the repricing.
/// * `curve_id` - Curve shifted in parallel by ±`rate_bump_bp`.
/// * `second` - `false` for delta `(PV₊ − PV₋) / 2h`, `true` for gamma
///   `(PV₊ − 2·PV + PV₋) / h²`, with `h` the bump as a decimal rate.
///
/// # Errors
///
/// The inner result fails when the bump configuration cannot be resolved,
/// the curve cannot be bumped, or a repricing fails.
pub(crate) fn hw1f_rate_greek(
    context: &MetricContext,
    curve_id: &CurveId,
    second: bool,
) -> Option<Result<f64>> {
    if context.pricing_model() != Some(ModelKey::HullWhite1F) {
        return None;
    }
    Some(rate_greek(context, curve_id, second))
}

fn rate_greek(context: &MetricContext, curve_id: &CurveId, second: bool) -> Result<f64> {
    let config = bump_config::resolve(context)?;
    let bump_bp = config.rate_bump_bp;
    let h = bump_bp * 1e-4;
    let target = context.curves.curve_target(curve_id)?;
    let price = |direction: f64| {
        let market = context.curves.bump([target
            .clone()
            .into_bump(BumpSpec::parallel_bp(bump_bp * direction))])?;
        context.reprice_raw(&market, context.as_of)
    };
    let up = price(1.0)?;
    let down = price(-1.0)?;
    if second {
        let base = context.reprice_raw(&context.curves, context.as_of)?;
        Ok((up - 2.0 * base + down) / (h * h))
    } else {
        Ok((up - down) / (2.0 * h))
    }
}

/// Recalibrate linked discount and forward curves for a parallel quote bump.
///
/// # Arguments
///
/// * `context` - Metric context supplying base curves and recalibration provider.
/// * `discount_curve_id` - Discount curve identifier in `context.curves`.
/// * `forward_curve_id` - Linked projection curve identifier in `context.curves`.
/// * `bump_bp` - Parallel quote shock in basis points.
pub(crate) fn bump_rate_market_cached(
    context: &MetricContext,
    discount_curve_id: &CurveId,
    forward_curve_id: &CurveId,
    bump_bp: f64,
) -> Result<Arc<MarketContext>> {
    rebuild_rate_market(
        context,
        RateMarketRecalibrationRequest::LinkedDiscountForward {
            market: Arc::clone(&context.curves),
            discount_curve_id: discount_curve_id.clone(),
            forward_curve_id: forward_curve_id.clone(),
            bump: QuoteBump::ParallelBp(bump_bp),
        },
        "dv01",
    )
}

/// Recalibrate a single OIS curve for a parallel quote bump.
///
/// # Arguments
///
/// * `context` - Metric context supplying base curves and recalibration provider.
/// * `curve_id` - Single OIS discount/projection curve identifier.
/// * `bump_bp` - Parallel quote shock in basis points.
pub(crate) fn bump_single_ois_rate_market_cached(
    context: &MetricContext,
    curve_id: &CurveId,
    bump_bp: f64,
) -> Result<Arc<MarketContext>> {
    rebuild_rate_market(
        context,
        RateMarketRecalibrationRequest::SingleOis {
            market: Arc::clone(&context.curves),
            curve_id: curve_id.clone(),
            bump: QuoteBump::ParallelBp(bump_bp),
        },
        "dv01",
    )
}

/// Execute one rate-market replay through the context's injected provider.
///
/// # Arguments
///
/// * `context` - Metric context carrying the recalibration provider.
/// * `request` - Rate-market recalibration request.
/// * `operation` - Diagnostic operation label surfaced when no provider is attached.
pub(crate) fn rebuild_rate_market(
    context: &MetricContext,
    request: RateMarketRecalibrationRequest,
    operation: &str,
) -> Result<Arc<MarketContext>> {
    context
        .recalibration_provider_ref(operation)?
        .rebuild_rate_market(&request)
}
