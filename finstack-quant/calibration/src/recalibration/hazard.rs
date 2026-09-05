//! Shared hazard curve bumping logic.

use super::cache::KeyedOnceCache;
use super::ensure_replay_fit_accepted;
use crate::api::schema::{HazardCurveParams, StepParams};
use crate::quotes::cds::CdsQuote;
use crate::quotes::ids::Pillar;
use crate::quotes::market_quote::MarketQuote;
use crate::step_runtime;
use crate::CalibrationConfig;
use finstack_quant_core::dates::DayCountContext;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    HazardCalibrationInput, HazardCalibrationRecipe, HazardCurve,
};
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CdsValuationConvention;
use finstack_quant_valuations::market::conventions::ids::CdsDocClause;
use finstack_quant_valuations::recalibration::{DealCdsQuoteOverride, QuoteBump};
#[cfg(test)]
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Copy)]
struct HazardParRecalibration<'a> {
    hazard: &'a HazardCurve,
    context: &'a MarketContext,
    identity_context: Option<&'a MarketContext>,
    discount_id: &'a CurveId,
    recovery_rate: f64,
    doc_clause: Option<CdsDocClause>,
    cds_valuation_convention: Option<CdsValuationConvention>,
    spread_bump: Option<&'a QuoteBump>,
    exact_spread_bump: Option<(usize, f64)>,
    replay_spread_risk_center: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum HazardBumpKey {
    None,
    Parallel(u64),
    Tenors(Vec<(u64, u64)>),
    ExactQuote { index: usize, bump_bp: u64 },
}

impl From<Option<&QuoteBump>> for HazardBumpKey {
    fn from(bump: Option<&QuoteBump>) -> Self {
        match bump {
            None => Self::None,
            Some(QuoteBump::ParallelBp(bp)) => Self::Parallel(bp.to_bits()),
            Some(QuoteBump::TenorsBp(tenors)) => Self::Tenors(
                tenors
                    .iter()
                    .map(|(tenor, bp)| (tenor.to_bits(), bp.to_bits()))
                    .collect(),
            ),
        }
    }
}

fn hazard_bump_key(request: &HazardParRecalibration<'_>) -> HazardBumpKey {
    request
        .exact_spread_bump
        .map(|(index, bump_bp)| HazardBumpKey::ExactQuote {
            index,
            bump_bp: bump_bp.to_bits(),
        })
        .unwrap_or_else(|| request.spread_bump.into())
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct HazardRecalibrationKey {
    hazard_id: String,
    discount_id: String,
    discount_fingerprint: u64,
    identity_discount_fingerprint: Option<u64>,
    recovery_rate: u64,
    doc_clause: Option<CdsDocClause>,
    cds_valuation_convention: Option<CdsValuationConvention>,
    bump: HazardBumpKey,
    /// Fingerprint of the source curve's par spreads and hazard knots.
    source_fingerprint: u64,
}

fn hazard_source_fingerprint(hazard: &HazardCurve) -> u64 {
    let mut hash = hazard.recovery_rate().to_bits();
    for (tenor, value) in hazard.par_spread_points() {
        hash = hash
            .wrapping_mul(0x0000_013B)
            .wrapping_add(tenor.to_bits())
            .wrapping_mul(0x0000_013B)
            .wrapping_add(value.to_bits());
    }
    hash = hash.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for (tenor, value) in hazard.knot_points() {
        hash = hash
            .wrapping_mul(0x0000_013B)
            .wrapping_add(tenor.to_bits())
            .wrapping_mul(0x0000_013B)
            .wrapping_add(value.to_bits());
    }
    hash = stable_hash_bytes(hash, b"hazard-calibration-recipe");
    match hazard.hazard_calibration() {
        Some(recipe) => {
            if let Ok(value) = serde_json::to_value(recipe) {
                hash = stable_hash_json(hash, &value);
            }
        }
        None => {
            hash = stable_hash_bytes(hash, b"none");
        }
    }
    hash
}

fn stable_hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

fn stable_hash_json(mut hash: u64, value: &serde_json::Value) -> u64 {
    match value {
        serde_json::Value::Null => stable_hash_bytes(hash, b"null"),
        serde_json::Value::Bool(value) => {
            stable_hash_bytes(hash, if *value { b"true" } else { b"false" })
        }
        serde_json::Value::Number(value) => stable_hash_bytes(hash, value.to_string().as_bytes()),
        serde_json::Value::String(value) => stable_hash_bytes(hash, value.as_bytes()),
        serde_json::Value::Array(values) => {
            hash = stable_hash_bytes(hash, b"[");
            for value in values {
                hash = stable_hash_json(hash, value);
                hash = stable_hash_bytes(hash, b",");
            }
            stable_hash_bytes(hash, b"]")
        }
        serde_json::Value::Object(values) => {
            hash = stable_hash_bytes(hash, b"{");
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort_unstable();
            for key in keys {
                hash = stable_hash_bytes(hash, key.as_bytes());
                hash = stable_hash_json(hash, &values[key]);
                hash = stable_hash_bytes(hash, b",");
            }
            stable_hash_bytes(hash, b"}")
        }
    }
}

/// Batch-local cache for hazard-curve recalibrations used by spread risk and
/// scenario ParCDS delivery.
///
/// A cache instance may be shared across independent applies that start from
/// the same unstressed market snapshot. Each key uses its own mutex so
/// concurrent callers requesting the same curve bump share the in-flight
/// calibration while different bumps can proceed in parallel. Failed
/// calibrations are not cached, preserving the original error.
///
/// Keys include a fingerprint of the source curve's par spreads and hazard
/// knots, so a second bump of an already-recalibrated curve does not reuse
/// the first result.
#[derive(Default)]
pub(crate) struct HazardRecalibrationCache {
    entries: KeyedOnceCache<HazardRecalibrationKey, HazardCurve>,
}

impl std::fmt::Debug for HazardRecalibrationCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HazardRecalibrationCache")
            .field("entries", &self.entries.len())
            .finish()
    }
}

