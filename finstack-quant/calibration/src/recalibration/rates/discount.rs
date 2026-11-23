//! Discount-curve quote-space replay and recalibration cache keys.

use super::super::cache::KeyedOnceCache;
use super::forward_ois::{
    apply_bump_to_rate_quotes, ensure_recipe_has_quotes, required_discount_rate_calibration,
};
use crate::CalibrationConfig;
use crate::api::schema::{DiscountCurveParams, StepParams};
use crate::quotes::MarketQuote;
use crate::quotes::RateQuote;
use crate::targets::discount::DiscountCurveTarget;
use crate::targets::rate_recipe::{original_execution, rate_quotes_from_recipe};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, RateCalibrationRecipe, RateQuoteBinding,
};
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::recalibration::QuoteBump;
#[cfg(test)]
use std::cell::Cell;
use std::sync::Arc;

#[cfg(test)]
std::thread_local! {
    pub(super) static DISCOUNT_CALIBRATION_RUNS: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum RateMarketRecalibrationKind {
    DiscountAndForward {
        discount_curve_id: String,
        forward_curve_id: String,
    },
    SingleOis {
        curve_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct RateMarketRecalibrationKey {
    pub(super) source_market_hash: String,
    pub(super) kind: RateMarketRecalibrationKind,
    pub(super) bump: RateBumpKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct DiscountRateRecalibrationKey {
    pub(super) source_market_hash: String,
    pub(super) source_curve_hash: String,
    pub(super) recipe_hash: String,
    pub(super) bump: RateBumpKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum RateBumpKey {
    Parallel(u64),
    Tenors(Vec<(u64, u64)>),
    Exact {
        curve_id: CurveId,
        quote_id: String,
        pillar_date: Date,
        bp: u64,
    },
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
            QuoteBump::RateQuoteBp {
                curve_id,
                quote_id,
                pillar_date,
                bp,
            } => Self::Exact {
                curve_id: curve_id.clone(),
                quote_id: quote_id.clone(),
                pillar_date: *pillar_date,
                bp: bp.to_bits(),
            },
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
    pub(super) market: KeyedOnceCache<RateMarketRecalibrationKey, MarketContext>,
    pub(super) discount: KeyedOnceCache<DiscountRateRecalibrationKey, DiscountCurve>,
}

pub(super) fn solve_discount_curve(
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

    super::super::ensure_replay_fit_accepted(params.curve_id.as_str(), config, &report)?;
    Ok(curve)
}

/// An exact resolved market quote in a recorded calibration recipe.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CalibrationQuoteCoordinate {
    /// Identifier of the authored market quote, without tenor normalization.
    pub quote_id: String,
    /// Contractual pillar of the original execution, after holiday adjustments.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub pillar_date: Date,
}

pub(super) enum RateQuoteShock<'a> {
    Bucket(&'a QuoteBump),
    Exact {
        target: &'a CalibrationQuoteCoordinate,
        bp: f64,
    },
}

impl RateQuoteShock<'_> {
    pub(super) fn apply(
        self,
        curve_id: &CurveId,
        quotes: Vec<RateQuote>,
        bindings: &[RateQuoteBinding],
    ) -> finstack_quant_core::Result<Vec<RateQuote>> {
        match self {
            Self::Bucket(QuoteBump::RateQuoteBp {
                curve_id: target,
                quote_id,
                pillar_date,
                bp,
            }) => {
                if target == curve_id {
                    Self::apply_exact(quotes, bindings, quote_id, *pillar_date, *bp)
                } else {
                    Ok(quotes)
                }
            }
            Self::Bucket(bump) => apply_bump_to_rate_quotes(quotes, bump, bindings),
            Self::Exact { target, bp } => {
                Self::apply_exact(quotes, bindings, &target.quote_id, target.pillar_date, bp)
            }
        }
    }

    pub(super) fn apply_exact(
        quotes: Vec<RateQuote>,
        bindings: &[RateQuoteBinding],
        quote_id: &str,
        pillar_date: Date,
        bp: f64,
    ) -> finstack_quant_core::Result<Vec<RateQuote>> {
        let invalid = |detail: &str| {
            finstack_quant_core::Error::Validation(format!(
                "Exact rate quote '{}' at {}: {detail}",
                quote_id, pillar_date
            ))
        };
        if quote_id.trim().is_empty() || !bp.is_finite() {
            return Err(invalid(
                "requires a nonempty quote ID and a finite basis-point shock",
            ));
        }
        let mut positions = bindings
            .iter()
            .enumerate()
            .filter(|(_, binding)| binding.quote_id == quote_id);
        let (index, binding) = positions
            .next()
            .ok_or_else(|| invalid("missing executed binding"))?;
        if positions.next().is_some() || binding.pillar_date != pillar_date {
            return Err(invalid("ambiguous quote ID or a different resolved pillar"));
        }
        let mut quotes = quotes;
        let quote = quotes
            .get_mut(index)
            .filter(|quote| quote.id().as_str() == quote_id)
            .ok_or_else(|| invalid("quote and executed binding orders differ"))?;
        *quote = quote.bump_rate_bp(bp);
        quote.validate()?;
        Ok(quotes)
    }
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
    if let QuoteBump::RateQuoteBp { curve_id, .. } = bump
        && curve_id != curve.id()
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "rate quote bump targets {curve_id}, but this replay owns {}",
            curve.id()
        )));
    }
    bump.validate()?;
    bump_discount_curve_from_rate_calibration_with_projection(
        curve,
        calibration,
        context,
        RateQuoteShock::Bucket(bump),
        None,
    )
}

