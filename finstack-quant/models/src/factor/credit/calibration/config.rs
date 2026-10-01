use finstack_quant_core::dates::{Date, DateExt};
use serde::{Deserialize, Serialize};

use crate::factor::credit::hierarchy::{CreditHierarchySpec, IssuerBetaPolicy};

/// Observation frequency of a complete, regular credit history panel.
///
/// Annualization used for sample/EWMA variance and Ledoit-Wolf covariance
/// is derived from this enum (`252` / `12` / `4`). There is no free
/// annualization float.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PanelFrequency {
    /// One observation per business day (Monday–Friday; market holidays may
    /// be absent). Annualizes with the market-standard 252.
    ///
    /// The grid check accepts any strictly increasing weekday series whose
    /// consecutive gaps are at most 7 calendar days, tolerating weekends and
    /// holiday closures up to a full week. It does not attempt to distinguish
    /// a weekly weekday series from a daily one — the frequency label is a
    /// caller contract, and mislabeling it mis-scales every annualized
    /// variance by the ratio of true to declared periods per year.
    Daily,
    /// One observation per month. Annualizes with 12.
    ///
    /// The regular grid is generated from `dates[0]` with [`DateExt::add_months`].
    /// When the first date is a month-end, subsequent dates stay on month-end
    /// (Jan 31 → Feb 28/29 → Mar 31).
    #[default]
    Monthly,
    /// One observation per quarter (three calendar months). Annualizes with 4.
    ///
    /// Same month-end-preserving step as [`PanelFrequency::Monthly`], advancing
    /// three months at a time.
    Quarterly,
}

impl PanelFrequency {
    /// Periods per year used to annualize per-period variance into bp².
    #[must_use]
    pub const fn annualization_factor(self) -> f64 {
        match self {
            Self::Daily => 252.0,
            Self::Monthly => 12.0,
            Self::Quarterly => 4.0,
        }
    }

    /// Date `steps` periods after `origin` on this frequency's regular grid.
    ///
    /// Monthly and quarterly grids are generated from `origin`, not by
    /// walking one step at a time. That keeps a 28th-of-month series on the
    /// 28th even when February is a month-end, and keeps a month-end origin
    /// on month-end (Jan 31 → Feb 28/29 → Mar 31).
    ///
    /// # Arguments
    ///
    /// * `origin` - First observation on the panel. Daily steps `steps`
    ///   business days (Monday–Friday, no holiday calendar) and requires a
    ///   weekday origin; monthly/quarterly use [`DateExt::add_months`] from
    ///   this origin (`steps * 1` or `steps * 3` months). When `origin` is
    ///   end-of-month, every later date is also end-of-month.
    /// * `steps` - Number of periods after `origin`. Must be non-negative.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when a daily step
    /// overflows the representable date range or a daily `origin` falls on a
    /// weekend.
    pub fn date_after(self, origin: Date, steps: i32) -> finstack_quant_core::Result<Date> {
        if steps < 0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CreditCalibrator: panel step count must be >= 0, got {steps}"
            )));
        }
        match self {
            Self::Daily => {
                if origin.is_weekend() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "CreditCalibrator: daily panel origin {origin:?} falls on a weekend; \
                         business-day grids start on a weekday"
                    )));
                }
                let mut date = origin;
                for _ in 0..steps {
                    loop {
                        date = date.next_day().ok_or_else(|| {
                            finstack_quant_core::Error::Validation(format!(
                                "CreditCalibrator: daily panel overflows the date range after {origin:?}"
                            ))
                        })?;
                        if !date.is_weekend() {
                            break;
                        }
                    }
                }
                Ok(date)
            }
            Self::Monthly => step_months_from_origin(origin, steps),
            Self::Quarterly => step_months_from_origin(
                origin,
                steps.checked_mul(3).ok_or_else(|| {
                    finstack_quant_core::Error::Validation(
                        "quarterly date step exceeds supported range".into(),
                    )
                })?,
            ),
        }
    }
}

fn step_months_from_origin(origin: Date, months: i32) -> finstack_quant_core::Result<Date> {
    let stepped = origin.add_months(months)?;
    Ok(if origin == origin.end_of_month() {
        stepped.end_of_month()
    } else {
        stepped
    })
}

/// Whether the calibrator works in price-difference (return) or raw-level space.
///
/// `Returns` (the default) matches the spec's reference math: `r_i(t) =
/// S_i(t) - S_i(t-1)` and the generic factor is differenced the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PanelSpace {
    /// Difference consecutive observations into a return panel before peeling.
    #[default]
    Returns,
    /// Use the raw level panel as-is.
    Levels,
}

