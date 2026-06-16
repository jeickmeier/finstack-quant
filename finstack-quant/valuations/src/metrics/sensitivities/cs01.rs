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
//! When par-spread points are unavailable (e.g. directly-specified hazard
//! curves), the helpers fall back to a parallel 1 bp **hazard-rate** shift
//! using the same symmetric central difference; this is exposed separately as
//! `cs01_hazard` / `bucketed_cs01_hazard` to keep the two regimes auditable.
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

use crate::calibration::bumps::hazard::{
    bump_hazard_shift, bump_hazard_spreads_with_doc_clause_and_valuation_convention,
};
use crate::calibration::bumps::BumpRequest;
use crate::instruments::credit_derivatives::cds::CdsValuationConvention;
use crate::market::conventions::ids::CdsDocClause;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricContext, MetricId};
use finstack_quant_core::market_data::context::MarketContext;
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
        "CS01 bump_bp must exceed {MIN_BUMP_BP_THRESHOLD} (got {bump_bp}); validate upstream"
    );
    if bump_bp.abs() <= MIN_BUMP_BP_THRESHOLD {
        return 0.0;
    }
    (pv_up - pv_down) / (2.0 * bump_bp)
}

/// Validate that a bucket grid is strictly increasing.
///
/// Mirrors the DV01 invariant at
/// `metrics::sensitivities::dv01::UnifiedDv01Calculator::compute_triangular_for_curve`:
/// unsorted or duplicate tenors silently produce wrong per-bucket sensitivities
/// (each duplicate tenor would be shocked twice and double-counted in the
/// series), so reject them up front with a clear error.
pub(crate) fn validate_buckets_strictly_increasing(
    buckets: &[f64],
) -> finstack_quant_core::Result<()> {
    for win in buckets.windows(2) {
        if win[1].partial_cmp(&win[0]) != Some(std::cmp::Ordering::Greater) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CS01 key-rate buckets must be strictly increasing, got {:?} \
                 (offending pair: {} -> {})",
                buckets, win[0], win[1]
            )));
        }
    }
    Ok(())
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
    discount_id: Option<&CurveId>,
    bump_bp: f64,
    doc_clause: Option<CdsDocClause>,
    cds_valuation_convention: Option<CdsValuationConvention>,
    mut revalue_raw: RevalFn,
) -> finstack_quant_core::Result<f64>
where
    RevalFn: FnMut(&MarketContext) -> finstack_quant_core::Result<f64>,
{
    let curves = Arc::clone(&context.curves);
    let base_ctx = curves.as_ref();
    let hazard = base_ctx.get_hazard(hazard_id.as_str())?;
    let hazard_ref = hazard.as_ref();
    let has_par_points = hazard_ref.par_spread_points().next().is_some();

    // If we have par spread points + a discount curve, CS01 is defined as the sensitivity
    // to *market par spreads* under a re-bootstrapped hazard curve. In that regime, we
    // must also compute the base PV under the unbumped (re-calibrated) curve; otherwise
    // we introduce a large "base effect" when the in-context hazard curve was not itself
    // calibrated from the stored par points.
    // Central differencing does not need the base PV, but we still probe whether
    // par-spread re-bootstrapping is available so both legs use the same methodology.
    let used_rebootstrap = if discount_id.is_some() && has_par_points {
        crate::calibration::bumps::hazard::bump_hazard_spreads_with_doc_clause_and_valuation_convention(
            hazard_ref,
            base_ctx,
            &BumpRequest::Parallel(0.0),
            discount_id,
            doc_clause,
            cds_valuation_convention,
        )
        .map_err(|e| finstack_quant_core::Error::Calibration {
            message: format!(
                "CS01 hazard curve re-calibration failed for '{}': {} \
                 (cannot compute CS01 under market-standard par spread bump methodology)",
                hazard_id.as_str(),
                e
            ),
            category: "cs01_rebootstrap".to_string(),
        })?;
        true
    } else {
        false
    };

    let bump_request_up = BumpRequest::Parallel(bump_bp);
    let bump_request_down = BumpRequest::Parallel(-bump_bp);

    let bumped_hazard_up = if used_rebootstrap {
        crate::calibration::bumps::hazard::bump_hazard_spreads_with_doc_clause_and_valuation_convention(
            hazard_ref,
            base_ctx,
            &bump_request_up,
            discount_id,
            doc_clause,
            cds_valuation_convention,
        )
        .map_err(|e| finstack_quant_core::Error::Calibration {
            message: format!(
                "CS01 up-bumped hazard curve re-calibration failed for '{}': {}",
                hazard_id.as_str(),
                e
            ),
            category: "cs01_rebootstrap".to_string(),
        })?
    } else {
        bump_hazard_shift(hazard_ref, &bump_request_up)?
    };

    let bumped_hazard_down = if used_rebootstrap {
        crate::calibration::bumps::hazard::bump_hazard_spreads_with_doc_clause_and_valuation_convention(
            hazard_ref,
            base_ctx,
            &bump_request_down,
            discount_id,
            doc_clause,
            cds_valuation_convention,
        )
        .map_err(|e| finstack_quant_core::Error::Calibration {
            message: format!(
                "CS01 down-bumped hazard curve re-calibration failed for '{}': {}",
                hazard_id.as_str(),
                e
            ),
            category: "cs01_rebootstrap".to_string(),
        })?
    } else {
        bump_hazard_shift(hazard_ref, &bump_request_down)?
    };

    let (pv_bumped_up, pv_bumped_down) = context.with_market_scratch(|_, scratch| {
        scratch.insert_mut(bumped_hazard_up);
        let pv_bumped_up = revalue_raw(scratch)?;
        scratch.insert_mut(std::sync::Arc::clone(&hazard));

        scratch.insert_mut(bumped_hazard_down);
        let pv_bumped_down = revalue_raw(scratch)?;
        scratch.insert_mut(std::sync::Arc::clone(&hazard));

        Ok((pv_bumped_up, pv_bumped_down))
    })?;

    Ok(sensitivity_central_diff(
        pv_bumped_up,
        pv_bumped_down,
        bump_bp,
    ))
}