/// Reprice one exact rate quote of a calibrated discount curve, holding the supplied dependencies fixed.
///
/// The unshocked calibration must reproduce the source curve and quote bindings.
/// The selected quote ID and contractual pillar must uniquely match the original
/// execution. A quote's native basis-point map handles futures price direction.
///
/// # Arguments
/// * `curve` - Original discount curve with a qualified and accepted rate calibration recipe.
/// * `context` - Immutable snapshot supplying the exact held discount, projection and fixing dependencies.
/// * `target` - Stable quote ID and resolved maturity from the curve's original execution.
/// * `bp` - Finite additive shock in basis points of the selected quoted rate.
///
/// # Errors
/// Returns an error if the source curve cannot replay, a dependency is absent,
/// the binding is missing or ambiguous, or the shocked solve fails.
pub fn bump_discount_rate_quote(
    curve: &DiscountCurve,
    context: &MarketContext,
    target: &CalibrationQuoteCoordinate,
    bp: f64,
) -> finstack_quant_core::Result<DiscountCurve> {
    let calibration = required_discount_rate_calibration(curve)?;
    bump_discount_curve_from_rate_calibration_with_projection(
        curve,
        calibration,
        context,
        RateQuoteShock::Exact { target, bp },
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

pub(super) fn bump_discount_curve_from_rate_calibration_with_projection(
    curve: &DiscountCurve,
    calibration: &RateCalibrationRecipe,
    context: &MarketContext,
    bump: RateQuoteShock<'_>,
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
    let quotes = bump.apply(
        curve.id(),
        quotes,
        &original_execution(calibration)?.bindings,
    )?;
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

pub(super) fn verify_replayed_bindings(
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

pub(super) fn replay_config(
    recipe: &RateCalibrationRecipe,
) -> finstack_quant_core::Result<CalibrationConfig> {
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

pub(super) fn verify_source_factor(
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

pub(super) fn replay_step(
    calibration: &RateCalibrationRecipe,
) -> finstack_quant_core::Result<StepParams> {
    serde_json::from_value(original_execution(calibration)?.step_params.clone()).map_err(|error| {
        finstack_quant_core::Error::Validation(format!("invalid recorded rate evaluator: {error}"))
    })
}
