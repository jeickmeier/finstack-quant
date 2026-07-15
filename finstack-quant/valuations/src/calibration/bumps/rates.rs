//! Shared rates curve bumping logic (plan-driven calibration).

use super::currency::infer_currency_from_id;
use super::BumpRequest;
use crate::calibration::api::schema::{DiscountCurveParams, ForwardCurveParams, StepParams};
use crate::calibration::config::CalibrationMethod;
use crate::calibration::config::RatesStepConventions;
use crate::calibration::step_runtime;
use crate::calibration::CalibrationConfig;
use crate::market::quotes::ids::{Pillar, QuoteId};
use crate::market::quotes::market_quote::MarketQuote;
use crate::market::quotes::rates::RateQuote;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, DiscountCurveRateCalibration, DiscountCurveRateQuoteType, ForwardCurve,
    ForwardCurveRateCalibration, ForwardCurveRateQuote, RateCalibrationCurveRole,
    RateCalibrationMethod, RateCalibrationOisCompounding, RateCalibrationPillar,
    RateCalibrationQuote, RateCalibrationRecipe,
};
use finstack_quant_core::math::interp::ExtrapolationPolicy;
use finstack_quant_core::types::{CurveId, IndexId};
#[cfg(test)]
use std::cell::Cell;
use std::collections::HashSet;
use time::Duration;

#[cfg(test)]
std::thread_local! {
    static DISCOUNT_CALIBRATION_RUNS: Cell<usize> = const { Cell::new(0) };
}

/// Infer currency from a discount curve ID using token-by-token heuristics.
///
/// Best-effort fallback for callers that don't have explicit currency metadata.
/// Returns USD if no known currency or benchmark-rate token appears in the ID.
pub fn infer_currency_from_discount_curve_id(curve: &DiscountCurve) -> Currency {
    infer_currency_from_id(curve.id().as_str())
}

/// Bump a discount curve by shocking rate quotes and re-calibrating.
///
/// This applies a [`BumpRequest`] to a collection of [`RateQuote`]s and
/// re-executes the calibration step to produce a new [`DiscountCurve`].
pub fn bump_discount_curve(
    quotes: &[RateQuote],
    params: &DiscountCurveParams,
    base_context: &MarketContext,
    bump: &BumpRequest,
) -> finstack_quant_core::Result<DiscountCurve> {
    // Recipe metadata currently persists the calibration method rather than
    // every numerical solver knob. Preserve the requested method here while
    // retaining documented CalibrationConfig defaults for legacy solver fields.
    let cfg = CalibrationConfig {
        calibration_method: params.method.clone(),
        ..CalibrationConfig::default()
    };
    bump_discount_curve_with_config(quotes, params, base_context, bump, &cfg)
}

