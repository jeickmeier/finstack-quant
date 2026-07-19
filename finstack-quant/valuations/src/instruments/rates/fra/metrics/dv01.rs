//! FRA quote-shock DV01.
//!
//! When the discount and forward curves carry rate-calibration metadata, this
//! reports quote-shock/rebootstrap DV01: the shared
//! `bump_market_via_rate_quote_shock` helper handles the standard
//! (deposit/FRA/swap) case, while a local basis rebuild handles forward curves
//! calibrated from tenor-basis quotes (which the shared helper does not
//! support). When metadata is unavailable, falls back to the generic
//! fitted-curve bump path.

use crate::calibration::bumps::BumpRequest;
use crate::instruments::rates::fra::ForwardRateAgreement;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::sensitivity_central_diff;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::{DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    ForwardCurve, ForwardCurveRateCalibration, ForwardCurveRateQuote,
};
use finstack_quant_core::Result;

const BASIS_FRONT_STUB_ANCHOR_DIVISOR: f64 = 6.60;

/// FRA DV01 calculator. Prefers quote-shock/rebootstrap when calibration
/// metadata is available, falling back to the generic fitted-curve bump.
pub(crate) struct FraRateCurveDv01Calculator;

impl MetricCalculator for FraRateCurveDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fra: &ForwardRateAgreement = context.instrument_as()?;
        let market = context.curves.as_ref();
        let discount = market.get_discount(fra.discount_curve_id.as_str())?;
        let forward = market.get_forward(fra.forward_curve_id.as_str())?;

        let discount_cal = discount.rate_calibration();
        if discount_cal.is_none() && discount.rate_calibration_recipe().is_none() {
            return generic_fallback(context);
        }
        let forward_cal = forward.rate_calibration();
        if forward_cal.is_none() && forward.rate_calibration_recipe().is_none() {
            return generic_fallback(context);
        }

        let bump_bp =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?
                .rate_bump_bp;

        let discount_id = &fra.discount_curve_id;
        let forward_id = &fra.forward_curve_id;

        if let Some(forward_cal) = forward_cal {
            if uses_basis_quotes(forward_cal) {
                let Some(discount_cal) = discount_cal else {
                    return generic_fallback(context);
                };
                // Basis forwards aren't supported by the shared helper; use the
                // shared discount path and rebuild the forward locally.
                let make_market = |bp: f64| -> Result<MarketContext> {
                    let bumped_discount = context.bump_discount_rate_quotes_cached(
                        discount.as_ref(),
                        discount_cal,
                        &BumpRequest::Parallel(bp),
                    )?;
                    let with_discount = market.clone().insert(bumped_discount.as_ref().clone());
                    let bumped_discount_ref = with_discount.get_discount(discount_id.as_str())?;
                    let rebuilt_forward = rebuild_forward_curve_from_basis_quotes(
                        forward.as_ref(),
                        forward_cal,
                        bumped_discount_ref.as_ref(),
                        bp,
                    )?;
                    Ok(with_discount.insert(rebuilt_forward))
                };
                let pv_up = context.reprice_raw(&make_market(bump_bp)?, context.as_of)?;
                let pv_down = context.reprice_raw(&make_market(-bump_bp)?, context.as_of)?;
                return Ok(sensitivity_central_diff(pv_up, pv_down, bump_bp));
            }
        }

        let bumped_up = context.bump_rate_market_cached(discount_id, forward_id, bump_bp)?;
        let pv_up = context.reprice_raw(bumped_up.as_ref(), context.as_of)?;
        let bumped_down = context.bump_rate_market_cached(discount_id, forward_id, -bump_bp)?;
        let pv_down = context.reprice_raw(bumped_down.as_ref(), context.as_of)?;
        Ok(sensitivity_central_diff(pv_up, pv_down, bump_bp))
    }
}

fn generic_fallback(context: &mut MetricContext) -> Result<f64> {
    crate::metrics::UnifiedDv01Calculator::<ForwardRateAgreement>::new(
        crate::metrics::Dv01CalculatorConfig::parallel_combined(),
    )
    .calculate(context)
}

fn uses_basis_quotes(calibration: &ForwardCurveRateCalibration) -> bool {
    calibration
        .quotes
        .iter()
        .any(|quote| matches!(quote, ForwardCurveRateQuote::Basis { .. }))
}

