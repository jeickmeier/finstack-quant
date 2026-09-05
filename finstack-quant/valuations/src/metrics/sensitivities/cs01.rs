//! Reusable helpers for parallel and bucketed CS01 (credit spread sensitivity).
//!
//! This module is the **canonical CS01 reference for the workspace.** All
//! credit-bearing instruments (CDS, CDS Index, CDS Option, CDS Tranche, Bond,
//! Term Loan, Revolving Credit, Structured Credit, Convertible) report CS01
//! against this convention; any per-instrument calculator that deviates must
//! call out the deviation explicitly in its module documentation.
//!
//! # Canonical Methodology
//!
//! Where the instrument has an associated par CDS / hazard curve, CS01 is a
//! **parallel 1 bp shock to the par CDS curve, re-bootstrapped, with a
//! symmetric (central) finite difference**:
//!
//! ```text
//! CS01 = (PV(s + 1bp) - PV(s - 1bp)) / 2
//! ```
//!
//! where `s` is the par-spread term structure used to bootstrap the hazard
//! curve. The bumped curve is re-bootstrapped under the same CDS conventions
//! (doc clause, valuation convention, discount curve) as the base curve so
//! that CS01 measures market par-spread sensitivity rather than incidental
//! curve-construction artefacts. The bucketed variant applies the same shock
//! one tenor at a time and reports a per-bucket series whose sum reconciles
//! to the parallel value.
//!
//! # Zero-Anchor Bucket (`0y`)
//!
//! When the hazard curve carries a **zero-anchor knot** (`t = 0`) and the
//! configured bucket grid does not include `0.0`, the bucketed series gains a
//! synthetic leading `0y` bucket. Under the piecewise-constant hazard
//! convention that knot governs the forward segment
//! `[0, first_positive_knot)`; without the extra bucket, that segment's
//! sensitivity would be silently dropped and the bucket sum would no longer
//! reconcile to the parallel CS01. Consumers should therefore expect a `0y`
//! label (e.g. `bucketed_cs01::<curve>::0y`) for such curves; it is part of
//! the decomposition, not an artefact.
//!
//! Standard CS01 requires a lossless calibration recipe and rejects
//! directly-specified or otherwise unreplayable hazard curves.
//!
//! # Units and Sign Convention
//!
//! - CS01 is expressed in **currency units per basis point** (`1 bp = 0.0001`).
//! - A CS01 of `-50` means the position loses $50 of PV when par credit
//!   spreads widen by 1 bp.
//! - Sign convention (consistent across **all** CS01 calculators in the
//!   workspace, regardless of which methodology they use):
//!
//!   | Position                         | Expected CS01 sign |
//!   |----------------------------------|--------------------|
//!   | Long bond / sell protection      | Negative           |
//!   | Short bond / buy protection      | Positive           |
//!
//!   In words: a long credit-risk holder (long bond, sell protection) loses
//!   when spreads widen, so CS01 is negative; a short credit-risk holder
//!   (short bond, buy protection) gains when spreads widen, so CS01 is
//!   positive.

use crate::instruments::credit_derivatives::cds::CdsValuationConvention;
use crate::market::conventions::ids::CdsDocClause;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricContext, MetricId};
use crate::recalibration::{DealCdsQuoteOverride, QuoteBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::math::NeumaierAccumulator;
use finstack_quant_core::types::CurveId;
use std::sync::Arc;

/// Minimum bump size threshold (in basis points) to avoid division by near-zero.
const MIN_BUMP_BP_THRESHOLD: f64 = 1e-10;

/// Central-difference sensitivity: `(pv_up - pv_down) / (2 * bump_bp)`.
///
/// `bump_bp` comes from config validated with `ensure_finite_positive`, so a
/// degenerate width is a misconfiguration rather than normal input. A
/// `debug_assert` flags it loudly in debug/test builds (a silent 0.0 is
/// indistinguishable from a true zero CS01); release builds fall back to 0.0
/// rather than divide by ~0 and emit inf/NaN.
#[inline]
pub(crate) fn sensitivity_central_diff(pv_up: f64, pv_down: f64, bump_bp: f64) -> f64 {
    debug_assert!(
        bump_bp.abs() > MIN_BUMP_BP_THRESHOLD,
        "bump_bp must exceed {MIN_BUMP_BP_THRESHOLD} (got {bump_bp}); validate upstream"
    );
    if bump_bp.abs() <= MIN_BUMP_BP_THRESHOLD {
        return 0.0;
    }
    (pv_up - pv_down) / (2.0 * bump_bp)
}

/// Validate that a key-rate bucket grid is strictly increasing.
///
/// Shared by bucketed DV01, CS01 and the sensitivities config loader:
/// unsorted or duplicate tenors silently produce wrong per-bucket sensitivities
/// (each duplicate tenor would be shocked twice and double-counted in the
/// series), so reject them up front with a clear error.
pub(crate) fn validate_buckets_strictly_increasing(
    buckets: &[f64],
) -> finstack_quant_core::Result<()> {
    for win in buckets.windows(2) {
        if win[1].partial_cmp(&win[0]) != Some(std::cmp::Ordering::Greater) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "key-rate buckets must be strictly increasing, got {:?} \
                 (offending pair: {} -> {})",
                buckets, win[0], win[1]
            )));
        }
    }
    Ok(())
}

