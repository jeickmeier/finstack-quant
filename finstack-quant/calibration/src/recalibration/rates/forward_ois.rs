//! Forward-curve and linked single-curve OIS quote-space recalibration.

use super::discount::*;
use crate::CalibrationConfig;
use crate::api::schema::{ForwardCurveParams, StepParams};
use crate::quotes::MarketQuote;
use crate::quotes::RateQuote;
use crate::step_runtime;
use crate::targets::rate_recipe::{original_execution, rate_quotes_from_recipe};
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
#[cfg(test)]
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, ForwardCurve, RateCalibrationCurveRole, RateCalibrationRecipe, RateQuoteBinding,
};
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::recalibration::QuoteBump;
use std::sync::Arc;
use time::Duration;

/// Bump a forward curve by shocking its stored market-rate calibration quotes
/// and globally recalibrating against source and target dependency markets.
///
/// Both contexts must contain the discount curve referenced by the calibration
/// recipe. The unbumped solve uses `source_context`; the bumped solve uses
/// `target_context`, including any simultaneous discount-curve shock. Supply
/// the same context for both when only projection quotes change. Basis-tenor
/// calibrations use their dedicated forward rebuild.
///
/// An accepted unshocked solve must reproduce the source factors before the
/// accepted target-market calibration can supply the shocked curve.
///
/// # Arguments
///
/// * `curve` - Stored forward curve whose factors must reproduce the unshocked fit.
/// * `calibration` - Exact typed recipe retained when the curve was calibrated.
/// * `source_context` - Unshocked dependency market belonging to the stored curve.
/// * `target_context` - Dependency market to use after the quote shock, including
///   the bumped linked discount curve when both curves change.
/// * `bump` - Parallel or tenor-specific quote shock in basis points.
pub(crate) fn bump_forward_curve_from_rate_calibration(
    curve: &ForwardCurve,
    calibration: &RateCalibrationRecipe,
    source_context: &MarketContext,
    target_context: &MarketContext,
    bump: &QuoteBump,
) -> finstack_quant_core::Result<ForwardCurve> {
    bump.validate()?;
    bump_forward_curve_with_shock(
        curve,
        calibration,
        source_context,
        target_context,
        RateQuoteShock::Bucket(bump),
    )
}

/// Rebuild a term projection curve after repricing one exact recorded rate quote.
///
/// # Arguments
/// * `curve` - Original calibrated projection curve.
/// * `context` - Immutable center market, including the linked discount curve and fixings.
/// * `target` - Stable quote ID and executed contractual maturity.
/// * `bp` - Additive basis-point shock to the selected rate.
///
/// # Errors
/// Rejects a mismatched quote identity, an unqualified source solve, or failed
/// repricing. A linked single-curve OIS projection requires one joint replay.
pub fn bump_forward_rate_quote(
    curve: &ForwardCurve,
    context: &MarketContext,
    target: &CalibrationQuoteCoordinate,
    bp: f64,
) -> finstack_quant_core::Result<ForwardCurve> {
    let calibration = required_forward_rate_calibration(curve)?;
    if calibration.ois_compounding.is_some() {
        return Err(finstack_quant_core::Error::Validation(
            "linked single-curve OIS projection requires a joint rate quote replay".into(),
        ));
    }
    bump_forward_curve_with_shock(
        curve,
        calibration,
        context,
        context,
        RateQuoteShock::Exact { target, bp },
    )
}

pub(super) fn bump_forward_curve_with_shock(
    curve: &ForwardCurve,
    calibration: &RateCalibrationRecipe,
    source_context: &MarketContext,
    target_context: &MarketContext,
    bump: RateQuoteShock<'_>,
) -> finstack_quant_core::Result<ForwardCurve> {
    ensure_recipe_has_quotes(curve.id(), calibration)?;
    let quotes = rate_quotes_from_recipe(calibration, curve.id())?;

    let StepParams::Forward(params) = replay_step(calibration)? else {
        return Err(finstack_quant_core::Error::Validation(
            "projection curve requires its original projection evaluator".into(),
        ));
    };
    if params.curve_id != *curve.id()
        || params.base_date != curve.base_date()
        || params.index_tenor != curve.index_tenor()
        || params.interpolation != curve.interp_style()
        || params.extrapolation != curve.extrapolation()
        || calibration.curve_day_count != curve.day_count()
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "projection curve {} differs from its recorded evaluator",
            curve.id()
        )));
    }
    let cfg = replay_config(calibration)?;
    let unbumped = rebootstrap_forward_curve(quotes.clone(), &params, source_context, &cfg)?;
    verify_replayed_bindings(calibration, required_forward_rate_calibration(&unbumped)?)?;
    let tolerance = cfg.forward_curve.validation_tolerance;
    for &time in curve.knots().iter().chain(unbumped.knots()) {
        verify_source_factor(
            curve.id(),
            time,
            curve.df(time)?,
            unbumped.df(time)?,
            tolerance,
        )?;
    }
    let quotes = bump.apply(
        curve.id(),
        quotes,
        &original_execution(calibration)?.bindings,
    )?;
    let bumped = rebootstrap_forward_curve(quotes, &params, target_context, &cfg)?;
    if let Some(policy) = curve.fx_policy() {
        return bumped
            .to_builder_with_id(curve.id().clone())
            .fx_policy(policy)
            .build();
    }
    Ok(bumped)
}