fn bump_discount_curve_with_config(
    quotes: &[RateQuote],
    params: &DiscountCurveParams,
    base_context: &MarketContext,
    bump: &BumpRequest,
    config: &CalibrationConfig,
) -> finstack_quant_core::Result<DiscountCurve> {
    #[cfg(test)]
    DISCOUNT_CALIBRATION_RUNS.with(|runs| runs.set(runs.get() + 1));
    let bumped_quotes = apply_bump_to_rate_quotes(quotes.to_vec(), bump, params.base_date);
    let market_quotes: Vec<MarketQuote> =
        bumped_quotes.into_iter().map(MarketQuote::Rates).collect();
    let step = StepParams::Discount(params.clone());
    let (ctx, _report) =
        step_runtime::execute_params_and_apply(&step, &market_quotes, base_context, config)?;

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
pub(crate) fn bump_discount_curve_from_rate_calibration(
    curve: &DiscountCurve,
    calibration: &DiscountCurveRateCalibration,
    context: &MarketContext,
    bump: &BumpRequest,
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

#[derive(Clone, Copy)]
enum DiscountReplayShape {
    DeltaOverlay,
    CalibratedOnSourceGrid,
}

fn bump_discount_curve_from_rate_calibration_with_projection(
    curve: &DiscountCurve,
    calibration: &DiscountCurveRateCalibration,
    context: &MarketContext,
    bump: &BumpRequest,
    pricing_forward_id_override: Option<CurveId>,
    replay_shape: DiscountReplayShape,
) -> finstack_quant_core::Result<DiscountCurve> {
    let recipe = curve.rate_calibration_recipe();
    let quotes = if let Some(recipe) = recipe.filter(|recipe| !recipe.quotes.is_empty()) {
        rate_quotes_from_recipe(recipe, curve.id())?
    } else {
        rate_quotes_from_legacy_discount_calibration(calibration, curve.id())?
    };

    let first_rate = quotes.first().map(rate_quote_level).unwrap_or(0.0);
    let fixings = ScalarTimeSeries::new(
        format!("FIXING:{}", curve.id()),
        vec![
            (curve.base_date() - Duration::days(3), first_rate),
            (curve.base_date() - Duration::days(2), first_rate),
            (curve.base_date() - Duration::days(1), first_rate),
            (curve.base_date(), first_rate),
        ],
        None,
    )?;
    let base_context = context.clone().insert_series(fixings);

    let (method, curve_day_count, ois_compounding, recipe_pricing_forward_id) =
        discount_replay_conventions(curve, recipe)?;
    let params = DiscountCurveParams {
        curve_id: curve.id().clone(),
        currency: recipe
            .and_then(|recipe| recipe.currency)
            .unwrap_or(calibration.currency),
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
        discount_curve: crate::calibration::DiscountCurveSolveConfig {
            allow_non_monotonic_final: Some(curve.allows_non_monotonic()),
            ..crate::calibration::DiscountCurveSolveConfig::default()
        },
        ..CalibrationConfig::default()
    };
    let bumped = bump_discount_curve_with_config(&quotes, &params, &base_context, bump, &cfg)?;
    if matches!(replay_shape, DiscountReplayShape::CalibratedOnSourceGrid) {
        let replayed_on_source_grid = curve
            .knots()
            .iter()
            .map(|&time| (time, bumped.df(time)))
            .collect::<Vec<_>>();
        return curve.rebuild_with_knots(replayed_on_source_grid);
    }
    let unbumped = bump_discount_curve_with_config(
        &quotes,
        &params,
        &base_context,
        &BumpRequest::Parallel(0.0),
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

fn rate_quotes_from_legacy_discount_calibration(
    calibration: &DiscountCurveRateCalibration,
    curve_id: &CurveId,
) -> finstack_quant_core::Result<Vec<RateQuote>> {
    let index = IndexId::new(calibration.index_id.as_str());
    calibration
        .quotes
        .iter()
        .map(|quote| {
            let pillar = Pillar::Tenor(quote.tenor.parse()?);
            let id = QuoteId::new(format!("{curve_id}-{}", quote.tenor));
            Ok(match quote.quote_type {
                DiscountCurveRateQuoteType::Deposit => RateQuote::Deposit {
                    id,
                    index: index.clone(),
                    pillar,
                    rate: quote.rate,
                },
                DiscountCurveRateQuoteType::Swap => RateQuote::Swap {
                    id,
                    index: index.clone(),
                    pillar,
                    rate: quote.rate,
                    spread_decimal: None,
                },
            })
        })
        .collect()
}

fn rate_quotes_from_recipe(
    recipe: &RateCalibrationRecipe,
    curve_id: &CurveId,
) -> finstack_quant_core::Result<Vec<RateQuote>> {
    recipe
        .quotes
        .iter()
        .enumerate()
        .map(|(index, quote)| {
            let id = QuoteId::new(format!("{curve_id}-REPLAY-{index}"));
            Ok(match quote {
                RateCalibrationQuote::Deposit {
                    index_id,
                    pillar,
                    rate,
                } => RateQuote::Deposit {
                    id,
                    index: index_id.clone(),
                    pillar: pillar_from_recipe(pillar),
                    rate: *rate,
                },
                RateCalibrationQuote::Fra {
                    index_id,
                    start,
                    end,
                    rate,
                } => RateQuote::Fra {
                    id,
                    index: index_id.clone(),
                    start: pillar_from_recipe(start),
                    end: pillar_from_recipe(end),
                    rate: *rate,
                },
                RateCalibrationQuote::Futures {
                    contract,
                    expiry,
                    price,
                    convexity_adjustment,
                    vol_surface_id,
                } => RateQuote::Futures {
                    id,
                    contract: crate::market::conventions::ids::IrFutureContractId::new(
                        contract.as_str(),
                    ),
                    expiry: *expiry,
                    price: *price,
                    convexity_adjustment: *convexity_adjustment,
                    vol_surface_id: vol_surface_id.clone(),
                },
                RateCalibrationQuote::Swap {
                    index_id,
                    pillar,
                    rate,
                    spread_decimal,
                } => RateQuote::Swap {
                    id,
                    index: index_id.clone(),
                    pillar: pillar_from_recipe(pillar),
                    rate: *rate,
                    spread_decimal: *spread_decimal,
                },
            })
        })
        .collect()
}

fn pillar_from_recipe(pillar: &RateCalibrationPillar) -> Pillar {
    match pillar {
        RateCalibrationPillar::Tenor(tenor) => Pillar::Tenor(*tenor),
        RateCalibrationPillar::Date(date) => Pillar::Date(*date),
    }
}

fn rate_quote_level(quote: &RateQuote) -> f64 {
    match quote {
        RateQuote::Deposit { rate, .. }
        | RateQuote::Fra { rate, .. }
        | RateQuote::Swap { rate, .. } => *rate,
        RateQuote::Futures {
            price,
            convexity_adjustment,
            ..
        } => (100.0 - price) / 100.0 - convexity_adjustment.unwrap_or(0.0),
    }
}

fn discount_replay_conventions(
    curve: &DiscountCurve,
    recipe: Option<&RateCalibrationRecipe>,
) -> finstack_quant_core::Result<(
    CalibrationMethod,
    DayCount,
    Option<crate::instruments::rates::irs::FloatingLegCompounding>,
    Option<CurveId>,
)> {
    let Some(recipe) = recipe else {
        // Legacy metadata replayed with the historical quote-shock defaults.
        return Ok((CalibrationMethod::Bootstrap, curve.day_count(), None, None));
    };
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
        calibration_method_from_recipe(&recipe.method),
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
/// `calibration.discount_curve_id` (in its bumped form, when bumping both curves
/// together). The helper does not support [`ForwardCurveRateQuote::Basis`]
/// quotes; callers handling basis-tenor calibrations must rebuild the forward
/// curve explicitly.
///
/// Like [`bump_discount_curve_from_rate_calibration`], the recalibration is
/// applied as a delta overlay on the stored curve: the bumped and unbumped
/// global solves are both run and only their forward-rate difference is added
/// to the stored knots, so transcribed curves keep their base shape.
pub(crate) fn bump_forward_curve_from_rate_calibration(
    curve: &ForwardCurve,
    calibration: &ForwardCurveRateCalibration,
    context: &MarketContext,
    bump: &BumpRequest,
) -> finstack_quant_core::Result<ForwardCurve> {
    let recipe = curve.rate_calibration_recipe();
    let quotes = if let Some(recipe) = recipe.filter(|recipe| !recipe.quotes.is_empty()) {
        rate_quotes_from_recipe(recipe, curve.id())?
    } else {
        rate_quotes_from_legacy_forward_calibration(calibration, curve.id())?
    };

    let (method, curve_day_count, ois_compounding, discount_curve_id) =
        forward_replay_conventions(curve, calibration, recipe)?;
    let params = ForwardCurveParams {
        curve_id: curve.id().clone(),
        currency: recipe
            .and_then(|recipe| recipe.currency)
            .unwrap_or(calibration.currency),
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
        .rate_calibration_recipe_opt(curve.rate_calibration_recipe().cloned())
        .fx_policy_opt(curve.fx_policy().map(ToOwned::to_owned))
        .build()
}

fn rate_quotes_from_legacy_forward_calibration(
    calibration: &ForwardCurveRateCalibration,
    curve_id: &CurveId,
) -> finstack_quant_core::Result<Vec<RateQuote>> {
    let index = IndexId::new(calibration.index_id.as_str());
    calibration
        .quotes
        .iter()
        .enumerate()
        .map(|(position, quote)| {
            let id = QuoteId::new(format!("{curve_id}-{position}"));
            Ok(match quote {
                ForwardCurveRateQuote::Deposit { tenor, rate } => RateQuote::Deposit {
                    id,
                    index: index.clone(),
                    pillar: Pillar::Tenor(tenor.parse()?),
                    rate: *rate,
                },
                ForwardCurveRateQuote::Fra { start, end, rate } => RateQuote::Fra {
                    id,
                    index: index.clone(),
                    start: Pillar::Date(*start),
                    end: Pillar::Date(*end),
                    rate: *rate,
                },
                ForwardCurveRateQuote::Swap {
                    tenor,
                    rate,
                    spread_decimal,
                } => RateQuote::Swap {
                    id,
                    index: index.clone(),
                    pillar: Pillar::Tenor(tenor.parse()?),
                    rate: *rate,
                    spread_decimal: *spread_decimal,
                },
                ForwardCurveRateQuote::Basis { .. } => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "forward curve {curve_id} calibration uses basis quotes; \
                         exact replay requires a typed rate_calibration_recipe"
                    )));
                }
            })
        })
        .collect()
}

fn forward_replay_conventions(
    curve: &ForwardCurve,
    calibration: &ForwardCurveRateCalibration,
    recipe: Option<&RateCalibrationRecipe>,
) -> finstack_quant_core::Result<(
    CalibrationMethod,
    DayCount,
    Option<crate::instruments::rates::irs::FloatingLegCompounding>,
    CurveId,
)> {
    let Some(recipe) = recipe else {
        // Legacy metadata replayed with the historical forward-curve defaults.
        return Ok((
            CalibrationMethod::GlobalSolve {
                use_analytical_jacobian: false,
            },
            curve.day_count(),
            None,
            calibration.discount_curve_id.clone(),
        ));
    };
    let discount_curve_id = match &recipe.role {
        RateCalibrationCurveRole::Projection { discount_curve_id } => {
            if discount_curve_id != &calibration.discount_curve_id {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "forward curve {} recipe links discount curve {}, but quote metadata links {}",
                    curve.id(),
                    discount_curve_id,
                    calibration.discount_curve_id
                )));
            }
            discount_curve_id.clone()
        }
        RateCalibrationCurveRole::Discount { .. } => {
            return Err(finstack_quant_core::Error::Validation(format!(
                "forward curve {} carries a discount calibration recipe",
                curve.id()
            )));
        }
    };
    Ok((
        calibration_method_from_recipe(&recipe.method),
        recipe.curve_day_count,
        recipe
            .ois_compounding
            .as_ref()
            .map(ois_compounding_from_recipe),
        discount_curve_id,
    ))
}