/// Return each effective direct-hazard node once, in curve order.
// Retained with the generic hazard engines until the next consolidation slice.
#[allow(dead_code)]
pub(crate) fn effective_hazard_node_times(hazard: &HazardCurve) -> Vec<f64> {
    let mut nodes: Vec<f64> = hazard.knot_points().map(|(time, _)| time).collect();
    nodes.dedup_by(|left, right| {
        let scale = left.abs().max(right.abs()).max(1.0);
        (*left - *right).abs() <= 1e-12 * scale
    });
    nodes
}

/// Format an actual hazard-node time without collapsing distinct nodes.
// Retained with the generic hazard engines until the next consolidation slice.
#[allow(dead_code)]
pub(crate) fn format_hazard_node_label(years: f64) -> std::borrow::Cow<'static, str> {
    if super::config::STANDARD_BUCKETS_YEARS
        .iter()
        .any(|standard| (years - standard).abs() <= 1e-12)
    {
        return super::config::format_bucket_label_cow(years);
    }

    let months = years * 12.0;
    if years < 1.0 && (months - months.round()).abs() <= 1e-9 {
        return std::borrow::Cow::Owned(format!("{:.0}m", months.round()));
    }

    let mut value = format!("{years:.10}");
    while value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.pop();
    }
    std::borrow::Cow::Owned(format!("{value}y"))
}

/// Require a lossless calibration recipe for quote-space spread risk.
pub(crate) fn require_hazard_replay(
    hazard: &HazardCurve,
    operation: &str,
) -> finstack_quant_core::Result<()> {
    if hazard.hazard_calibration().is_some() {
        return Ok(());
    }
    Err(finstack_quant_core::Error::Calibration {
        message: format!(
            "{operation} requires a lossless calibration recipe for hazard curve '{}' \
             (a curve produced by the calibration pipeline, not one built directly from \
             hazard rates)",
            hazard.id()
        ),
        category: "cs01_rebootstrap".to_string(),
    })
}

fn require_cs01_discount_id<'a>(
    discount_id: Option<&'a CurveId>,
    hazard: &HazardCurve,
) -> finstack_quant_core::Result<&'a CurveId> {
    discount_id.ok_or_else(|| finstack_quant_core::Error::Calibration {
        message: format!(
            "quote-space CS01 requires the calibration discount curve for hazard curve '{}'",
            hazard.id()
        ),
        category: "cs01_rebootstrap".to_string(),
    })
}

/// Compute parallel quote-space CS01 through an explicit recalibration provider.
///
/// This is the common engine for direct pricer APIs and registered metrics. It
/// owns the symmetric quote bumps, replay requests, error classification, and
/// one-basis-point normalization; callers own only the bumped-hazard repricing
/// closure.
///
/// # Errors
///
/// Returns an error when the hazard is not replayable, the bump is invalid,
/// recalibration fails, or `revalue_raw` cannot price a bumped hazard curve.
/// Bump size and CDS bootstrap convention shared by the parallel and
/// key-rate quote-space CS01 engines.
pub(crate) struct Cs01Request {
    /// Symmetric bump applied to each par-spread quote, in basis points.
    pub(crate) bump_bp: f64,
    /// Discount curve used when re-bootstrapping the hazard curve.
    pub(crate) discount_curve_id: CurveId,
    /// Deal doc clause, when the instrument specifies one.
    pub(crate) doc_clause: Option<CdsDocClause>,
    /// Deal valuation convention, when the instrument specifies one.
    pub(crate) cds_valuation_convention: Option<CdsValuationConvention>,
    /// Optional deal-level clean-spread override.
    pub(crate) deal_quote_override: Option<DealCdsQuoteOverride>,
}