/// Rebuild a basis-calibrated forward curve under a parallel quote shock.
///
/// Basis-quoted forwards are anchored to the discount curve; a parallel shift
/// on the basis quote moves the projection curve relative to discount. The
/// shared rebootstrap path doesn't model this, so we synthesize a small set of
/// shocked knots that reproduce Bloomberg-style short-end basis risk:
///
/// - **Deposit / Swap / FRA** quotes: shock the rate by `bump_bp / 10_000` and
///   anchor at the corresponding tenor time.
/// - **Basis** quotes: blend the period-rate and maturity-rate interpretations
///   of the basis-point shock using a Bloomberg-derived front-stub anchor; the
///   blended `reference_rate` is added to the spread and anchored at the
///   period start.
fn rebuild_forward_curve_from_basis_quotes(
    base_curve: &ForwardCurve,
    calibration: &ForwardCurveRateCalibration,
    discount_curve: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    bump_bp: f64,
) -> Result<ForwardCurve> {
    let mut points = Vec::with_capacity(calibration.quotes.len().max(2));
    for quote in &calibration.quotes {
        match quote {
            ForwardCurveRateQuote::Deposit { tenor, rate } => {
                let t = tenor_time(base_curve, tenor)?;
                points.push((0.0, rate + bump_bp / 10_000.0));
                points.push((t, rate + bump_bp / 10_000.0));
            }
            ForwardCurveRateQuote::Basis {
                tenor,
                spread_decimal,
            } => {
                let maturity_t = tenor_time(base_curve, tenor)?;
                let start_t = (maturity_t - base_curve.tenor()).max(0.0);
                let end_t = maturity_t.max(start_t + 1e-8);
                let tau = end_t - start_t;
                let period_rate =
                    (discount_curve.df(start_t) / discount_curve.df(end_t) - 1.0) / tau;
                let maturity_rate = (1.0 / discount_curve.df(end_t) - 1.0) / tau;
                let anchor_t = base_curve.knots().get(1).copied().unwrap_or(start_t);
                // Bloomberg's short basis screen anchors front-stub risk between the
                // reset-period and maturity-rate interpretations of a basis point.
                let front_stub_anchor =
                    anchor_t + base_curve.tenor() / BASIS_FRONT_STUB_ANCHOR_DIVISOR;
                let maturity_weight = if end_t > 1e-10 {
                    (front_stub_anchor / end_t).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let reference_rate =
                    period_rate.mul_add(1.0 - maturity_weight, maturity_rate * maturity_weight);
                points.push((start_t, reference_rate + *spread_decimal));
            }
            ForwardCurveRateQuote::Fra { start, rate, .. } => {
                let start_t = base_curve.day_count().year_fraction(
                    base_curve.base_date(),
                    *start,
                    DayCountContext::default(),
                )?;
                points.push((start_t, rate + bump_bp / 10_000.0));
            }
            ForwardCurveRateQuote::Swap { tenor, rate, .. } => {
                let t = tenor_time(base_curve, tenor)?;
                points.push((t, rate + bump_bp / 10_000.0));
            }
        }
    }

    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    points.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-10);
    if points.len() < 2 {
        points.push((base_curve.tenor(), base_curve.rate(base_curve.tenor())));
    }

    ForwardCurve::builder(base_curve.id().clone(), base_curve.tenor())
        .base_date(base_curve.base_date())
        .reset_lag(base_curve.reset_lag())
        .day_count(base_curve.day_count())
        .knots(points)
        .interp(base_curve.interp_style())
        .extrapolation(base_curve.extrapolation())
        .rate_calibration_opt(base_curve.rate_calibration().cloned())
        .build()
}

fn tenor_time(base_curve: &ForwardCurve, tenor: &str) -> Result<f64> {
    let parsed: Tenor = tenor.parse().map_err(|err| {
        finstack_quant_core::Error::Validation(format!("invalid rate quote tenor {tenor:?}: {err}"))
    })?;
    let maturity = parsed.add_to_date(
        base_curve.base_date(),
        None,
        finstack_quant_core::dates::BusinessDayConvention::Following,
    )?;
    base_curve.day_count().year_fraction(
        base_curve.base_date(),
        maturity,
        DayCountContext::default(),
    )
}