/// Recalibrate a forward curve from the supplied quotes under the original evaluator.
pub(super) fn rebootstrap_forward_curve(
    quotes: Vec<RateQuote>,
    params: &ForwardCurveParams,
    context: &MarketContext,
    cfg: &CalibrationConfig,
) -> finstack_quant_core::Result<ForwardCurve> {
    let market_quotes: Vec<MarketQuote> = quotes.into_iter().map(MarketQuote::Rates).collect();
    let step = StepParams::Forward(params.clone());
    let (ctx, report) =
        step_runtime::execute_params_and_apply(&step, &market_quotes, context, cfg)?;
    super::super::ensure_replay_fit_accepted(params.curve_id.as_str(), cfg, &report)?;
    Ok(ctx.get_forward(params.curve_id.as_str())?.as_ref().clone())
}

pub(super) fn has_linked_single_curve_ois_recipes(
    discount: &DiscountCurve,
    forward: &ForwardCurve,
    discount_curve_id: &CurveId,
    forward_curve_id: &CurveId,
) -> finstack_quant_core::Result<bool> {
    let discount_recipe = discount.rate_calibration();
    let forward_recipe = forward.rate_calibration();
    // A term-index projection normally points at its discount curve too. The
    // OIS compounding marker plus a role pointing at the other representation
    // is what declares that this pair participates in shared single-curve
    // replay. Once either side makes that declaration, reciprocity is required.
    let discount_declares_link = discount_recipe.is_some_and(|recipe| {
        recipe.ois_compounding.is_some()
            && matches!(
                &recipe.role,
                RateCalibrationCurveRole::Discount {
                    projection_curve_id
                } if projection_curve_id == forward_curve_id
            )
    });
    let forward_declares_link = forward_recipe.is_some_and(|recipe| {
        recipe.ois_compounding.is_some()
            && matches!(
                &recipe.role,
                RateCalibrationCurveRole::Projection {
                    discount_curve_id: linked_discount_curve_id
                } if linked_discount_curve_id == discount_curve_id
            )
    });

    if !discount_declares_link && !forward_declares_link {
        return Ok(false);
    }
    if !discount_declares_link {
        return Err(finstack_quant_core::Error::Validation(format!(
            "projection curve {forward_curve_id} declares a linked single-curve OIS recipe \
             with discount curve {discount_curve_id}, but the discount recipe is missing or \
             does not reciprocally link projection curve {forward_curve_id}"
        )));
    }
    if !forward_declares_link {
        return Err(finstack_quant_core::Error::Validation(format!(
            "discount curve {discount_curve_id} declares a linked single-curve OIS recipe \
             with projection curve {forward_curve_id}, but the projection recipe is missing or \
             does not reciprocally link discount curve {discount_curve_id}"
        )));
    }

    let discount_recipe = discount_recipe.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "linked single-curve OIS recipe for {discount_curve_id}/{forward_curve_id} \
             is missing its discount representation"
        ))
    })?;
    let forward_recipe = forward_recipe.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "linked single-curve OIS recipe for {discount_curve_id}/{forward_curve_id} \
             is missing its projection representation"
        ))
    })?;
    if discount_recipe.ois_compounding != forward_recipe.ois_compounding {
        return Err(finstack_quant_core::Error::Validation(format!(
            "linked single-curve OIS recipe for {discount_curve_id}/{forward_curve_id} \
             has inconsistent OIS compounding"
        )));
    }
    if discount_recipe.currency != forward_recipe.currency {
        return Err(finstack_quant_core::Error::Validation(format!(
            "linked single-curve OIS recipe for {discount_curve_id}/{forward_curve_id} \
             has inconsistent currencies"
        )));
    }
    if discount_recipe.curve_day_count != forward_recipe.curve_day_count {
        return Err(finstack_quant_core::Error::Validation(format!(
            "linked single-curve OIS recipe for {discount_curve_id}/{forward_curve_id} \
             has inconsistent curve day counts"
        )));
    }
    if discount_recipe.quotes != forward_recipe.quotes {
        return Err(finstack_quant_core::Error::Validation(format!(
            "linked single-curve OIS recipe for {discount_curve_id}/{forward_curve_id} \
             must carry the same shared quote set on both representations"
        )));
    }
    Ok(true)
}