fn calibration_method_from_recipe(method: &RateCalibrationMethod) -> CalibrationMethod {
    match method {
        RateCalibrationMethod::Bootstrap => CalibrationMethod::Bootstrap,
        RateCalibrationMethod::GlobalSolve {
            use_analytical_jacobian,
        } => CalibrationMethod::GlobalSolve {
            use_analytical_jacobian: *use_analytical_jacobian,
        },
    }
}

fn ois_compounding_from_recipe(
    compounding: &RateCalibrationOisCompounding,
) -> crate::instruments::rates::irs::FloatingLegCompounding {
    use crate::instruments::rates::irs::FloatingLegCompounding;
    match compounding {
        RateCalibrationOisCompounding::Simple => FloatingLegCompounding::Simple,
        RateCalibrationOisCompounding::CompoundedInArrears {
            lookback_days,
            observation_shift,
        } => FloatingLegCompounding::CompoundedInArrears {
            lookback_days: *lookback_days,
            observation_shift: *observation_shift,
        },
        RateCalibrationOisCompounding::CompoundedWithObservationShift { shift_days } => {
            FloatingLegCompounding::CompoundedWithObservationShift {
                shift_days: *shift_days,
            }
        }
        RateCalibrationOisCompounding::CompoundedWithRateCutoff { cutoff_days } => {
            FloatingLegCompounding::CompoundedWithRateCutoff {
                cutoff_days: *cutoff_days,
            }
        }
    }
}

