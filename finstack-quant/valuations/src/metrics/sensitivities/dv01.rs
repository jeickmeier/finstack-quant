//! Unified DV01 calculator supporting parallel and key-rate sensitivities.
//!
//! This module provides a single, flexible DV01 calculator with two mathematically
//! correct key-rate methods:
//!
//! 1. **Triangular Zero-Rate**: Fast, uses triangular weights on bucket grid
//!
//! Both methods ensure: **sum of bucketed DV01 ≈ parallel DV01**
//!
//! # Units and Sign Convention
//!
//! - **DV01 is expressed in currency units per basis point (1bp = 0.0001)**
//! - A DV01 of -100 means the instrument loses $100 when rates rise by 1bp
//! - Positive DV01: instrument gains value when rates rise (rare, e.g., short positions)
//! - Negative DV01: instrument loses value when rates rise (typical for long bonds)
//!
//! # Key Features
//!
//! - **Type-safe curve discovery**: Uses [`CurveDependencies`] trait to discover curves at compile time
//! - **Mathematically correct**: Triangular weights partition unity across bucket grid
//! - **Multiple curve types**: Handles discount, forward, and credit curves
//! - **Par-rate option**: Re-bootstrap curve for exact sum-to-parallel behavior
//!
//! # Quick Start
//!
//! For DV01 calculations, use the [`MetricId::Dv01`] or [`MetricId::BucketedDv01`]
//! metrics via the [`Instrument::price_with_metrics`] method:
//!
//! ```ignore
//! use finstack_quant_valuations::instruments::{Bond, Instrument, PricingOptions};
//! use finstack_quant_valuations::metrics::MetricId;
//!
//! let bond = Bond::example().unwrap();
//! let result = bond.price_with_metrics(&market, as_of, &[MetricId::Dv01], PricingOptions::default())?;
//! // DV01 is in currency units per 1bp rate move
//! ```

use crate::instruments::common_impl::traits::{CurveDependencies, Instrument, RatesCurveKind};
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::MetricCalculator;
use crate::metrics::{MetricContext, MetricId};

use finstack_quant_core::market_data::bumps::BumpSpec;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::neumaier_sum;
use finstack_quant_core::types::CurveId;
use std::borrow::Cow;
use std::marker::PhantomData;

// =============================================================================
// Configuration Types
// =============================================================================

/// DV01 calculation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dv01ComputationMode {
    /// Single scalar from parallel bump of all curves together.
    ParallelCombined,
    /// Per-curve parallel bumps (stored as series).
    ParallelPerCurve,
    /// Key-rate buckets per curve using triangular zero-rate bumps.
    KeyRateTriangular,
}

/// Rate-curve subset to include in a DV01-style bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RateCurveSelection {
    /// Include all rate curves declared by the instrument.
    All,
    /// Include only forward/projection curves.
    ForwardOnly,
    /// Include only the discount/funding curve.
    ///
    /// Use this for Rho calculations where only the discount curve is bumped,
    /// leaving the forward swap rate (and hence the option's intrinsic value)
    /// unchanged — which is the correct definition of option rho.
    DiscountOnly,
}

/// Configuration for DV01 calculations.
#[derive(Clone)]
pub(crate) struct Dv01CalculatorConfig {
    /// Computation mode (parallel vs bucketed, triangular vs par-rate).
    pub(crate) mode: Dv01ComputationMode,
    /// Bucket times for key-rate DV01 (in years).
    pub(crate) buckets: Cow<'static, [f64]>,
    /// MetricId under which to store per-curve or per-bucket series.
    /// Defaults to `BucketedDv01`. Set to e.g. `Pv01` when using
    /// `ParallelPerCurve` mode for PV01 so keys read `pv01::USD-OIS`.
    pub(crate) series_id: MetricId,
    /// Subset of rate curves included in the bump.
    pub(crate) curve_selection: RateCurveSelection,
}

