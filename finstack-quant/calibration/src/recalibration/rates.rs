//! Shared rates curve bumping logic (plan-driven calibration).

use super::cache::KeyedOnceCache;
use crate::CalibrationConfig;
use crate::api::schema::{DiscountCurveParams, ForwardCurveParams, StepParams};
#[cfg(test)]
use crate::config::CalibrationMethod;
#[cfg(test)]
use crate::config::RatesStepConventions;
use crate::quotes::MarketQuote;
#[cfg(test)]
use crate::quotes::Pillar;
#[cfg(test)]
use crate::quotes::QuoteId;
use crate::quotes::RateQuote;
use crate::step_runtime;
use crate::targets::discount::DiscountCurveTarget;
use crate::targets::rate_recipe::{original_execution, rate_quotes_from_recipe};
#[cfg(test)]
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
#[cfg(test)]
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
#[cfg(test)]
use finstack_quant_core::market_data::term_structures::RateCalibrationQuote;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, ForwardCurve, RateCalibrationCurveRole, RateCalibrationRecipe, RateQuoteBinding,
};
#[cfg(test)]
use finstack_quant_core::math::interp::ExtrapolationPolicy;
use finstack_quant_core::types::CurveId;
#[cfg(test)]
use finstack_quant_core::types::IndexId;
use finstack_quant_valuations_core::recalibration::QuoteBump;
#[cfg(test)]
use std::cell::Cell;
use std::sync::Arc;
use time::Duration;