/// Compute key-rate CS01 series by bumping par spreads at specific tenors.
///
/// - `bucket_times_years` are maturities in years (e.g., 0.25, 0.5, 1.0, ...)
/// - For bootstrapped curves, bumps the par quote corresponding to the bucket.
/// - `bump_bp` is the bump size in basis points (typically 1.0 for CS01)
///
/// # Errors
///
/// Returns an error if hazard curve re-calibration fails. This ensures that CS01
/// is computed under a consistent definition rather than silently falling back.
/// Inputs that define the key-rate CS01 bump grid and CDS bootstrap convention.
pub(crate) struct KeyRateCs01Request<I> {
    pub(crate) series_id: MetricId,
    pub(crate) bucket_times_years: I,
    pub(crate) bump_bp: f64,
    pub(crate) doc_clause: Option<CdsDocClause>,
    pub(crate) cds_valuation_convention: Option<CdsValuationConvention>,
}

pub(crate) fn compute_key_rate_cs01_series_with_context_raw<I, RevalFn>(
    context: &mut MetricContext,
    hazard_id: &CurveId,
    discount_id: Option<&CurveId>,
    request: KeyRateCs01Request<I>,
    mut revalue_raw: RevalFn,
) -> finstack_quant_core::Result<f64>
where
    I: IntoIterator<Item = f64>,
    RevalFn: FnMut(&MarketContext) -> finstack_quant_core::Result<f64>,
{
    let KeyRateCs01Request {
        series_id,
        bucket_times_years,
        bump_bp,
        doc_clause,
        cds_valuation_convention,
    } = request;
    let curves = Arc::clone(&context.curves);
    let base_ctx = curves.as_ref();
    let hazard = base_ctx.get_hazard(hazard_id.as_str())?;
    let hazard_ref = hazard.as_ref();
    let has_par_points = hazard_ref.par_spread_points().next().is_some();

    // Align the bucket grid with the hazard-curve knots so the sum of key-rate
    // CS01s reconciles to the parallel CS01.
    //
    // Under the piecewise-constant hazard convention the knot at `t_i` controls
    // the forward segment `[t_i, t_{i+1})`; in particular a *zero anchor* knot
    // (`t = 0`) governs `[0, first_positive_knot)`. The standard bucket grid
    // typically starts at the first positive tenor (e.g. `0.25`), so without
    // explicitly including the zero anchor that leading segment's sensitivity
    // is silently dropped from the bucketed decomposition. Prepending a
    // synthetic `0.0` bucket when (and only when) the hazard curve actually
    // carries a zero-anchor knot restores the invariant
    // `sum(bucketed_cs01) ≈ parallel_cs01` without affecting curves that are
    // natively supported by the grid.
    let mut buckets: Vec<f64> = bucket_times_years.into_iter().collect();
    let has_zero_anchor = hazard_ref
        .knot_points()
        .next()
        .is_some_and(|(t, _)| t.abs() <= 1e-9);
    if has_zero_anchor && !buckets.iter().any(|t| t.abs() <= 1e-9) {
        buckets.insert(0, 0.0);
    }
    // Validate after optional zero-anchor insertion.
    validate_buckets_strictly_increasing(&buckets)?;

    // Central differencing does not need the base PV, but we still probe whether
    // par-spread re-bootstrapping is available so all buckets use the same methodology.
    let used_rebootstrap = if discount_id.is_some() && has_par_points {
        bump_hazard_spreads_with_doc_clause_and_valuation_convention(
            hazard_ref,
            base_ctx,
            &BumpRequest::Parallel(0.0),
            discount_id,
            doc_clause,
            cds_valuation_convention,
        )
        .map_err(|e| finstack_quant_core::Error::Calibration {
            message: format!(
                "CS01 hazard curve re-calibration failed for '{}': {}",
                hazard_id.as_str(),
                e
            ),
            category: "cs01_rebootstrap".to_string(),
        })?;
        true
    } else {
        false
    };

    let (series, total) = context.with_market_scratch(|_, scratch| {
        let mut series: Vec<(std::borrow::Cow<'static, str>, f64)> = Vec::new();
        let mut total_acc = NeumaierAccumulator::new();

        for t in buckets {
            let label = super::config::format_bucket_label_cow(t);

            let bump_request_up = BumpRequest::Tenors(vec![(t, bump_bp)]);
            let bump_request_down = BumpRequest::Tenors(vec![(t, -bump_bp)]);

            let bumped_hazard_up = if used_rebootstrap {
                bump_hazard_spreads_with_doc_clause_and_valuation_convention(
                    hazard_ref,
                    base_ctx,
                    &bump_request_up,
                    discount_id,
                    doc_clause,
                    cds_valuation_convention,
                )
                .map_err(|e| finstack_quant_core::Error::Calibration {
                    message: format!(
                        "CS01 bucket '{}' up-bump hazard re-calibration failed: {}",
                        label, e
                    ),
                    category: "cs01_rebootstrap".to_string(),
                })?
            } else {
                bump_hazard_shift(hazard_ref, &bump_request_up)?
            };

            let bumped_hazard_down = if used_rebootstrap {
                bump_hazard_spreads_with_doc_clause_and_valuation_convention(
                    hazard_ref,
                    base_ctx,
                    &bump_request_down,
                    discount_id,
                    doc_clause,
                    cds_valuation_convention,
                )
                .map_err(|e| finstack_quant_core::Error::Calibration {
                    message: format!(
                        "CS01 bucket '{}' down-bump hazard re-calibration failed: {}",
                        label, e
                    ),
                    category: "cs01_rebootstrap".to_string(),
                })?
            } else {
                bump_hazard_shift(hazard_ref, &bump_request_down)?
            };

            scratch.insert_mut(bumped_hazard_up);
            let pv_bumped_up = revalue_raw(scratch)?;
            scratch.insert_mut(std::sync::Arc::clone(&hazard));

            scratch.insert_mut(bumped_hazard_down);
            let pv_bumped_down = revalue_raw(scratch)?;
            scratch.insert_mut(std::sync::Arc::clone(&hazard));

            let cs01 = sensitivity_central_diff(pv_bumped_up, pv_bumped_down, bump_bp);
            series.push((label, cs01));
            total_acc.add(cs01);
        }

        Ok((series, total_acc.total()))
    })?;

    context.store_bucketed_series(series_id, series);
    Ok(total)
}

// ===== Generic Calculators =====

use crate::instruments::common_impl::traits::{CurveDependencies, Instrument};
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
fn resolve_cs01_curves<I: Instrument + CurveDependencies>(
    instrument: &I,
) -> finstack_quant_core::Result<Cs01Curves> {
    let curves = instrument.curve_dependencies()?;
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

fn resolve_optional_cs01_curves<I: Instrument + CurveDependencies>(
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
    I: Instrument + CurveDependencies + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let Some((hazard_id, discount_id)) =
            resolve_optional_cs01_curves(instrument, self.empty_credit_curve_zero, "CS01")?
        else {
            return Ok(0.0);
        };

        let bump_bp =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?
                .credit_spread_bump_bp;

        let reval = cs01_reval(context, true);

        let cs01 = compute_parallel_cs01_with_context_raw(
            context,
            &hazard_id,
            discount_id.as_ref(),
            bump_bp,
            None,
            None,
            reval,
        )?;

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
    I: Instrument + CurveDependencies + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let Some((hazard_id, discount_id)) =
            resolve_optional_cs01_curves(instrument, self.empty_credit_curve_zero, "CS01")?
        else {
            return Ok(0.0);
        };

        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;
        let buckets = defaults.cs01_buckets_years;
        let bump_bp = defaults.credit_spread_bump_bp;

        let reval = cs01_reval(context, true);

        let series_id = MetricId::custom(format!("bucketed_cs01::{}", hazard_id.as_str()));

        let total = compute_key_rate_cs01_series_with_context_raw(
            context,
            &hazard_id,
            discount_id.as_ref(),
            KeyRateCs01Request {
                series_id,
                bucket_times_years: buckets,
                bump_bp,
                doc_clause: None,
                cds_valuation_convention: None,
            },
            reval,
        )?;

        Ok(total)
    }
}

// ===== Hazard-Rate CS01 Calculators =====

/// Generic parallel CS01 calculator using direct hazard-rate bumps.
///
/// Unlike `GenericParallelCs01` which bumps par spreads and re-bootstraps,
/// this directly shifts hazard rates. Registered as `MetricId::Cs01Hazard`.
pub(crate) struct GenericParallelCs01Hazard<I> {
    /// When `true`, an instrument with no credit curve reports CS01 as `0.0`
    /// instead of raising a validation error.
    empty_credit_curve_zero: bool,
    _phantom: PhantomData<I>,
}

impl<I> Default for GenericParallelCs01Hazard<I> {
    fn default() -> Self {
        Self {
            empty_credit_curve_zero: false,
            _phantom: PhantomData,
        }
    }
}

impl<I> GenericParallelCs01Hazard<I> {
    /// Construct a calculator that reports CS01 as `0.0` for instruments with
    /// no credit curve, rather than raising a validation error.
    pub(crate) fn with_empty_credit_curve_zero() -> Self {
        Self {
            empty_credit_curve_zero: true,
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for GenericParallelCs01Hazard<I>
where
    I: Instrument + CurveDependencies + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let Some((hazard_id, _discount_id)) =
            resolve_optional_cs01_curves(instrument, self.empty_credit_curve_zero, "CS01Hazard")?
        else {
            return Ok(0.0);
        };

        let bump_bp =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?
                .credit_spread_bump_bp;

        let curves = Arc::clone(&context.curves);
        let base_ctx = curves.as_ref();
        let hazard = base_ctx.get_hazard(hazard_id.as_str())?;
        let hazard_ref = hazard.as_ref();

        let as_of = context.as_of;

        let bumped_up = bump_hazard_shift(hazard_ref, &BumpRequest::Parallel(bump_bp))?;
        let bumped_down = bump_hazard_shift(hazard_ref, &BumpRequest::Parallel(-bump_bp))?;

        let (pv_up, pv_down) = context.with_market_scratch(|ctx, scratch| {
            scratch.insert_mut(bumped_up);
            let pv_up = ctx.reprice_raw(scratch, as_of)?;
            scratch.insert_mut(std::sync::Arc::clone(&hazard));

            scratch.insert_mut(bumped_down);
            let pv_down = ctx.reprice_raw(scratch, as_of)?;
            scratch.insert_mut(std::sync::Arc::clone(&hazard));

            Ok((pv_up, pv_down))
        })?;

        let cs01 = sensitivity_central_diff(pv_up, pv_down, bump_bp);

        context.computed.insert(
            MetricId::custom(format!("cs01_hazard::{}", hazard_id.as_str())),
            cs01,
        );

        Ok(cs01)
    }
}

/// Generic bucketed CS01 calculator using direct hazard-rate bumps.
///
/// Unlike `GenericBucketedCs01` which bumps par spreads and re-bootstraps,
/// this directly shifts hazard rates at each tenor. Registered as
/// `MetricId::BucketedCs01Hazard`.
pub(crate) struct GenericBucketedCs01Hazard<I> {
    /// When `true`, an instrument with no credit curve reports CS01 as `0.0`
    /// instead of raising a validation error.
    empty_credit_curve_zero: bool,
    _phantom: PhantomData<I>,
}

impl<I> Default for GenericBucketedCs01Hazard<I> {
    fn default() -> Self {
        Self {
            empty_credit_curve_zero: false,
            _phantom: PhantomData,
        }
    }
}

impl<I> GenericBucketedCs01Hazard<I> {
    /// Construct a calculator that reports CS01 as `0.0` for instruments with
    /// no credit curve, rather than raising a validation error.
    pub(crate) fn with_empty_credit_curve_zero() -> Self {
        Self {
            empty_credit_curve_zero: true,
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for GenericBucketedCs01Hazard<I>
where
    I: Instrument + CurveDependencies + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let Some((hazard_id, _discount_id)) = resolve_optional_cs01_curves(
            instrument,
            self.empty_credit_curve_zero,
            "BucketedCs01Hazard",
        )?
        else {
            return Ok(0.0);
        };

        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;
        let buckets = defaults.cs01_buckets_years;
        let bump_bp = defaults.credit_spread_bump_bp;
        validate_buckets_strictly_increasing(&buckets)?;

        let curves = Arc::clone(&context.curves);
        let base_ctx = curves.as_ref();
        let hazard = base_ctx.get_hazard(hazard_id.as_str())?;
        let hazard_ref = hazard.as_ref();

        let as_of = context.as_of;

        let (series, total) = context.with_market_scratch(|ctx, scratch| {
            let mut series: Vec<(std::borrow::Cow<'static, str>, f64)> = Vec::new();
            let mut total_acc = NeumaierAccumulator::new();

            for t in buckets {
                let label = super::config::format_bucket_label_cow(t);

                let bumped_up =
                    bump_hazard_shift(hazard_ref, &BumpRequest::Tenors(vec![(t, bump_bp)]))?;
                let bumped_down =
                    bump_hazard_shift(hazard_ref, &BumpRequest::Tenors(vec![(t, -bump_bp)]))?;

                scratch.insert_mut(bumped_up);
                let pv_up = ctx.reprice_raw(scratch, as_of)?;
                scratch.insert_mut(std::sync::Arc::clone(&hazard));

                scratch.insert_mut(bumped_down);
                let pv_down = ctx.reprice_raw(scratch, as_of)?;
                scratch.insert_mut(std::sync::Arc::clone(&hazard));

                let cs01 = sensitivity_central_diff(pv_up, pv_down, bump_bp);
                series.push((label, cs01));
                total_acc.add(cs01);
            }

            Ok((series, total_acc.total()))
        })?;

        let series_id = MetricId::custom(format!("bucketed_cs01_hazard::{}", hazard_id.as_str()));
        context.store_bucketed_series(series_id, series);

        Ok(total)
    }
}

// ===== Credit-Convention-Aware CS01 Calculators =====

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

    /// Optional replacement market context applied for the duration of the
    /// CS01 compute (e.g. a CDS with a deal-level quote override swaps in a
    /// hazard curve rebuilt from that quote). `None` leaves `context.curves`
    /// unchanged.
    fn cs01_curve_override(
        &self,
        _curves: &MarketContext,
        _hazard_id: &CurveId,
        _as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<MarketContext>> {
        Ok(None)
    }

    /// Whether the reval closure should route through the pricer registry
    /// (`true`, the standard path) or call `Instrument::value_raw` directly
    /// (`false`). CDS options price via their `value` path; routing through
    /// the registry would skip scenario overrides, so they opt out.
    fn cs01_use_pricer_registry(&self) -> bool {
        true
    }
}

/// Build the reval closure used by CS01 calculators, honouring the
/// instrument's [`CdsCs01Conventions::cs01_use_pricer_registry`] preference.
fn cs01_reval(
    context: &MetricContext,
    use_registry: bool,
) -> impl FnMut(&MarketContext) -> finstack_quant_core::Result<f64> {
    let inst_arc = Arc::clone(&context.instrument);
    let (model, registry) = context.clone_pricer_dispatch();
    let as_of = context.as_of;
    move |temp_ctx: &MarketContext| {
        if use_registry {
            if let (Some(model), Some(registry)) = (model, registry.as_ref()) {
                return registry
                    .price_raw(inst_arc.as_ref(), model, temp_ctx, as_of)
                    .map_err(Into::into);
            }
        }
        inst_arc.value_raw(temp_ctx, as_of)
    }
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
    I: Instrument + CurveDependencies + CdsCs01Conventions + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let hazard_id = match resolve_cs01_curves(instrument)? {
            Cs01Curves::Resolved(hazard_id, _discount_id) => hazard_id,
            Cs01Curves::NoCreditCurve => {
                return Err(missing_credit_curve_error(instrument, "CS01"))
            }
        };

        // Re-borrow per step to keep the instrument borrow short-lived.
        if let Some(v) = context
            .instrument_as::<I>()?
            .cs01_precheck(context, &hazard_id)?
        {
            return Ok(v);
        }

        let (doc_clause, valuation_convention) = context
            .instrument_as::<I>()?
            .cs01_bootstrap_convention(context.as_of)?;
        let use_registry = context.instrument_as::<I>()?.cs01_use_pricer_registry();

        let original_curves = Arc::clone(&context.curves);
        if let Some(override_ctx) = context.instrument_as::<I>()?.cs01_curve_override(
            original_curves.as_ref(),
            &hazard_id,
            context.as_of,
        )? {
            context.curves = Arc::new(override_ctx);
        }

        // Resolve the discount curve from whatever context is now active.
        let discount_id = context
            .instrument_as::<I>()?
            .curve_dependencies()?
            .discount_curves
            .first()
            .cloned();

        let bump_bp =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?
                .credit_spread_bump_bp;

        let reval = cs01_reval(context, use_registry);

        let cs01_result = compute_parallel_cs01_with_context_raw(
            context,
            &hazard_id,
            discount_id.as_ref(),
            bump_bp,
            Some(doc_clause),
            Some(valuation_convention),
            reval,
        );
        context.curves = original_curves;
        let cs01 = cs01_result?;

        context.computed.insert(
            MetricId::custom(format!("cs01::{}", hazard_id.as_str())),
            cs01,
        );

        Ok(cs01)
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
    I: Instrument + CurveDependencies + CdsCs01Conventions + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let hazard_id = match resolve_cs01_curves(instrument)? {
            Cs01Curves::Resolved(hazard_id, _discount_id) => hazard_id,
            Cs01Curves::NoCreditCurve => {
                return Err(missing_credit_curve_error(instrument, "bucketed CS01"))
            }
        };

        if let Some(v) = context
            .instrument_as::<I>()?
            .cs01_precheck(context, &hazard_id)?
        {
            return Ok(v);
        }

        let (doc_clause, valuation_convention) = context
            .instrument_as::<I>()?
            .cs01_bootstrap_convention(context.as_of)?;
        let use_registry = context.instrument_as::<I>()?.cs01_use_pricer_registry();

        let original_curves = Arc::clone(&context.curves);
        if let Some(override_ctx) = context.instrument_as::<I>()?.cs01_curve_override(
            original_curves.as_ref(),
            &hazard_id,
            context.as_of,
        )? {
            context.curves = Arc::new(override_ctx);
        }

        let discount_id = context
            .instrument_as::<I>()?
            .curve_dependencies()?
            .discount_curves
            .first()
            .cloned();

        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;
        let buckets = defaults.cs01_buckets_years;
        let bump_bp = defaults.credit_spread_bump_bp;

        let reval = cs01_reval(context, use_registry);

        let series_id = MetricId::custom(format!("bucketed_cs01::{}", hazard_id.as_str()));
        let bucketed_result = compute_key_rate_cs01_series_with_context_raw(
            context,
            &hazard_id,
            discount_id.as_ref(),
            KeyRateCs01Request {
                series_id,
                bucket_times_years: buckets,
                bump_bp,
                doc_clause: Some(doc_clause),
                cds_valuation_convention: Some(valuation_convention),
            },
            reval,
        );
        context.curves = original_curves;
        bucketed_result
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