impl Cs01Request {
    /// Request with no deal-specific convention (generic calculators).
    pub(crate) fn generic(bump_bp: f64, discount_curve_id: CurveId) -> Self {
        Self {
            bump_bp,
            discount_curve_id,
            doc_clause: None,
            cds_valuation_convention: None,
            deal_quote_override: None,
        }
    }
}

/// Swap `bumped` into `scratch`, revalue, and restore `original`.
pub(crate) fn reprice_with_hazard<B, F>(
    scratch: &mut MarketContext,
    bumped: B,
    original: &Arc<HazardCurve>,
    mut revalue_raw: F,
) -> finstack_quant_core::Result<f64>
where
    B: Into<finstack_quant_core::market_data::context::CurveStorage>,
    F: FnMut(&MarketContext) -> finstack_quant_core::Result<f64>,
{
    scratch.insert_mut(bumped);
    let pv = revalue_raw(scratch);
    scratch.insert_mut(Arc::clone(original));
    pv
}

pub(crate) fn compute_parallel_cs01_with_provider_raw<RevalFn>(
    provider: &dyn crate::recalibration::RecalibrationProvider,
    hazard: Arc<HazardCurve>,
    source_market: Arc<MarketContext>,
    target_market: Arc<MarketContext>,
    request: &Cs01Request,
    mut revalue_raw: RevalFn,
) -> finstack_quant_core::Result<f64>
where
    RevalFn: FnMut(Arc<HazardCurve>) -> finstack_quant_core::Result<f64>,
{
    let Cs01Request {
        bump_bp,
        discount_curve_id,
        doc_clause,
        cds_valuation_convention,
        deal_quote_override,
    } = request;
    let bump_bp = *bump_bp;
    require_hazard_replay(hazard.as_ref(), "quote-space CS01")?;
    if !bump_bp.is_finite() || bump_bp <= MIN_BUMP_BP_THRESHOLD {
        return Err(finstack_quant_core::Error::Validation(format!(
            "CS01 bump size must be finite and greater than {MIN_BUMP_BP_THRESHOLD} bp, got {bump_bp}"
        )));
    }

    let rebuild = |bump: QuoteBump, direction: &str| {
        provider
            .rebuild_hazard_curve(&crate::recalibration::HazardRecalibrationRequest {
                hazard: Arc::clone(&hazard),
                source_market: Arc::clone(&source_market),
                target_market: Arc::clone(&target_market),
                discount_curve_id: discount_curve_id.clone(),
                doc_clause: *doc_clause,
                cds_valuation_convention: *cds_valuation_convention,
                deal_quote_override: *deal_quote_override,
                action: crate::recalibration::HazardRecalibrationAction::SpreadBump(bump),
            })
            .map_err(|error| finstack_quant_core::Error::Calibration {
                message: format!(
                    "CS01 {direction}-bumped hazard curve re-calibration failed for '{}': {error}",
                    hazard.id()
                ),
                category: "cs01_rebootstrap".to_string(),
            })
    };

    let bumped_up = rebuild(QuoteBump::ParallelBp(bump_bp), "up")?;
    let bumped_down = rebuild(QuoteBump::ParallelBp(-bump_bp), "down")?;
    let pv_up = revalue_raw(bumped_up)?;
    let pv_down = revalue_raw(bumped_down)?;
    Ok(sensitivity_central_diff(pv_up, pv_down, bump_bp))
}