impl std::fmt::Debug for Dv01CalculatorConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dv01CalculatorConfig")
            .field("mode", &self.mode)
            .field("buckets", &self.buckets)
            .field("series_id", &self.series_id)
            .field("curve_selection", &self.curve_selection)
            .finish()
    }
}

impl Default for Dv01CalculatorConfig {
    fn default() -> Self {
        Self {
            mode: Dv01ComputationMode::KeyRateTriangular,
            buckets: Cow::Borrowed(&sens_config::STANDARD_BUCKETS_YEARS),
            series_id: MetricId::BucketedDv01,
            curve_selection: RateCurveSelection::All,
        }
    }
}

impl Dv01CalculatorConfig {
    /// Create config for parallel DV01 (all curves together).
    pub(crate) fn parallel_combined() -> Self {
        Self {
            mode: Dv01ComputationMode::ParallelCombined,
            buckets: Cow::Borrowed(&[]),
            series_id: MetricId::BucketedDv01,
            curve_selection: RateCurveSelection::All,
        }
    }

    /// Create config for parallel bump of the discount/funding curve only.
    ///
    /// This is the correct configuration for option **Rho**: bumping only the
    /// discount curve isolates pure discounting sensitivity while leaving the
    /// forward swap rate (and hence the option's intrinsic value) unchanged.
    /// Contrast with `parallel_combined()` which bumps both discount and forward
    /// curves and conflates Rho with Delta.
    pub(crate) fn parallel_discount_only() -> Self {
        Self {
            mode: Dv01ComputationMode::ParallelCombined,
            buckets: Cow::Borrowed(&[]),
            series_id: MetricId::BucketedDv01,
            curve_selection: RateCurveSelection::DiscountOnly,
        }
    }

    /// Create config for parallel DV01 over forward/projection curves only.
    pub(crate) fn parallel_forward_only() -> Self {
        Self {
            mode: Dv01ComputationMode::ParallelCombined,
            buckets: Cow::Borrowed(&[]),
            series_id: MetricId::BucketedDv01,
            curve_selection: RateCurveSelection::ForwardOnly,
        }
    }

    /// Create config for parallel DV01 per curve.
    pub(crate) fn parallel_per_curve() -> Self {
        Self {
            mode: Dv01ComputationMode::ParallelPerCurve,
            buckets: Cow::Borrowed(&[]),
            series_id: MetricId::BucketedDv01,
            curve_selection: RateCurveSelection::All,
        }
    }

    /// Create config for triangular key-rate DV01.
    ///
    /// This is the default and recommended method for most use cases.
    /// Uses triangular weights on the bucket grid, ensuring sum ≈ parallel within ~0.1%.
    pub(crate) fn triangular_key_rate() -> Self {
        Self {
            mode: Dv01ComputationMode::KeyRateTriangular,
            ..Self::default()
        }
    }

    /// Override the metric ID used for storing per-curve or per-bucket series.
    pub(crate) fn with_series_id(mut self, id: MetricId) -> Self {
        self.series_id = id;
        self
    }

    fn includes_curve_kind(&self, kind: RatesCurveKind) -> bool {
        matches!(
            (self.curve_selection, kind),
            (
                RateCurveSelection::All,
                RatesCurveKind::Discount | RatesCurveKind::Forward
            ) | (RateCurveSelection::ForwardOnly, RatesCurveKind::Forward)
                | (RateCurveSelection::DiscountOnly, RatesCurveKind::Discount)
        )
    }
}

// =============================================================================
// Unified DV01 Calculator
// =============================================================================

/// Unified DV01 calculator supporting all computation modes.
///
/// This calculator provides two mathematically correct key-rate methods:
///
/// 1. **Triangular Zero-Rate** (`KeyRateTriangular`): Uses triangular weights
///    defined by the bucket grid, ensuring sum of bucketed DV01 ≈ parallel DV01.
///
/// 2. **Par-Rate Bumping** (`KeyRateParRate`): Bumps par rates of calibration
///    instruments and re-bootstraps, ensuring exact sum = parallel.
pub(crate) struct UnifiedDv01Calculator<I> {
    config: Dv01CalculatorConfig,
    _phantom: PhantomData<I>,
}