pub(super) fn date_from_forward_time(
    curve: &ForwardCurve,
    time: f64,
) -> finstack_quant_core::Result<Date> {
    if !time.is_finite() || time < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "cannot map invalid forward-curve time {time} to a calendar date"
        )));
    }
    if time == 0.0 {
        return Ok(curve.base_date());
    }

    let base = curve.base_date();
    match curve.day_count() {
        DayCount::Act360 => {
            return base
                .checked_add(Duration::days((time * 360.0).round() as i64))
                .ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "forward-curve date overflow at time {time}"
                    ))
                });
        }
        DayCount::Act365F => {
            return base
                .checked_add(Duration::days((time * 365.0).round() as i64))
                .ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "forward-curve date overflow at time {time}"
                    ))
                });
        }
        _ => {}
    }

    let day_count = curve.day_count();
    let context = curve.day_count_context();
    let mut low_days = 0_i64;
    let mut high_days = (time * 500.0).ceil() as i64 + 366;
    while low_days < high_days {
        let mid_days = low_days + (high_days - low_days) / 2;
        let date = base.checked_add(Duration::days(mid_days)).ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "forward-curve date overflow at time {time}"
            ))
        })?;
        let year_fraction = day_count.year_fraction(base, date, context)?;
        if year_fraction < time {
            low_days = mid_days + 1;
        } else {
            high_days = mid_days;
        }
    }

    let upper = base.checked_add(Duration::days(low_days)).ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "forward-curve date overflow at time {time}"
        ))
    })?;
    if low_days == 0 {
        return Ok(upper);
    }
    let lower = base
        .checked_add(Duration::days(low_days - 1))
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "forward-curve date overflow at time {time}"
            ))
        })?;
    let upper_error = (day_count.year_fraction(base, upper, context)? - time).abs();
    let lower_error = (day_count.year_fraction(base, lower, context)? - time).abs();
    Ok(if lower_error <= upper_error {
        lower
    } else {
        upper
    })
}

pub(super) fn rebuild_linked_ois_projection(
    source: &ForwardCurve,
    discount: &DiscountCurve,
) -> finstack_quant_core::Result<ForwardCurve> {
    if source.base_date() != discount.base_date() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "linked single-curve OIS representations {} and {} have different base dates",
            discount.id(),
            source.id()
        )));
    }

    let knots = source
        .knots()
        .iter()
        .map(|&time| {
            let date = date_from_forward_time(source, time)?;
            Ok((time, discount.df_on_date(date)?))
        })
        .collect::<finstack_quant_core::Result<Vec<_>>>()?;

    let mut recipe = required_forward_rate_calibration(source)?.clone();
    recipe.quotes = required_discount_rate_calibration(discount)?.quotes.clone();

    source
        .rebuild_with_knots(knots)?
        .to_builder_with_id(source.id().clone())
        .rate_calibration(recipe)
        .build()
}

/// Re-bootstrap both a discount curve and its dependent forward curve from
/// stored typed rate-calibration recipes under a quote-space shock.
///
/// Both solves use the source market's observed fixings. A quote shock must
/// not synthesize historical or valuation-date fixings from market quotes.
pub(crate) fn bump_market_via_rate_quote_shock(
    market: &MarketContext,
    discount_curve_id: &CurveId,
    forward_curve_id: &CurveId,
    bump: &QuoteBump,
) -> finstack_quant_core::Result<MarketContext> {
    bump.validate()?;
    let discount = market.get_discount(discount_curve_id.as_str())?;
    let forward = market.get_forward(forward_curve_id.as_str())?;
    let linked_single_curve = has_linked_single_curve_ois_recipes(
        discount.as_ref(),
        forward.as_ref(),
        discount_curve_id,
        forward_curve_id,
    )?;
    if let QuoteBump::RateQuoteBp { curve_id, .. } = bump
        && curve_id != discount_curve_id
        && (linked_single_curve || curve_id != forward_curve_id)
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "rate quote bump targets {curve_id}, which is not an independent source of this replay"
        )));
    }
    let discount_cal = required_discount_rate_calibration(discount.as_ref())?;
    let forward_cal = required_forward_rate_calibration(forward.as_ref())?;

    let bumped_discount = if linked_single_curve {
        verify_linked_projection(&forward, &discount, discount_cal)?;
        bump_discount_curve_from_rate_calibration_with_projection(
            discount.as_ref(),
            discount_cal,
            market,
            RateQuoteShock::Bucket(bump),
            Some(discount_curve_id.clone()),
        )?
    } else if matches!(bump, QuoteBump::RateQuoteBp { curve_id, .. } if curve_id == forward_curve_id)
    {
        discount.as_ref().clone()
    } else {
        bump_discount_curve_from_rate_calibration(discount.as_ref(), discount_cal, market, bump)?
    };
    let target_market = market.clone().try_insert(bumped_discount.clone())?;

    let bumped_forward = if linked_single_curve {
        let bumped_discount = target_market.get_discount(discount_curve_id.as_str())?;
        rebuild_linked_ois_projection(forward.as_ref(), bumped_discount.as_ref())?
    } else {
        bump_forward_curve_from_rate_calibration(
            forward.as_ref(),
            forward_cal,
            market,
            &target_market,
            bump,
        )?
    };
    market
        .clone()
        .try_insert(bumped_discount)?
        .try_insert(bumped_forward)
}