/// Compute parallel CS01 by bumping par spreads and re-calibrating.
///
/// Calculates credit spread sensitivity by shifting the par spreads in parallel
/// and re-bootstrapping the hazard curve.
///
/// # Arguments
///
/// * `context` - Metric context containing instrument and market data
/// * `hazard_id` - ID of the hazard curve to bump
/// * `discount_id` - ID of the discount curve used for calibration (optional)
/// * `bump_bp` - Bump size in basis points (typically 1.0 for CS01)
/// * `revalue_raw` - Closure that reprices the instrument with a bumped context,
///   returning raw f64 for precision
///
/// # Errors
///
/// Returns an error if hazard curve re-calibration fails. This ensures that CS01
/// is computed under a consistent definition (par spread bump + rebootstrap) rather
/// than silently falling back to a different methodology.
pub(crate) fn compute_parallel_cs01_with_context_raw<RevalFn>(
    context: &mut MetricContext,
    hazard_id: &CurveId,
    request: &Cs01Request,
    mut revalue_raw: RevalFn,
) -> finstack_quant_core::Result<f64>
where
    RevalFn: FnMut(&MarketContext) -> finstack_quant_core::Result<f64>,
{
    let curves = Arc::clone(&context.curves);
    let base_ctx = curves.as_ref();
    let hazard = base_ctx.get_hazard(hazard_id.as_str())?;
    let discount_id = require_cs01_discount_id(Some(&request.discount_curve_id), hazard.as_ref())?;
    debug_assert_eq!(discount_id, &request.discount_curve_id);
    // Preserve the quote-space contract before looking up the execution
    // provider, so an unreplayable curve reports the primary input defect.
    require_hazard_replay(hazard.as_ref(), "quote-space CS01")?;
    let provider = context.recalibration_provider("cs01")?;
    compute_parallel_cs01_with_provider_raw(
        provider.as_ref(),
        Arc::clone(&hazard),
        Arc::clone(&curves),
        curves,
        request,
        |bumped_hazard| {
            context.with_market_scratch(|_, scratch| {
                reprice_with_hazard(scratch, bumped_hazard, &hazard, &mut revalue_raw)
            })
        },
    )
}