fn discount_fingerprint(context: &MarketContext, id: &CurveId) -> finstack_quant_core::Result<u64> {
    let curve = context.get_discount(id)?;
    let value = serde_json::to_value(curve.as_ref()).map_err(|error| {
        finstack_quant_core::Error::Validation(format!(
            "cannot fingerprint discount curve '{id}': {error}"
        ))
    })?;
    Ok(stable_hash_json(0, &value))
}

/// Recalibrate `request`, memoising through `cache` when one is supplied.
fn recalibrate_from_par_spreads_cached(
    cache: Option<&HazardRecalibrationCache>,
    request: HazardParRecalibration<'_>,
) -> finstack_quant_core::Result<Arc<HazardCurve>> {
    let key = HazardRecalibrationKey {
        hazard_id: request.hazard.id().to_string(),
        discount_id: request.discount_id.to_string(),
        discount_fingerprint: discount_fingerprint(request.context, request.discount_id)?,
        identity_discount_fingerprint: request
            .identity_context
            .map(|context| discount_fingerprint(context, request.discount_id))
            .transpose()?,
        recovery_rate: request.recovery_rate.to_bits(),
        doc_clause: request.doc_clause,
        cds_valuation_convention: request.cds_valuation_convention,
        bump: hazard_bump_key(&request),
        source_fingerprint: hazard_source_fingerprint(request.hazard),
    };
    KeyedOnceCache::get_or_compute(cache.map(|c| &c.entries), key, || {
        recalibrate_from_par_spreads(request)
    })
}

fn require_discount_id(discount_id: Option<&CurveId>) -> finstack_quant_core::Result<&CurveId> {
    discount_id.ok_or_else(|| {
        finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
            id: "discount curve for hazard recalibration".to_string(),
        })
    })
}

fn recipe_inputs(
    hazard: &HazardCurve,
) -> finstack_quant_core::Result<(
    HazardCurveParams,
    Vec<ReplayQuote>,
    Vec<ReplayQuote>,
    CalibrationConfig,
)> {
    let recipe = hazard.hazard_calibration().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "hazard curve '{}' has no lossless calibration recipe; quote-space spread risk is unavailable",
            hazard.id()
        ))
    })?;
    let params = serde_json::from_value(recipe.hazard_params.clone()).map_err(|error| {
        finstack_quant_core::Error::Validation(format!(
            "hazard curve '{}' contains invalid replay parameters: {error}",
            hazard.id()
        ))
    })?;
    recipe.validate()?;
    crate::targets::hazard::validate_hazard_recipe_bindings(&params, recipe)?;
    let calibration_inputs =
        decode_recipe_inputs(&recipe.calibration_inputs, hazard, "calibration")?;
    let spread_risk_inputs =
        decode_recipe_inputs(&recipe.spread_risk_inputs, hazard, "spread-risk")?;
    let config = serde_json::from_value(recipe.calibration_config.clone()).map_err(|error| {
        finstack_quant_core::Error::Validation(format!(
            "hazard curve '{}' contains an invalid replay policy: {error}",
            hazard.id()
        ))
    })?;
    Ok((params, calibration_inputs, spread_risk_inputs, config))
}

#[derive(Clone)]
struct ReplayQuote {
    quote: CdsQuote,
    pillar_date: finstack_quant_core::dates::Date,
    pillar_time: f64,
}

/// Exact spread-risk quote binding used by bucketed CS01.
#[derive(Clone, Debug)]
pub(crate) struct HazardSpreadRiskBucket {
    pub(super) index: usize,
    pub(super) quote_id: String,
    pub(super) pillar_date: finstack_quant_core::dates::Date,
    pub(super) pillar_time: f64,
}