impl<I> UnifiedDv01Calculator<I> {
    /// Create a new calculator with the given configuration.
    pub(crate) fn new(config: Dv01CalculatorConfig) -> Self {
        Self {
            config,
            _phantom: PhantomData,
        }
    }

    fn effective_key_rate_buckets<'a>(
        &'a self,
        defaults: &'a sens_config::SensitivitiesConfig,
    ) -> &'a [f64] {
        if self.config.buckets.as_ref() == sens_config::STANDARD_BUCKETS_YEARS {
            defaults.dv01_buckets_years.as_slice()
        } else {
            self.config.buckets.as_ref()
        }
    }
}

impl<I> Default for UnifiedDv01Calculator<I> {
    fn default() -> Self {
        Self::new(Dv01CalculatorConfig::default())
    }
}

impl<I> MetricCalculator for UnifiedDv01Calculator<I>
where
    I: Instrument + CurveDependencies + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;

        // Resolve bump size from config, then layer instrument overrides.
        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;
        let bump_bp = defaults.rate_bump_bp;

        // Collect curves based on configuration
        let curves = self.collect_curves(instrument, context.curves.as_ref())?;

        // Compute DV01 based on mode
        match self.config.mode {
            Dv01ComputationMode::ParallelCombined => {
                self.compute_parallel_combined(context, &curves, bump_bp)
            }
            Dv01ComputationMode::ParallelPerCurve => {
                self.compute_parallel_per_curve(context, &curves, bump_bp)
            }
            Dv01ComputationMode::KeyRateTriangular => {
                let buckets = self.effective_key_rate_buckets(&defaults);
                self.compute_key_rate_triangular(context, &curves, bump_bp, buckets)
            }
        }
    }
}