/// Compute key-rate CS01 series by bumping par spreads at specific tenors.
///
/// - Buckets are the exact ordered `spread_risk_inputs` replay bindings.
/// - Each bucket bumps one atomic replay quote by identity, never by tenor proximity.
/// - `bump_bp` is the bump size in basis points (typically 1.0 for CS01)
///
/// # Errors
///
/// Returns an error if hazard curve re-calibration fails. This ensures that CS01
/// is computed under a consistent definition rather than silently falling back.
/// `series_id` names the bucketed series stored on the context; `request`
/// carries the bump size and CDS bootstrap convention.
pub(crate) fn compute_key_rate_cs01_series_with_context_raw<RevalFn>(
    context: &mut MetricContext,
    hazard_id: &CurveId,
    series_id: MetricId,
    request: &Cs01Request,
    mut revalue_raw: RevalFn,
) -> finstack_quant_core::Result<f64>
where
    RevalFn: FnMut(&MarketContext) -> finstack_quant_core::Result<f64>,
{
    let Cs01Request {
        bump_bp,
        discount_curve_id,
        doc_clause,
        cds_valuation_convention,
        deal_quote_override,
    } = request;
    let bump_bp = *bump_bp;
    let curves = Arc::clone(&context.curves);
    let base_ctx = curves.as_ref();
    let hazard = base_ctx.get_hazard(hazard_id.as_str())?;
    let hazard_ref = hazard.as_ref();
    require_hazard_replay(hazard_ref, "quote-space bucketed CS01")?;
    let discount_id = require_cs01_discount_id(Some(discount_curve_id), hazard_ref)?;
    debug_assert_eq!(discount_id, discount_curve_id);
    let buckets = context.hazard_spread_risk_buckets(hazard_ref)?;
    let base_labels: Vec<_> = buckets
        .iter()
        .map(|bucket| super::config::format_bucket_label_cow(bucket.pillar_time))
        .collect();

    let (series, total) = context.with_market_scratch(|context, scratch| {
        let mut series: Vec<(std::borrow::Cow<'static, str>, f64)> = Vec::new();
        let mut total_acc = NeumaierAccumulator::new();

        for (bucket, base_label) in buckets.iter().zip(&base_labels) {
            let duplicate_label = base_labels
                .iter()
                .filter(|candidate| candidate.as_ref() == base_label.as_ref())
                .count()
                > 1;
            let label = if duplicate_label {
                std::borrow::Cow::Owned(format!(
                    "{}@{}@{}",
                    base_label, bucket.pillar_date, bucket.quote_id
                ))
            } else {
                base_label.clone()
            };

            let rebuild = |bp: f64, direction: &str| {
                context
                    .rebuild_hazard_curve(
                        crate::recalibration::HazardRecalibrationRequest {
                            hazard: Arc::clone(&hazard),
                            source_market: Arc::clone(&curves),
                            target_market: Arc::clone(&curves),
                            discount_curve_id: discount_curve_id.clone(),
                            doc_clause: *doc_clause,
                            cds_valuation_convention: *cds_valuation_convention,
                            deal_quote_override: *deal_quote_override,
                            action:
                                crate::recalibration::HazardRecalibrationAction::ExactQuoteIndexBump {
                                    quote_index: bucket.quote_index,
                                    bump_bp: bp,
                                },
                        },
                        "bucketed_cs01",
                    )
                    .map_err(|e| finstack_quant_core::Error::Calibration {
                        message: format!(
                            "CS01 bucket '{}' {direction}-bump hazard re-calibration failed: {}",
                            label, e
                        ),
                        category: "cs01_rebootstrap".to_string(),
                    })
            };
            let bumped_hazard_up = rebuild(bump_bp, "up")?;
            let bumped_hazard_down = rebuild(-bump_bp, "down")?;

            let pv_bumped_up =
                reprice_with_hazard(scratch, bumped_hazard_up, &hazard, &mut revalue_raw)?;
            let pv_bumped_down =
                reprice_with_hazard(scratch, bumped_hazard_down, &hazard, &mut revalue_raw)?;

            let cs01 = sensitivity_central_diff(pv_bumped_up, pv_bumped_down, bump_bp);
            series.push((label, cs01));
            total_acc.add(cs01);
        }

        Ok((series, total_acc.total()))
    })?;

    context.store_bucketed_series(series_id, series);
    Ok(total)
}

use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::MetricCalculator;
use std::marker::PhantomData;

/// Outcome of resolving an instrument's CS01 curve dependencies.
enum Cs01Curves {
    /// Instrument declares a credit curve; carries the resolved hazard and
    /// (optional) discount curve IDs.
    Resolved(CurveId, Option<CurveId>),
    /// Instrument declares no credit curve. Calculators configured with
    /// `empty_credit_curve_zero` report CS01 as `0.0` in this case; otherwise
    /// this is surfaced as a validation error.
    NoCreditCurve,
}

/// Resolve the primary credit (hazard) and discount curve IDs from an instrument's
/// declared curve dependencies.
///
/// Returns [`Cs01Curves::NoCreditCurve`] when no credit curve is declared so the
/// caller can decide whether that is a hard error or a graceful `0.0`.
fn resolve_cs01_curves<I: Instrument>(instrument: &I) -> finstack_quant_core::Result<Cs01Curves> {
    let curves = instrument.market_dependencies()?.curves;
    let Some(hazard_id) = curves.credit_curves.first().cloned() else {
        return Ok(Cs01Curves::NoCreditCurve);
    };
    let discount_id = curves.discount_curves.first().cloned();
    Ok(Cs01Curves::Resolved(hazard_id, discount_id))
}

/// Build the validation error raised when a CS01 calculator that requires a
/// credit curve is applied to an instrument that declares none.
fn missing_credit_curve_error<I: Instrument>(
    instrument: &I,
    metric_name: &str,
) -> finstack_quant_core::Error {
    finstack_quant_core::Error::Validation(format!(
        "Instrument {} has no credit curve dependencies for {} calculation",
        instrument.id(),
        metric_name
    ))
}

fn resolve_optional_cs01_curves<I: Instrument>(
    instrument: &I,
    empty_credit_curve_zero: bool,
    metric_name: &str,
) -> finstack_quant_core::Result<Option<(CurveId, Option<CurveId>)>> {
    match resolve_cs01_curves(instrument)? {
        Cs01Curves::Resolved(hazard_id, discount_id) => Ok(Some((hazard_id, discount_id))),
        Cs01Curves::NoCreditCurve if empty_credit_curve_zero => Ok(None),
        Cs01Curves::NoCreditCurve => Err(missing_credit_curve_error(instrument, metric_name)),
    }
}

/// Generic BucketedCs01 calculator that works for any instrument implementing
/// the required traits.
pub(crate) struct GenericBucketedCs01<I> {
    /// When `true`, an instrument with no credit curve reports CS01 as `0.0`
    /// instead of raising a validation error.
    empty_credit_curve_zero: bool,
    _phantom: PhantomData<I>,
}

/// Generic parallel CS01 calculator that returns a scalar (not bucketed).
///
/// Computes CS01 by applying a parallel bump to the entire hazard curve.
pub(crate) struct GenericParallelCs01<I> {
    /// When `true`, an instrument with no credit curve reports CS01 as `0.0`
    /// instead of raising a validation error.
    empty_credit_curve_zero: bool,
    _phantom: PhantomData<I>,
}

impl<I> Default for GenericParallelCs01<I> {
    fn default() -> Self {
        Self {
            empty_credit_curve_zero: false,
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for GenericParallelCs01<I>
where
    I: Instrument + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let Some((hazard_id, discount_id)) =
            resolve_optional_cs01_curves(instrument, self.empty_credit_curve_zero, "CS01")?
        else {
            return Ok(0.0);
        };

        let bump_bp = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_overrides(),
        )?
        .credit_spread_bump_bp;

        let reval = cs01_reval(context);

        let request = Cs01Request::generic(
            bump_bp,
            discount_id.ok_or_else(|| finstack_quant_core::Error::Calibration {
                message: "CS01 requires a discount curve identifier".to_string(),
                category: "cs01_rebootstrap".to_string(),
            })?,
        );
        let cs01 = compute_parallel_cs01_with_context_raw(context, &hazard_id, &request, reval)?;

        context.computed.insert(
            MetricId::custom(format!("cs01::{}", hazard_id.as_str())),
            cs01,
        );

        Ok(cs01)
    }
}

impl<I> Default for GenericBucketedCs01<I> {
    fn default() -> Self {
        Self {
            empty_credit_curve_zero: false,
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for GenericBucketedCs01<I>
where
    I: Instrument + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let Some((hazard_id, discount_id)) =
            resolve_optional_cs01_curves(instrument, self.empty_credit_curve_zero, "CS01")?
        else {
            return Ok(0.0);
        };

        let defaults = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_overrides(),
        )?;
        let bump_bp = defaults.credit_spread_bump_bp;

        let reval = cs01_reval(context);

        let series_id = MetricId::custom(format!("bucketed_cs01::{}", hazard_id.as_str()));

        let request = Cs01Request::generic(
            bump_bp,
            discount_id.ok_or_else(|| finstack_quant_core::Error::Calibration {
                message: "bucketed CS01 requires a discount curve identifier".to_string(),
                category: "cs01_rebootstrap".to_string(),
            })?,
        );
        let total = compute_key_rate_cs01_series_with_context_raw(
            context, &hazard_id, series_id, &request, reval,
        )?;

        Ok(total)
    }
}

/// Per-deal CS01 conventions supplied by credit instruments (CDS, CDS Option).
///
/// The generic [`GenericParallelCs01`] / [`GenericBucketedCs01`] calculators
/// re-bootstrap the hazard curve under fixed `doc_clause: None` /
/// `cds_valuation_convention: None`. For genuine credit instruments those are
/// **per-deal** quote-convention inputs to the par-spread→hazard bootstrap, so
/// they must be read off the specific instrument at `calculate()` time rather
/// than baked into a per-type calculator.
///
/// This trait collects every credit-specific CS01 input so the
/// [`CreditParallelCs01`] / [`CreditBucketedCs01`] calculators can fully
/// reproduce the behaviour of the former bespoke `cds` / `cds_option` CS01
/// calculators with byte-identical output.
pub(crate) trait CdsCs01Conventions {
    /// Doc clause and valuation convention used to re-bootstrap the hazard
    /// curve from par spreads.
    ///
    /// `as_of` is supplied so instruments that derive the convention from a
    /// date-dependent synthetic underlying (e.g. a CDS option) can build it
    /// against the actual valuation date.
    fn cs01_bootstrap_convention(
        &self,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<(CdsDocClause, CdsValuationConvention)>;

    /// Optional pre-check run before the CS01 compute.
    ///
    /// Returning `Ok(Some(v))` short-circuits CS01 to `v` (e.g. a CDS option
    /// past expiry reports `0.0`). Returning `Err(..)` surfaces a hard
    /// validation/calibration error (e.g. a CDS option whose hazard curve
    /// carries no par-spread points). `Ok(None)` proceeds normally.
    fn cs01_precheck(
        &self,
        _context: &MetricContext,
        _hazard_id: &CurveId,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(None)
    }

    /// Optional deal-level clean-spread override passed to the provider.
    fn cs01_deal_quote_override(&self) -> Option<DealCdsQuoteOverride> {
        None
    }
}

/// Resolved CDS spread-risk inputs shared by CS01 and CS-Gamma.
pub(crate) struct PreparedCdsRiskContext {
    pub(crate) hazard_id: CurveId,
    pub(crate) discount_id: CurveId,
    pub(crate) doc_clause: CdsDocClause,
    pub(crate) valuation_convention: CdsValuationConvention,
    pub(crate) deal_quote_override: Option<DealCdsQuoteOverride>,
}

impl PreparedCdsRiskContext {
    /// Build the quote-space CS01 request for this deal's conventions.
    pub(crate) fn cs01_request(&self, bump_bp: f64) -> Cs01Request {
        Cs01Request {
            bump_bp,
            discount_curve_id: self.discount_id.clone(),
            doc_clause: Some(self.doc_clause),
            cds_valuation_convention: Some(self.valuation_convention),
            deal_quote_override: self.deal_quote_override,
        }
    }
}

/// Resolve CDS spread-risk inputs, apply any deal-quote market override, run
/// the calculation, and restore the original market on both success and error.
pub(crate) fn with_prepared_cds_risk_context<I>(
    context: &mut MetricContext,
    missing_credit_curve_value: Option<f64>,
    operation: &str,
    calculate: impl FnOnce(
        &mut MetricContext,
        &PreparedCdsRiskContext,
    ) -> finstack_quant_core::Result<f64>,
) -> finstack_quant_core::Result<f64>
where
    I: Instrument + CdsCs01Conventions + 'static,
{
    let instrument: &I = context.instrument_as()?;
    let Some((hazard_id, discount_id)) =
        resolve_optional_cs01_curves(instrument, missing_credit_curve_value.is_some(), operation)?
    else {
        return Ok(missing_credit_curve_value.unwrap_or(0.0));
    };

    if let Some(value) = context
        .instrument_as::<I>()?
        .cs01_precheck(context, &hazard_id)?
    {
        return Ok(value);
    }

    let discount_id = discount_id.ok_or_else(|| finstack_quant_core::Error::Calibration {
        message: format!(
            "{operation} requires the calibration discount curve for hazard curve '{}'",
            hazard_id.as_str()
        ),
        category: "cs01_rebootstrap".to_string(),
    })?;
    let (doc_clause, valuation_convention) = context
        .instrument_as::<I>()?
        .cs01_bootstrap_convention(context.as_of)?;
    let deal_quote_override = context.instrument_as::<I>()?.cs01_deal_quote_override();

    let prepared = PreparedCdsRiskContext {
        hazard_id,
        discount_id,
        doc_clause,
        valuation_convention,
        deal_quote_override,
    };
    calculate(context, &prepared)
}

/// Build the reval closure used by CS01 calculators.
pub(crate) fn cs01_reval(
    context: &MetricContext,
) -> impl FnMut(&MarketContext) -> finstack_quant_core::Result<f64> {
    let inst_arc = Arc::clone(&context.instrument);
    let dispatch = context.clone_pricer_dispatch();
    let as_of = context.as_of;
    move |temp_ctx: &MarketContext| dispatch.price_raw(inst_arc.as_ref(), temp_ctx, as_of)
}

/// Credit-convention-aware parallel CS01 calculator.
///
/// Behaves like [`GenericParallelCs01`] but reads the per-deal `doc_clause`
/// and `cds_valuation_convention` (and optional pre-check / curve override)
/// from the instrument via [`CdsCs01Conventions`]. Used by `CreditDefaultSwap`
/// and `CDSOption`.
pub(crate) struct CreditParallelCs01<I> {
    _phantom: PhantomData<I>,
}

impl<I> Default for CreditParallelCs01<I> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for CreditParallelCs01<I>
where
    I: Instrument + CdsCs01Conventions + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        with_prepared_cds_risk_context::<I>(context, None, "CS01", |context, prepared| {
            let bump_bp = sens_config::from_context_or_default(
                context.get_config(),
                context.get_metric_overrides(),
            )?
            .credit_spread_bump_bp;
            let reval = cs01_reval(context);
            let request = prepared.cs01_request(bump_bp);
            let cs01 = compute_parallel_cs01_with_context_raw(
                context,
                &prepared.hazard_id,
                &request,
                reval,
            )?;
            context.computed.insert(
                MetricId::custom(format!("cs01::{}", prepared.hazard_id.as_str())),
                cs01,
            );
            Ok(cs01)
        })
    }
}