pub(crate) fn hazard_with_deal_quote_override(
    hazard: &HazardCurve,
    deal_override: DealCdsQuoteOverride,
) -> finstack_quant_core::Result<HazardCurve> {
    if !deal_override.spread_bp.is_finite() {
        return Err(finstack_quant_core::Error::Validation(
            "deal CDS quote override must be finite".to_string(),
        ));
    }
    let source_recipe =
        hazard
            .hazard_calibration()
            .ok_or_else(|| finstack_quant_core::Error::Calibration {
                message: format!(
                    "CDS quote override for '{}' requires a hazard replay recipe",
                    hazard.id()
                ),
                category: "cs01_rebootstrap".to_string(),
            })?;
    let mut risk_inputs = source_recipe.spread_risk_inputs.clone();
    let template_input =
        risk_inputs
            .first()
            .ok_or_else(|| finstack_quant_core::Error::Calibration {
                message: format!(
                    "CDS quote override for '{}' requires at least one spread-risk replay input",
                    hazard.id()
                ),
                category: "cs01_rebootstrap".to_string(),
            })?;
    let template_quote: CdsQuote =
        serde_json::from_value(template_input.quote.clone()).map_err(|error| {
            finstack_quant_core::Error::Validation(format!(
                "CDS quote override for '{}' found an invalid spread-risk quote: {error}",
                hazard.id()
            ))
        })?;
    let deal_quote = match template_quote {
        CdsQuote::CdsParSpread {
            id,
            entity,
            convention,
            recovery_rate,
            ..
        } => CdsQuote::CdsParSpread {
            id,
            entity,
            convention,
            pillar: Pillar::Date(deal_override.contract_end),
            spread_bp: deal_override.spread_bp,
            recovery_rate,
        },
        CdsQuote::CdsUpfront { .. } => {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "CDS quote override for '{}' found an upfront quote in par-spread risk inputs",
                    hazard.id()
                ),
                category: "cs01_rebootstrap".to_string(),
            });
        }
    };
    let build_ctx = crate::build::BuildCtx::new(
        hazard.base_date(),
        1.0,
        finstack_quant_core::HashMap::default(),
    );
    let contractual_pillar =
        crate::build::cds::resolve_cds_quote_dates(&deal_quote, &build_ctx)?.maturity;
    let contractual_time = hazard.day_count().year_fraction(
        hazard.base_date(),
        contractual_pillar,
        DayCountContext::default(),
    )?;
    let target_index = risk_inputs
        .iter()
        .enumerate()
        .find(|(_, input)| {
            let scale = input
                .pillar_time
                .abs()
                .max(contractual_time.abs())
                .max(1.0);
            input.pillar_date == contractual_pillar
                && (input.pillar_time - contractual_time).abs() <= 1e-12 * scale
        })
        .map(|(index, _)| index)
        .ok_or_else(|| finstack_quant_core::Error::Calibration {
            message: format!(
                "CDS quote override for '{}' has no exact spread-risk replay pillar for contractual date {} (time {})",
                hazard.id(), contractual_pillar, contractual_time
            ),
            category: "cs01_rebootstrap".to_string(),
        })?;
    let source_quote: CdsQuote = serde_json::from_value(risk_inputs[target_index].quote.clone())
        .map_err(|error| {
            finstack_quant_core::Error::Validation(format!(
                "CDS quote override for '{}' found an invalid spread-risk quote: {error}",
                hazard.id()
            ))
        })?;
    let overridden_quote = match source_quote {
        CdsQuote::CdsParSpread {
            id,
            entity,
            convention,
            pillar,
            recovery_rate,
            ..
        } => CdsQuote::CdsParSpread {
            id,
            entity,
            convention,
            pillar,
            spread_bp: deal_override.spread_bp,
            recovery_rate,
        },
        CdsQuote::CdsUpfront { .. } => {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "CDS quote override for '{}' found an upfront quote in par-spread risk inputs",
                    hazard.id()
                ),
                category: "cs01_rebootstrap".to_string(),
            });
        }
    };
    risk_inputs[target_index].quote = serde_json::to_value(overridden_quote).map_err(|error| {
        finstack_quant_core::Error::Validation(format!(
            "failed to persist CDS quote override for '{}': {error}",
            hazard.id()
        ))
    })?;
    let derived_recipe = HazardCalibrationRecipe::new(
        source_recipe.hazard_params.clone(),
        source_recipe.calibration_inputs.clone(),
        risk_inputs,
        source_recipe.calibration_config.clone(),
    )?;
    hazard
        .to_builder_with_id(hazard.id().clone())
        .hazard_calibration(derived_recipe)
        .build()
}

fn decode_recipe_inputs(
    inputs: &[HazardCalibrationInput],
    hazard: &HazardCurve,
    input_kind: &str,
) -> finstack_quant_core::Result<Vec<ReplayQuote>> {
    inputs
        .iter()
        .map(|input| {
            let quote: CdsQuote = serde_json::from_value(input.quote.clone()).map_err(|error| {
                finstack_quant_core::Error::Validation(format!(
                    "hazard curve '{}' contains an invalid {input_kind} replay quote: {error}",
                    hazard.id(),
                ))
            })?;
            Ok(ReplayQuote {
                quote,
                pillar_date: input.pillar_date,
                pillar_time: input.pillar_time,
            })
        })
        .collect()
}

/// Return the exact, deterministically ordered spread-risk replay bindings.
pub(crate) fn hazard_spread_risk_buckets(
    hazard: &HazardCurve,
) -> finstack_quant_core::Result<Vec<HazardSpreadRiskBucket>> {
    let (_, _, spread_risk_inputs, _) = recipe_inputs(hazard)?;
    Ok(spread_risk_inputs
        .into_iter()
        .enumerate()
        .map(|(index, input)| HazardSpreadRiskBucket {
            index,
            quote_id: input.quote.id().as_str().to_string(),
            pillar_date: input.pillar_date,
            pillar_time: input.pillar_time,
        })
        .collect())
}