/// Globally recalibrate a forward curve from (optionally bumped) rate quotes
/// using the stored curve's conventions.
fn rebootstrap_forward_curve(
    curve: &ForwardCurve,
    quotes: Vec<RateQuote>,
    params: &ForwardCurveParams,
    context: &MarketContext,
    bump: Option<&BumpRequest>,
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
    let (ctx, _report) =
        step_runtime::execute_params_and_apply(&step, &market_quotes, context, &cfg)?;
    Ok(ctx.get_forward(params.curve_id.as_str())?.as_ref().clone())
}

fn has_linked_single_curve_ois_recipes(
    discount: &DiscountCurve,
    forward: &ForwardCurve,
    discount_curve_id: &CurveId,
    forward_curve_id: &CurveId,
) -> finstack_quant_core::Result<bool> {
    let discount_recipe = discount.rate_calibration_recipe();
    let forward_recipe = forward.rate_calibration_recipe();
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
        .rate_calibration_recipe_opt(source.rate_calibration_recipe().cloned())
        .fx_policy_opt(source.fx_policy().map(ToOwned::to_owned))
        .build()
}

/// Re-bootstrap both a discount curve and its dependent forward curve from
/// stored rate-calibration metadata under a parallel quote shock.
///
/// Both curves must carry an exact typed recipe or a legacy rate-calibration
/// sidecar. Index fixings are seeded from recipe quote indices and curve IDs so
/// the calibration engine has the reference fixings it needs while replaying.
/// Legacy forward basis quotes still require a bespoke forward rebuild.
pub(crate) fn bump_market_via_rate_quote_shock(
    market: &MarketContext,
    discount_curve_id: &CurveId,
    forward_curve_id: &CurveId,
    bump_bp: f64,
) -> finstack_quant_core::Result<MarketContext> {
    let discount = market.get_discount(discount_curve_id.as_str())?;
    let forward = market.get_forward(forward_curve_id.as_str())?;
    let linked_single_curve = has_linked_single_curve_ois_recipes(
        discount.as_ref(),
        forward.as_ref(),
        discount_curve_id,
        forward_curve_id,
    )?;
    let discount_cal = discount
        .rate_calibration()
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| discount_calibration_from_recipe(discount.as_ref()))?;
    let forward_cal = forward
        .rate_calibration()
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| forward_calibration_from_recipe(forward.as_ref()))?;

    let fixing_sources = CalibrationFixingSources {
        discount: discount.as_ref(),
        discount_curve_id,
        discount_cal: &discount_cal,
        forward: forward.as_ref(),
        forward_curve_id,
        forward_cal: &forward_cal,
    };
    let seeded = seed_calibration_fixings(market, discount.base_date(), &fixing_sources)?;

    let bump = BumpRequest::Parallel(bump_bp);

    let bumped_discount = if linked_single_curve {
        bump_discount_curve_from_rate_calibration_with_projection(
            discount.as_ref(),
            &discount_cal,
            &seeded,
            &bump,
            Some(discount_curve_id.clone()),
            DiscountReplayShape::CalibratedOnSourceGrid,
        )?
    } else {
        bump_discount_curve_from_rate_calibration(discount.as_ref(), &discount_cal, &seeded, &bump)?
    };
    let seeded_with_discount = seeded.insert(bumped_discount);

    let bumped_forward = if linked_single_curve {
        let bumped_discount = seeded_with_discount.get_discount(discount_curve_id.as_str())?;
        rebuild_linked_ois_projection(forward.as_ref(), bumped_discount.as_ref())?
    } else {
        bump_forward_curve_from_rate_calibration(
            forward.as_ref(),
            &forward_cal,
            &seeded_with_discount,
            &bump,
        )?
    };
    Ok(seeded_with_discount.insert(bumped_forward))
}