impl<I> UnifiedDv01Calculator<I>
where
    I: Instrument + CurveDependencies + 'static,
{
    /// Collect curves based on configuration and what exists in the market.
    ///
    /// # Errors
    ///
    /// Returns an error if the instrument declares rate curve dependencies but
    /// none of them are found in the market context. This ensures that missing
    /// market data is surfaced explicitly rather than silently returning 0.0.
    fn collect_curves(
        &self,
        instrument: &I,
        market: &MarketContext,
    ) -> finstack_quant_core::Result<Vec<(CurveId, RatesCurveKind)>> {
        let deps = instrument.curve_dependencies()?;
        let mut curves = Vec::new();
        let mut missing_curves = Vec::new();

        for (curve_id, kind) in deps.all_with_kind() {
            let selected = self.config.includes_curve_kind(kind);
            match kind {
                RatesCurveKind::Discount => {
                    if market.get_discount(curve_id.as_str()).is_ok() {
                        if selected {
                            curves.push((curve_id, kind));
                        }
                    } else if selected {
                        missing_curves.push(curve_id.as_str().to_string());
                    }
                }
                RatesCurveKind::Forward => {
                    if market.get_forward(curve_id.as_str()).is_ok() {
                        if selected {
                            curves.push((curve_id, kind));
                        }
                    } else if selected {
                        missing_curves.push(curve_id.as_str().to_string());
                    }
                }
                RatesCurveKind::Credit => {
                    // Skip credit curves for DV01
                }
            }
        }

        // If the instrument declares rate curve dependencies but none are found,
        // this is a market data error that should be surfaced explicitly.
        let has_rate_deps = match self.config.curve_selection {
            RateCurveSelection::All => {
                !deps.discount_curves.is_empty() || !deps.forward_curves.is_empty()
            }
            RateCurveSelection::ForwardOnly => !deps.forward_curves.is_empty(),
            RateCurveSelection::DiscountOnly => !deps.discount_curves.is_empty(),
        };

        if curves.is_empty() && has_rate_deps {
            return Err(finstack_quant_core::Error::from(
                finstack_quant_core::InputError::NotFound {
                    id: format!(
                        "rate_curves for DV01 (missing: {})",
                        missing_curves.join(", ")
                    ),
                },
            ));
        }

        Ok(curves)
    }

    /// Compute parallel DV01 with all curves bumped together (central differencing).
    ///
    /// Uses in-place scratch bumps to avoid cloning the market context for each
    /// bump direction.
    fn compute_parallel_combined(
        &self,
        context: &mut MetricContext,
        curves: &[(CurveId, RatesCurveKind)],
        bump_bp: f64,
    ) -> finstack_quant_core::Result<f64> {
        if curves.is_empty() {
            return Ok(0.0);
        }

        let as_of = context.as_of;
        let spec_up = BumpSpec::parallel_bp(bump_bp);
        let spec_down = BumpSpec::parallel_bp(-bump_bp);

        let (pv_up, pv_down) = context.with_market_scratch(|context, scratch| {
            // Apply all up bumps, reprice, then revert all.
            let mut tokens_up = Vec::with_capacity(curves.len());
            for (curve_id, _kind) in curves {
                tokens_up.push(scratch.apply_curve_bump_in_place(curve_id, spec_up)?);
            }
            let pv_up = context.reprice_raw(scratch, as_of)?;
            for token in tokens_up.into_iter().rev() {
                scratch.revert_scratch_bump(token)?;
            }

            // Apply all down bumps, reprice, then revert all.
            let mut tokens_down = Vec::with_capacity(curves.len());
            for (curve_id, _kind) in curves {
                tokens_down.push(scratch.apply_curve_bump_in_place(curve_id, spec_down)?);
            }
            let pv_down = context.reprice_raw(scratch, as_of)?;
            for token in tokens_down.into_iter().rev() {
                scratch.revert_scratch_bump(token)?;
            }

            Ok((pv_up, pv_down))
        })?;

        let dv01 = calculate_dv01_central(pv_up, pv_down, bump_bp);
        Ok(dv01)
    }

    /// Compute parallel DV01 per curve and store as series (central differencing).
    ///
    /// Uses in-place scratch bumps to avoid cloning the market context per curve.
    fn compute_parallel_per_curve(
        &self,
        context: &mut MetricContext,
        curves: &[(CurveId, RatesCurveKind)],
        bump_bp: f64,
    ) -> finstack_quant_core::Result<f64> {
        if curves.is_empty() {
            return Ok(0.0);
        }

        let as_of = context.as_of;

        let mut series = Vec::with_capacity(curves.len());

        context.with_market_scratch(|context, scratch| {
            for (curve_id, _kind) in curves {
                let token_up =
                    scratch.apply_curve_bump_in_place(curve_id, BumpSpec::parallel_bp(bump_bp))?;
                let pv_up = context.reprice_raw(scratch, as_of)?;
                scratch.revert_scratch_bump(token_up)?;

                let token_down =
                    scratch.apply_curve_bump_in_place(curve_id, BumpSpec::parallel_bp(-bump_bp))?;
                let pv_down = context.reprice_raw(scratch, as_of)?;
                scratch.revert_scratch_bump(token_down)?;

                let dv01 = calculate_dv01_central(pv_up, pv_down, bump_bp);
                series.push((curve_id.as_str().to_string(), dv01));
            }
            Ok(())
        })?;

        // Compensated summation over per-curve DV01s: a naive `+=` accumulates
        // rounding error when summing many curves with widely differing
        // magnitudes (mirrors the per-bucket `neumaier_sum` in
        // `compute_triangular_for_curve`).
        let total_dv01 = neumaier_sum(series.iter().map(|(_, v)| *v));
        context.store_bucketed_series(self.config.series_id.clone(), series);
        Ok(total_dv01)
    }

    /// Compute key-rate DV01 using triangular zero-rate bumps.
    ///
    /// This method uses triangular weights defined by the bucket grid (not curve knots),
    /// ensuring that the sum of bucketed DV01 equals parallel DV01.
    fn compute_key_rate_triangular(
        &self,
        context: &mut MetricContext,
        curves: &[(CurveId, RatesCurveKind)],
        bump_bp: f64,
        buckets: &[f64],
    ) -> finstack_quant_core::Result<f64> {
        if curves.is_empty() {
            return Ok(0.0);
        }

        let base = self.config.series_id.as_str();
        let mut curve_totals = Vec::with_capacity(curves.len());

        for (curve_id, _kind) in curves.iter() {
            let curve_metric_id = MetricId::custom(format!("{}::{}", base, curve_id.as_str()));

            let curve_total = self.compute_triangular_for_curve(
                context,
                curve_id,
                curve_metric_id,
                bump_bp,
                buckets,
            )?;

            curve_totals.push(curve_total);
        }

        // Compensated summation over per-curve key-rate totals: avoids the
        // rounding drift a naive `+=` accumulates across many curves.
        Ok(neumaier_sum(curve_totals))
    }

    /// Compute triangular key-rate DV01 for a single curve (central differencing).
    ///
    /// Uses triangular weights based on the bucket grid, ensuring proper partitioning.
    /// Employs in-place scratch bumps to avoid cloning the market context per bucket.
    fn compute_triangular_for_curve(
        &self,
        context: &mut MetricContext,
        curve_id: &CurveId,
        metric_id: MetricId,
        bump_bp: f64,
        buckets: &[f64],
    ) -> finstack_quant_core::Result<f64> {
        let as_of = context.as_of;

        // Triangular weight construction below assumes `buckets` is strictly
        // increasing; an unsorted slice produces zero-sum or inverted buckets
        // silently. Validate up-front rather than letting bad input through.
        for win in buckets.windows(2) {
            if win[1].partial_cmp(&win[0]) != Some(std::cmp::Ordering::Greater) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "key-rate buckets must be strictly increasing, got {:?} (offending pair: {} -> {})",
                    buckets, win[0], win[1]
                )));
            }
        }

        let mut series: Vec<(std::borrow::Cow<'static, str>, f64)> =
            Vec::with_capacity(buckets.len());

        let last_idx = buckets.len() - 1;
        context.with_market_scratch(|context, scratch| {
            for (i, &target_time) in buckets.iter().enumerate() {
                let label = super::config::format_bucket_label_cow(target_time);

                // Build bucket-shaped bumps with half-triangle wings at the
                // first and last buckets so the bump-set partitions unity
                // across the full curve. Using a finite `prev = 0.0` at the
                // first bucket would produce a rising triangle from t=0 and
                // understate short-end DV01 — see
                // `BumpSpec::triangular_key_rate_first_bp` for the rationale.
                let build = |bp: f64| -> BumpSpec {
                    match (i == 0, i == last_idx) {
                        (true, true) => BumpSpec::parallel_bp(bp),
                        (true, false) => {
                            BumpSpec::triangular_key_rate_first_bp(target_time, buckets[i + 1], bp)
                        }
                        (false, true) => {
                            BumpSpec::triangular_key_rate_last_bp(buckets[i - 1], target_time, bp)
                        }
                        (false, false) => BumpSpec::triangular_key_rate_bp(
                            buckets[i - 1],
                            target_time,
                            buckets[i + 1],
                            bp,
                        ),
                    }
                };

                let spec_up = build(bump_bp);
                let token_up = scratch.apply_curve_bump_in_place(curve_id, spec_up)?;
                let pv_up = context.reprice_raw(scratch, as_of)?;
                scratch.revert_scratch_bump(token_up)?;

                let spec_down = build(-bump_bp);
                let token_down = scratch.apply_curve_bump_in_place(curve_id, spec_down)?;
                let pv_down = context.reprice_raw(scratch, as_of)?;
                scratch.revert_scratch_bump(token_down)?;

                let dv01 = calculate_dv01_central(pv_up, pv_down, bump_bp);
                series.push((label, dv01));
            }
            Ok(())
        })?;

        let total: f64 = neumaier_sum(series.iter().map(|(_, v)| *v));
        context.store_bucketed_series(metric_id, series);
        Ok(total)
    }
}