fn bump_for_pillar(tenor_years: f64, bump: Option<&QuoteBump>) -> f64 {
    match bump {
        Some(QuoteBump::ParallelBp(bp)) => *bp,
        Some(QuoteBump::TenorsBp(targets)) => targets
            .iter()
            .filter(|(target, _)| (tenor_years - target).abs() < 0.1)
            .map(|(_, bp)| *bp)
            .sum(),
        None => 0.0,
    }
}

fn with_quote_recovery(quote: &CdsQuote, recovery_rate: f64) -> CdsQuote {
    match quote {
        CdsQuote::CdsParSpread {
            id,
            entity,
            convention,
            pillar,
            spread_bp,
            ..
        } => CdsQuote::CdsParSpread {
            id: id.clone(),
            entity: entity.clone(),
            convention: convention.clone(),
            pillar: pillar.clone(),
            spread_bp: *spread_bp,
            recovery_rate,
        },
        CdsQuote::CdsUpfront {
            id,
            entity,
            convention,
            pillar,
            running_spread_bp,
            upfront_pct,
            ..
        } => CdsQuote::CdsUpfront {
            id: id.clone(),
            entity: entity.clone(),
            convention: convention.clone(),
            pillar: pillar.clone(),
            running_spread_bp: *running_spread_bp,
            upfront_pct: *upfront_pct,
            recovery_rate,
        },
    }
}

fn replay_once(
    request: &HazardParRecalibration<'_>,
    mut params: HazardCurveParams,
    quotes: &[ReplayQuote],
    config: &CalibrationConfig,
    spread_bump: Option<&QuoteBump>,
    exact_spread_bump: Option<(usize, f64)>,
) -> finstack_quant_core::Result<HazardCurve> {
    params.recovery_rate = request.recovery_rate;
    let market_quotes = quotes
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let bump_bp = exact_spread_bump
                .filter(|(target_index, _)| *target_index == index)
                .map_or_else(
                    || bump_for_pillar(input.pillar_time, spread_bump),
                    |(_, bump_bp)| bump_bp,
                );
            MarketQuote::Cds(
                with_quote_recovery(&input.quote, request.recovery_rate).bump_spread_bp(bump_bp),
            )
        })
        .collect::<Vec<_>>();
    let step = StepParams::Hazard(params.clone());
    let (context, report) =
        step_runtime::execute_params_and_apply(&step, &market_quotes, request.context, config)?;
    ensure_replay_fit_accepted(params.curve_id.as_str(), config, &report)?;
    let replayed = context
        .get_hazard(params.curve_id.as_str())?
        .as_ref()
        .clone();
    replayed
        .to_builder_with_id(replayed.id().clone())
        .hazard_calibration_opt(replayed.hazard_calibration().cloned())
        .fx_policy_opt(request.hazard.fx_policy().map(ToOwned::to_owned))
        .build()
}

fn require_zero_shock_identity(
    expected: &HazardCurve,
    replayed: &HazardCurve,
) -> finstack_quant_core::Result<()> {
    let expected_knots: Vec<_> = expected.knot_points().collect();
    let replayed_knots: Vec<_> = replayed.knot_points().collect();
    let same_knots = expected_knots.len() == replayed_knots.len()
        && expected_knots.iter().zip(&replayed_knots).all(
            |((expected_t, expected_h), (replayed_t, replayed_h))| {
                let t_scale = expected_t.abs().max(replayed_t.abs()).max(1.0);
                let h_scale = expected_h.abs().max(replayed_h.abs()).max(1.0);
                (expected_t - replayed_t).abs() <= 1e-12 * t_scale
                    && (expected_h - replayed_h).abs() <= 1e-10 * h_scale
            },
        );
    if !same_knots {
        return Err(finstack_quant_core::Error::Calibration {
            message: format!(
                "zero-shock hazard replay for '{}' does not reproduce the stored curve",
                expected.id()
            ),
            category: "hazard_replay_identity".to_string(),
        });
    }
    Ok(())
}

fn recalibrate_from_par_spreads(
    request: HazardParRecalibration<'_>,
) -> finstack_quant_core::Result<HazardCurve> {
    let (params, calibration_inputs, spread_risk_inputs, config) = recipe_inputs(request.hazard)?;
    if &params.curve_id != request.hazard.id() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "hazard replay recipe curve ID '{}' does not match stored curve '{}'",
            params.curve_id,
            request.hazard.id()
        )));
    }
    if &params.discount_curve_id != request.discount_id {
        return Err(finstack_quant_core::Error::Validation(format!(
            "hazard replay requires discount curve '{}', not '{}'",
            params.discount_curve_id, request.discount_id
        )));
    }
    if let (Some(requested), Some(stored)) = (request.doc_clause, params.doc_clause.as_deref()) {
        if stored != requested.as_str() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "hazard replay documentation clause {requested:?} conflicts with stored clause {stored:?}"
            )));
        }
    }
    if let (Some(requested), Some(stored)) = (
        request.cds_valuation_convention,
        params.cds_valuation_convention,
    ) {
        if stored != requested {
            return Err(finstack_quant_core::Error::Validation(format!(
                "hazard replay valuation convention {requested:?} conflicts with stored convention {stored:?}"
            )));
        }
    }

    let base_request = HazardParRecalibration {
        context: request.identity_context.unwrap_or(request.context),
        identity_context: None,
        recovery_rate: params.recovery_rate,
        spread_bump: None,
        exact_spread_bump: None,
        replay_spread_risk_center: false,
        ..request
    };
    let replayed_base = replay_once(
        &base_request,
        params.clone(),
        &calibration_inputs,
        &config,
        None,
        None,
    )?;
    require_zero_shock_identity(request.hazard, &replayed_base)?;

    if let Some((index, _)) = request.exact_spread_bump {
        if index >= spread_risk_inputs.len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "hazard spread-risk quote index {index} is out of range for '{}' ({})",
                request.hazard.id(),
                spread_risk_inputs.len()
            )));
        }
    }
    if request.recovery_rate.to_bits() == params.recovery_rate.to_bits()
        && request.spread_bump.is_none_or(QuoteBump::is_zero)
        && request
            .exact_spread_bump
            .is_none_or(|(_, bump_bp)| bump_bp == 0.0)
        && !request.replay_spread_risk_center
        && request.identity_context.is_none()
    {
        return Ok(replayed_base);
    }
    let replay_inputs = if request.replay_spread_risk_center
        || request.spread_bump.is_some()
        || request.exact_spread_bump.is_some()
        || request.recovery_rate.to_bits() != params.recovery_rate.to_bits()
    {
        &spread_risk_inputs
    } else {
        &calibration_inputs
    };
    replay_once(
        &request,
        params,
        replay_inputs,
        &config,
        request.spread_bump,
        request.exact_spread_bump,
    )
}