/// Credit-convention-aware bucketed (key-rate) CS01 calculator.
///
/// Behaves like [`GenericBucketedCs01`] but reads the per-deal CDS bootstrap
/// convention from the instrument via [`CdsCs01Conventions`].
pub(crate) struct CreditBucketedCs01<I> {
    _phantom: PhantomData<I>,
}

impl<I> Default for CreditBucketedCs01<I> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for CreditBucketedCs01<I>
where
    I: Instrument + CdsCs01Conventions + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        with_prepared_cds_risk_context::<I>(context, None, "bucketed CS01", |context, prepared| {
            let defaults = sens_config::from_context_or_default(
                context.get_config(),
                context.get_metric_overrides(),
            )?;
            let bump_bp = defaults.credit_spread_bump_bp;
            let series_id =
                MetricId::custom(format!("bucketed_cs01::{}", prepared.hazard_id.as_str()));
            let cds_cache = match context.instrument_as::<
                    crate::instruments::credit_derivatives::cds::CreditDefaultSwap,
                >() {
                    Ok(cds) => Some(
                        crate::instruments::credit_derivatives::cds::pricing::CdsHazardRepriceCache::try_new(
                            cds,
                            context.curves.as_ref(),
                            context.as_of,
                        )?,
                    ),
                    Err(_) => None,
                };
            let request = prepared.cs01_request(bump_bp);
            if let Some(cache) = cds_cache {
                let hazard_key = prepared.hazard_id.clone();
                compute_key_rate_cs01_series_with_context_raw(
                    context,
                    &prepared.hazard_id,
                    series_id,
                    &request,
                    move |temp_ctx: &MarketContext| {
                        let surv = temp_ctx.get_hazard(hazard_key.as_str())?;
                        cache.npv(surv.as_ref())
                    },
                )
            } else {
                let reval = cs01_reval(context);
                compute_key_rate_cs01_series_with_context_raw(
                    context,
                    &prepared.hazard_id,
                    series_id,
                    &request,
                    reval,
                )
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::validate_buckets_strictly_increasing;
    use finstack_quant_core::math::NeumaierAccumulator;

    #[test]
    fn test_central_vs_forward_difference() {
        // Central difference: (f(x+h) - f(x-h)) / (2h) = f'(x) + O(h^2)
        // Forward difference: (f(x+h) - f(x)) / h = f'(x) + O(h)
        // For f(x) = x^2, f'(x) = 2x at x=1, h=0.1:
        // Central: (1.21 - 0.81) / 0.2 = 2.0 (exact)
        // Forward: (1.21 - 1.0) / 0.1 = 2.1 (has error)
        let f = |x: f64| x * x;
        let x = 1.0;
        let h = 0.1;
        let central = (f(x + h) - f(x - h)) / (2.0 * h);
        let forward = (f(x + h) - f(x)) / h;
        assert!(
            (central - 2.0).abs() < 1e-14,
            "Central difference should be exact for quadratics"
        );
        assert!(
            (forward - 2.0).abs() > 0.09,
            "Forward difference should have O(h) error"
        );
    }

    #[test]
    fn cs01_bucket_total_uses_compensated_summation() {
        let n = 1_000_000usize;
        let mut values = vec![1.0e16];
        values.extend(std::iter::repeat_n(1.0, n));
        values.push(-1.0e16);

        let naive: f64 = values.iter().fold(0.0_f64, |acc, v| acc + v);

        let mut acc = NeumaierAccumulator::new();
        for v in &values {
            acc.add(*v);
        }
        let compensated = acc.total();

        assert!(
            (compensated - n as f64).abs() < 1e-6,
            "compensated summation must recover the exact total {n}, got {compensated}"
        );
        assert!(
            (naive - n as f64).abs() > 1.0,
            "naive summation is expected to lose precision here (got {naive}, \
             exact {n}); the CS01 bucket totals must therefore use \
             NeumaierAccumulator"
        );
    }

    #[test]
    fn validate_buckets_rejects_unsorted_and_duplicate_grids() {
        validate_buckets_strictly_increasing(&[0.25, 0.5, 1.0, 5.0])
            .expect("sorted grid must validate");
        validate_buckets_strictly_increasing(&[]).expect("empty grid is vacuously valid");
        validate_buckets_strictly_increasing(&[1.0]).expect("single-element grid is valid");

        let dup = validate_buckets_strictly_increasing(&[0.25, 1.0, 1.0, 5.0])
            .expect_err("duplicate tenor must error");
        assert!(
            matches!(dup, finstack_quant_core::Error::Validation(_)),
            "duplicate tenor must surface as Validation, got {dup:?}"
        );

        let unsorted = validate_buckets_strictly_increasing(&[0.25, 5.0, 1.0])
            .expect_err("unsorted grid must error");
        assert!(
            matches!(unsorted, finstack_quant_core::Error::Validation(_)),
            "unsorted grid must surface as Validation, got {unsorted:?}"
        );
    }
}