/// Re-bootstrap a single OIS discount curve under a parallel market-quote shock.
///
/// This path is used when discounting and compounded-overnight projection are
/// two views of the same curve and no separate [`ForwardCurve`] is stored.
/// Pricing derives overnight forwards directly from discount-factor ratios.
pub(crate) fn bump_single_ois_market_via_rate_quote_shock(
    market: &MarketContext,
    curve_id: &CurveId,
    bump_bp: f64,
) -> finstack_quant_core::Result<MarketContext> {
    let discount = market.get_discount(curve_id.as_str())?;
    let discount_cal = discount
        .rate_calibration()
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| discount_calibration_from_recipe(discount.as_ref()))?;

    let mut seeded = market.clone();
    let mut seeded_indices = HashSet::new();
    if let Some(recipe) = discount.rate_calibration_recipe() {
        seeded = seed_recipe_fixings(seeded, recipe, discount.base_date(), &mut seeded_indices)?;
    }
    let first_rate = discount
        .rate_calibration_recipe()
        .and_then(|recipe| recipe.quotes.first())
        .map(rate_calibration_quote_level)
        .or_else(|| discount_cal.quotes.first().map(|quote| quote.rate));
    if let Some(rate) = first_rate {
        if discount.rate_calibration_recipe().is_none() {
            seeded = seeded.insert_series(fixing_seed(
                &discount_cal.index_id,
                discount.base_date(),
                rate,
            )?);
        }
        seeded = seeded.insert_series(fixing_seed(curve_id.as_str(), discount.base_date(), rate)?);
    }

    let bumped = bump_discount_curve_from_rate_calibration_with_projection(
        discount.as_ref(),
        &discount_cal,
        &seeded,
        &BumpRequest::Parallel(bump_bp),
        Some(curve_id.clone()),
        DiscountReplayShape::CalibratedOnSourceGrid,
    )?;
    Ok(seeded.insert(bumped))
}

/// Seed bootstrap-time fixings for both curve and index identifiers so the
/// calibration engine has the reference rates it needs when re-bootstrapping
/// after a quote shock. Uses the first quote of each calibration set as the
/// historical fixing — sufficient for risk re-bootstrapping where only the
/// shape of the curve matters, not the historical realized path.
struct CalibrationFixingSources<'a> {
    discount: &'a DiscountCurve,
    discount_curve_id: &'a CurveId,
    discount_cal: &'a DiscountCurveRateCalibration,
    forward: &'a ForwardCurve,
    forward_curve_id: &'a CurveId,
    forward_cal: &'a ForwardCurveRateCalibration,
}

fn seed_calibration_fixings(
    market: &MarketContext,
    base_date: Date,
    sources: &CalibrationFixingSources<'_>,
) -> finstack_quant_core::Result<MarketContext> {
    let mut seeded = market.clone();
    let mut seeded_indices = HashSet::new();
    if let Some(recipe) = sources.discount.rate_calibration_recipe() {
        seeded = seed_recipe_fixings(seeded, recipe, base_date, &mut seeded_indices)?;
    }
    let discount_rate = sources
        .discount
        .rate_calibration_recipe()
        .and_then(|recipe| recipe.quotes.first())
        .map(rate_calibration_quote_level)
        .or_else(|| sources.discount_cal.quotes.first().map(|quote| quote.rate));
    if let Some(rate) = discount_rate {
        if sources.discount.rate_calibration_recipe().is_none() {
            seeded = seeded.insert_series(fixing_seed(
                &sources.discount_cal.index_id,
                base_date,
                rate,
            )?);
        }
        seeded = seeded.insert_series(fixing_seed(
            sources.discount_curve_id.as_str(),
            base_date,
            rate,
        )?);
    }
    if let Some(recipe) = sources.forward.rate_calibration_recipe() {
        seeded = seed_recipe_fixings(seeded, recipe, base_date, &mut seeded_indices)?;
    }
    let forward_rate = sources
        .forward
        .rate_calibration_recipe()
        .and_then(|recipe| recipe.quotes.first())
        .map(rate_calibration_quote_level)
        .or_else(|| first_forward_calibration_rate(sources.forward_cal));
    if let Some(rate) = forward_rate {
        if sources.forward.rate_calibration_recipe().is_none() {
            seeded =
                seeded.insert_series(fixing_seed(&sources.forward_cal.index_id, base_date, rate)?);
        }
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
            | RateCalibrationQuote::Swap { index_id, .. } => Some(index_id),
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
        RateCalibrationQuote::Futures {
            price,
            convexity_adjustment,
            ..
        } => (100.0 - price) / 100.0 - convexity_adjustment.unwrap_or(0.0),
    }
}

fn discount_calibration_from_recipe(
    curve: &DiscountCurve,
) -> finstack_quant_core::Result<DiscountCurveRateCalibration> {
    let recipe = curve.rate_calibration_recipe().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "discount curve {} has no rate calibration metadata; cannot quote-shock DV01",
            curve.id()
        ))
    })?;
    if recipe.quotes.is_empty() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "discount curve {} recipe has no typed quotes and no legacy sidecar",
            curve.id()
        )));
    }
    let currency = recipe.currency.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "discount curve {} has a legacy recipe without currency",
            curve.id()
        ))
    })?;
    Ok(DiscountCurveRateCalibration {
        index_id: first_recipe_index_id(recipe)
            .unwrap_or_else(|| curve.id().as_str())
            .to_string(),
        currency,
        quotes: Vec::new(),
    })
}