#[cfg(test)]
std::thread_local! {
    static DISCOUNT_CALIBRATION_RUNS: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum RateMarketRecalibrationKind {
    DiscountAndForward {
        discount_curve_id: String,
        forward_curve_id: String,
    },
    SingleOis {
        curve_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RateMarketRecalibrationKey {
    source_market_hash: String,
    kind: RateMarketRecalibrationKind,
    bump: RateBumpKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DiscountRateRecalibrationKey {
    source_market_hash: String,
    source_curve_hash: String,
    recipe_hash: String,
    bump: RateBumpKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum RateBumpKey {
    Parallel(u64),
    Tenors(Vec<(u64, u64)>),
}

impl From<&QuoteBump> for RateBumpKey {
    fn from(bump: &QuoteBump) -> Self {
        match bump {
            QuoteBump::ParallelBp(bp) => Self::Parallel(bp.to_bits()),
            QuoteBump::TenorsBp(tenors) => Self::Tenors(
                tenors
                    .iter()
                    .map(|(tenor, bp)| (tenor.to_bits(), bp.to_bits()))
                    .collect(),
            ),
        }
    }
}

/// Batch-local cache for rate-curve recalibrations used by quote-shock risk.
///
/// Keys include the complete source snapshot and recipe identities, so a
/// provider may safely serve multiple immutable markets. Per-key locks
/// allow unrelated curve sets and bump scenarios to calibrate concurrently,
/// while identical portfolio requests share the in-flight result. Failed
/// calibrations are not cached.
#[derive(Default)]
pub(crate) struct RateRecalibrationCache {
    market: KeyedOnceCache<RateMarketRecalibrationKey, MarketContext>,
    discount: KeyedOnceCache<DiscountRateRecalibrationKey, DiscountCurve>,
}

fn solve_discount_curve(
    quotes: &[RateQuote],
    params: &DiscountCurveParams,
    base_context: &MarketContext,
    config: &CalibrationConfig,
) -> finstack_quant_core::Result<DiscountCurve> {
    #[cfg(test)]
    DISCOUNT_CALIBRATION_RUNS.with(|runs| runs.set(runs.get() + 1));
    let market_quotes: Vec<_> = quotes.iter().cloned().map(MarketQuote::Rates).collect();
    for quote in &market_quotes {
        quote.validate()?;
    }
    let (curve, report) = DiscountCurveTarget::solve(params, &market_quotes, base_context, config)?;

    super::ensure_replay_fit_accepted(params.curve_id.as_str(), config, &report)?;
    Ok(curve)
}

/// Bump a discount curve by shocking its stored market-rate calibration quotes.
///
/// The unshocked fit must reproduce the supplied curve. A successful bump
/// returns the curve produced by the accepted solve with its shocked quotes.
///
/// # Arguments
///
/// * `curve` - Source discount curve whose factors must reproduce the unshocked fit.
/// * `calibration` - Exact typed recipe retained when the curve was calibrated.
/// * `context` - Market context supplying the recipe's pricing dependencies.
/// * `bump` - Parallel or tenor-specific quote shock in basis points.
pub fn bump_discount_curve_from_rate_calibration(
    curve: &DiscountCurve,
    calibration: &RateCalibrationRecipe,
    context: &MarketContext,
    bump: &QuoteBump,
) -> finstack_quant_core::Result<DiscountCurve> {
    bump.validate()?;
    bump_discount_curve_from_rate_calibration_with_projection(
        curve,
        calibration,
        context,
        bump,
        None,
    )
}

pub(crate) fn bump_discount_curve_from_rate_calibration_cached(
    cache: &RateRecalibrationCache,
    curve: &DiscountCurve,
    calibration: &RateCalibrationRecipe,
    context: &MarketContext,
    bump: &QuoteBump,
) -> finstack_quant_core::Result<Arc<DiscountCurve>> {
    bump.validate()?;
    let key = DiscountRateRecalibrationKey {
        source_market_hash: finstack_quant_core::canonical::content_hash(context)?,
        source_curve_hash: finstack_quant_core::canonical::content_hash(curve)?,
        recipe_hash: finstack_quant_core::canonical::content_hash(calibration)?,
        bump: bump.into(),
    };
    cache.discount.get_or_compute(key, || {
        bump_discount_curve_from_rate_calibration(curve, calibration, context, bump)
    })
}

fn bump_discount_curve_from_rate_calibration_with_projection(
    curve: &DiscountCurve,
    calibration: &RateCalibrationRecipe,
    context: &MarketContext,
    bump: &QuoteBump,
    pricing_forward_id_override: Option<CurveId>,
) -> finstack_quant_core::Result<DiscountCurve> {
    ensure_recipe_has_quotes(curve.id(), calibration)?;
    let quotes = rate_quotes_from_recipe(calibration, curve.id())?;

    let StepParams::Discount(mut params) = replay_step(calibration)? else {
        return Err(finstack_quant_core::Error::Validation(
            "discount curve requires its original discount evaluator".into(),
        ));
    };
    if params.curve_id != *curve.id()
        || params.base_date != curve.base_date()
        || params.interpolation != curve.interp_style()
        || params.extrapolation != curve.extrapolation()
        || calibration.curve_day_count != curve.day_count()
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "discount curve {} differs from its recorded evaluator",
            curve.id()
        )));
    }
    if let Some(projection) = pricing_forward_id_override {
        params.pricing_forward_id = Some(projection);
    }

    let cfg = replay_config(calibration)?;
    let unbumped = solve_discount_curve(&quotes, &params, context, &cfg)?;
    verify_replayed_bindings(calibration, required_discount_rate_calibration(&unbumped)?)?;
    for &time in curve.knots().iter().chain(unbumped.knots()) {
        verify_source_factor(
            curve.id(),
            time,
            curve.df(time)?,
            unbumped.df(time)?,
            cfg.discount_curve.validation_tolerance,
        )?;
    }
    let quotes =
        apply_bump_to_rate_quotes(quotes, bump, &original_execution(calibration)?.bindings);
    let bumped = solve_discount_curve(&quotes, &params, context, &cfg)?;
    let mut recipe = required_discount_rate_calibration(&bumped)?.clone();
    recipe.role = calibration.role.clone();
    if let Some(execution) = &mut recipe.execution {
        execution.step_params = original_execution(calibration)?.step_params.clone();
    }
    bumped
        .to_builder_with_id(curve.id().clone())
        .rate_calibration(recipe)
        .fx_policy_opt(curve.fx_policy().map(str::to_owned))
        .build()
}

fn verify_replayed_bindings(
    source: &RateCalibrationRecipe,
    reconstructed: &RateCalibrationRecipe,
) -> finstack_quant_core::Result<()> {
    if source.method != reconstructed.method
        || source.currency != reconstructed.currency
        || source.curve_day_count != reconstructed.curve_day_count
        || source.ois_compounding != reconstructed.ois_compounding
        || original_execution(source)?.bindings != original_execution(reconstructed)?.bindings
    {
        return Err(finstack_quant_core::Error::Validation(
            "unshocked rate quote bindings differ from the executed calibration".into(),
        ));
    }
    Ok(())
}

fn replay_config(recipe: &RateCalibrationRecipe) -> finstack_quant_core::Result<CalibrationConfig> {
    let mut config: CalibrationConfig = serde_json::from_value(
        original_execution(recipe)?.calibration_config.clone(),
    )
    .map_err(|error| {
        finstack_quant_core::Error::Validation(format!(
            "invalid rate calibration configuration: {error}"
        ))
    })?;
    config.validate()?;
    // A diagnostic solve may report a failed fit. A shocked market must be publishable.
    config.fail_on_bad_fit = true;
    Ok(config)
}

fn verify_source_factor(
    id: &CurveId,
    time: f64,
    source: f64,
    replay: f64,
    tolerance: f64,
) -> finstack_quant_core::Result<()> {
    if (source - replay).abs() <= tolerance {
        return Ok(());
    }
    Err(finstack_quant_core::Error::Validation(format!(
        "curve {id} source factor {source} at time {time} differs from unshocked calibration factor {replay}"
    )))
}

fn replay_step(calibration: &RateCalibrationRecipe) -> finstack_quant_core::Result<StepParams> {
    serde_json::from_value(original_execution(calibration)?.step_params.clone()).map_err(|error| {
        finstack_quant_core::Error::Validation(format!("invalid recorded rate evaluator: {error}"))
    })
}

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
    let quotes =
        apply_bump_to_rate_quotes(quotes, bump, &original_execution(calibration)?.bindings);
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
fn rebootstrap_forward_curve(
    quotes: Vec<RateQuote>,
    params: &ForwardCurveParams,
    context: &MarketContext,
    cfg: &CalibrationConfig,
) -> finstack_quant_core::Result<ForwardCurve> {
    let market_quotes: Vec<MarketQuote> = quotes.into_iter().map(MarketQuote::Rates).collect();
    let step = StepParams::Forward(params.clone());
    let (ctx, report) =
        step_runtime::execute_params_and_apply(&step, &market_quotes, context, cfg)?;
    super::ensure_replay_fit_accepted(params.curve_id.as_str(), cfg, &report)?;
    Ok(ctx.get_forward(params.curve_id.as_str())?.as_ref().clone())
}

fn has_linked_single_curve_ois_recipes(
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

fn date_from_forward_time(curve: &ForwardCurve, time: f64) -> finstack_quant_core::Result<Date> {
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

fn rebuild_linked_ois_projection(
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
    let discount_cal = required_discount_rate_calibration(discount.as_ref())?;
    let forward_cal = required_forward_rate_calibration(forward.as_ref())?;

    let bumped_discount = if linked_single_curve {
        let tolerance = replay_config(discount_cal)?
            .discount_curve
            .validation_tolerance;
        for (&time, &factor) in forward.knots().iter().zip(forward.factors()) {
            let date = date_from_forward_time(&forward, time)?;
            verify_source_factor(
                forward.id(),
                time,
                factor,
                discount.df_on_date(date)?,
                tolerance,
            )?;
        }
        bump_discount_curve_from_rate_calibration_with_projection(
            discount.as_ref(),
            discount_cal,
            market,
            bump,
            Some(discount_curve_id.clone()),
        )?
    } else {
        bump_discount_curve_from_rate_calibration(discount.as_ref(), discount_cal, market, bump)?
    };
    let target_market = market.clone().insert(bumped_discount.clone());

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
    Ok(market
        .clone()
        .insert(bumped_discount)
        .insert(bumped_forward))
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
    let discount = market.get_discount(curve_id.as_str())?;
    let discount_cal = required_discount_rate_calibration(discount.as_ref())?;

    let bumped = bump_discount_curve_from_rate_calibration_with_projection(
        discount.as_ref(),
        discount_cal,
        market,
        bump,
        Some(curve_id.clone()),
    )?;
    Ok(market.clone().insert(bumped))
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

fn required_discount_rate_calibration(
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

fn required_forward_rate_calibration(
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

fn ensure_recipe_has_quotes(
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
fn fixing_seed(
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

/// Apply shocks to the quote whose executed pillar is nearest each requested curve time.
fn apply_bump_to_rate_quotes(
    mut quotes: Vec<RateQuote>,
    bump: &QuoteBump,
    bindings: &[RateQuoteBinding],
) -> Vec<RateQuote> {
    match bump {
        QuoteBump::ParallelBp(bp) => quotes
            .into_iter()
            .map(|quote| quote.bump_rate_bp(*bp))
            .collect(),
        QuoteBump::TenorsBp(targets) => {
            for (time, bp) in targets {
                if let Some((i, _)) = bindings.iter().enumerate().min_by(|(_, a), (_, b)| {
                    (a.pillar_time - time)
                        .abs()
                        .total_cmp(&(b.pillar_time - time).abs())
                }) {
                    quotes[i] = quotes[i].bump_rate_bp(*bp);
                }
            }
            quotes
        }
    }
}

#[cfg(test)]
#[path = "tests/rates_review.rs"]
mod review_regressions;

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use finstack_quant_core::market_data::term_structures::{
        RateCalibrationMethod, RateCalibrationPillar,
    };
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_valuations_core::market::conventions::ids::IrFutureContractId;

    fn linked_single_curve_ois_market(
        forward_discount_curve_id: CurveId,
    ) -> (MarketContext, CurveId, CurveId, Vec<f64>) {
        let base_date =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid date");
        let discount_curve_id = CurveId::new("USD-OIS");
        let forward_curve_id = CurveId::new("USD-SOFR-OIS");
        let index_id = IndexId::new("USD-SOFR-OIS");
        let quotes: Vec<_> = [("6M", 0.0430), ("1Y", 0.0410), ("2Y", 0.0390)]
            .into_iter()
            .map(|(tenor, rate)| RateQuote::Deposit {
                id: QuoteId::new(format!("USD-DEP-{tenor}")),
                index: index_id.clone(),
                pillar: Pillar::Tenor(tenor.parse().expect("valid tenor")),
                rate,
            })
            .collect();
        let params = DiscountCurveParams {
            curve_id: discount_curve_id.clone(),
            currency: Currency::USD,
            base_date,
            method: CalibrationMethod::Bootstrap,
            interpolation: InterpStyle::LogLinear,
            extrapolation: ExtrapolationPolicy::Extend,
            pricing_discount_id: Some(discount_curve_id.clone()),
            pricing_forward_id: Some(forward_curve_id.clone()),
            conventions: RatesStepConventions {
                ois_compounding: Some(
                    finstack_quant_cashflows::builder::FloatingLegCompounding::Simple,
                ),
                curve_day_count: Some(DayCount::Act365F),
            },
        };
        let discount = solve_discount_curve(
            &quotes,
            &params,
            &MarketContext::new(),
            &CalibrationConfig::default(),
        )
        .expect("fit source deposit quotes")
        .to_builder_with_id(discount_curve_id.clone())
        .fx_policy("single_curve_ois::USD")
        .build()
        .expect("discount representation");

        let projection_grid = vec![0.0, 0.5, 1.0, 1.5, 2.0];
        let forward_knots = projection_grid.iter().map(|&time: &f64| {
            let date = base_date + Duration::days((time * 360.0).round() as i64);
            (time, discount.df_on_date(date).expect("projection factor"))
        });

        let mut forward_recipe = discount
            .rate_calibration()
            .expect("executed deposit recipe")
            .clone();
        forward_recipe.method = RateCalibrationMethod::GlobalSolve {
            use_analytical_jacobian: false,
        };
        forward_recipe.role = RateCalibrationCurveRole::Projection {
            discount_curve_id: forward_discount_curve_id,
        };
        let forward = ForwardCurve::builder(forward_curve_id.clone())
            .base_date(base_date)
            .index_tenor(finstack_quant_core::dates::Tenor::semi_annual())
            .day_count(DayCount::Act360)
            .knots(forward_knots)
            .interp(InterpStyle::CubicHermite)
            .extrapolation(ExtrapolationPolicy::Extend)
            .rate_calibration(forward_recipe)
            .fx_policy("single_curve_ois::USD")
            .build()
            .expect("projection representation");

        (
            MarketContext::new().insert(discount).insert(forward),
            discount_curve_id,
            forward_curve_id,
            projection_grid,
        )
    }

    #[test]
    fn linked_single_curve_ois_quote_shock_derives_projection_from_discount() {
        let (market, discount_curve_id, forward_curve_id, projection_grid) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));

        let shocked = bump_market_via_rate_quote_shock(
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(5.0),
        )
        .expect("linked OIS quote shock");
        let shocked_discount = shocked
            .get_discount(discount_curve_id.as_str())
            .expect("shocked discount representation");
        let shocked_forward = shocked
            .get_forward(forward_curve_id.as_str())
            .expect("shocked projection representation");
        let source_discount = market
            .get_discount(discount_curve_id.as_str())
            .expect("source discount representation");
        let source_forward = market
            .get_forward(forward_curve_id.as_str())
            .expect("source projection representation");

        for period in projection_grid.windows(2) {
            let (start, end) = (period[0], period[1]);
            let start_date =
                shocked_forward.base_date() + Duration::days((start * 360.0).round() as i64);
            let end_date =
                shocked_forward.base_date() + Duration::days((end * 360.0).round() as i64);
            let discount_implied = (shocked_discount
                .df_on_date(start_date)
                .expect("discount factor on reset date")
                / shocked_discount
                    .df_on_date(end_date)
                    .expect("discount factor on payment date")
                - 1.0)
                / (end - start);
            let projected = shocked_forward
                .rate_between(start, end, end - start)
                .expect("projection-grid forward");
            assert!(
                (projected - discount_implied).abs() < 1e-12,
                "linked OIS representations diverged over [{start}, {end}]: \
                 projection={projected:.12}, discount-implied={discount_implied:.12}"
            );
        }
        assert!(
            (shocked_discount.df(1.0).expect("valid") - source_discount.df(1.0).expect("valid"))
                .abs()
                > 1e-8,
            "non-zero shared quote shock must move the linked discount curve"
        );

        assert_eq!(shocked_forward.id(), source_forward.id());
        assert_eq!(shocked_forward.knots(), source_forward.knots());
        assert_eq!(shocked_forward.index_tenor(), source_forward.index_tenor());
        assert_eq!(shocked_forward.day_count(), source_forward.day_count());
        assert_eq!(
            shocked_forward.interp_style(),
            source_forward.interp_style()
        );
        assert_eq!(
            shocked_forward.extrapolation(),
            source_forward.extrapolation()
        );
        assert_eq!(shocked_forward.fx_policy(), source_forward.fx_policy());
        let mut expected_recipe = source_forward
            .rate_calibration()
            .expect("source recipe")
            .clone();
        expected_recipe.quotes = shocked_discount
            .rate_calibration()
            .expect("shocked discount recipe")
            .quotes
            .clone();
        assert_ne!(
            expected_recipe.quotes,
            source_forward
                .rate_calibration()
                .expect("source recipe")
                .quotes
        );
        assert_eq!(shocked_forward.rate_calibration(), Some(&expected_recipe));
    }

    #[test]
    fn linked_ois_projection_uses_forward_grid_dates_across_day_counts() {
        let (market, discount_curve_id, forward_curve_id, projection_grid) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));

        let shocked = bump_market_via_rate_quote_shock(
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(5.0),
        )
        .expect("linked OIS quote shock");
        let shocked_discount = shocked
            .get_discount(discount_curve_id.as_str())
            .expect("shocked discount representation");
        let shocked_forward = shocked
            .get_forward(forward_curve_id.as_str())
            .expect("shocked projection representation");

        assert_eq!(shocked_forward.day_count(), DayCount::Act360);
        assert_eq!(shocked_discount.day_count(), DayCount::Act365F);
        for period in projection_grid.windows(2) {
            let (start, end) = (period[0], period[1]);
            let start_date =
                shocked_forward.base_date() + Duration::days((start * 360.0).round() as i64);
            let end_date =
                shocked_forward.base_date() + Duration::days((end * 360.0).round() as i64);
            let date_implied = (shocked_discount
                .df_on_date(start_date)
                .expect("discount factor on reset date")
                / shocked_discount
                    .df_on_date(end_date)
                    .expect("discount factor on payment date")
                - 1.0)
                / (end - start);
            let projected = shocked_forward
                .rate_between(start, end, end - start)
                .expect("projection-grid forward");
            assert!(
                (projected - date_implied).abs() < 1e-12,
                "mixed-day-count linked projection diverged over {start_date}..{end_date}: \
                 projection={projected:.12}, date-implied={date_implied:.12}"
            );
        }
    }

    #[test]
    fn linked_replay_rejects_a_projection_inconsistent_with_its_source_discount() {
        let (market, discount_id, forward_id, _) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));
        let source = market.get_forward(forward_id.as_str()).unwrap();
        let altered = source
            .rebuild_with_knots(
                source
                    .knots()
                    .iter()
                    .zip(source.factors())
                    .map(|(&time, &df)| (time, if time > 0.0 { df * 0.95 } else { df })),
            )
            .unwrap()
            .to_builder_with_id(source.id().clone())
            .rate_calibration(source.rate_calibration().unwrap().clone())
            .build()
            .unwrap();
        let error = bump_market_via_rate_quote_shock(
            &market.insert(altered),
            &discount_id,
            &forward_id,
            &QuoteBump::ParallelBp(1.0),
        )
        .unwrap_err();
        assert!(error.to_string().contains("source factor"), "{error}");
    }

    #[test]
    fn linked_ois_quote_shock_verifies_source_before_calibrating_target() {
        let (market, discount_curve_id, forward_curve_id, _) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));
        DISCOUNT_CALIBRATION_RUNS.with(|runs| runs.set(0));

        let shocked = bump_market_via_rate_quote_shock(
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(5.0),
        )
        .expect("linked OIS quote shock");

        assert_eq!(
            DISCOUNT_CALIBRATION_RUNS.with(Cell::get),
            2,
            "one solve verifies the source, one fits the shocked quotes"
        );
        assert_eq!(
            shocked
                .get_discount(discount_curve_id.as_str())
                .expect("shocked discount curve")
                .knots(),
            market
                .get_discount(discount_curve_id.as_str())
                .expect("source discount curve")
                .knots(),
            "single calibration must still be sampled on the source discount grid"
        );
    }

    #[test]
    fn rate_recalibration_cache_reuses_identical_market_shock() {
        let (market, discount_curve_id, forward_curve_id, _) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));
        let cache = RateRecalibrationCache::default();
        DISCOUNT_CALIBRATION_RUNS.with(|runs| runs.set(0));

        let first = bump_market_via_rate_quote_shock_cached(
            &cache,
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(5.0),
        )
        .expect("first cached quote shock");
        let second = bump_market_via_rate_quote_shock_cached(
            &cache,
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(5.0),
        )
        .expect("reused cached quote shock");

        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(
            DISCOUNT_CALIBRATION_RUNS.with(Cell::get),
            2,
            "identical requests share both verification and shocked calibration"
        );
    }

    #[test]
    fn linked_rate_cache_preserves_each_source_market() {
        use finstack_quant_core::market_data::scalars::MarketScalar;

        let (market, discount_id, forward_id, _) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));
        let first_market = market.clone().insert_price(
            "EQUITY",
            MarketScalar::unitless(100.0).expect("valid scalar"),
        );
        let second_market = market.insert_price(
            "EQUITY",
            MarketScalar::unitless(200.0).expect("valid scalar"),
        );
        let cache = RateRecalibrationCache::default();
        let bump = QuoteBump::ParallelBp(1.0);
        let first = bump_market_via_rate_quote_shock_cached(
            &cache,
            &first_market,
            &discount_id,
            &forward_id,
            &bump,
        )
        .expect("first market replay");
        let second = bump_market_via_rate_quote_shock_cached(
            &cache,
            &second_market,
            &discount_id,
            &forward_id,
            &bump,
        )
        .expect("second market replay");
        assert!(
            first
                .get_price("EQUITY")
                .is_ok_and(|s| s.get_unitless() == Some(100.0))
        );
        assert!(
            second
                .get_price("EQUITY")
                .is_ok_and(|s| s.get_unitless() == Some(200.0))
        );
        assert!(!Arc::ptr_eq(&first, &second));
    }

    fn single_ois_example_market(quote_shift: f64) -> MarketContext {
        let mut envelope: serde_json::Value = serde_json::from_str(include_str!(
            "../../examples/market_bootstrap/01_usd_discount.json"
        ))
        .expect("example envelope");
        for quote in envelope["market_data"].as_array_mut().expect("quotes") {
            quote["rate"] = serde_json::json!(quote["rate"].as_f64().expect("rate") + quote_shift);
        }
        let result = crate::api::engine::calibrate_from_json(&envelope.to_string())
            .expect("calibrated single-OIS market");
        MarketContext::try_from(result.result.final_market).expect("materialized market")
    }

    #[test]
    fn single_ois_cache_uses_current_source_quotes() {
        let first_market = single_ois_example_market(0.0);
        let second_market = single_ois_example_market(0.01);
        let cache = RateRecalibrationCache::default();
        let id = CurveId::new("USD-OIS");
        let bump = QuoteBump::ParallelBp(1.0);
        let first =
            bump_single_ois_market_via_rate_quote_shock_cached(&cache, &first_market, &id, &bump)
                .expect("first market replay");
        let second =
            bump_single_ois_market_via_rate_quote_shock_cached(&cache, &second_market, &id, &bump)
                .expect("second market replay");
        let expected = bump_single_ois_market_via_rate_quote_shock(&second_market, &id, &bump)
            .expect("independent second market replay");
        let first_df = first
            .get_discount(&id)
            .expect("first curve")
            .df(1.0)
            .expect("df");
        let second_df = second
            .get_discount(&id)
            .expect("second curve")
            .df(1.0)
            .expect("df");
        let expected_df = expected
            .get_discount(&id)
            .expect("expected curve")
            .df(1.0)
            .expect("df");
        assert!((second_df - expected_df).abs() < 1e-12);
        assert!((second_df - first_df).abs() > 1e-3);
    }

    #[test]
    fn discount_cache_uses_source_curve_and_explicit_recipe() {
        let market = single_ois_example_market(0.0);
        let source = market.get_discount("USD-OIS").expect("source curve");
        let recipe = source.rate_calibration().expect("recipe");
        let different_curve = source
            .rebuild_with_knots(
                source
                    .knots()
                    .iter()
                    .zip(source.dfs())
                    .map(|(&t, &df)| (t, df * (-0.01 * t).exp())),
            )
            .expect("different stored source curve");
        let cache = RateRecalibrationCache::default();
        let bump = QuoteBump::ParallelBp(1.0);
        let first = bump_discount_curve_from_rate_calibration_cached(
            &cache,
            source.as_ref(),
            recipe,
            &market,
            &bump,
        )
        .expect("first source curve replay");
        let different = bump_discount_curve_from_rate_calibration_cached(
            &cache,
            &different_curve,
            recipe,
            &market,
            &bump,
        )
        .expect_err("stored curve differs from quote fit");
        assert!(different.to_string().contains("source factor"));
        assert!(first.df(1.0).expect("valid").is_finite());

        let mut different_recipe = recipe.clone();
        for quote in &mut different_recipe.quotes {
            if let RateCalibrationQuote::Deposit { rate, .. }
            | RateCalibrationQuote::Swap { rate, .. } = quote
            {
                *rate += 0.01;
            }
        }
        let mismatch = bump_discount_curve_from_rate_calibration_cached(
            &cache,
            source.as_ref(),
            &different_recipe,
            &market,
            &bump,
        )
        .expect_err("a different quote record cannot describe this source");
        assert!(mismatch.to_string().contains("source factor"));
    }

    #[test]
    fn rate_replay_preserves_observed_fixings_and_does_not_publish_seeds() {
        let (market, discount_id, forward_id, _) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));
        let base_date = market
            .get_discount(&discount_id)
            .expect("curve")
            .base_date();
        let observed = ScalarTimeSeries::new(
            "FIXING:USD-SOFR-OIS",
            vec![
                (base_date - Duration::days(30), 0.066),
                (base_date - Duration::days(1), 0.067),
            ],
            None,
            finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
            finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
        )
        .expect("observed fixing history");
        let with_history = market.clone().insert_series(observed.clone());
        for bump in [QuoteBump::ParallelBp(0.0), QuoteBump::ParallelBp(1.0)] {
            let replayed =
                bump_market_via_rate_quote_shock(&with_history, &discount_id, &forward_id, &bump)
                    .expect("linked market replay");
            assert_eq!(
                replayed
                    .get_series(observed.id())
                    .expect("preserved history")
                    .observations(),
                observed.observations()
            );
            assert!(replayed.get_series("FIXING:USD-OIS").is_err());
        }
        let no_history = bump_market_via_rate_quote_shock(
            &market,
            &discount_id,
            &forward_id,
            &QuoteBump::ParallelBp(0.0),
        )
        .expect("replay with private calibration seeds");
        assert!(no_history.get_series(observed.id()).is_err());

        let single_market = single_ois_example_market(0.0);
        let single_base = single_market
            .get_discount("USD-OIS")
            .expect("curve")
            .base_date();
        let single_history = ScalarTimeSeries::new(
            "FIXING:USD-SOFR-OIS",
            vec![
                (single_base - Duration::days(30), 0.066),
                (single_base - Duration::days(1), 0.067),
            ],
            None,
            finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
            finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
        )
        .expect("single-OIS history");
        let single_market = single_market.insert_series(single_history.clone());
        let replayed = bump_single_ois_market_via_rate_quote_shock(
            &single_market,
            &CurveId::new("USD-OIS"),
            &QuoteBump::ParallelBp(0.0),
        )
        .expect("single-OIS replay");
        assert_eq!(
            replayed
                .get_series(single_history.id())
                .expect("preserved single-OIS history")
                .observations(),
            single_history.observations()
        );
        assert!(replayed.get_series("FIXING:USD-OIS").is_err());
    }

    #[test]
    fn malformed_linked_single_curve_ois_recipe_fails_explicitly() {
        let (market, discount_curve_id, forward_curve_id, _) =
            linked_single_curve_ois_market(CurveId::new("USD-OTHER"));

        let error = bump_market_via_rate_quote_shock(
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(1.0),
        )
        .expect_err("non-reciprocal linked OIS roles must fail");

        assert!(
            error.to_string().contains("linked single-curve OIS recipe"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn forward_declared_ois_link_without_discount_recipe_fails_explicitly() {
        let (market, discount_curve_id, forward_curve_id, _) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));
        let discount = market
            .get_discount(discount_curve_id.as_str())
            .expect("discount representation");
        let discount_without_recipe = discount
            .to_builder_with_id(discount_curve_id.clone())
            .rate_calibration_opt(None)
            .build()
            .expect("discount representation without recipe");
        let market = market.insert(discount_without_recipe);

        let error = bump_market_via_rate_quote_shock(
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(1.0),
        )
        .expect_err("one-sided forward OIS link must fail");

        assert!(
            error.to_string().contains("linked single-curve OIS recipe"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn linkage_validation_precedes_missing_discount_calibration_error() {
        let (market, discount_curve_id, forward_curve_id, _) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));
        let discount = market
            .get_discount(discount_curve_id.as_str())
            .expect("discount representation");
        let discount_without_metadata = discount
            .to_builder_with_id(discount_curve_id.clone())
            .rate_calibration_opt(None)
            .build()
            .expect("discount representation without calibration metadata");
        let market = market.insert(discount_without_metadata);

        let error = bump_market_via_rate_quote_shock(
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(1.0),
        )
        .expect_err("linkage validation must run before calibration validation");
        let message = error.to_string();

        assert!(
            message.contains("linked single-curve OIS recipe"),
            "unexpected error: {error}"
        );
        assert!(
            !message.contains("no rate calibration metadata"),
            "generic metadata error escaped before link validation: {error}"
        );
    }

    #[test]
    fn forward_declared_ois_link_with_reverse_mismatch_fails_explicitly() {
        let (market, discount_curve_id, forward_curve_id, _) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));
        let discount = market
            .get_discount(discount_curve_id.as_str())
            .expect("discount representation");
        let mut mismatched_recipe = discount
            .rate_calibration()
            .expect("discount recipe")
            .clone();
        mismatched_recipe.role = RateCalibrationCurveRole::Discount {
            projection_curve_id: CurveId::new("USD-OTHER-PROJECTION"),
        };
        let mismatched_discount = discount
            .to_builder_with_id(discount_curve_id.clone())
            .rate_calibration(mismatched_recipe)
            .build()
            .expect("mismatched discount representation");
        let market = market.insert(mismatched_discount);

        let error = bump_market_via_rate_quote_shock(
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(1.0),
        )
        .expect_err("reverse-mismatched OIS link must fail");

        assert!(
            error.to_string().contains("linked single-curve OIS recipe"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn discount_declared_ois_link_without_forward_recipe_fails_explicitly() {
        let (market, discount_curve_id, forward_curve_id, _) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));
        let forward = market
            .get_forward(forward_curve_id.as_str())
            .expect("projection representation");
        let forward_without_recipe = forward
            .to_builder_with_id(forward_curve_id.clone())
            .rate_calibration_opt(None)
            .build()
            .expect("projection representation without recipe");
        let market = market.insert(forward_without_recipe);

        let error = bump_market_via_rate_quote_shock(
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(1.0),
        )
        .expect_err("one-sided discount OIS link must fail");

        assert!(
            error.to_string().contains("linked single-curve OIS recipe"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn reciprocal_roles_with_one_sided_ois_metadata_fail_explicitly() {
        let (market, discount_curve_id, forward_curve_id, _) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));
        let forward = market
            .get_forward(forward_curve_id.as_str())
            .expect("projection representation");
        let mut partial_recipe = forward
            .rate_calibration()
            .expect("projection recipe")
            .clone();
        partial_recipe.ois_compounding = None;
        let partial_forward = forward
            .to_builder_with_id(forward_curve_id.clone())
            .rate_calibration(partial_recipe)
            .build()
            .expect("projection representation with partial OIS metadata");
        let market = market.insert(partial_forward);

        let error = bump_market_via_rate_quote_shock(
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(1.0),
        )
        .expect_err("one-sided OIS convention metadata must fail");

        assert!(
            error.to_string().contains("linked single-curve OIS recipe"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn term_index_relabel_cannot_reuse_an_ois_derived_projection() {
        let (market, discount_curve_id, forward_curve_id, _) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));
        let discount = market
            .get_discount(discount_curve_id.as_str())
            .expect("discount representation");
        let mut discount_recipe = discount
            .rate_calibration()
            .expect("discount recipe")
            .clone();
        discount_recipe.role = RateCalibrationCurveRole::Discount {
            projection_curve_id: discount_curve_id.clone(),
        };
        let discount = discount
            .to_builder_with_id(discount_curve_id.clone())
            .rate_calibration(discount_recipe)
            .build()
            .expect("self-projected discount representation");

        let forward = market
            .get_forward(forward_curve_id.as_str())
            .expect("projection representation");
        let mut forward_recipe = forward
            .rate_calibration()
            .expect("projection recipe")
            .clone();
        forward_recipe.ois_compounding = None;
        let forward = forward
            .to_builder_with_id(forward_curve_id.clone())
            .rate_calibration(forward_recipe)
            .build()
            .expect("term-index projection representation");
        let market = market.insert(discount).insert(forward);

        let mismatch = bump_market_via_rate_quote_shock(
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(1.0),
        )
        .expect_err("independent term-index replay requires an independently fitted source");
        assert!(
            mismatch.to_string().contains("projection")
                || mismatch.to_string().contains("source factor")
        );
    }

    #[test]
    fn parallel_bump_shifts_futures_implied_rate_up() {
        let quotes = vec![
            RateQuote::Deposit {
                id: QuoteId::new("USD-DEP-3M"),
                index: IndexId::new("USD-SOFR-3M"),
                pillar: Pillar::Tenor("3M".parse().expect("valid tenor")),
                rate: 0.05,
            },
            RateQuote::Futures {
                id: QuoteId::new("USD-FUT-SEP26"),
                contract: IrFutureContractId::new("CME:SR3"),
                expiry: Date::from_calendar_date(2026, time::Month::September, 16)
                    .expect("valid date"),
                price: 96.00, // implied rate 4%
                convexity_adjustment: 0.0,
            },
            RateQuote::Swap {
                id: QuoteId::new("USD-SWAP-2Y"),
                index: IndexId::new("USD-SOFR-OIS"),
                pillar: Pillar::Tenor("2Y".parse().expect("valid tenor")),
                rate: 0.045,
                spread_decimal: None,
            },
        ];

        let implied_rate = |q: &RateQuote| -> f64 {
            match q {
                RateQuote::Deposit { rate, .. }
                | RateQuote::Fra { rate, .. }
                | RateQuote::Swap { rate, .. } => *rate,
                RateQuote::Futures { price, .. } => (100.0 - price) / 100.0,
            }
        };
        let base_rates: Vec<f64> = quotes.iter().map(implied_rate).collect();

        let bumped = apply_bump_to_rate_quotes(quotes, &QuoteBump::ParallelBp(1.0), &[]);

        for (q, base) in bumped.iter().zip(base_rates.iter()) {
            let moved = implied_rate(q) - base;
            assert!(
                (moved - 1e-4).abs() < 1e-12,
                "{}: implied rate must move +1bp, moved {moved:.8}",
                q.id().as_str()
            );
        }
    }

    #[test]
    fn projection_replay_returns_the_accepted_solve_and_rejects_a_shifted_source() {
        let base_date = time::macros::date!(2025 - 01 - 02);
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()
            .expect("discount curve");
        let context = MarketContext::new().insert(discount);
        let params = ForwardCurveParams {
            curve_id: CurveId::new("USD-TERM"),
            base_date,
            currency: Currency::USD,
            index_tenor: Some(finstack_quant_core::dates::Tenor::quarterly()),
            discount_curve_id: CurveId::new("USD-OIS"),
            method: CalibrationMethod::GlobalSolve {
                use_analytical_jacobian: false,
            },
            interpolation: InterpStyle::LogLinear,
            extrapolation: ExtrapolationPolicy::Extend,
            conventions: RatesStepConventions {
                curve_day_count: Some(DayCount::Act360),
                ois_compounding: None,
            },
        };
        let quotes: Vec<_> = [("3M", 0.04), ("6M", 0.042)]
            .into_iter()
            .map(|(tenor, rate)| RateQuote::Deposit {
                id: QuoteId::new(format!("SOFR-{tenor}")),
                index: IndexId::new("USD-SOFR-3M"),
                pillar: Pillar::Tenor(tenor.parse().unwrap()),
                rate,
            })
            .collect();
        let config = CalibrationConfig::default();
        let source = rebootstrap_forward_curve(quotes.clone(), &params, &context, &config).unwrap();
        let source = source
            .to_builder_with_id(source.id().clone())
            .fx_policy("projection-policy")
            .build()
            .unwrap();
        let record = required_forward_rate_calibration(&source).unwrap();
        let bump = QuoteBump::ParallelBp(1.0);
        let shifted =
            source
                .rebuild_with_knots(source.knots().iter().zip(source.factors()).map(
                    |(&time, &factor)| (time, if time == 0.0 { factor } else { 0.95 * factor }),
                ))
                .unwrap();
        assert!(
            bump_forward_curve_from_rate_calibration(&shifted, record, &context, &context, &bump)
                .is_err()
        );
        let actual =
            bump_forward_curve_from_rate_calibration(&source, record, &context, &context, &bump)
                .unwrap();
        let fitted = rebootstrap_forward_curve(
            quotes.iter().map(|quote| quote.bump_rate_bp(1.0)).collect(),
            &params,
            &context,
            &config,
        )
        .unwrap();
        assert_eq!(actual.knots(), fitted.knots());
        assert_eq!(actual.factors(), fitted.factors());
        assert_eq!(actual.rate_calibration(), fitted.rate_calibration());
        assert_eq!(actual.fx_policy(), source.fx_policy());
    }

    #[test]
    fn replay_checks_source_unit_cashflow_before_publishing_target() {
        let base = time::macros::date!(2025 - 01 - 02);
        let maturity = time::macros::date!(2026 - 01 - 02);
        let params = DiscountCurveParams {
            curve_id: CurveId::new("GBP-SONIA"),
            currency: Currency::GBP,
            base_date: base,
            method: CalibrationMethod::Bootstrap,
            interpolation: InterpStyle::LogLinear,
            extrapolation: ExtrapolationPolicy::Extend,
            pricing_discount_id: None,
            pricing_forward_id: None,
            conventions: RatesStepConventions {
                curve_day_count: Some(DayCount::Act365F),
                ois_compounding: None,
            },
        };
        // SONIA deposits start on trade date. The one-year unit payment has
        // discount factor 1/(1+r) since its ACT/365F accrual is exactly one.
        let rate = 1.0 / 0.95 - 1.0;
        let quotes =
            [time::macros::date!(2025 - 07 - 02), maturity].map(|end| RateQuote::Deposit {
                id: QuoteId::new(format!("SONIA-{end}")),
                index: IndexId::new("GBP-SONIA-OIS"),
                pillar: Pillar::Date(end),
                rate,
            });
        let context = MarketContext::new();
        let source =
            solve_discount_curve(&quotes, &params, &context, &CalibrationConfig::default())
                .unwrap();
        let recipe = required_discount_rate_calibration(&source).unwrap();
        let source_factor = source.df_on_date(maturity).unwrap();
        assert!((source_factor - 0.95).abs() < 1e-10);
        let bump = QuoteBump::ParallelBp((1.0 / 0.9499 - 1.0 / 0.95) * 10_000.0);
        let altered =
            source
                .rebuild_with_knots(source.knots().iter().zip(source.dfs()).map(
                    |(&time, &factor)| {
                        (
                            time,
                            if (time - 1.0).abs() < 1e-12 {
                                0.90
                            } else {
                                factor
                            },
                        )
                    },
                ))
                .unwrap();
        let err = bump_discount_curve_from_rate_calibration(&altered, recipe, &context, &bump)
            .unwrap_err();
        assert!(err.to_string().contains("source factor"));
        let replayed =
            bump_discount_curve_from_rate_calibration(&source, recipe, &context, &bump).unwrap();
        let target = replayed.df_on_date(maturity).unwrap();
        assert!((target - 0.9499).abs() < 1e-10);
        assert!((target - source_factor + 0.0001).abs() < 1e-10);
        assert!((target * (1.0 + rate + (1.0 / 0.9499 - 1.0 / 0.95)) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn sofr_cutoff_recipe_replays_zero_and_symmetric_quote_shocks() {
        let base_date =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid date");
        let index = IndexId::new("USD-SOFR-OIS");
        let quotes = vec![
            RateQuote::Deposit {
                id: QuoteId::new("SOFR-DEP-1M"),
                index: index.clone(),
                pillar: Pillar::Tenor("1M".parse().expect("valid tenor")),
                rate: 0.0430,
            },
            RateQuote::Swap {
                id: QuoteId::new("SOFR-OIS-1Y"),
                index: index.clone(),
                pillar: Pillar::Tenor("1Y".parse().expect("valid tenor")),
                rate: 0.0410,
                spread_decimal: None,
            },
            RateQuote::Swap {
                id: QuoteId::new("SOFR-OIS-2Y"),
                index: index.clone(),
                pillar: Pillar::Tenor("2Y".parse().expect("valid tenor")),
                rate: 0.0390,
                spread_decimal: None,
            },
        ];
        let params = DiscountCurveParams {
            curve_id: CurveId::new("USD-OIS"),
            currency: Currency::USD,
            base_date,
            method: CalibrationMethod::Bootstrap,
            interpolation: InterpStyle::Linear,
            extrapolation: ExtrapolationPolicy::Extend,
            pricing_discount_id: None,
            pricing_forward_id: None,
            conventions: RatesStepConventions {
                curve_day_count: Some(DayCount::Act365F),
                ois_compounding: Some(
                    finstack_quant_cashflows::builder::FloatingLegCompounding::CompoundedWithRateCutoff {
                        cutoff_days: 1,
                    },
                ),
            },
        };
        let context = MarketContext::new().insert_series(
            fixing_seed(index.as_str(), base_date, 0.0430).expect("SOFR fixing seed"),
        );
        let cfg = CalibrationConfig {
            ..CalibrationConfig::default()
        };
        let source = solve_discount_curve(&quotes, &params, &context, &cfg)
            .expect("source SOFR calibration");
        let calibration = source
            .rate_calibration()
            .cloned()
            .expect("calibrated curve recipe metadata");
        let altered =
            source
                .rebuild_with_knots(source.knots().iter().zip(source.dfs()).map(
                    |(&time, &factor)| {
                        (
                            time,
                            if time > 0.0 {
                                factor * 0.90 / 0.95
                            } else {
                                factor
                            },
                        )
                    },
                ))
                .expect("valid curve with mismatched prices");
        let error = bump_discount_curve_from_rate_calibration(
            &altered,
            &calibration,
            &context,
            &QuoteBump::ParallelBp(1.0),
        )
        .expect_err("reject source level before publishing bumped quotes");
        assert!(error.to_string().contains("source factor"));
        let recipe = source
            .rate_calibration()
            .expect("calibration target must stamp replay recipe");
        assert!(matches!(
            recipe.ois_compounding,
            Some(
                finstack_quant_core::market_data::term_structures::RateCalibrationOisCompounding::CompoundedWithRateCutoff {
                    cutoff_days: 1
                }
            )
        ));

        let zero = bump_discount_curve_from_rate_calibration(
            &source,
            &calibration,
            &context,
            &QuoteBump::ParallelBp(0.0),
        )
        .expect("zero quote shock");
        for (&time, &source_df) in source.knots().iter().zip(source.dfs()) {
            assert!(
                (zero.df(time).expect("valid") - source_df).abs() < 1e-12,
                "zero shock changed DF at {time}"
            );
        }

        for bump_bp in [-1.0, 1.0] {
            let replayed = bump_discount_curve_from_rate_calibration(
                &source,
                &calibration,
                &context,
                &QuoteBump::ParallelBp(bump_bp),
            )
            .expect("stored-recipe quote shock");
            let direct = solve_discount_curve(
                &quotes
                    .iter()
                    .map(|quote| quote.bump_rate_bp(bump_bp))
                    .collect::<Vec<_>>(),
                &params,
                &context,
                &cfg,
            )
            .expect("explicit-recipe quote shock");
            for &time in source.knots() {
                assert!(
                    (replayed.df(time).expect("valid") - direct.df(time).expect("valid")).abs()
                        < 1e-12,
                    "{bump_bp:+}bp replay mismatch at {time}: replayed={}, direct={}",
                    replayed.df(time).expect("valid"),
                    direct.df(time).expect("valid")
                );
            }
        }
    }

    #[test]
    fn discount_replay_rejects_a_failed_fit_before_returning_curve() {
        let params: DiscountCurveParams = serde_json::from_value(serde_json::json!({
            "curve_id": "USD-DISC", "currency": "USD", "base_date": "2025-01-02",
            "method": {"global_solve": {"use_analytical_jacobian": false}},
            "interpolation": "log_linear"
        }))
        .expect("params");
        let quotes = vec![RateQuote::Deposit {
            id: QuoteId::new("DEP"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor("5Y".parse().expect("tenor")),
            rate: 0.20,
        }];
        let mut config = CalibrationConfig::default();
        config.solver = config.solver.with_max_iterations(1);
        config.discount_curve.bootstrap_seed_global_solve = false;
        let market_quotes = quotes
            .iter()
            .cloned()
            .map(MarketQuote::Rates)
            .collect::<Vec<_>>();
        let (_, report) = step_runtime::execute_params_and_apply(
            &StepParams::Discount(params.clone()),
            &market_quotes,
            &MarketContext::new(),
            &config,
        )
        .expect("optimizer returns diagnostics");
        assert!(
            !report.success,
            "fixture must fail the fit gate: {report:?}"
        );
        let error = solve_discount_curve(&quotes, &params, &MarketContext::new(), &config)
            .expect_err("strict replay");
        assert!(error.to_string().contains("failed fit acceptance"));
        config.fail_on_bad_fit = false;
        solve_discount_curve(&quotes, &params, &MarketContext::new(), &config)
            .expect("explicit diagnostic policy");
    }

    #[test]
    fn typed_recipe_replay_restores_mixed_quote_fields() {
        let date = Date::from_calendar_date(2025, time::Month::September, 17).expect("valid date");
        let recipe = RateCalibrationRecipe {
            execution: Some(finstack_quant_core::market_data::term_structures::RateCalibrationExecution {
                bindings: ["deposit", "fra", "futures", "swap"].map(|id| finstack_quant_core::market_data::term_structures::RateQuoteBinding {
                    quote_id: id.to_owned(), pillar_date: date, pillar_time: 1.0,
                }).to_vec(), accepted: true, step_params: serde_json::json!({}), calibration_config: serde_json::json!({}),
            }),
            currency: Currency::USD,
            method: RateCalibrationMethod::Bootstrap,
            curve_day_count: DayCount::Act365F,
            ois_compounding: None,
            role: RateCalibrationCurveRole::Discount {
                projection_curve_id: CurveId::new("USD-OIS"),
            },
            quotes: vec![
                RateCalibrationQuote::Deposit {
                    index_id: IndexId::new("USD-SOFR-OIS"),
                    pillar: RateCalibrationPillar::Date(date),
                    rate: 0.043,
                },
                RateCalibrationQuote::Fra {
                    index_id: IndexId::new("USD-SOFR-3M"),
                    start: RateCalibrationPillar::Tenor(
                        "3M".parse().expect("valid start tenor"),
                    ),
                    end: RateCalibrationPillar::Date(date),
                    rate: 0.041,
                },
                RateCalibrationQuote::Futures {
                    contract: finstack_quant_core::market_data::term_structures::RateCalibrationFutureContractId::new("CME:SR3"),
                    expiry: date,
                    price: 95.75,
                    convexity_adjustment: Some(0.0001),
                },
                RateCalibrationQuote::Swap {
                    index_id: IndexId::new("USD-SOFR-OIS"),
                    pillar: RateCalibrationPillar::Tenor(
                        "5Y".parse().expect("valid swap tenor"),
                    ),
                    rate: 0.039,
                    spread_decimal: Some(0.00025),
                },
            ],
        };

        let restored =
            rate_quotes_from_recipe(&recipe, &CurveId::new("USD-OIS")).expect("typed replay");

        assert!(matches!(
            &restored[0],
            RateQuote::Deposit {
                pillar: Pillar::Date(value),
                ..
            } if *value == date
        ));
        assert!(matches!(
            &restored[1],
            RateQuote::Fra {
                start: Pillar::Tenor(_),
                end: Pillar::Date(value),
                ..
            } if *value == date
        ));
        assert!(matches!(
            &restored[2],
            RateQuote::Futures {
                contract,
                convexity_adjustment: value,
                ..
            } if contract.as_str() == "CME:SR3" && (*value - 0.0001).abs() < f64::EPSILON
        ));
        assert!(matches!(
            &restored[3],
            RateQuote::Swap {
                spread_decimal: Some(value),
                ..
            } if (*value - 0.00025).abs() < f64::EPSILON
        ));
    }
}
