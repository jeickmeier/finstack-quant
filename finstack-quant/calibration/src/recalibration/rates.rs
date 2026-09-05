//! Shared rates curve bumping logic (plan-driven calibration).

use super::cache::KeyedOnceCache;
use crate::api::schema::{DiscountCurveParams, ForwardCurveParams, StepParams};
use crate::config::CalibrationMethod;
use crate::config::RatesStepConventions;
use crate::quotes::ids::Pillar;
#[cfg(test)]
use crate::quotes::ids::QuoteId;
use crate::quotes::market_quote::MarketQuote;
use crate::quotes::rates::RateQuote;
use crate::step_runtime;
use crate::targets::rate_recipe::{ois_compounding_from_recipe, rate_quotes_from_recipe};
use crate::CalibrationConfig;
#[cfg(test)]
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, ForwardCurve, RateCalibrationCurveRole, RateCalibrationQuote,
    RateCalibrationRecipe,
};
#[cfg(test)]
use finstack_quant_core::math::interp::ExtrapolationPolicy;
use finstack_quant_core::types::{CurveId, IndexId};
use finstack_quant_valuations::recalibration::QuoteBump;
#[cfg(test)]
use std::cell::Cell;
use std::collections::HashSet;
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
    kind: RateMarketRecalibrationKind,
    bump: RateBumpKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DiscountRateRecalibrationKey {
    curve_id: String,
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
/// A cache instance is scoped to one immutable market snapshot. Per-key locks
/// allow unrelated curve sets and bump scenarios to calibrate concurrently,
/// while identical portfolio requests share the in-flight result. Failed
/// calibrations are not cached.
#[derive(Default)]
pub(crate) struct RateRecalibrationCache {
    market: KeyedOnceCache<RateMarketRecalibrationKey, MarketContext>,
    discount: KeyedOnceCache<DiscountRateRecalibrationKey, DiscountCurve>,
}

/// Bump a discount curve by shocking rate quotes and re-calibrating.
///
/// This applies a [`QuoteBump`] to a collection of [`RateQuote`]s and
/// re-executes the calibration step to produce a new [`DiscountCurve`].
///
/// # Arguments
///
/// * `quotes` - Original rate calibration quotes to shock and bootstrap;
///   quote IDs and maturity conventions must match `params`.
/// * `params` - Discount-curve calibration recipe, including base date,
///   curve ID, conventions, and calibration method.
/// * `base_context` - Unshocked market context supplying dependencies needed
///   by the calibration step.
/// * `bump` - Parallel or tenor-specific rate shock, expressed in basis points
///   as defined by [`QuoteBump`].
/// * `config` - Solver and validation policy to apply during re-calibration.
///   Preserve `params.method` on `config.calibration_method` when the recipe
///   method should override other documented defaults.
pub(crate) fn bump_discount_curve(
    quotes: &[RateQuote],
    params: &DiscountCurveParams,
    base_context: &MarketContext,
    bump: &QuoteBump,
    config: &CalibrationConfig,
) -> finstack_quant_core::Result<DiscountCurve> {
    #[cfg(test)]
    DISCOUNT_CALIBRATION_RUNS.with(|runs| runs.set(runs.get() + 1));
    let bumped_quotes = apply_bump_to_rate_quotes(quotes.to_vec(), bump, params.base_date);
    let market_quotes: Vec<MarketQuote> =
        bumped_quotes.into_iter().map(MarketQuote::Rates).collect();
    let step = StepParams::Discount(params.clone());
    let (ctx, report) =
        step_runtime::execute_params_and_apply(&step, &market_quotes, base_context, config)?;

    super::ensure_replay_fit_accepted(params.curve_id.as_str(), config, &report)?;
    Ok(ctx.get_discount(params.curve_id.as_str())?.as_ref().clone())
}

/// Bump a discount curve by shocking its stored market-rate calibration quotes.
///
/// The re-bootstrapped curves are applied as a *delta overlay* on the stored
/// curve: both the bumped and the unbumped quote sets are bootstrapped, and
/// only their discount-factor ratio is applied to the stored knots. Stored
/// curves transcribed from an external source (e.g. Bloomberg screen
/// fixtures) are not necessarily the exact bootstrap solution of their stored
/// quotes; repricing risk directly off a re-bootstrapped curve would shift
/// the base level and contaminate the sensitivity with a base-shape change.
/// For self-consistent curves the unbumped re-bootstrap reproduces the stored
/// curve and the overlay is exact.
///
/// # Arguments
///
/// * `curve` - Stored discount curve whose shape and validation policy are preserved.
/// * `calibration` - Exact typed recipe retained when the curve was calibrated.
/// * `context` - Market context supplying the recipe's pricing dependencies.
/// * `bump` - Parallel or tenor-specific quote shock in basis points.
pub fn bump_discount_curve_from_rate_calibration(
    curve: &DiscountCurve,
    calibration: &RateCalibrationRecipe,
    context: &MarketContext,
    bump: &QuoteBump,
) -> finstack_quant_core::Result<DiscountCurve> {
    bump_discount_curve_from_rate_calibration_with_projection(
        curve,
        calibration,
        context,
        bump,
        None,
        DiscountReplayShape::DeltaOverlay,
    )
}

pub(crate) fn bump_discount_curve_from_rate_calibration_cached(
    cache: Option<&RateRecalibrationCache>,
    curve: &DiscountCurve,
    calibration: &RateCalibrationRecipe,
    context: &MarketContext,
    bump: &QuoteBump,
) -> finstack_quant_core::Result<Arc<DiscountCurve>> {
    let key = DiscountRateRecalibrationKey {
        curve_id: curve.id().to_string(),
        bump: bump.into(),
    };
    KeyedOnceCache::get_or_compute(cache.map(|c| &c.discount), key, || {
        bump_discount_curve_from_rate_calibration(curve, calibration, context, bump)
    })
}

#[derive(Clone, Copy)]
enum DiscountReplayShape {
    DeltaOverlay,
    CalibratedOnSourceGrid,
}

fn bump_discount_curve_from_rate_calibration_with_projection(
    curve: &DiscountCurve,
    calibration: &RateCalibrationRecipe,
    context: &MarketContext,
    bump: &QuoteBump,
    pricing_forward_id_override: Option<CurveId>,
    replay_shape: DiscountReplayShape,
) -> finstack_quant_core::Result<DiscountCurve> {
    ensure_recipe_has_quotes(curve.id(), calibration)?;
    let quotes = rate_quotes_from_recipe(calibration, curve.id())?;

    let first_rate = quotes.first().map(RateQuote::implied_rate).unwrap_or(0.0);
    let fixings = fixing_seed(curve.id().as_str(), curve.base_date(), first_rate)?;
    let base_context = context.clone().insert_series(fixings);

    let (method, curve_day_count, ois_compounding, recipe_pricing_forward_id) =
        discount_replay_conventions(curve, calibration)?;
    let params = DiscountCurveParams {
        curve_id: curve.id().clone(),
        currency: calibration.currency,
        base_date: curve.base_date(),
        method,
        interpolation: curve.interp_style(),
        extrapolation: curve.extrapolation(),
        pricing_discount_id: Some(curve.id().clone()),
        pricing_forward_id: pricing_forward_id_override.or(recipe_pricing_forward_id),
        conventions: RatesStepConventions {
            ois_compounding,
            curve_day_count: Some(curve_day_count),
        },
    };

    let cfg = CalibrationConfig {
        calibration_method: params.method.clone(),
        validation: crate::validation::ValidationConfig {
            allow_negative_rates: curve.allows_non_monotonic(),
            ..crate::validation::ValidationConfig::default()
        },
        discount_curve: crate::DiscountCurveSolveConfig {
            allow_non_monotonic_final: Some(curve.allows_non_monotonic()),
            ..crate::DiscountCurveSolveConfig::default()
        },
        ..CalibrationConfig::default()
    };
    let bumped = bump_discount_curve(&quotes, &params, &base_context, bump, &cfg)?;
    if matches!(replay_shape, DiscountReplayShape::CalibratedOnSourceGrid) {
        let replayed_on_source_grid = curve
            .knots()
            .iter()
            .map(|&time| (time, bumped.df(time)))
            .collect::<Vec<_>>();
        return curve.rebuild_with_knots(replayed_on_source_grid);
    }
    let unbumped = bump_discount_curve(
        &quotes,
        &params,
        &base_context,
        &QuoteBump::ParallelBp(0.0),
        &cfg,
    )?;

    let overlaid: Vec<(f64, f64)> = curve
        .knots()
        .iter()
        .zip(curve.dfs())
        .map(|(&t, &df)| {
            let base_df = unbumped.df(t);
            let ratio = if base_df > 0.0 {
                bumped.df(t) / base_df
            } else {
                1.0
            };
            (t, df * ratio)
        })
        .collect();

    curve.rebuild_with_knots(overlaid)
}

fn discount_replay_conventions(
    curve: &DiscountCurve,
    recipe: &RateCalibrationRecipe,
) -> finstack_quant_core::Result<(
    CalibrationMethod,
    DayCount,
    Option<finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding>,
    Option<CurveId>,
)> {
    let projection_curve_id = match &recipe.role {
        RateCalibrationCurveRole::Discount {
            projection_curve_id,
        } => Some(projection_curve_id.clone()),
        RateCalibrationCurveRole::Projection { .. } => {
            return Err(finstack_quant_core::Error::Validation(format!(
                "discount curve {} carries a projection calibration recipe",
                curve.id()
            )));
        }
    };
    Ok((
        CalibrationMethod::from(&recipe.method),
        recipe.curve_day_count,
        recipe
            .ois_compounding
            .as_ref()
            .map(ois_compounding_from_recipe),
        projection_curve_id,
    ))
}

/// Bump a forward curve by shocking its stored market-rate calibration quotes
/// and globally recalibrating against the supplied market context.
///
/// The provided `context` must already contain the discount curve referenced by
/// the calibration recipe (in its bumped form, when bumping both curves
/// together). Basis-tenor calibrations use their dedicated forward rebuild.
///
/// Like [`bump_discount_curve_from_rate_calibration`], the recalibration is
/// applied as a delta overlay on the stored curve: the bumped and unbumped
/// global solves are both run and only their forward-rate difference is added
/// to the stored knots, so transcribed curves keep their base shape.
///
/// # Arguments
///
/// * `curve` - Stored forward curve whose shape and pricing grid are preserved.
/// * `calibration` - Exact typed recipe retained when the curve was calibrated.
/// * `context` - Market context containing the linked discount curve.
/// * `bump` - Parallel or tenor-specific quote shock in basis points.
pub fn bump_forward_curve_from_rate_calibration(
    curve: &ForwardCurve,
    calibration: &RateCalibrationRecipe,
    context: &MarketContext,
    bump: &QuoteBump,
) -> finstack_quant_core::Result<ForwardCurve> {
    ensure_recipe_has_quotes(curve.id(), calibration)?;
    let quotes = rate_quotes_from_recipe(calibration, curve.id())?;

    let (method, curve_day_count, ois_compounding, discount_curve_id) =
        forward_replay_conventions(curve, calibration)?;
    let params = ForwardCurveParams {
        curve_id: curve.id().clone(),
        currency: calibration.currency,
        base_date: curve.base_date(),
        tenor_years: curve.tenor(),
        discount_curve_id,
        method,
        interpolation: curve.interp_style(),
        conventions: RatesStepConventions {
            ois_compounding,
            curve_day_count: Some(curve_day_count),
        },
    };

    let bumped = rebootstrap_forward_curve(curve, quotes.clone(), &params, context, Some(bump))?;
    let unbumped = rebootstrap_forward_curve(curve, quotes, &params, context, None)?;

    let overlaid: Vec<(f64, f64)> = curve
        .knots()
        .iter()
        .zip(curve.forwards())
        .map(|(&t, &fwd)| (t, fwd + bumped.rate(t) - unbumped.rate(t)))
        .collect();

    ForwardCurve::builder(curve.id().clone(), curve.tenor())
        .base_date(curve.base_date())
        .reset_lag(curve.reset_lag())
        .day_count(curve.day_count())
        .knots(overlaid)
        .projection_grid_opt(
            curve
                .projection_grid()
                .map(<[f64]>::to_vec)
                .or_else(|| unbumped.projection_grid().map(<[f64]>::to_vec)),
        )
        .interp(curve.interp_style())
        .extrapolation(curve.extrapolation())
        .rate_calibration(calibration.clone())
        .fx_policy_opt(curve.fx_policy().map(ToOwned::to_owned))
        .build()
}

fn forward_replay_conventions(
    curve: &ForwardCurve,
    recipe: &RateCalibrationRecipe,
) -> finstack_quant_core::Result<(
    CalibrationMethod,
    DayCount,
    Option<finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding>,
    CurveId,
)> {
    let discount_curve_id = match &recipe.role {
        RateCalibrationCurveRole::Projection { discount_curve_id } => discount_curve_id.clone(),
        RateCalibrationCurveRole::Discount { .. } => {
            return Err(finstack_quant_core::Error::Validation(format!(
                "forward curve {} carries a discount calibration recipe",
                curve.id()
            )));
        }
    };
    Ok((
        CalibrationMethod::from(&recipe.method),
        recipe.curve_day_count,
        recipe
            .ois_compounding
            .as_ref()
            .map(ois_compounding_from_recipe),
        discount_curve_id,
    ))
}

/// Globally recalibrate a forward curve from (optionally bumped) rate quotes
/// using the stored curve's conventions.
fn rebootstrap_forward_curve(
    curve: &ForwardCurve,
    quotes: Vec<RateQuote>,
    params: &ForwardCurveParams,
    context: &MarketContext,
    bump: Option<&QuoteBump>,
) -> finstack_quant_core::Result<ForwardCurve> {
    let quotes = match bump {
        Some(bump) => apply_bump_to_rate_quotes(quotes, bump, curve.base_date()),
        None => quotes,
    };
    let market_quotes: Vec<MarketQuote> = quotes.into_iter().map(MarketQuote::Rates).collect();
    let step = StepParams::Forward(params.clone());
    let cfg = CalibrationConfig {
        calibration_method: params.method.clone(),
        ..CalibrationConfig::default()
    };
    let (ctx, report) =
        step_runtime::execute_params_and_apply(&step, &market_quotes, context, &cfg)?;
    super::ensure_replay_fit_accepted(params.curve_id.as_str(), &cfg, &report)?;
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
    let context = DayCountContext::default();
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

fn discount_implied_simple_forward(
    source: &ForwardCurve,
    discount: &DiscountCurve,
    start: f64,
    end: f64,
) -> finstack_quant_core::Result<f64> {
    if !(start.is_finite() && end.is_finite()) || end <= start {
        return Err(finstack_quant_core::Error::Validation(format!(
            "cannot derive linked OIS projection over invalid interval [{start}, {end}]"
        )));
    }
    let start_date = date_from_forward_time(source, start)?;
    let end_date = date_from_forward_time(source, end)?;
    let start_df = discount.df_on_date_curve(start_date)?;
    let end_df = discount.df_on_date_curve(end_date)?;
    if !(start_df.is_finite() && start_df > 0.0 && end_df.is_finite() && end_df > 0.0) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "cannot derive linked OIS projection over {start_date}..{end_date}: \
             invalid discount factors {start_df}/{end_df}"
        )));
    }
    let rate = (start_df / end_df - 1.0) / (end - start);
    if !rate.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "cannot derive finite linked OIS projection over [{start}, {end}]"
        )));
    }
    Ok(rate)
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

    let knots = if let Some(grid) = source.projection_grid() {
        let mut knots = Vec::with_capacity(grid.len());
        for period in grid.windows(2) {
            knots.push((
                period[0],
                discount_implied_simple_forward(source, discount, period[0], period[1])?,
            ));
        }
        let terminal = *grid.last().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "linked single-curve OIS projection {} has an empty pricing grid",
                source.id()
            ))
        })?;
        knots.push((
            terminal,
            discount_implied_simple_forward(source, discount, terminal, terminal + source.tenor())?,
        ));
        knots
    } else {
        source
            .knots()
            .iter()
            .map(|&start| {
                Ok((
                    start,
                    discount_implied_simple_forward(
                        source,
                        discount,
                        start,
                        start + source.tenor(),
                    )?,
                ))
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?
    };

    ForwardCurve::builder(source.id().clone(), source.tenor())
        .base_date(source.base_date())
        .reset_lag(source.reset_lag())
        .day_count(source.day_count())
        .knots(knots)
        .projection_grid_opt(source.projection_grid().map(<[f64]>::to_vec))
        .interp(source.interp_style())
        .extrapolation(source.extrapolation())
        .rate_calibration_opt(source.rate_calibration().cloned())
        .fx_policy_opt(source.fx_policy().map(ToOwned::to_owned))
        .build()
}

/// Re-bootstrap both a discount curve and its dependent forward curve from
/// stored typed rate-calibration recipes under a quote-space shock.
///
/// Index fixings are seeded from recipe quote indices and curve IDs so the
/// calibration engine has the reference fixings it needs while replaying.
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

    let fixing_sources = CalibrationFixingSources {
        discount_curve_id,
        discount_cal,
        forward_curve_id,
        forward_cal,
    };
    let seeded = seed_calibration_fixings(market, discount.base_date(), &fixing_sources)?;

    let bumped_discount = if linked_single_curve {
        bump_discount_curve_from_rate_calibration_with_projection(
            discount.as_ref(),
            discount_cal,
            &seeded,
            bump,
            Some(discount_curve_id.clone()),
            DiscountReplayShape::CalibratedOnSourceGrid,
        )?
    } else {
        bump_discount_curve_from_rate_calibration(discount.as_ref(), discount_cal, &seeded, bump)?
    };
    let seeded_with_discount = seeded.insert(bumped_discount);

    let bumped_forward = if linked_single_curve {
        let bumped_discount = seeded_with_discount.get_discount(discount_curve_id.as_str())?;
        rebuild_linked_ois_projection(forward.as_ref(), bumped_discount.as_ref())?
    } else {
        bump_forward_curve_from_rate_calibration(
            forward.as_ref(),
            forward_cal,
            &seeded_with_discount,
            bump,
        )?
    };
    Ok(seeded_with_discount.insert(bumped_forward))
}