fn forward_calibration_from_recipe(
    curve: &ForwardCurve,
) -> finstack_quant_core::Result<ForwardCurveRateCalibration> {
    let recipe = curve.rate_calibration_recipe().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "forward curve {} has no rate calibration metadata; cannot quote-shock DV01",
            curve.id()
        ))
    })?;
    if recipe.quotes.is_empty() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "forward curve {} recipe has no typed quotes and no legacy sidecar",
            curve.id()
        )));
    }
    let currency = recipe.currency.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "forward curve {} has a legacy recipe without currency",
            curve.id()
        ))
    })?;
    let discount_curve_id = match &recipe.role {
        RateCalibrationCurveRole::Projection { discount_curve_id } => discount_curve_id.clone(),
        RateCalibrationCurveRole::Discount { .. } => {
            return Err(finstack_quant_core::Error::Validation(format!(
                "forward curve {} carries a discount calibration recipe",
                curve.id()
            )));
        }
    };
    Ok(ForwardCurveRateCalibration {
        index_id: first_recipe_index_id(recipe)
            .unwrap_or_else(|| curve.id().as_str())
            .to_string(),
        currency,
        discount_curve_id,
        quotes: Vec::new(),
    })
}

fn first_recipe_index_id(recipe: &RateCalibrationRecipe) -> Option<&str> {
    recipe.quotes.iter().find_map(|quote| match quote {
        RateCalibrationQuote::Deposit { index_id, .. }
        | RateCalibrationQuote::Fra { index_id, .. }
        | RateCalibrationQuote::Swap { index_id, .. } => Some(index_id.as_str()),
        RateCalibrationQuote::Futures { .. } => None,
    })
}