/// Bump hazard par spreads and re-calibrate, optionally reusing a batch cache.
///
/// # Arguments
///
/// * `cache` - Optional batch-local cache. When `Some`, identical bumps of
///   the same source curve (same identifier, discounting, recovery, and
///   source par/hazard fingerprint) reuse the bootstrapped result. `None`
///   always re-bootstraps.
/// * `hazard` - Existing hazard curve carrying its lossless calibration recipe.
/// * `context` - Market context supplying the original calibration dependencies.
/// * `bump` - Parallel or tenor-specific CDS spread shock in [`QuoteBump`]
///   basis point units.
/// * `discount_id` - Discount curve ID, which must match the stored recipe.
/// * `doc_clause` - Optional documentation-clause assertion. When supplied, it
///   must match the stored recipe; `None` uses the stored value.
/// * `cds_valuation_convention` - Optional valuation-convention assertion.
///   When supplied, it must match the stored recipe; `None` uses the stored value.
pub(crate) fn bump_hazard_spreads_cached(
    cache: Option<&HazardRecalibrationCache>,
    hazard: &HazardCurve,
    context: &MarketContext,
    bump: &QuoteBump,
    discount_id: Option<&CurveId>,
    doc_clause: Option<CdsDocClause>,
    cds_valuation_convention: Option<CdsValuationConvention>,
) -> finstack_quant_core::Result<Arc<HazardCurve>> {
    let discount_id = require_discount_id(discount_id)?;
    let request = HazardParRecalibration {
        hazard,
        context,
        identity_context: None,
        discount_id,
        recovery_rate: hazard.recovery_rate(),
        doc_clause,
        cds_valuation_convention,
        spread_bump: Some(bump),
        exact_spread_bump: None,
        replay_spread_risk_center: false,
    };
    recalibrate_from_par_spreads_cached(cache, request)
}

/// Replay a scenario spread shock while verifying against its source dependencies.
pub(crate) fn bump_hazard_on_target_market(
    cache: &HazardRecalibrationCache,
    request: &finstack_quant_valuations::recalibration::HazardRecalibrationRequest,
    hazard: &HazardCurve,
    bump: &QuoteBump,
) -> finstack_quant_core::Result<Arc<HazardCurve>> {
    let source_discount = request
        .source_market
        .get_discount(&request.discount_curve_id)?;
    let target_discount = request
        .target_market
        .get_discount(&request.discount_curve_id)?;
    let identity_context = (!Arc::ptr_eq(&source_discount, &target_discount))
        .then_some(request.source_market.as_ref());
    recalibrate_from_par_spreads_cached(
        Some(cache),
        HazardParRecalibration {
            hazard,
            context: &request.target_market,
            identity_context,
            discount_id: &request.discount_curve_id,
            recovery_rate: hazard.recovery_rate(),
            doc_clause: request.doc_clause,
            cds_valuation_convention: request.cds_valuation_convention,
            spread_bump: Some(bump),
            exact_spread_bump: None,
            replay_spread_risk_center: false,
        },
    )
}

/// Bump exactly one spread-risk replay binding and recalibrate.
///
/// # Arguments
///
/// * `cache` - Optional batch-local recalibration cache.
/// * `hazard` - Existing hazard curve carrying its lossless replay recipe.
/// * `context` - Market context supplying calibration dependencies.
/// * `quote_bump` - Exact ordered `spread_risk_inputs` index and additive
///   spread shock in basis points.
/// * `discount_id` - Discount curve ID required by the stored recipe.
/// * `doc_clause` - Optional documentation-clause assertion.
/// * `cds_valuation_convention` - Optional valuation-convention assertion.
pub(crate) fn bump_hazard_spread_risk_input_cached(
    cache: Option<&HazardRecalibrationCache>,
    hazard: &HazardCurve,
    context: &MarketContext,
    quote_bump: (usize, f64),
    discount_id: Option<&CurveId>,
    doc_clause: Option<CdsDocClause>,
    cds_valuation_convention: Option<CdsValuationConvention>,
) -> finstack_quant_core::Result<Arc<HazardCurve>> {
    let discount_id = require_discount_id(discount_id)?;
    let request = HazardParRecalibration {
        hazard,
        context,
        identity_context: None,
        discount_id,
        recovery_rate: hazard.recovery_rate(),
        doc_clause,
        cds_valuation_convention,
        spread_bump: None,
        exact_spread_bump: Some(quote_bump),
        replay_spread_risk_center: false,
    };
    recalibrate_from_par_spreads_cached(cache, request)
}