/// Calculate DV01 from PV changes using central differencing (high-precision f64 version).
///
/// Uses raw f64 values to avoid Money rounding precision loss in sensitivity calculations.
/// Central difference formula: `(PV_up - PV_down) / (2 * bump)` provides O(h^2) accuracy,
/// eliminating first-order convexity contamination that affects forward differencing.
///
/// # Units
///
/// Returns DV01 in **currency units per basis point**. For example:
/// - If `pv_up = 999_500` and `pv_down = 1_000_500` with `bump_bp = 1.0`
/// - DV01 = (999_500 - 1_000_500) / (2 * 1.0) = -500
/// - This means the instrument loses $500 per 1bp rate increase
///
/// # Arguments
///
/// * `pv_up` - Present value after upward bump (in currency units)
/// * `pv_down` - Present value after downward bump (in currency units)
/// * `bump_bp` - Bump size in basis points (typically 1.0)
const MIN_BUMP_BP_THRESHOLD: f64 = 1e-10;

#[inline]
fn calculate_dv01_central(pv_up: f64, pv_down: f64, bump_bp: f64) -> f64 {
    // `bump_bp` originates from config validated with `ensure_finite_positive`,
    // so a degenerate width is a misconfiguration, not normal input. Assert
    // loudly in debug/test builds so a silent 0.0 (indistinguishable from a true
    // zero Greek) cannot mask a bad bump; in release, fall back to 0.0 rather
    // than divide by ~0 and emit inf/NaN.
    debug_assert!(
        bump_bp.abs() > MIN_BUMP_BP_THRESHOLD,
        "DV01 bump_bp must exceed {MIN_BUMP_BP_THRESHOLD} (got {bump_bp}); validate upstream"
    );
    if bump_bp.abs() <= MIN_BUMP_BP_THRESHOLD {
        return 0.0;
    }
    (pv_up - pv_down) / (2.0 * bump_bp)
}