pub(crate) fn bump_market_via_rate_quote_shock_cached(
    cache: Option<&RateRecalibrationCache>,
    market: &MarketContext,
    discount_curve_id: &CurveId,
    forward_curve_id: &CurveId,
    bump: &QuoteBump,
) -> finstack_quant_core::Result<Arc<MarketContext>> {
    bump.validate()?;
    let key = RateMarketRecalibrationKey {
        kind: RateMarketRecalibrationKind::DiscountAndForward {
            discount_curve_id: discount_curve_id.to_string(),
            forward_curve_id: forward_curve_id.to_string(),
        },
        bump: bump.into(),
    };
    KeyedOnceCache::get_or_compute(cache.map(|c| &c.market), key, || {
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

    let mut seeded = market.clone();
    let mut seeded_indices = HashSet::new();
    seeded = seed_recipe_fixings(
        seeded,
        discount_cal,
        discount.base_date(),
        &mut seeded_indices,
    )?;
    let first_rate = discount_cal
        .quotes
        .first()
        .map(rate_calibration_quote_level);
    if let Some(rate) = first_rate {
        seeded = seeded.insert_series(fixing_seed(curve_id.as_str(), discount.base_date(), rate)?);
    }

    let bumped = bump_discount_curve_from_rate_calibration_with_projection(
        discount.as_ref(),
        discount_cal,
        &seeded,
        bump,
        Some(curve_id.clone()),
        DiscountReplayShape::CalibratedOnSourceGrid,
    )?;
    Ok(seeded.insert(bumped))
}

pub(crate) fn bump_single_ois_market_via_rate_quote_shock_cached(
    cache: Option<&RateRecalibrationCache>,
    market: &MarketContext,
    curve_id: &CurveId,
    bump: &QuoteBump,
) -> finstack_quant_core::Result<Arc<MarketContext>> {
    bump.validate()?;
    let key = RateMarketRecalibrationKey {
        kind: RateMarketRecalibrationKind::SingleOis {
            curve_id: curve_id.to_string(),
        },
        bump: bump.into(),
    };
    KeyedOnceCache::get_or_compute(cache.map(|c| &c.market), key, || {
        bump_single_ois_market_via_rate_quote_shock(market, curve_id, bump)
    })
}

/// Seed bootstrap-time fixings for both curve and index identifiers so the
/// calibration engine has the reference rates it needs when re-bootstrapping
/// after a quote shock. Uses the first quote of each calibration set as the
/// historical fixing — sufficient for risk re-bootstrapping where only the
/// shape of the curve matters, not the historical realized path.
struct CalibrationFixingSources<'a> {
    discount_curve_id: &'a CurveId,
    discount_cal: &'a RateCalibrationRecipe,
    forward_curve_id: &'a CurveId,
    forward_cal: &'a RateCalibrationRecipe,
}

fn seed_calibration_fixings(
    market: &MarketContext,
    base_date: Date,
    sources: &CalibrationFixingSources<'_>,
) -> finstack_quant_core::Result<MarketContext> {
    let mut seeded = market.clone();
    let mut seeded_indices = HashSet::new();
    seeded = seed_recipe_fixings(seeded, sources.discount_cal, base_date, &mut seeded_indices)?;
    let discount_rate = sources
        .discount_cal
        .quotes
        .first()
        .map(rate_calibration_quote_level);
    if let Some(rate) = discount_rate {
        seeded = seeded.insert_series(fixing_seed(
            sources.discount_curve_id.as_str(),
            base_date,
            rate,
        )?);
    }
    seeded = seed_recipe_fixings(seeded, sources.forward_cal, base_date, &mut seeded_indices)?;
    let forward_rate = sources
        .forward_cal
        .quotes
        .first()
        .map(rate_calibration_quote_level);
    if let Some(rate) = forward_rate {
        seeded = seeded.insert_series(fixing_seed(
            sources.forward_curve_id.as_str(),
            base_date,
            rate,
        )?);
    }
    Ok(seeded)
}

fn seed_recipe_fixings(
    mut market: MarketContext,
    recipe: &RateCalibrationRecipe,
    base_date: Date,
    seeded_indices: &mut HashSet<IndexId>,
) -> finstack_quant_core::Result<MarketContext> {
    for quote in &recipe.quotes {
        let index_id = match quote {
            RateCalibrationQuote::Deposit { index_id, .. }
            | RateCalibrationQuote::Fra { index_id, .. }
            | RateCalibrationQuote::Swap { index_id, .. }
            | RateCalibrationQuote::Basis { index_id, .. } => Some(index_id),
            RateCalibrationQuote::Futures { .. } => None,
        };
        if let Some(index_id) = index_id {
            if !seeded_indices.insert(index_id.clone()) {
                continue;
            }
            market = market.insert_series(fixing_seed(
                index_id.as_str(),
                base_date,
                rate_calibration_quote_level(quote),
            )?);
        }
    }
    Ok(market)
}

fn rate_calibration_quote_level(quote: &RateCalibrationQuote) -> f64 {
    match quote {
        RateCalibrationQuote::Deposit { rate, .. }
        | RateCalibrationQuote::Fra { rate, .. }
        | RateCalibrationQuote::Swap { rate, .. } => *rate,
        RateCalibrationQuote::Basis { spread_decimal, .. } => *spread_decimal,
        RateCalibrationQuote::Futures {
            price,
            convexity_adjustment,
            ..
        } => (100.0 - price) / 100.0 - convexity_adjustment.unwrap_or(0.0),
    }
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
    )
}