/// Bump hazard curve by shocking par spreads and re-calibrating.
///
/// The source curve's lossless calibration recipe supplies the original typed
/// CDS quotes, pillars, conventions, and solver policy. A zero-shock replay must
/// reproduce the source curve before any nonzero shock is accepted.
///
/// # Arguments
///
/// * `hazard` - Existing hazard curve carrying its lossless calibration recipe.
/// * `context` - Market context supplying the original calibration dependencies.
/// * `bump` - Parallel or tenor-specific CDS spread shock in [`QuoteBump`]
///   basis point units.
/// * `discount_id` - Discount curve ID, which must match the stored recipe.
/// * `doc_clause` - Optional documentation-clause assertion. When supplied, it
///   must match the stored recipe; `None` uses the stored value.
/// * `cds_valuation_convention` - Optional valuation-convention assertion.
///   When supplied, it must match the stored recipe; `None` uses the stored value.
pub fn bump_hazard_spreads(
    hazard: &HazardCurve,
    context: &MarketContext,
    bump: &QuoteBump,
    discount_id: Option<&CurveId>,
    doc_clause: Option<CdsDocClause>,
    cds_valuation_convention: Option<CdsValuationConvention>,
) -> finstack_quant_core::Result<HazardCurve> {
    bump_hazard_spreads_cached(
        None,
        hazard,
        context,
        bump,
        discount_id,
        doc_clause,
        cds_valuation_convention,
    )
    .map(|curve| curve.as_ref().clone())
}

/// Replay the curve from its quote-space spread-risk center.
///
/// Unlike a normal zero bump, this explicitly uses `spread_risk_inputs`.
/// That distinction is required when a deal quote overrides one risk pillar
/// while `calibration_inputs` retain the original calibration provenance.
///
/// # Arguments
///
/// * `hazard` - Existing hazard curve carrying its lossless calibration recipe.
/// * `context` - Market context supplying the original calibration dependencies.
/// * `discount_id` - Discount curve ID, which must match the stored recipe.
/// * `doc_clause` - Optional documentation-clause assertion.
/// * `cds_valuation_convention` - Optional valuation-convention assertion.
pub(crate) fn replay_hazard_spread_risk_center(
    hazard: &HazardCurve,
    context: &MarketContext,
    discount_id: Option<&CurveId>,
    doc_clause: Option<CdsDocClause>,
    cds_valuation_convention: Option<CdsValuationConvention>,
) -> finstack_quant_core::Result<HazardCurve> {
    let discount_id = require_discount_id(discount_id)?;
    let zero_bump = QuoteBump::ParallelBp(0.0);
    recalibrate_from_par_spreads(HazardParRecalibration {
        hazard,
        context,
        identity_context: None,
        discount_id,
        recovery_rate: hazard.recovery_rate(),
        doc_clause,
        cds_valuation_convention,
        spread_bump: Some(&zero_bump),
        exact_spread_bump: None,
        replay_spread_risk_center: true,
    })
}