/// Shock one executed OIS yield quote in its shared discount and projection representations.
///
/// # Arguments
/// * `market` - Immutable valuation market containing both reciprocal OIS representations and their fixings.
/// * `discount_curve_id` - Identifier of the discount representation of the single economic curve.
/// * `forward_curve_id` - Identifier of the matching projection representation.
/// * `target` - Identifier and executed contractual pillar of the selected quote.
/// * `bp` - Finite shock in quoted-rate basis points.
///
/// # Errors
/// Rejects unqualified or mismatching OIS recipes, missing dependencies or quote,
/// and failed source reconstruction or shocked repricing.
pub fn bump_linked_ois_rate_quote(
    market: &MarketContext,
    discount_curve_id: &CurveId,
    forward_curve_id: &CurveId,
    target: &CalibrationQuoteCoordinate,
    bp: f64,
) -> finstack_quant_core::Result<MarketContext> {
    let discount = market.get_discount(discount_curve_id.as_str())?;
    let forward = market.get_forward(forward_curve_id.as_str())?;
    if !has_linked_single_curve_ois_recipes(
        &discount,
        &forward,
        discount_curve_id,
        forward_curve_id,
    )? {
        return Err(finstack_quant_core::Error::Validation(
            "joint exact quote replay requires two qualified views of the same OIS curve".into(),
        ));
    }
    let calibration = required_discount_rate_calibration(&discount)?;
    verify_linked_projection(&forward, &discount, calibration)?;
    let bumped = bump_discount_curve_from_rate_calibration_with_projection(
        &discount,
        calibration,
        market,
        RateQuoteShock::Exact { target, bp },
        Some(discount_curve_id.clone()),
    )?;
    let projection = rebuild_linked_ois_projection(&forward, &bumped)?;
    market.clone().try_insert(bumped)?.try_insert(projection)
}

pub(super) fn verify_linked_projection(
    forward: &ForwardCurve,
    discount: &DiscountCurve,
    discount_cal: &RateCalibrationRecipe,
) -> finstack_quant_core::Result<()> {
    let tolerance = replay_config(discount_cal)?
        .discount_curve
        .validation_tolerance;
    for (&time, &factor) in forward.knots().iter().zip(forward.factors()) {
        let date = date_from_forward_time(forward, time)?;
        verify_source_factor(
            forward.id(),
            time,
            factor,
            discount.df_on_date(date)?,
            tolerance,
        )?;
    }
    Ok(())
}

pub(crate) fn bump_market_via_rate_quote_shock_cached(
    cache: &RateRecalibrationCache,
    market: &MarketContext,
    discount_curve_id: &CurveId,
    forward_curve_id: &CurveId,
    bump: &QuoteBump,
) -> finstack_quant_core::Result<Arc<MarketContext>> {
    bump.validate()?;
    let key = RateMarketRecalibrationKey {
        source_market_hash: finstack_quant_core::canonical::content_hash(market)?,
        kind: RateMarketRecalibrationKind::DiscountAndForward {
            discount_curve_id: discount_curve_id.to_string(),
            forward_curve_id: forward_curve_id.to_string(),
        },
        bump: bump.into(),
    };
    cache.market.get_or_compute(key, || {
        bump_market_via_rate_quote_shock(market, discount_curve_id, forward_curve_id, bump)
    })
}