/// Volatility model selector for the per-factor variance forecast.
///
/// `Sample` is the plain (unbiased) sample variance. `Ewma` is the RiskMetrics
/// finite-window exponentially weighted variance estimator (Longerstaey &
/// Spencer, 1996, §5.2): both are fully supported by the calibrator.
///
/// The two differ in centering: `Sample` demeans the series before squaring
/// (the usual `Var(x) = E[(x − x̄)²]`), while `Ewma` does not — it recurses
/// directly on squared observations (`σ²_t = λσ²_{t−1} + (1−λ)r²_{t−1}`),
/// matching the RiskMetrics convention of treating financial return series as
/// zero-mean. Both estimators always operate on factor and adder *moves*:
/// under [`PanelSpace::Levels`] the calibrator first-differences the peeled
/// level series before estimating variance, so the zero-mean convention is
/// sound in either panel space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VolModelChoice {
    /// Plain sample variance (unbiased, Bessel-corrected); demeans before
    /// squaring.
    Sample,
    /// RiskMetrics exponentially weighted moving-average variance.
    ///
    /// Uncentered (zero-mean) squared moves, no demeaning step — see the
    /// enum-level docs above. Implemented by `ewma_variance`. `lambda` must
    /// be in the open interval `(0, 1)`; validated by
    /// `validate_calibration_config`.
    ///
    /// # References
    ///
    /// - Longerstaey, J., & Spencer, M. (1996). *RiskMetrics — Technical
    ///   Document* (4th ed.). J.P. Morgan/Reuters. §5.2. `docs/REFERENCES.md#jpmorgan1996RiskMetrics`
    Ewma {
        /// Smoothing parameter λ ∈ (0, 1) (RiskMetrics daily default 0.94).
        #[cfg_attr(feature = "json-schema", schemars(extend("exclusiveMinimum" = 0.0, "exclusiveMaximum" = 1.0)))]
        lambda: f64,
    },
}

/// Strategy for assembling the factor covariance matrix.
///
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CovarianceStrategy {
    /// Diagonal Σ = diag(σ²) under identity correlation.
    Diagonal,
    /// Sample correlation (PSD-repaired if needed) plus diagonal ridge:
    /// Σ = D·ρ·D + α·I. Requires `alpha >= 0`. See design spec §4.1.
    Ridge {
        /// Ridge regularisation in annualized **bp²**; must be `>= 0`.
        #[cfg_attr(feature = "json-schema", schemars(range(min = 0.0)))]
        alpha: f64,
    },
    /// Full sample covariance with PSD repair via nearest-correlation projection:
    /// Σ = D·ρ_repaired·D. See design spec §4.1.
    FullSampleRepaired,
    /// Ledoit-Wolf (2004) identity-target shrinkage over complete-case
    /// observations: `Σ = periods_per_year · (δ*·μ·I + (1 − δ*)·S)` with
    /// the analytic optimal intensity `δ*`, and `ρ` derived from `Σ`.
    ///
    /// Only dates where **every** factor is observed enter the estimate;
    /// calibration fails with a validation error when fewer than 2 such dates
    /// exist (use [`CovarianceStrategy::Ridge`] or
    /// [`CovarianceStrategy::FullSampleRepaired`] for very sparse panels).
    ///
    /// The resulting `config.covariance` is authoritative for point-in-time
    /// risk but diverges from the vol-forecast rebuild `D·ρ·D` on both the
    /// diagonal and off-diagonal, because `vol_state` variances come from a
    /// different estimator (the configured
    /// [`VolModelChoice`]) over a different observation set (per-factor
    /// all-available rows, not the complete-case rows used here). See
    /// [`CreditFactorModel::static_correlation`][crate::factor::credit::hierarchy::CreditFactorModel::static_correlation]
    /// for the full explanation.
    ///
    /// Reference: Ledoit, O., & Wolf, M. (2004). "A well-conditioned estimator
    /// for large-dimensional covariance matrices." *Journal of Multivariate
    /// Analysis*, 88(2), 365–411.
    LedoitWolf,
}