/// Requote the conditional-survival curve at the horizon, then persist a fresh
/// calibrated recipe. Old quote payloads are provenance, never horizon quotes.
pub(crate) fn replay_hazard_at_horizon(
    request: &finstack_quant_valuations::recalibration::HazardRecalibrationRequest,
) -> finstack_quant_core::Result<HazardCurve> {
    if request.deal_quote_override.is_some() {
        return Err(finstack_quant_core::Error::Validation(
            "hazard horizon replay does not accept deal quote overrides".to_string(),
        ));
    }
    let source = &request.hazard;
    // Verify the source recipe and all caller-supplied convention assertions.
    bump_hazard_spreads(
        source,
        &request.source_market,
        &QuoteBump::ParallelBp(0.0),
        Some(&request.discount_curve_id),
        request.doc_clause,
        request.cds_valuation_convention,
    )?;
    let (mut params, _, quotes, config) = recipe_inputs(source)?;
    let rolled = request.target_market.get_hazard(source.id())?;
    if rolled.base_date() <= source.base_date() {
        return Err(finstack_quant_core::Error::Validation(
            "hazard horizon replay requires a later target base date".to_string(),
        ));
    }
    params.base_date = rolled.base_date();
    let mut curve_ids = finstack_quant_core::HashMap::default();
    curve_ids.insert("discount".to_string(), params.discount_curve_id.to_string());
    curve_ids.insert("credit".to_string(), params.curve_id.to_string());
    let build_ctx = crate::build::BuildCtx::new(params.base_date, params.notional, curve_ids)
        .with_cds_valuation_convention(params.cds_valuation_convention);
    let mut horizon_quotes = Vec::new();
    for input in quotes
        .into_iter()
        .filter(|input| input.pillar_date > params.base_date)
    {
        let mut quote = match input.quote {
            CdsQuote::CdsParSpread {
                id,
                entity,
                convention,
                recovery_rate,
                spread_bp,
                ..
            } => CdsQuote::CdsParSpread {
                id,
                entity,
                convention,
                recovery_rate,
                pillar: Pillar::Date(input.pillar_date),
                spread_bp,
            },
            CdsQuote::CdsUpfront { .. } => {
                return Err(finstack_quant_core::Error::Validation(
                    "hazard spread-risk inputs must contain par quotes".to_string(),
                ))
            }
        };
        let instrument = crate::build::cds::build_cds_instrument(&quote, &build_ctx)?;
        let cds = instrument.as_any().downcast_ref::<
            finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap>()
            .ok_or_else(|| finstack_quant_core::Error::Validation("expected CDS quote instrument".to_string()))?;
        let par = cds.get_par_spread(&request.target_market, params.base_date)?;
        if let CdsQuote::CdsParSpread { spread_bp, .. } = &mut quote {
            *spread_bp = par;
        }
        horizon_quotes.push(MarketQuote::Cds(quote));
    }
    if horizon_quotes.is_empty() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "hazard curve '{}' has no surviving calibration pillars at {}",
            source.id(),
            params.base_date
        )));
    }
    let step = StepParams::Hazard(params.clone());
    let (context, report) = step_runtime::execute_params_and_apply(
        &step,
        &horizon_quotes,
        &request.target_market,
        &config,
    )?;
    ensure_replay_fit_accepted(params.curve_id.as_str(), &config, &report)?;
    let rebuilt = context.get_hazard(params.curve_id.as_str())?;
    // Requoting must retain the conditional survival economics, including the
    // intervals between pillars. Reject any material projection drift.
    let times: Vec<f64> = rolled.knot_points().map(|(t, _)| t).collect();
    for t in times
        .iter()
        .copied()
        .chain(times.windows(2).map(|w| (w[0] + w[1]) * 0.5))
    {
        if (rebuilt.sp(t) - rolled.sp(t)).abs() > 1e-8 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "horizon hazard replay for '{}' changed conditional survival at t={t}",
                source.id()
            )));
        }
    }
    rebuilt
        .to_builder_with_id(rebuilt.id().clone())
        .hazard_calibration_opt(rebuilt.hazard_calibration().cloned())
        .fx_policy_opt(source.fx_policy().map(ToOwned::to_owned))
        .build()
}

/// Replay unchanged hazard quotes against a different dependency market.
///
/// The source market is used only to prove that the stored recipe reproduces
/// `hazard`; the same quotes are then calibrated against `target_context`.
///
/// # Arguments
///
/// * `hazard` - Source curve carrying its lossless calibration recipe.
/// * `source_context` - Original market used for zero-shock identity validation.
/// * `target_context` - Market containing the changed calibration dependencies.
/// * `discount_id` - Discount curve ID required by the stored recipe.
/// * `doc_clause` - Optional documentation-clause assertion.
/// * `cds_valuation_convention` - Optional valuation-convention assertion.
pub(crate) fn replay_hazard_on_dependency_market(
    hazard: &HazardCurve,
    source_context: &MarketContext,
    target_context: &MarketContext,
    discount_id: Option<&CurveId>,
    doc_clause: Option<CdsDocClause>,
    cds_valuation_convention: Option<CdsValuationConvention>,
) -> finstack_quant_core::Result<HazardCurve> {
    let discount_id = require_discount_id(discount_id)?;
    recalibrate_from_par_spreads(HazardParRecalibration {
        hazard,
        context: target_context,
        identity_context: Some(source_context),
        discount_id,
        recovery_rate: hazard.recovery_rate(),
        doc_clause,
        cds_valuation_convention,
        spread_bump: None,
        exact_spread_bump: None,
        replay_spread_risk_center: false,
    })
}