/// Apply a [`QuoteBump`] to a vector of [`RateQuote`]s.
///
/// Parallel bumps shift every quote; tenor bumps locate the closest quote to
/// each target year fraction and shift only that quote. Pure data transform —
/// no calibration engine involvement.
fn apply_bump_to_rate_quotes(
    quotes: Vec<RateQuote>,
    bump: &QuoteBump,
    as_of: Date,
) -> Vec<RateQuote> {
    match bump {
        QuoteBump::ParallelBp(bp) => quotes.into_iter().map(|q| q.bump_rate_bp(*bp)).collect(),
        QuoteBump::TenorsBp(targets) => {
            let mut q = quotes;
            for (target_t, bp) in targets {
                if let Some(idx) = find_closest_quote(&q, *target_t, as_of) {
                    q[idx] = q[idx].bump_rate_bp(*bp);
                }
            }
            q
        }
    }
}

/// Helper to resolve maturity date of a quote.
fn resolve_maturity(q: &RateQuote, base_date: Date) -> Option<Date> {
    // Basic resolution using base_date + pillar
    // This ignores spot lag or BDC, but is sufficient for "closest quote" heuristics.
    match q {
        RateQuote::Deposit { pillar, .. } | RateQuote::Swap { pillar, .. } => {
            resolve_pillar(pillar, base_date)
        }
        RateQuote::Fra { end, .. } => resolve_pillar(end, base_date),
        RateQuote::Futures { expiry, .. } => Some(*expiry),
    }
}