/// How bucket factor means are weighted across issuers in the bucket.
///
/// DTS (duration-times-spread) is the desk-standard credit-factor weight
/// (Ben Dor / Barclays): it avoids overweighting tight or short-duration
/// names. Position risk exposure remains `β × CS01`; DTS does not replace
/// CS01. [`BucketWeighting::Equal`] is the opt-out for tests and simple
/// equally-weighted books.
///
/// The historical peel weights each date by **contemporaneous** DTS
/// (`SD × begin-of-period spread` in Returns space, `SD × same-date spread`
/// in Levels space), so no as-of information enters historical factor
/// construction. The anchor peel and decomposition use the as-of / current
/// cross-section DTS. Spread durations are a single per-issuer value across
/// the window; duration drift within the window is a documented
/// simplification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BucketWeighting {
    /// Equal weight on every non-folded issuer in the bucket.
    Equal,
    /// Weight `i` by `SD_i (years) × s_i (bp)`, normalized within the bucket.
    #[default]
    Dts,
}

/// OLS β shrinkage rule.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BetaShrinkage {
    /// No shrinkage; use the OLS estimate directly.
    None,
    /// Convex shrinkage toward 1.0: `β ← (1 - α) · β_fit + α · 1.0`.
    TowardOne {
        /// Shrinkage weight in `[0, 1]`.
        #[cfg_attr(feature = "json-schema", schemars(range(min = 0.0, max = 1.0)))]
        alpha: f64,
    },
}

/// Per-level minimum-bucket-size thresholds used to gate fold-up of sparse
/// hierarchy buckets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BucketSizeThresholds {
    /// Threshold per hierarchy level. Levels beyond `per_level.len()` use the
    /// default of 5.
    pub per_level: Vec<usize>,
}

impl BucketSizeThresholds {
    pub(super) fn threshold_for_level(&self, k: usize) -> usize {
        self.per_level.get(k).copied().unwrap_or(5)
    }

    /// Default thresholds for `n` hierarchy levels (5 per level).
    #[must_use]
    pub fn default_for_levels(n: usize) -> Self {
        Self {
            per_level: vec![5; n],
        }
    }
}

/// Configuration for the calibrator.
///
/// Every field has a serde default (see [`Default`]), so a partial JSON
/// object such as `{"hierarchy": {...}}` deserializes with the remaining
/// fields at their defaults. Unknown fields are still rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct CreditCalibrationConfig {
    /// Issuer-beta classification policy.
    pub policy: IssuerBetaPolicy,
    /// Hierarchy specification (broadest → narrowest).
    pub hierarchy: CreditHierarchySpec,
    /// Per-level minimum-bucket-size thresholds.
    pub min_bucket_size_per_level: BucketSizeThresholds,
    /// Vol-model choice for the per-factor variance forecast (sample or EWMA).
    pub vol_model: VolModelChoice,
    /// Covariance assembly strategy.
    pub covariance_strategy: CovarianceStrategy,
    /// Optional shrinkage applied to OLS β estimates.
    pub beta_shrinkage: BetaShrinkage,
    /// Whether to differentiate the panel before peeling.
    pub use_returns_or_levels: PanelSpace,
    /// Regular observation frequency of the history panel.
    ///
    /// Derives the annualization used for variance and Ledoit-Wolf
    /// (`252` / `12` / `4`). The panel dates must form a complete regular
    /// grid of this frequency from `dates[0]`.
    pub panel_frequency: PanelFrequency,
    /// Bucket-mean weighting. Default [`BucketWeighting::Dts`].
    ///
    /// [`BucketWeighting::Dts`] requires [`super::inputs::CreditCalibrationInputs::spread_durations`].
    pub bucket_weighting: BucketWeighting,
}

impl Default for CreditCalibrationConfig {
    fn default() -> Self {
        Self {
            policy: IssuerBetaPolicy::GloballyOff,
            hierarchy: CreditHierarchySpec { levels: vec![] },
            min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![] },
            vol_model: VolModelChoice::Sample,
            // FullSampleRepaired, not Diagonal: an identity-correlation
            // default silently drops cross-factor correlation, understating
            // the vol of a correlated long book by tens of percent. The
            // sample correlation (PSD-repaired when needed) works on sparse
            // panels; opt into Diagonal explicitly for stress-isolated runs.
            covariance_strategy: CovarianceStrategy::FullSampleRepaired,
            beta_shrinkage: BetaShrinkage::None,
            use_returns_or_levels: PanelSpace::Returns,
            panel_frequency: PanelFrequency::Monthly,
            bucket_weighting: BucketWeighting::Dts,
        }
    }
}