fn first_forward_calibration_rate(calibration: &ForwardCurveRateCalibration) -> Option<f64> {
    calibration.quotes.first().map(|q| match q {
        ForwardCurveRateQuote::Deposit { rate, .. }
        | ForwardCurveRateQuote::Fra { rate, .. }
        | ForwardCurveRateQuote::Swap { rate, .. } => *rate,
        ForwardCurveRateQuote::Basis { spread_decimal, .. } => *spread_decimal,
    })
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

/// Apply a [`BumpRequest`] to a vector of [`RateQuote`]s.
///
/// Parallel bumps shift every quote; tenor bumps locate the closest quote to
/// each target year fraction and shift only that quote. Pure data transform —
/// no calibration engine involvement.
fn apply_bump_to_rate_quotes(
    quotes: Vec<RateQuote>,
    bump: &BumpRequest,
    as_of: Date,
) -> Vec<RateQuote> {
    match bump {
        BumpRequest::Parallel(bp) => quotes.into_iter().map(|q| q.bump_rate_bp(*bp)).collect(),
        BumpRequest::Tenors(targets) => {
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
    let dc = DayCount::Act365F; // Simple day count for proximity check
    quotes
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let a_date = resolve_maturity(a, as_of).unwrap_or(as_of);
            let b_date = resolve_maturity(b, as_of).unwrap_or(as_of);

            let a_yf = dc
                .year_fraction(as_of, a_date, DayCountContext::default())
                .unwrap_or(0.0);
            let b_yf = dc
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

/// Bump discount curve by synthesizing par instruments from the curve, shocking them, and re-calibrating.
///
/// Used when original quotes are unavailable. It implies par rates from
/// the current curve discount factors, applies shocks, and re-bootstraps.
///
/// # Arguments
/// * `currency` - Currency of the curve (required; DiscountCurve does not carry currency metadata).
pub fn bump_discount_curve_synthetic(
    curve: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    context: &MarketContext,
    bump: &BumpRequest,
    as_of: Date,
    currency: Currency,
) -> finstack_quant_core::Result<DiscountCurve> {
    let curve_id = curve.id();
    let base_date = as_of;
    let knots = curve.knots();

    // Choose synthetic indices. Deposits use a short-dated money-market index,
    // while swaps must use the corresponding OIS index conventions.
    let deposit_index_id = match currency {
        // Align with `rate_index_conventions.json` (there is no `EUR-ESTR-1M` alias today).
        Currency::EUR => "EUR-ESTR-OIS",
        Currency::GBP => "GBP-SONIA-1M",
        Currency::JPY => "JPY-TONA-1M",
        _ => "USD-SOFR-1M",
    };

    // Synthesize quotes for each knot (excluding t≈0) and re-calibrate.
    // Use deposit-style quotes for all maturities here. The synthetic bump path
    // is a deterministic approximation used when original quotes are unavailable,
    // and staying in deposit space avoids OIS swap seasoning/fixings requirements
    // during scenario shock application.

    let mut quotes = Vec::new();
    let dc = DayCount::Act365F;
    let dc_ctx = DayCountContext::default();

    for &t in knots {
        if t <= 0.0001 {
            continue;
        }

        let df = curve.df(t);
        let maturity_days = (t * 365.25).round() as i64;
        let maturity = base_date + time::Duration::days(maturity_days);

        let yf = dc.year_fraction(base_date, maturity, dc_ctx).unwrap_or(t);

        let rate = if yf > 1e-4 {
            (1.0 / df - 1.0) / yf
        } else {
            0.0
        };
        quotes.push(RateQuote::Deposit {
            id: QuoteId::new(format!("SYNTH-DEP-{}", t)),
            index: finstack_quant_core::types::IndexId::new(deposit_index_id),
            pillar: Pillar::Date(maturity),
            rate,
        });
    }

    let params = DiscountCurveParams {
        curve_id: curve_id.clone(),
        currency,
        base_date,
        method: CalibrationMethod::Bootstrap,
        interpolation: Default::default(),
        extrapolation: ExtrapolationPolicy::FlatForward,
        pricing_discount_id: None,
        pricing_forward_id: None,
        conventions: RatesStepConventions {
            ois_compounding: None,
            curve_day_count: Some(DayCount::Act365F),
        },
    };

    bump_discount_curve(&quotes, &params, context, bump)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::market::conventions::ids::IrFutureContractId;
    use finstack_quant_core::math::interp::InterpStyle;

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
        let discount_calibration = DiscountCurveRateCalibration {
            index_id: "USD-SOFR-OIS".to_string(),
            currency: Currency::USD,
            quotes: Vec::new(),
        };
        let discount_recipe = RateCalibrationRecipe {
            currency: Some(Currency::USD),
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
            .rate_calibration(discount_calibration)
            .rate_calibration_recipe(discount_recipe)
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

        let forward_calibration = ForwardCurveRateCalibration {
            index_id: "USD-SOFR-OIS".to_string(),
            currency: Currency::USD,
            discount_curve_id: forward_discount_curve_id.clone(),
            quotes: Vec::new(),
        };
        let forward_recipe = RateCalibrationRecipe {
            currency: Some(Currency::USD),
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
            .rate_calibration(forward_calibration)
            .rate_calibration_recipe(forward_recipe)
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

        let shocked =
            bump_market_via_rate_quote_shock(&market, &discount_curve_id, &forward_curve_id, 5.0)
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
            shocked_forward.rate_calibration_recipe(),
            source_forward.rate_calibration_recipe()
        );
        let shocked_calibration = shocked_forward
            .rate_calibration()
            .expect("projection calibration provenance");
        let source_calibration = source_forward
            .rate_calibration()
            .expect("source projection calibration provenance");
        assert_eq!(shocked_calibration.index_id, source_calibration.index_id);
        assert_eq!(shocked_calibration.currency, source_calibration.currency);
        assert_eq!(
            shocked_calibration.discount_curve_id,
            source_calibration.discount_curve_id
        );
        assert_eq!(
            shocked_calibration.quotes.len(),
            source_calibration.quotes.len()
        );
    }

    #[test]
    fn linked_ois_projection_uses_forward_grid_dates_across_day_counts() {
        let (market, discount_curve_id, forward_curve_id, projection_grid) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));

        let shocked =
            bump_market_via_rate_quote_shock(&market, &discount_curve_id, &forward_curve_id, 5.0)
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

        let shocked =
            bump_market_via_rate_quote_shock(&market, &discount_curve_id, &forward_curve_id, 5.0)
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
    fn malformed_linked_single_curve_ois_recipe_fails_explicitly() {
        let (market, discount_curve_id, forward_curve_id, _) =
            linked_single_curve_ois_market(CurveId::new("USD-OTHER"));

        let error =
            bump_market_via_rate_quote_shock(&market, &discount_curve_id, &forward_curve_id, 1.0)
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
            .rate_calibration_recipe_opt(None)
            .build()
            .expect("discount representation without recipe");
        let market = market.insert(discount_without_recipe);

        let error =
            bump_market_via_rate_quote_shock(&market, &discount_curve_id, &forward_curve_id, 1.0)
                .expect_err("one-sided forward OIS link must fail");

        assert!(
            error.to_string().contains("linked single-curve OIS recipe"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn linkage_validation_precedes_missing_discount_sidecar_reconstruction() {
        let (market, discount_curve_id, forward_curve_id, _) =
            linked_single_curve_ois_market(CurveId::new("USD-OIS"));
        let discount = market
            .get_discount(discount_curve_id.as_str())
            .expect("discount representation");
        let discount_without_metadata = discount
            .to_builder_with_id(discount_curve_id.clone())
            .rate_calibration_opt(None)
            .rate_calibration_recipe_opt(None)
            .build()
            .expect("discount representation without calibration metadata");
        let market = market.insert(discount_without_metadata);

        let error =
            bump_market_via_rate_quote_shock(&market, &discount_curve_id, &forward_curve_id, 1.0)
                .expect_err("linkage validation must run before sidecar reconstruction");
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
            .rate_calibration_recipe()
            .expect("discount recipe")
            .clone();
        mismatched_recipe.role = RateCalibrationCurveRole::Discount {
            projection_curve_id: CurveId::new("USD-OTHER-PROJECTION"),
        };
        let mismatched_discount = discount
            .to_builder_with_id(discount_curve_id.clone())
            .rate_calibration_recipe(mismatched_recipe)
            .build()
            .expect("mismatched discount representation");
        let market = market.insert(mismatched_discount);

        let error =
            bump_market_via_rate_quote_shock(&market, &discount_curve_id, &forward_curve_id, 1.0)
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
            .rate_calibration_recipe_opt(None)
            .build()
            .expect("projection representation without recipe");
        let market = market.insert(forward_without_recipe);

        let error =
            bump_market_via_rate_quote_shock(&market, &discount_curve_id, &forward_curve_id, 1.0)
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
            .rate_calibration_recipe()
            .expect("projection recipe")
            .clone();
        partial_recipe.ois_compounding = None;
        let partial_forward = forward
            .to_builder_with_id(forward_curve_id.clone())
            .rate_calibration_recipe(partial_recipe)
            .build()
            .expect("projection representation with partial OIS metadata");
        let market = market.insert(partial_forward);

        let error =
            bump_market_via_rate_quote_shock(&market, &discount_curve_id, &forward_curve_id, 1.0)
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
            .rate_calibration_recipe()
            .expect("discount recipe")
            .clone();
        discount_recipe.role = RateCalibrationCurveRole::Discount {
            projection_curve_id: discount_curve_id.clone(),
        };
        let discount = discount
            .to_builder_with_id(discount_curve_id.clone())
            .rate_calibration_recipe(discount_recipe)
            .build()
            .expect("self-projected discount representation");

        let forward = market
            .get_forward(forward_curve_id.as_str())
            .expect("projection representation");
        let mut forward_recipe = forward
            .rate_calibration_recipe()
            .expect("projection recipe")
            .clone();
        forward_recipe.ois_compounding = None;
        let forward = forward
            .to_builder_with_id(forward_curve_id.clone())
            .rate_calibration_recipe(forward_recipe)
            .build()
            .expect("term-index projection representation");
        let market = market.insert(discount).insert(forward);

        let shocked =
            bump_market_via_rate_quote_shock(&market, &discount_curve_id, &forward_curve_id, 1.0)
                .expect("term-index recipes must route independently");
        let shocked_forward = shocked
            .get_forward(forward_curve_id.as_str())
            .expect("independently replayed term-index projection");
        assert_eq!(
            shocked_forward
                .rate_calibration_recipe()
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
                convexity_adjustment: Some(0.0),
                vol_surface_id: None,
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

        let bumped = apply_bump_to_rate_quotes(quotes, &BumpRequest::Parallel(1.0), as_of);

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
        let calibration = ForwardCurveRateCalibration {
            index_id: "USD-SOFR-3M".to_string(),
            currency: Currency::USD,
            discount_curve_id: CurveId::new("USD-OIS"),
            quotes: vec![
                ForwardCurveRateQuote::Deposit {
                    tenor: "3M".to_string(),
                    rate: 0.0400,
                },
                ForwardCurveRateQuote::Deposit {
                    tenor: "6M".to_string(),
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
            &BumpRequest::Parallel(1.0),
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
            &BumpRequest::Parallel(0.0),
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
        assert_eq!(shocked_calibration.index_id, calibration.index_id);
        assert_eq!(shocked_calibration.currency, calibration.currency);
        assert_eq!(
            shocked_calibration.discount_curve_id,
            calibration.discount_curve_id
        );
        assert_eq!(shocked_calibration.quotes.len(), calibration.quotes.len());
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
                    crate::instruments::rates::irs::FloatingLegCompounding::CompoundedWithRateCutoff {
                        cutoff_days: 1,
                    },
                ),
            },
        };
        let context = MarketContext::new().insert_series(
            fixing_seed(index.as_str(), base_date, 0.0430).expect("SOFR fixing seed"),
        );
        let source = bump_discount_curve(&quotes, &params, &context, &BumpRequest::Parallel(0.0))
            .expect("source SOFR calibration");
        let calibration = source
            .rate_calibration()
            .cloned()
            .expect("calibrated curve recipe metadata");
        let recipe = source
            .rate_calibration_recipe()
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
            &BumpRequest::Parallel(0.0),
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
                &BumpRequest::Parallel(bump_bp),
            )
            .expect("stored-recipe quote shock");
            let direct =
                bump_discount_curve(&quotes, &params, &context, &BumpRequest::Parallel(bump_bp))
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
    fn discount_quote_overlay_preserves_source_validation_policy() {
        let base_date =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid date");
        let calibration = DiscountCurveRateCalibration {
            index_id: "USD-SOFR-OIS".to_string(),
            currency: Currency::USD,
            quotes: vec![
                finstack_quant_core::market_data::term_structures::DiscountCurveRateQuote {
                    quote_type: DiscountCurveRateQuoteType::Deposit,
                    tenor: "1Y".to_string(),
                    rate: -0.01,
                },
                finstack_quant_core::market_data::term_structures::DiscountCurveRateQuote {
                    quote_type: DiscountCurveRateQuoteType::Deposit,
                    tenor: "2Y".to_string(),
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
            &BumpRequest::Parallel(0.0),
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
            currency: Some(Currency::USD),
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
                    vol_surface_id: Some(CurveId::new("USD-SR3-VOL")),
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
                convexity_adjustment: Some(value),
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
            currency: Some(Currency::USD),
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