/// Re-bootstrap a single OIS discount curve under a market-quote shock.
///
/// This path is used when discounting and compounded-overnight projection are
/// two views of the same curve and no separate [`ForwardCurve`] is stored.
/// Pricing derives overnight forwards directly from discount-factor ratios.
pub(crate) fn bump_single_ois_market_via_rate_quote_shock(
    market: &MarketContext,
    curve_id: &CurveId,
    bump: &QuoteBump,
) -> finstack_quant_core::Result<MarketContext> {
    bump.validate()?;
    if let QuoteBump::RateQuoteBp {
        curve_id: target, ..
    } = bump
        && target != curve_id
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "rate quote bump targets {target}, but this replay owns {curve_id}"
        )));
    }
    let discount = market.get_discount(curve_id.as_str())?;
    let discount_cal = required_discount_rate_calibration(discount.as_ref())?;

    let bumped = bump_discount_curve_from_rate_calibration_with_projection(
        discount.as_ref(),
        discount_cal,
        market,
        RateQuoteShock::Bucket(bump),
        Some(curve_id.clone()),
    )?;
    market.clone().try_insert(bumped)
}

pub(crate) fn bump_single_ois_market_via_rate_quote_shock_cached(
    cache: &RateRecalibrationCache,
    market: &MarketContext,
    curve_id: &CurveId,
    bump: &QuoteBump,
) -> finstack_quant_core::Result<Arc<MarketContext>> {
    bump.validate()?;
    let key = RateMarketRecalibrationKey {
        source_market_hash: finstack_quant_core::canonical::content_hash(market)?,
        kind: RateMarketRecalibrationKind::SingleOis {
            curve_id: curve_id.to_string(),
        },
        bump: bump.into(),
    };
    cache.market.get_or_compute(key, || {
        bump_single_ois_market_via_rate_quote_shock(market, curve_id, bump)
    })
}

pub(super) fn required_discount_rate_calibration(
    curve: &DiscountCurve,
) -> finstack_quant_core::Result<&RateCalibrationRecipe> {
    let calibration = curve.rate_calibration().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "discount curve {} has no rate calibration; cannot quote-shock DV01",
            curve.id()
        ))
    })?;
    ensure_recipe_has_quotes(curve.id(), calibration)?;
    Ok(calibration)
}

pub(super) fn required_forward_rate_calibration(
    curve: &ForwardCurve,
) -> finstack_quant_core::Result<&RateCalibrationRecipe> {
    let calibration = curve.rate_calibration().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "forward curve {} has no rate calibration; cannot quote-shock DV01",
            curve.id()
        ))
    })?;
    ensure_recipe_has_quotes(curve.id(), calibration)?;
    Ok(calibration)
}

pub(super) fn ensure_recipe_has_quotes(
    curve_id: &CurveId,
    calibration: &RateCalibrationRecipe,
) -> finstack_quant_core::Result<()> {
    if calibration.quotes.is_empty() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "curve {curve_id} rate calibration has no quotes"
        )));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn fixing_seed(
    id: &str,
    base_date: Date,
    rate: f64,
) -> finstack_quant_core::Result<ScalarTimeSeries> {
    ScalarTimeSeries::new(
        format!("FIXING:{id}"),
        vec![
            (base_date - Duration::days(3), rate),
            (base_date - Duration::days(2), rate),
            (base_date - Duration::days(1), rate),
            (base_date, rate),
        ],
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
}

/// Apply a parallel quote move or exact, nonambiguous pillar moves.
pub(super) fn apply_bump_to_rate_quotes(
    mut quotes: Vec<RateQuote>,
    bump: &QuoteBump,
    bindings: &[RateQuoteBinding],
) -> finstack_quant_core::Result<Vec<RateQuote>> {
    match bump {
        QuoteBump::ParallelBp(bp) => Ok(quotes
            .into_iter()
            .map(|quote| quote.bump_rate_bp(*bp))
            .collect()),
        QuoteBump::TenorsBp(targets) => {
            for &(time, bp) in targets {
                let mut matches = bindings
                    .iter()
                    .filter(|binding| binding.pillar_time.to_bits() == time.to_bits());
                let target = matches
                    .next()
                    .filter(|_| matches.next().is_none())
                    .ok_or_else(|| {
                        finstack_quant_core::Error::Validation(format!(
                            "rate quote bump pillar {time} requires one exact executed binding"
                        ))
                    })?;
                quotes = RateQuoteShock::apply_exact(
                    quotes,
                    bindings,
                    &target.quote_id,
                    target.pillar_date,
                    bp,
                )?;
            }
            Ok(quotes)
        }
        QuoteBump::RateQuoteBp { .. } => Err(finstack_quant_core::Error::Internal(
            "rate quote shock requires the owning curve identity".into(),
        )),
    }
}