fn resolve_pillar(pillar: &Pillar, base_date: Date) -> Option<Date> {
    match pillar {
        Pillar::Date(d) => Some(*d),
        Pillar::Tenor(t) => {
            // Approx add tenor
            // For bumping grouping, exact BDC usually doesn't change the "closest" logic significantly.
            t.add_to_date(
                base_date,
                None,
                finstack_quant_core::dates::BusinessDayConvention::Following,
            )
            .ok()
        }
    }
}

/// Find the quote closest to the target maturity.
pub(crate) fn find_closest_quote(
    quotes: &[RateQuote],
    target_years: f64,
    as_of: Date,
) -> Option<usize> {
    let day_count = DayCount::Act365F; // Simple day count for proximity check
    quotes
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let a_date = resolve_maturity(a, as_of).unwrap_or(as_of);
            let b_date = resolve_maturity(b, as_of).unwrap_or(as_of);

            let a_yf = day_count
                .year_fraction(as_of, a_date, DayCountContext::default())
                .unwrap_or(0.0);
            let b_yf = day_count
                .year_fraction(as_of, b_date, DayCountContext::default())
                .unwrap_or(0.0);
            let a_dist = (a_yf - target_years).abs();
            let b_dist = (b_yf - target_years).abs();
            a_dist
                .partial_cmp(&b_dist)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use finstack_quant_core::market_data::term_structures::{
        RateCalibrationMethod, RateCalibrationOisCompounding, RateCalibrationPillar,
    };
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_valuations::market::conventions::ids::IrFutureContractId;

    fn linked_single_curve_ois_market(
        forward_discount_curve_id: CurveId,
    ) -> (MarketContext, CurveId, CurveId, Vec<f64>) {
        let base_date =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid date");
        let discount_curve_id = CurveId::new("USD-OIS");
        let forward_curve_id = CurveId::new("USD-SOFR-OIS");
        let index_id = IndexId::new("USD-SOFR-OIS");
        let quotes = vec![
            RateCalibrationQuote::Deposit {
                index_id: index_id.clone(),
                pillar: RateCalibrationPillar::Tenor("6M".parse().expect("valid tenor")),
                rate: 0.0430,
            },
            RateCalibrationQuote::Deposit {
                index_id: index_id.clone(),
                pillar: RateCalibrationPillar::Tenor("1Y".parse().expect("valid tenor")),
                rate: 0.0410,
            },
            RateCalibrationQuote::Deposit {
                index_id,
                pillar: RateCalibrationPillar::Tenor("2Y".parse().expect("valid tenor")),
                rate: 0.0390,
            },
        ];
        let discount_recipe = RateCalibrationRecipe {
            currency: Currency::USD,
            method: RateCalibrationMethod::Bootstrap,
            curve_day_count: DayCount::Act365F,
            ois_compounding: Some(RateCalibrationOisCompounding::Simple),
            role: RateCalibrationCurveRole::Discount {
                projection_curve_id: forward_curve_id.clone(),
            },
            quotes: quotes.clone(),
        };
        let discount = DiscountCurve::builder(discount_curve_id.clone())
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (0.5, 0.979), (1.0, 0.960), (2.0, 0.925)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::FlatForward)
            .rate_calibration(discount_recipe)
            .fx_policy("single_curve_ois::USD")
            .build()
            .expect("discount representation");

        let projection_grid = vec![0.0, 0.5, 1.0, 1.5, 2.0];
        let mut forward_knots: Vec<(f64, f64)> = projection_grid
            .windows(2)
            .map(|period| {
                let (start, end) = (period[0], period[1]);
                (
                    start,
                    (discount.df(start) / discount.df(end) - 1.0) / (end - start),
                )
            })
            .collect();
        let terminal = *projection_grid.last().expect("terminal projection time");
        forward_knots.push((
            terminal,
            (discount.df(terminal) / discount.df(terminal + 0.5) - 1.0) / 0.5,
        ));

        let forward_recipe = RateCalibrationRecipe {
            currency: Currency::USD,
            method: RateCalibrationMethod::GlobalSolve {
                use_analytical_jacobian: false,
            },
            curve_day_count: DayCount::Act365F,
            ois_compounding: Some(RateCalibrationOisCompounding::Simple),
            role: RateCalibrationCurveRole::Projection {
                discount_curve_id: forward_discount_curve_id,
            },
            quotes,
        };
        let forward = ForwardCurve::builder(forward_curve_id.clone(), 0.5)
            .base_date(base_date)
            .reset_lag(1)
            .day_count(DayCount::Act360)
            .knots(forward_knots)
            .projection_grid(projection_grid.clone())
            .interp(InterpStyle::CubicHermite)
            .extrapolation(ExtrapolationPolicy::FlatForward)
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
                .df_on_date_curve(start_date)
                .expect("discount factor on reset date")
                / shocked_discount
                    .df_on_date_curve(end_date)
                    .expect("discount factor on payment date")
                - 1.0)
                / (end - start);
            let projected = shocked_forward
                .rate_between(start, end)
                .expect("projection-grid forward");
            assert!(
                (projected - discount_implied).abs() < 1e-12,
                "linked OIS representations diverged over [{start}, {end}]: \
                 projection={projected:.12}, discount-implied={discount_implied:.12}"
            );
        }
        assert!(
            (shocked_discount.df(1.0) - source_discount.df(1.0)).abs() > 1e-8,
            "non-zero shared quote shock must move the linked discount curve"
        );

        assert_eq!(shocked_forward.id(), source_forward.id());
        assert_eq!(
            shocked_forward.projection_grid(),
            source_forward.projection_grid()
        );
        assert_eq!(shocked_forward.reset_lag(), source_forward.reset_lag());
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
        assert_eq!(
            shocked_forward.rate_calibration(),
            source_forward.rate_calibration()
        );
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
                .df_on_date_curve(start_date)
                .expect("discount factor on reset date")
                / shocked_discount
                    .df_on_date_curve(end_date)
                    .expect("discount factor on payment date")
                - 1.0)
                / (end - start);
            let projected = shocked_forward
                .rate_between(start, end)
                .expect("projection-grid forward");
            assert!(
                (projected - date_implied).abs() < 1e-12,
                "mixed-day-count linked projection diverged over {start_date}..{end_date}: \
                 projection={projected:.12}, date-implied={date_implied:.12}"
            );
        }
    }

    #[test]
    fn linked_ois_quote_shock_calibrates_discount_once() {
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
            1,
            "linked quote shock must not run a redundant zero-bump discount calibration"
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
            Some(&cache),
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(5.0),
        )
        .expect("first cached quote shock");
        let second = bump_market_via_rate_quote_shock_cached(
            Some(&cache),
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(5.0),
        )
        .expect("reused cached quote shock");

        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(
            DISCOUNT_CALIBRATION_RUNS.with(Cell::get),
            1,
            "an identical batch request must share one calibration"
        );
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
    fn term_index_projection_recipe_remains_independent() {
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

        let shocked = bump_market_via_rate_quote_shock(
            &market,
            &discount_curve_id,
            &forward_curve_id,
            &QuoteBump::ParallelBp(1.0),
        )
        .expect("term-index recipes must route independently");
        let shocked_forward = shocked
            .get_forward(forward_curve_id.as_str())
            .expect("independently replayed term-index projection");
        assert_eq!(
            shocked_forward
                .rate_calibration()
                .expect("term-index recipe")
                .ois_compounding,
            None
        );
        let shocked_discount = shocked
            .get_discount(discount_curve_id.as_str())
            .expect("independently replayed discount curve");
        let grid = shocked_forward
            .projection_grid()
            .expect("term-index projection grid");
        let first_period = &grid[0..2];
        let projected = shocked_forward
            .rate_between(first_period[0], first_period[1])
            .expect("term-index forward");
        let start_date =
            shocked_forward.base_date() + Duration::days((first_period[0] * 360.0).round() as i64);
        let end_date =
            shocked_forward.base_date() + Duration::days((first_period[1] * 360.0).round() as i64);
        let discount_implied = (shocked_discount
            .df_on_date_curve(start_date)
            .expect("discount factor on reset date")
            / shocked_discount
                .df_on_date_curve(end_date)
                .expect("discount factor on payment date")
            - 1.0)
            / (first_period[1] - first_period[0]);
        assert!(
            (projected - discount_implied).abs() > 1e-8,
            "term-index replay was silently replaced by discount-derived projection: \
             projection={projected:.12}, discount-implied={discount_implied:.12}"
        );
    }

    /// Parallel "rate bp" bumps must shock every quote's *rate* by +1bp,
    /// including futures, where price = 100·(1 − rate) means the price must
    /// fall by 0.01. Regression for the bug where the decimal bump was added
    /// to the futures price verbatim (wrong sign, 1/100 magnitude), silently
    /// mis-shocking futures pillars in plan-driven parallel/key-rate bumps.
    #[test]
    fn parallel_bump_shifts_futures_implied_rate_up() {
        let as_of = Date::from_calendar_date(2026, time::Month::June, 9).expect("valid date");
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

        let bumped = apply_bump_to_rate_quotes(quotes, &QuoteBump::ParallelBp(1.0), as_of);

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
    fn quote_shock_preserves_source_projection_grid_and_zero_shock_forwards() {
        let base_date =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid date");
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()
            .expect("discount curve");
        let index_id = IndexId::new("USD-SOFR-3M");
        let calibration = RateCalibrationRecipe {
            currency: Currency::USD,
            method: RateCalibrationMethod::GlobalSolve {
                use_analytical_jacobian: false,
            },
            curve_day_count: DayCount::Act360,
            ois_compounding: None,
            role: RateCalibrationCurveRole::Projection {
                discount_curve_id: CurveId::new("USD-OIS"),
            },
            quotes: vec![
                RateCalibrationQuote::Deposit {
                    index_id: index_id.clone(),
                    pillar: RateCalibrationPillar::Tenor("3M".parse().expect("valid tenor")),
                    rate: 0.0400,
                },
                RateCalibrationQuote::Deposit {
                    index_id,
                    pillar: RateCalibrationPillar::Tenor("6M".parse().expect("valid tenor")),
                    rate: 0.0420,
                },
            ],
        };
        let cap_projection_grid = vec![0.0, 91.0 / 360.0, 182.0 / 360.0, 273.0 / 360.0];
        let source = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(base_date)
            .reset_lag(2)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.0400), (0.25, 0.0410), (0.50, 0.0420)])
            .projection_grid(cap_projection_grid.clone())
            .interp(InterpStyle::CubicHermite)
            .extrapolation(ExtrapolationPolicy::FlatForward)
            .rate_calibration(calibration.clone())
            .fx_policy("xccy_basis::USD/EUR")
            .build()
            .expect("source forward curve");
        let context = MarketContext::new().insert(discount);

        let shocked = bump_forward_curve_from_rate_calibration(
            &source,
            &calibration,
            &context,
            &QuoteBump::ParallelBp(1.0),
        )
        .expect("parallel quote shock");

        assert_eq!(
            shocked.projection_grid(),
            Some(cap_projection_grid.as_slice()),
            "quote-shock overlay must retain the source pricing grid"
        );
        let zero_shocked = bump_forward_curve_from_rate_calibration(
            &source,
            &calibration,
            &context,
            &QuoteBump::ParallelBp(0.0),
        )
        .expect("zero quote shock");
        for period in cap_projection_grid.windows(2) {
            let source_forward = source
                .rate_between(period[0], period[1])
                .expect("source contractual forward");
            let shocked_forward = zero_shocked
                .rate_between(period[0], period[1])
                .expect("shocked contractual forward");
            assert!(
                (shocked_forward - source_forward).abs() < 1e-12,
                "zero shock changed contractual forward over [{:.12}, {:.12}]: \
                 source={source_forward:.12}, shocked={shocked_forward:.12}",
                period[0],
                period[1]
            );
        }
        assert_eq!(shocked.reset_lag(), source.reset_lag());
        assert_eq!(shocked.day_count(), source.day_count());
        assert_eq!(shocked.interp_style(), source.interp_style());
        assert_eq!(shocked.extrapolation(), source.extrapolation());
        assert_eq!(shocked.fx_policy(), source.fx_policy());
        let shocked_calibration = shocked
            .rate_calibration()
            .expect("calibration metadata must survive quote shock");
        assert_eq!(shocked_calibration, &calibration);
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
            extrapolation: ExtrapolationPolicy::FlatForward,
            pricing_discount_id: None,
            pricing_forward_id: None,
            conventions: RatesStepConventions {
                curve_day_count: Some(DayCount::Act365F),
                ois_compounding: Some(
                    finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding::CompoundedWithRateCutoff {
                        cutoff_days: 1,
                    },
                ),
            },
        };
        let context = MarketContext::new().insert_series(
            fixing_seed(index.as_str(), base_date, 0.0430).expect("SOFR fixing seed"),
        );
        let cfg = CalibrationConfig {
            calibration_method: params.method.clone(),
            ..CalibrationConfig::default()
        };
        let source = bump_discount_curve(
            &quotes,
            &params,
            &context,
            &QuoteBump::ParallelBp(0.0),
            &cfg,
        )
        .expect("source SOFR calibration");
        let calibration = source
            .rate_calibration()
            .cloned()
            .expect("calibrated curve recipe metadata");
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
                (zero.df(time) - source_df).abs() < 1e-12,
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
            let direct = bump_discount_curve(
                &quotes,
                &params,
                &context,
                &QuoteBump::ParallelBp(bump_bp),
                &cfg,
            )
            .expect("explicit-recipe quote shock");
            for &time in source.knots() {
                assert!(
                    (replayed.df(time) - direct.df(time)).abs() < 1e-12,
                    "{bump_bp:+}bp replay mismatch at {time}: replayed={}, direct={}",
                    replayed.df(time),
                    direct.df(time)
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
        let error = bump_discount_curve(
            &quotes,
            &params,
            &MarketContext::new(),
            &QuoteBump::ParallelBp(0.0),
            &config,
        )
        .expect_err("strict replay");
        assert!(error.to_string().contains("failed fit acceptance"));
        config.fail_on_bad_fit = false;
        bump_discount_curve(
            &quotes,
            &params,
            &MarketContext::new(),
            &QuoteBump::ParallelBp(0.0),
            &config,
        )
        .expect("explicit diagnostic policy");
    }

    #[test]
    fn discount_quote_overlay_preserves_source_validation_policy() {
        let base_date =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid date");
        let index_id = IndexId::new("USD-SOFR-OIS");
        let calibration = RateCalibrationRecipe {
            currency: Currency::USD,
            method: RateCalibrationMethod::Bootstrap,
            curve_day_count: DayCount::Act365F,
            ois_compounding: None,
            role: RateCalibrationCurveRole::Discount {
                projection_curve_id: CurveId::new("USD-OIS"),
            },
            quotes: vec![
                RateCalibrationQuote::Deposit {
                    index_id: index_id.clone(),
                    pillar: RateCalibrationPillar::Tenor("1Y".parse().expect("valid tenor")),
                    rate: -0.01,
                },
                RateCalibrationQuote::Deposit {
                    index_id,
                    pillar: RateCalibrationPillar::Tenor("2Y".parse().expect("valid tenor")),
                    rate: 0.005,
                },
            ],
        };
        let source = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 1.01), (2.0, 0.99)])
            .rate_calibration(calibration.clone())
            .validation(
                finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                    allow_non_monotonic: true,
                    forward_floor: Some(-0.02),
                },
            )
            .build()
            .expect("negative-rate source curve");

        let overlaid = bump_discount_curve_from_rate_calibration(
            &source,
            &calibration,
            &MarketContext::new(),
            &QuoteBump::ParallelBp(0.0),
        )
        .expect("zero quote overlay must preserve permissive source policy");
        let serialized = serde_json::to_value(overlaid).expect("serialize overlaid curve");

        assert_eq!(serialized["allow_non_monotonic"], true);
        assert_eq!(serialized["min_forward_rate"], -0.02);
    }

    #[test]
    fn typed_recipe_replay_restores_mixed_quote_fields() {
        let date = Date::from_calendar_date(2025, time::Month::September, 17).expect("valid date");
        let recipe = RateCalibrationRecipe {
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

    #[test]
    fn typed_recipe_seeds_each_index_from_first_quote_once() {
        let base_date =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid date");
        let shared_index = IndexId::new("USD-SOFR-OIS");
        let other_index = IndexId::new("USD-SOFR-3M");
        let recipe = RateCalibrationRecipe {
            currency: Currency::USD,
            method: RateCalibrationMethod::Bootstrap,
            curve_day_count: DayCount::Act365F,
            ois_compounding: None,
            role: RateCalibrationCurveRole::Discount {
                projection_curve_id: CurveId::new("USD-OIS"),
            },
            quotes: vec![
                RateCalibrationQuote::Deposit {
                    index_id: shared_index.clone(),
                    pillar: RateCalibrationPillar::Tenor(
                        "1M".parse().expect("valid deposit tenor"),
                    ),
                    rate: 0.011,
                },
                RateCalibrationQuote::Swap {
                    index_id: shared_index,
                    pillar: RateCalibrationPillar::Tenor("5Y".parse().expect("valid swap tenor")),
                    rate: 0.099,
                    spread_decimal: Some(0.0002),
                },
                RateCalibrationQuote::Fra {
                    index_id: other_index,
                    start: RateCalibrationPillar::Tenor("3M".parse().expect("valid FRA start")),
                    end: RateCalibrationPillar::Tenor("6M".parse().expect("valid FRA end")),
                    rate: 0.022,
                },
            ],
        };

        let replayed =
            rate_quotes_from_recipe(&recipe, &CurveId::new("USD-OIS")).expect("exact replay");
        assert!(matches!(
            (&replayed[0], &replayed[1]),
            (
                RateQuote::Deposit { rate: first, .. },
                RateQuote::Swap {
                    rate: second,
                    spread_decimal: Some(spread),
                    ..
                }
            ) if (*first - 0.011).abs() < f64::EPSILON
                && (*second - 0.099).abs() < f64::EPSILON
                && (*spread - 0.0002).abs() < f64::EPSILON
        ));

        let seeded = seed_recipe_fixings(
            MarketContext::new(),
            &recipe,
            base_date,
            &mut HashSet::new(),
        )
        .expect("seed recipe fixings");
        assert!(
            (seeded
                .get_series("FIXING:USD-SOFR-OIS")
                .expect("shared index fixing")
                .value_on_exact(base_date)
                .expect("shared index fixing value")
                - 0.011)
                .abs()
                < f64::EPSILON
        );
        assert!(
            (seeded
                .get_series("FIXING:USD-SOFR-3M")
                .expect("other index fixing")
                .value_on_exact(base_date)
                .expect("other index fixing value")
                - 0.022)
                .abs()
                < f64::EPSILON
        );
    }
}