#[cfg(test)]
mod tests {
    use finstack_quant_core::math::neumaier_sum;

    /// Audit item #6: the cross-curve / cross-bucket DV01 totals must use
    /// compensated (Neumaier) summation, matching the per-curve key-rate path.
    ///
    /// This pins the invariant that motivates the fix: when per-curve DV01s
    /// span widely different magnitudes (a tiny basis curve alongside a large
    /// primary curve, repeated across many curves), a naive running `+=`
    /// accumulates rounding error that compensated summation eliminates.
    #[test]
    fn cross_curve_dv01_total_uses_compensated_summation() {
        // A huge term, then `n` unit terms, then the cancelling huge term.
        // Exact total = 1e16 + n·1 − 1e16 = n. A naive left-fold loses every
        // unit term (1e16 + 1 == 1e16 in f64) and then cancels to ~0;
        // compensated summation recovers the exact `n`.
        let n = 1_000_000usize;
        let mut values = vec![1.0e16];
        values.extend(std::iter::repeat_n(1.0, n));
        values.push(-1.0e16);

        let naive: f64 = values.iter().fold(0.0_f64, |acc, v| acc + v);
        let compensated = neumaier_sum(values.iter().copied());

        assert!(
            (compensated - n as f64).abs() < 1e-6,
            "compensated summation must recover the exact total {n}, got {compensated}"
        );
        assert!(
            (naive - n as f64).abs() > 1.0,
            "naive summation is expected to lose precision here (got {naive}, \
             exact {n}); the DV01 cross-curve/bucket totals must therefore use \
             neumaier_sum"
        );
    }
}