/// Re-bootstrap a hazard curve with a *new* recovery assumption while holding
/// observed CDS par spreads constant.
///
/// This is the operation a Recovery01 metric needs in order to capture the
/// indirect effect of recovery changes on the survival probability term
/// structure. Because `h ≈ S / (1 - R)` to first order, raising `R` while
/// holding `S` constant requires the bootstrap to lift `h` (and vice versa).
/// A naive Recovery01 that bumps the instrument LGD without recalibrating
/// the hazard curve typically *understates* the true recovery sensitivity by
/// 2-5x, which matters materially for distressed credits.
///
/// # Errors
///
/// Returns an error if the curve has no lossless calibration recipe, if a
/// caller-supplied convention conflicts with that recipe, if zero-shock replay
/// is not identical, or if the original calibration cannot be replayed.
///
/// # Arguments
///
/// * `hazard` — source curve carrying its lossless calibration recipe
/// * `new_recovery` — recovery rate used for the replay, clamped to `[0, 1)`
/// * `context` — market context providing the stored calibration dependencies
/// * `discount_id` — discount curve identifier, which must match the stored recipe
/// * `doc_clause` — optional documentation-clause assertion
/// * `cds_valuation_convention` — optional valuation-convention assertion
pub fn recalibrate_hazard_with_recovery(
    hazard: &HazardCurve,
    new_recovery: f64,
    context: &MarketContext,
    discount_id: Option<&CurveId>,
    doc_clause: Option<CdsDocClause>,
    cds_valuation_convention: Option<CdsValuationConvention>,
) -> finstack_quant_core::Result<HazardCurve> {
    // Clamp recovery to a numerically safe range. R = 1 leaves zero LGD which
    // makes spreads non-identifying; we leave a small floor below 1.
    let new_recovery = new_recovery.clamp(0.0, 0.999_999);

    let discount_id = require_discount_id(discount_id)?;
    recalibrate_from_par_spreads(HazardParRecalibration {
        hazard,
        context,
        identity_context: None,
        discount_id,
        recovery_rate: new_recovery,
        doc_clause,
        cds_valuation_convention,
        spread_bump: None,
        exact_spread_bump: None,
        replay_spread_risk_center: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quotes::ids::{Pillar, QuoteId};
    use crate::CalibrationReport;
    use finstack_quant_core::market_data::term_structures::HazardCalibrationRecipe;
    use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
    use finstack_quant_valuations::market::conventions::ids::CdsConventionKey;
    use std::collections::BTreeMap;

    fn failed_report() -> CalibrationReport {
        CalibrationReport::new(
            BTreeMap::from([("CDS-1Y".to_string(), 2e-7), ("CDS-5Y".to_string(), -3e-5)]),
            12,
            false,
            "residual tolerance exceeded",
        )
        .with_success_tolerance(1e-8)
    }

    #[test]
    fn hazard_replay_strict_bad_fit_is_rejected_with_worst_quote() {
        let config = CalibrationConfig::default();
        let error = ensure_replay_fit_accepted("ACME-HZD", &config, &failed_report())
            .expect_err("strict replay must reject a failed fit");
        let message = error.to_string();
        assert!(message.contains("1.000e-8"), "{message}");
        assert!(message.contains("CDS-5Y"), "{message}");
        assert!(message.contains("-3.000e-5"), "{message}");
    }

    #[test]
    fn hazard_replay_permissive_bad_fit_is_allowed() {
        let config = CalibrationConfig {
            fail_on_bad_fit: false,
            ..CalibrationConfig::default()
        };
        ensure_replay_fit_accepted("ACME-HZD", &config, &failed_report())
            .expect("permissive replay should preserve the reported curve");
    }

    fn deal_quote_source_curve(cds: &CreditDefaultSwap) -> HazardCurve {
        let base_date = cds.premium.start;
        let pillar_date = cds.premium.end;
        let pillar_time = finstack_quant_core::dates::DayCount::Act365F
            .year_fraction(
                base_date,
                pillar_date,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .expect("valid pillar time");
        let quote = CdsQuote::CdsParSpread {
            id: QuoteId::new("ACME-5Y"),
            entity: "ACME".to_string(),
            convention: CdsConventionKey {
                currency: cds.notional.currency(),
                doc_clause: CdsDocClause::IsdaNa,
            },
            pillar: Pillar::Date(pillar_date),
            spread_bp: 150.0,
            recovery_rate: cds.protection.recovery_rate,
        };
        let input = HazardCalibrationInput {
            quote: serde_json::to_value(quote).expect("serialize source quote"),
            pillar_date,
            pillar_time,
        };
        let recipe = HazardCalibrationRecipe::new(
            serde_json::json!({"curve_id": "ACME-HZD"}),
            vec![input.clone()],
            vec![input],
            serde_json::json!({"fail_on_bad_fit": true}),
        )
        .expect("valid deal quote recipe");
        HazardCurve::builder("ACME-HZD")
            .base_date(base_date)
            .recovery_rate(cds.protection.recovery_rate)
            .knots([(0.0, 0.02), (pillar_time, 0.02)])
            .par_spreads([(pillar_time, 150.0)])
            .hazard_calibration(recipe)
            .build()
            .expect("deal quote curve")
    }

    #[test]
    fn hazard_cache_identity_distinguishes_deal_quote_replay_inputs() {
        let mut low_deal = CreditDefaultSwap::example();
        low_deal
            .instrument_pricing_overrides
            .market_quotes
            .cds_quote_bp = Some(100.0);
        let mut high_deal = low_deal.clone();
        high_deal
            .instrument_pricing_overrides
            .market_quotes
            .cds_quote_bp = Some(300.0);
        let source = deal_quote_source_curve(&low_deal);
        let low_quote = hazard_with_deal_quote_override(
            &source,
            DealCdsQuoteOverride {
                contract_end: low_deal.premium.end,
                spread_bp: 100.0,
            },
        )
        .expect("low deal quote override");
        let high_quote = hazard_with_deal_quote_override(
            &source,
            DealCdsQuoteOverride {
                contract_end: high_deal.premium.end,
                spread_bp: 300.0,
            },
        )
        .expect("high deal quote override");

        let low_fingerprint = hazard_source_fingerprint(&low_quote);
        let high_fingerprint = hazard_source_fingerprint(&high_quote);
        assert_ne!(
            low_fingerprint, high_fingerprint,
            "deal curves with distinct replay quotes must not alias in the batch cache"
        );
        let low_then_high = HashMap::from([(low_fingerprint, 100.0), (high_fingerprint, 300.0)]);
        let high_then_low = HashMap::from([(high_fingerprint, 300.0), (low_fingerprint, 100.0)]);
        assert_eq!(
            low_then_high, high_then_low,
            "cache results must be independent of deal evaluation order"
        );
    }
}
