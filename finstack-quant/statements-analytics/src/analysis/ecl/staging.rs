//! IFRS 9 stage classification engine.
//!
//! Implements the waterfall logic for assigning exposures to impairment stages
//! based on quantitative triggers (PD delta, DPD backstops) and qualitative
//! flags (watchlist, forbearance, adverse conditions).
//!
//! The classification waterfall is:
//! 1. Stage 3 DPD backstop: DPD >= `dpd_stage3_threshold` (absolute, non-rebuttable)
//! 2. Stage 3 qualitative default evidence (IFRS 9 B5.5.37 "unlikely to
//!    pay"): bankruptcy / distressed modification / cross-default / user-
//!    supplied default-evidence flags.
//! 3. Stage 2 quantitative: PD delta exceeds thresholds (absolute OR relative),
//!    OR rating downgrade exceeds the configured notch threshold (IFRS 9
//!    B5.5.17(f))
//! 4. Stage 2 qualitative SICR flags (IFRS 9 B5.5.17)
//! 5. Stage 2 DPD backstop: DPD >= `dpd_stage2_threshold` (rebuttable presumption)
//! 6. Curing: If previously Stage 2/3 and now meets cure criteria, allow step-down
//! 7. Default: Stage 1
//!
//! # Comparator convention
//!
//! Stage 2/3 DPD backstops fire when `days_past_due >= threshold`. Defaults
//! are 30 / 90, so 30 DPD is Stage 2 and 90 DPD is Stage 3. This matches
//! CECL's impaired-DPD test and common bank / Basel IRB practice. IFRS 9
//! B5.5.37's "more than 30 / 90 days" wording is not applied literally.
//!
//! # References
//!
//! - IFRS 9 Financial Instruments, Section 5.5 -- Impairment `docs/REFERENCES.md#ifrs-9-impairment`
//! - IFRS 9 B5.5.17 -- Qualitative indicators of SICR `docs/REFERENCES.md#ifrs-9-impairment`
//! - IFRS 9 B5.5.19 -- 30 days past due rebuttable presumption `docs/REFERENCES.md#ifrs-9-impairment`
//! - IFRS 9 B5.5.37 -- 90 days past due default presumption `docs/REFERENCES.md#ifrs-9-impairment`

use finstack_quant_core::Result;
use finstack_quant_models::credit::migration::RatingScale;
use serde::{Deserialize, Serialize};

use super::types::{Exposure, PdTermStructure, Stage};

/// Cap on the horizon used for SICR lifetime-PD comparison, in years.
///
/// Bounds the PD-delta lookup for very long-dated exposures so the
/// comparison stays within the range where PD curves are typically
/// calibrated. The effective SICR horizon is
/// `min(remaining_maturity_years, MAX_SICR_HORIZON_YEARS)` — SICR is never
/// assessed beyond the exposure's own remaining maturity.
pub const MAX_SICR_HORIZON_YEARS: f64 = 30.0;

/// Reason for stage assignment (audit trail).
///
/// Each variant captures the specific values that triggered the classification,
/// enabling regulatory reporting and model governance review.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum StagingTrigger {
    /// DPD exceeded Stage 3 threshold.
    DpdStage3 {
        /// Actual days past due.
        dpd: u32,
        /// Configured threshold.
        threshold: u32,
    },
    /// Objective evidence of default (IFRS 9 B5.5.37 "unlikely to pay").
    Stage3Qualitative {
        /// Name of the default-evidence flag (e.g. `bankruptcy`,
        /// `distressed_modification`, `cross_default`, or a user
        /// supplied key from `other_default_evidence`).
        flag: String,
    },
    /// Absolute PD increase exceeded threshold.
    PdDeltaAbsolute {
        /// Actual PD delta (current minus origination).
        delta: f64,
        /// Configured threshold.
        threshold: f64,
    },
    /// Relative PD increase exceeded threshold.
    PdDeltaRelative {
        /// Actual ratio (current PD / origination PD).
        ratio: f64,
        /// Configured threshold.
        threshold: f64,
    },
    /// Rating downgraded by N or more notches (IFRS 9 B5.5.17(f): external
    /// credit rating downgrade as a SICR indicator).
    ///
    /// Notches are counted as `position(current) - position(origination)`
    /// on the configured rating scale (see
    /// [`StagingConfig::rating_scale_labels`]); a value of `notches >=
    /// threshold` fires this trigger.
    RatingDowngrade {
        /// Actual downgrade notches.
        notches: u32,
        /// Configured threshold.
        threshold: u32,
    },
    /// DPD exceeded Stage 2 backstop.
    DpdStage2 {
        /// Actual days past due.
        dpd: u32,
        /// Configured threshold.
        threshold: u32,
    },
    /// Qualitative flag active.
    Qualitative {
        /// Name of the active flag.
        flag: String,
    },
    /// No triggers active: Stage 1 default.
    NoTrigger,
}

impl core::fmt::Display for StagingTrigger {
    /// Render the trigger as a short human-readable reason string.
    ///
    /// This is the canonical trigger-reason wire text consumed by the host
    /// bindings (e.g. Python `classify_stage`); treat the exact format as a
    /// stable contract.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StagingTrigger::DpdStage3 { dpd, threshold } => {
                write!(f, "dpd_stage3 (dpd={} >= {})", dpd, threshold)
            }
            StagingTrigger::DpdStage2 { dpd, threshold } => {
                write!(f, "dpd_stage2 (dpd={} >= {})", dpd, threshold)
            }
            StagingTrigger::PdDeltaAbsolute { delta, threshold } => {
                write!(
                    f,
                    "pd_delta_absolute (delta={:.4} > {:.4})",
                    delta, threshold
                )
            }
            StagingTrigger::PdDeltaRelative { ratio, threshold } => {
                write!(
                    f,
                    "pd_delta_relative (ratio={:.2}x > {:.2}x)",
                    ratio, threshold
                )
            }
            StagingTrigger::RatingDowngrade { notches, threshold } => {
                write!(f, "rating_downgrade ({} >= {} notches)", notches, threshold)
            }
            StagingTrigger::Qualitative { flag } => write!(f, "qualitative:{}", flag),
            StagingTrigger::Stage3Qualitative { flag } => {
                write!(f, "stage3_qualitative:{}", flag)
            }
            StagingTrigger::NoTrigger => write!(f, "no_trigger"),
        }
    }
}

/// Result of stage classification with audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResult {
    /// Assigned stage.
    pub stage: Stage,
    /// Which trigger(s) caused the classification.
    pub triggers: Vec<StagingTrigger>,
    /// Whether the exposure was cured from a higher stage.
    pub cured: bool,
}

/// Configuration for IFRS 9 stage classification.
///
/// Controls the quantitative and qualitative thresholds used in the staging
/// waterfall, plus curing rules for step-down from higher stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagingConfig {
    /// Absolute PD increase threshold for SICR (e.g., 0.01 = 1 pp).
    /// If the lifetime PD has increased by more than this amount since
    /// origination, the exposure is classified as Stage 2.
    pub pd_delta_absolute: f64,

    /// Relative PD increase threshold for SICR (e.g., 2.0 = PD doubled).
    /// Applied as: current_pd / origination_pd > threshold.
    pub pd_delta_relative: f64,

    /// Rating downgrade notches that trigger Stage 2 (IFRS 9 B5.5.17(f):
    /// external credit rating downgrade as a SICR indicator).
    ///
    /// [`classify_stage`] fires [`StagingTrigger::RatingDowngrade`] when
    /// `position(current) - position(origination) >= rating_downgrade_notches`
    /// on the configured scale (see [`Self::rating_scale_labels`]). Example:
    /// 3 means a 3-notch downgrade from origination triggers SICR. A `0`
    /// threshold disables the trigger entirely.
    pub rating_downgrade_notches: u32,

    /// Ordered rating-scale labels (best -> worst) used to count downgrade
    /// notches for the `rating_downgrade_notches` trigger. When `None`, the
    /// core 10-state S&P/Fitch scale (AAA, AA, A, BBB, BB, B, CCC, CC, C, D)
    /// is used. Ratings not present on the scale never fire the trigger.
    #[serde(default)]
    pub rating_scale_labels: Option<Vec<String>>,

    /// DPD threshold for Stage 2 backstop (IFRS 9 B5.5.19 rebuttable
    /// presumption). Default: 30.
    pub dpd_stage2_threshold: u32,

    /// DPD threshold for Stage 3 (absolute backstop). Default: 90.
    pub dpd_stage3_threshold: u32,

    /// Whether any qualitative SICR flag triggers Stage 2.
    pub qualitative_triggers_enabled: bool,

    /// Whether objective default-evidence flags (bankruptcy,
    /// distressed modification, cross-default,
    /// `other_default_evidence`) trigger Stage 3 classification,
    /// independently of the 90 DPD backstop. Defaults to `true`.
    ///
    /// Disable only when such triggers are controlled by an upstream
    /// credit-event engine that already mapped them into DPD or
    /// previous-stage state.
    pub stage3_qualitative_triggers_enabled: bool,

    /// Curing configuration: minimum consecutive performing periods
    /// required to move from Stage 2 back to Stage 1. Default: 3.
    ///
    /// The default is a pragmatic 3-period observation window; EBA
    /// GL on default uses a 3-month probation for Stage 2 → 1 (see
    /// EBA/GL/2016/07 §32).
    pub cure_periods_stage2_to_1: u32,

    /// Curing configuration: minimum consecutive performing periods
    /// required to move from Stage 3 to Stage 2. Default: 12.
    ///
    /// The default matches the EBA GL on default probation window of
    /// 12 months for cure out of default (EBA/GL/2016/07 §71–§72). If
    /// your internal policy uses a shorter probation, override this
    /// field explicitly.
    pub cure_periods_stage3_to_2: u32,
}

/// Count downgrade notches between two rating labels on the configured scale.
///
/// Returns `None` when either label is absent from the scale (the trigger is
/// skipped rather than erroring, so custom label sets remain usable without a
/// configured scale) or when the move is not a downgrade. The default scale
/// is core's 10-state S&P/Fitch [`RatingScale::standard`] label list.
fn rating_downgrade_notches(orig: &str, curr: &str, config: &StagingConfig) -> Option<u32> {
    let default_labels;
    let labels: &[String] = match &config.rating_scale_labels {
        Some(labels) => labels.as_slice(),
        None => {
            default_labels = RatingScale::standard().labels().to_vec();
            &default_labels
        }
    };
    let orig_idx = labels.iter().position(|l| l == orig)?;
    let curr_idx = labels.iter().position(|l| l == curr)?;
    (curr_idx > orig_idx).then(|| (curr_idx - orig_idx) as u32)
}

/// Classify an exposure into an IFRS 9 stage.
///
/// The classification waterfall evaluates triggers in priority order:
///
/// 1. **Stage 3 backstop**: DPD is at or above `dpd_stage3_threshold`
///    (non-rebuttable)
/// 2. **Stage 2 quantitative**: PD delta exceeds absolute OR relative
///    threshold, or a rating downgrade exceeds `rating_downgrade_notches`
///    (IFRS 9 B5.5.17(f))
/// 3. **Stage 2 qualitative**: Any qualitative flag is active (if enabled)
/// 4. **Stage 2 DPD backstop**: DPD is at or above `dpd_stage2_threshold`
/// 5. **Curing**: Previous stage was higher, sufficient performing periods
/// 6. **Default**: Stage 1 (no triggers active)
///
/// # Arguments
///
/// * `exposure` - Credit exposure with DPD, ratings, qualitative flags, and
///   prior-stage information used by the classification waterfall.
/// * `pd_source` - PD term structure used to compare origination and current
///   lifetime default risk when SICR testing is required.
/// * `config` - Staging policy thresholds, backstops, and curing conditions.
///
/// # Returns
///
/// A [`StageResult`] containing the assigned stage, trigger reasons, and
/// whether curing was applied.
///
/// # Errors
///
/// Propagates PD-term-structure lookup errors when both origination and
/// current ratings are present on the PD source and the SICR comparison
/// requires cumulative probabilities. A rating that is absent from the
/// source skips the PD-delta test (same as an unknown notch label) and
/// does not fail the run.
pub fn classify_stage(
    exposure: &Exposure,
    pd_source: &dyn PdTermStructure,
    config: &StagingConfig,
) -> Result<StageResult> {
    classify_stage_with_pds(exposure, config, || {
        let (Some(orig), Some(curr)) = (&exposure.origination_rating, &exposure.current_rating)
        else {
            return Ok(None);
        };
        if !pd_source.contains_rating(orig) || !pd_source.contains_rating(curr) {
            return Ok(None);
        }
        let horizon = exposure
            .remaining_maturity_years
            .min(MAX_SICR_HORIZON_YEARS);
        Ok(Some((
            pd_source.cumulative_pd(orig, horizon)?,
            pd_source.cumulative_pd(curr, horizon)?,
        )))
    })
}

pub(super) fn classify_stage_with_pds(
    exposure: &Exposure,
    config: &StagingConfig,
    pds: impl FnOnce() -> Result<Option<(f64, f64)>>,
) -> Result<StageResult> {
    if !exposure.remaining_maturity_years.is_finite()
        || exposure.remaining_maturity_years < 0.0
        || !config.pd_delta_absolute.is_finite()
        || config.pd_delta_absolute < 0.0
        || config.pd_delta_relative.is_nan()
        || config.pd_delta_relative < 0.0
    {
        return Err(finstack_quant_core::Error::Validation(
            "Staging maturity and thresholds must be non-negative and finite (positive infinity disables relative PD)".into(),
        ));
    }
    let mut triggers = Vec::new();

    // 1. Stage 3 DPD backstop (non-rebuttable)
    if exposure.days_past_due >= config.dpd_stage3_threshold {
        triggers.push(StagingTrigger::DpdStage3 {
            dpd: exposure.days_past_due,
            threshold: config.dpd_stage3_threshold,
        });
        return Ok(StageResult {
            stage: Stage::Stage3,
            triggers,
            cured: false,
        });
    }

    // 2. Stage 3 qualitative: objective evidence of default
    //    (IFRS 9 B5.5.37 "unlikely to pay"). Collect every active flag
    //    so the audit trail captures all of them, not just the first.
    if config.stage3_qualitative_triggers_enabled
        && exposure.qualitative_flags.has_default_evidence()
    {
        for flag in exposure.qualitative_flags.active_default_evidence() {
            triggers.push(StagingTrigger::Stage3Qualitative { flag });
        }
        return Ok(StageResult {
            stage: Stage::Stage3,
            triggers,
            cured: false,
        });
    }

    // PD snapshots are independent of the labels used for rating-notch tests.
    if let Some((orig_pd, curr_pd)) = pds()? {
        if !(0.0..=1.0).contains(&orig_pd) || !(0.0..=1.0).contains(&curr_pd) {
            return Err(finstack_quant_core::Error::Validation(
                "Staging PDs must be finite probabilities in [0, 1]".into(),
            ));
        }
        let delta = curr_pd - orig_pd;
        if delta > config.pd_delta_absolute {
            triggers.push(StagingTrigger::PdDeltaAbsolute {
                delta,
                threshold: config.pd_delta_absolute,
            });
        }

        if orig_pd > 0.0 {
            let ratio = curr_pd / orig_pd;
            if ratio > config.pd_delta_relative {
                triggers.push(StagingTrigger::PdDeltaRelative {
                    ratio,
                    threshold: config.pd_delta_relative,
                });
            }
        }
    }

    if let (Some(orig_rating), Some(curr_rating)) =
        (&exposure.origination_rating, &exposure.current_rating)
    {
        // 3b. Stage 2 quantitative: rating downgrade in notches
        //     (IFRS 9 B5.5.17(f)). A zero threshold disables the trigger.
        if config.rating_downgrade_notches > 0 {
            if let Some(notches) = rating_downgrade_notches(orig_rating, curr_rating, config) {
                if notches >= config.rating_downgrade_notches {
                    triggers.push(StagingTrigger::RatingDowngrade {
                        notches,
                        threshold: config.rating_downgrade_notches,
                    });
                }
            }
        }
    }

    // 4. Stage 2 qualitative SICR flags
    if config.qualitative_triggers_enabled && exposure.qualitative_flags.any_active() {
        for flag_name in exposure.qualitative_flags.active_flags() {
            triggers.push(StagingTrigger::Qualitative { flag: flag_name });
        }
    }

    // 5. Stage 2 DPD backstop
    if exposure.days_past_due >= config.dpd_stage2_threshold {
        triggers.push(StagingTrigger::DpdStage2 {
            dpd: exposure.days_past_due,
            threshold: config.dpd_stage2_threshold,
        });
    }

    // If any Stage 2 trigger fired, classify as Stage 2 — but respect the
    // Stage 3 -> Stage 2 cure probation. An exposure that was credit-
    // impaired last period must stay in Stage 3 until it has accumulated
    // `cure_periods_stage3_to_2` consecutive performing periods; otherwise
    // the fact that today it "only" exhibits Stage-2 behaviour would let
    // it bypass probation, which IFRS 9 B5.5.26 / EBA GL on default do
    // not permit.
    if !triggers.is_empty() {
        if let Some(Stage::Stage3) = &exposure.previous_stage {
            if exposure.consecutive_performing_periods < config.cure_periods_stage3_to_2 {
                return Ok(StageResult {
                    stage: Stage::Stage3,
                    triggers,
                    cured: false,
                });
            }
        }
        return Ok(StageResult {
            stage: Stage::Stage2,
            triggers,
            cured: false,
        });
    }

    // 6. Curing logic: no Stage 2/3 triggers fired in this period, so
    //    if the obligor was previously in a higher stage, we walk the
    //    probation ladder. Probation windows are taken from
    //    `config.cure_periods_stage*`:
    //
    //    - Stage 2 → Stage 1 requires `cure_periods_stage2_to_1`.
    //    - Stage 3 → Stage 2 requires `cure_periods_stage3_to_2`.
    //    - Stage 3 → Stage 1 requires
    //      `cure_periods_stage3_to_2 + cure_periods_stage2_to_1`
    //      (full-cure after passing through Stage 2 probation).
    let periods = exposure.consecutive_performing_periods;
    let (stage, cured) = match exposure.previous_stage {
        Some(Stage::Stage3) => {
            if periods < config.cure_periods_stage3_to_2 {
                (Stage::Stage3, false)
            } else if periods
                < config
                    .cure_periods_stage3_to_2
                    .saturating_add(config.cure_periods_stage2_to_1)
            {
                (Stage::Stage2, true)
            } else {
                (Stage::Stage1, true)
            }
        }
        Some(Stage::Stage2) => {
            if periods < config.cure_periods_stage2_to_1 {
                (Stage::Stage2, false)
            } else {
                (Stage::Stage1, true)
            }
        }
        Some(Stage::Stage1) | None => (Stage::Stage1, false),
    };

    Ok(StageResult {
        stage,
        triggers: vec![StagingTrigger::NoTrigger],
        cured,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ecl::types::{QualitativeFlags, RatingPdMap, RawPdCurve};
    use indexmap::IndexMap;

    fn make_pd_curve() -> RawPdCurve {
        // A simple BBB curve: cumulative PD increases over time
        RawPdCurve {
            rating: "BBB".to_string(),
            knots: vec![(0.0, 0.0), (1.0, 0.02), (5.0, 0.10), (10.0, 0.20)],
        }
    }

    fn make_multi_rating_curves() -> MultiRatingCurve {
        MultiRatingCurve {
            curves: vec![
                (
                    "A".to_string(),
                    vec![(0.0, 0.0), (1.0, 0.005), (5.0, 0.03), (10.0, 0.06)],
                ),
                (
                    "BBB".to_string(),
                    vec![(0.0, 0.0), (1.0, 0.02), (5.0, 0.10), (10.0, 0.20)],
                ),
                (
                    "BB".to_string(),
                    vec![(0.0, 0.0), (1.0, 0.05), (5.0, 0.20), (10.0, 0.40)],
                ),
                (
                    "B".to_string(),
                    vec![(0.0, 0.0), (1.0, 0.08), (5.0, 0.30), (10.0, 0.55)],
                ),
            ],
        }
    }

    /// Helper: multi-rating PD source for staging tests.
    struct MultiRatingCurve {
        curves: Vec<(String, Vec<(f64, f64)>)>,
    }

    impl PdTermStructure for MultiRatingCurve {
        fn cumulative_pd(&self, rating: &str, t: f64) -> finstack_quant_core::Result<f64> {
            for (r, knots) in &self.curves {
                if r == rating {
                    return Ok(finstack_quant_core::math::interp::interp_knots_flat(
                        knots, t,
                    ));
                }
            }
            Err(finstack_quant_core::Error::Validation(format!(
                "Rating '{}' not found",
                rating
            )))
        }
    }

    fn base_exposure() -> Exposure {
        Exposure {
            id: "EXP-001".to_string(),
            segments: vec!["corporate".to_string()],
            ead: 1_000_000.0,
            eir: 0.05,
            remaining_maturity_years: 5.0,
            lgd: 0.45,
            days_past_due: 0,
            current_rating: Some("BBB".to_string()),
            origination_rating: Some("BBB".to_string()),
            qualitative_flags: QualitativeFlags::default(),
            consecutive_performing_periods: 0,
            previous_stage: None,
            ead_schedule: None,
            undrawn: 0.0,
            ccf: 0.75,
        }
    }

    /// Config that disables PD-delta triggers so only the rating-downgrade
    /// trigger can fire.
    fn downgrade_only_config(notches: u32) -> StagingConfig {
        StagingConfig {
            pd_delta_absolute: 10.0, // unreachable (cum PDs are <= 1)
            pd_delta_relative: 1e12, // unreachable
            rating_downgrade_notches: notches,
            ..StagingConfig::default()
        }
    }

    #[test]
    fn rating_downgrade_at_threshold_triggers_stage2() {
        // Standard 10-state scale: A (index 2) -> B (index 5) = 3 notches.
        // Threshold 3 -> trigger fires -> Stage 2.
        let mut exposure = base_exposure();
        exposure.origination_rating = Some("A".to_string());
        exposure.current_rating = Some("B".to_string());

        let result = classify_stage(
            &exposure,
            &make_multi_rating_curves(),
            &downgrade_only_config(3),
        )
        .unwrap();

        assert_eq!(result.stage, Stage::Stage2);
        assert!(result.triggers.iter().any(|t| matches!(
            t,
            StagingTrigger::RatingDowngrade {
                notches: 3,
                threshold: 3
            }
        )));
    }

    #[test]
    fn rating_downgrade_below_threshold_does_not_trigger() {
        // BBB (3) -> B (5) = 2 notches < 3 -> no trigger -> Stage 1.
        let mut exposure = base_exposure();
        exposure.origination_rating = Some("BBB".to_string());
        exposure.current_rating = Some("B".to_string());

        let result = classify_stage(
            &exposure,
            &make_multi_rating_curves(),
            &downgrade_only_config(3),
        )
        .unwrap();
        assert_eq!(result.stage, Stage::Stage1);

        // Upgrades never trigger.
        let mut upgraded = base_exposure();
        upgraded.origination_rating = Some("BB".to_string());
        upgraded.current_rating = Some("A".to_string());
        let result = classify_stage(
            &upgraded,
            &make_multi_rating_curves(),
            &downgrade_only_config(1),
        )
        .unwrap();
        assert_eq!(result.stage, Stage::Stage1);
    }

    #[test]
    fn rating_downgrade_custom_scale() {
        // Custom internal scale, 2-notch threshold: Strong (0) -> Weak (2) = 2.
        let mut config = downgrade_only_config(2);
        config.rating_scale_labels = Some(vec![
            "Strong".to_string(),
            "Medium".to_string(),
            "Weak".to_string(),
        ]);

        let mut exposure = base_exposure();
        exposure.origination_rating = Some("Strong".to_string());
        exposure.current_rating = Some("Weak".to_string());

        // PD-delta comparison needs curves for the custom labels: reuse the "A"
        // and "BB" knots under the custom names.
        let pd = MultiRatingCurve {
            curves: vec![
                (
                    "Strong".to_string(),
                    vec![(0.0, 0.0), (1.0, 0.005), (10.0, 0.06)],
                ),
                (
                    "Weak".to_string(),
                    vec![(0.0, 0.0), (1.0, 0.05), (10.0, 0.40)],
                ),
            ],
        };
        let result = classify_stage(&exposure, &pd, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage2);
        assert!(result.triggers.iter().any(|t| matches!(
            t,
            StagingTrigger::RatingDowngrade {
                notches: 2,
                threshold: 2
            }
        )));
    }

    #[test]
    fn rating_downgrade_skips_unknown_labels_and_zero_threshold() {
        // Notched label "BBB+" is not on the default 10-state scale: no trigger,
        // no error (preserves pre-change behavior for unmatched labels).
        let mut exposure = base_exposure();
        exposure.origination_rating = Some("BBB".to_string());
        exposure.current_rating = Some("BBB".to_string());
        let pd = make_multi_rating_curves();
        let mut config = downgrade_only_config(0); // threshold 0 must never fire
        let result = classify_stage(&exposure, &pd, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage1);

        config = downgrade_only_config(3);
        config.rating_scale_labels = Some(vec!["AAA".to_string(), "D".to_string()]);
        // Ratings absent from the configured scale: skipped.
        let result = classify_stage(&exposure, &pd, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage1);
    }

    #[test]
    fn staging_trigger_display_is_the_stable_reason_contract() {
        // The exact strings are consumed by host bindings (Python
        // `classify_stage` trigger reasons); a change here is a wire break.
        let cases: Vec<(StagingTrigger, &str)> = vec![
            (
                StagingTrigger::DpdStage3 {
                    dpd: 90,
                    threshold: 90,
                },
                "dpd_stage3 (dpd=90 >= 90)",
            ),
            (
                StagingTrigger::DpdStage2 {
                    dpd: 30,
                    threshold: 30,
                },
                "dpd_stage2 (dpd=30 >= 30)",
            ),
            (
                StagingTrigger::PdDeltaAbsolute {
                    delta: 0.0123456,
                    threshold: 0.01,
                },
                "pd_delta_absolute (delta=0.0123 > 0.0100)",
            ),
            (
                StagingTrigger::PdDeltaRelative {
                    ratio: 2.345,
                    threshold: 2.0,
                },
                "pd_delta_relative (ratio=2.35x > 2.00x)",
            ),
            (
                StagingTrigger::RatingDowngrade {
                    notches: 3,
                    threshold: 3,
                },
                "rating_downgrade (3 >= 3 notches)",
            ),
            (
                StagingTrigger::Qualitative {
                    flag: "watchlist".to_string(),
                },
                "qualitative:watchlist",
            ),
            (
                StagingTrigger::Stage3Qualitative {
                    flag: "bankruptcy".to_string(),
                },
                "stage3_qualitative:bankruptcy",
            ),
            (StagingTrigger::NoTrigger, "no_trigger"),
        ];
        for (trigger, expected) in cases {
            assert_eq!(trigger.to_string(), expected);
        }
    }

    #[test]
    fn staging_config_serde_is_additive() {
        // Pre-change StagingConfig JSON (no rating_scale_labels) must still parse
        // under deny_unknown_fields.
        let json = r#"{
            "pd_delta_absolute": 0.01, "pd_delta_relative": 2.0,
            "rating_downgrade_notches": 3, "dpd_stage2_threshold": 30,
            "dpd_stage3_threshold": 90, "qualitative_triggers_enabled": true,
            "stage3_qualitative_triggers_enabled": true,
            "cure_periods_stage2_to_1": 3, "cure_periods_stage3_to_2": 12
        }"#;
        let parsed: StagingConfig = serde_json::from_str(json).unwrap();
        assert!(parsed.rating_scale_labels.is_none());
    }

    #[test]
    fn test_stage1_default() {
        let curve = make_pd_curve();
        let config = StagingConfig::default();
        let exposure = base_exposure();

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage1);
        assert!(!result.cured);
    }

    #[test]
    fn test_stage3_dpd_backstop() {
        let curve = make_pd_curve();
        let config = StagingConfig::default();
        let mut exposure = base_exposure();
        exposure.days_past_due = 90;

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage3);
        assert!(matches!(
            result.triggers[0],
            StagingTrigger::DpdStage3 {
                dpd: 90,
                threshold: 90
            }
        ));
    }

    #[test]
    fn test_stage2_dpd_backstop() {
        let curve = make_pd_curve();
        let config = StagingConfig::default();
        let mut exposure = base_exposure();
        exposure.days_past_due = 30;

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage2);
        assert!(result.triggers.iter().any(|t| matches!(
            t,
            StagingTrigger::DpdStage2 {
                dpd: 30,
                threshold: 30
            }
        )));
    }

    #[test]
    fn test_stage2_pd_delta() {
        let curves = make_multi_rating_curves();
        let config = StagingConfig::default();
        let mut exposure = base_exposure();
        // Origination was A, now BBB -> PD increased significantly
        exposure.origination_rating = Some("A".to_string());
        exposure.current_rating = Some("BB".to_string());

        let result = classify_stage(&exposure, &curves, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage2);
        assert!(result
            .triggers
            .iter()
            .any(|t| matches!(t, StagingTrigger::PdDeltaAbsolute { .. })));
    }

    #[test]
    fn test_stage2_qualitative_watchlist() {
        let curve = make_pd_curve();
        let config = StagingConfig::default();
        let mut exposure = base_exposure();
        exposure.qualitative_flags.watchlist = true;

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage2);
        assert!(result
            .triggers
            .iter()
            .any(|t| matches!(t, StagingTrigger::Qualitative { .. })));
    }

    #[test]
    fn test_stage2_qualitative_disabled() {
        let curve = make_pd_curve();
        let config = StagingConfig {
            qualitative_triggers_enabled: false,
            ..Default::default()
        };
        let mut exposure = base_exposure();
        exposure.qualitative_flags.watchlist = true;

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        // With qualitative triggers disabled, watchlist alone doesn't trigger Stage 2
        assert_eq!(result.stage, Stage::Stage1);
    }

    #[test]
    fn test_curing_stage2_to_stage1() {
        let curve = make_pd_curve();
        let config = StagingConfig::default();
        let mut exposure = base_exposure();
        exposure.previous_stage = Some(Stage::Stage2);
        exposure.consecutive_performing_periods = 3;

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage1);
        assert!(result.cured);
    }

    #[test]
    fn test_curing_stage2_insufficient_periods() {
        let curve = make_pd_curve();
        let config = StagingConfig::default();
        let mut exposure = base_exposure();
        exposure.previous_stage = Some(Stage::Stage2);
        exposure.consecutive_performing_periods = 2; // Need 3

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage2);
        assert!(!result.cured);
    }

    #[test]
    fn test_curing_stage3_to_stage2() {
        let curve = make_pd_curve();
        let config = StagingConfig::default();
        let mut exposure = base_exposure();
        exposure.previous_stage = Some(Stage::Stage3);
        // Default cure_periods_stage3_to_2 is 12 (EBA GL) and
        // cure_periods_stage2_to_1 is 3, so 12 performing periods is
        // enough for Stage 3 → Stage 2 but not yet Stage 2 → Stage 1.
        exposure.consecutive_performing_periods = 12;

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage2);
        assert!(result.cured);
    }

    #[test]
    fn test_curing_stage3_to_stage1() {
        let curve = make_pd_curve();
        let config = StagingConfig::default();
        let mut exposure = base_exposure();
        exposure.previous_stage = Some(Stage::Stage3);
        // 12 + 3 = fully cured from Stage 3 all the way to Stage 1.
        exposure.consecutive_performing_periods = 15;

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage1);
        assert!(result.cured);
    }

    /// Regression: if the exposure was Stage 3 last period and today
    /// shows Stage 2 triggers (e.g. watchlist, PD delta) but has NOT
    /// accumulated enough performing periods to clear the cure
    /// probation, the classification must stay Stage 3. Previously the
    /// code short-circuited to Stage 2 on any trigger hit, bypassing
    /// probation. IFRS 9 B5.5.26 / EBA GL on default do not permit
    /// this.
    #[test]
    fn test_stage3_probation_blocks_downgrade_to_stage2_via_triggers() {
        let curve = make_pd_curve();
        let config = StagingConfig::default();
        let mut exposure = base_exposure();
        exposure.previous_stage = Some(Stage::Stage3);
        exposure.consecutive_performing_periods = 3; // < 12 = cure_periods_stage3_to_2
        exposure.qualitative_flags.watchlist = true; // fires a Stage-2 trigger

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        assert_eq!(
            result.stage,
            Stage::Stage3,
            "Stage 3 probation must override Stage 2 triggers"
        );
        assert!(!result.cured);
    }

    /// Sibling test: once probation *is* complete, Stage 2 triggers
    /// correctly move the exposure to Stage 2.
    #[test]
    fn test_stage3_probation_complete_allows_stage2_via_triggers() {
        let curve = make_pd_curve();
        let config = StagingConfig::default();
        let mut exposure = base_exposure();
        exposure.previous_stage = Some(Stage::Stage3);
        exposure.consecutive_performing_periods = 12; // == cure_periods_stage3_to_2
        exposure.qualitative_flags.watchlist = true;

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage2);
    }

    /// Objective evidence of default (IFRS 9 B5.5.37) triggers Stage 3
    /// independently of DPD.
    #[test]
    fn test_stage3_bankruptcy_flag() {
        let curve = make_pd_curve();
        let config = StagingConfig::default();
        let mut exposure = base_exposure();
        exposure.qualitative_flags.bankruptcy = true;

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage3);
        assert!(result
            .triggers
            .iter()
            .any(|t| matches!(t, StagingTrigger::Stage3Qualitative { .. })));
    }

    /// Distressed modification is Stage 3 evidence. Plain forbearance
    /// (no NPV loss) remains a Stage 2 SICR trigger.
    #[test]
    fn test_distressed_modification_stage3_vs_forbearance_stage2() {
        let curve = make_pd_curve();
        let config = StagingConfig::default();

        let mut exposure_distressed = base_exposure();
        exposure_distressed
            .qualitative_flags
            .distressed_modification = true;
        let r = classify_stage(&exposure_distressed, &curve, &config).unwrap();
        assert_eq!(r.stage, Stage::Stage3);

        let mut exposure_forbearance = base_exposure();
        exposure_forbearance.qualitative_flags.forbearance = true;
        let r = classify_stage(&exposure_forbearance, &curve, &config).unwrap();
        assert_eq!(r.stage, Stage::Stage2);
    }

    /// Operator can disable Stage 3 qualitative triggers when an
    /// upstream engine already maps default evidence into DPD.
    #[test]
    fn test_stage3_qualitative_can_be_disabled() {
        let curve = make_pd_curve();
        let config = StagingConfig {
            stage3_qualitative_triggers_enabled: false,
            ..StagingConfig::default()
        };
        let mut exposure = base_exposure();
        exposure.qualitative_flags.bankruptcy = true;

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        // Without DPD and with Stage-3 qualitatives disabled, the
        // obligor falls back through the Stage-2 waterfall. Bankruptcy
        // is not one of the SICR flags, so we end up at Stage 1.
        assert_eq!(result.stage, Stage::Stage1);
    }

    #[test]
    fn missing_rating_on_raw_curve_skips_pd_delta() {
        // A → BBB on a BBB-only curve: PD-delta is skipped (A is absent),
        // and a 1-notch downgrade is below the default 3-notch threshold.
        let curve = make_pd_curve();
        let config = StagingConfig::default();
        let mut exposure = base_exposure();
        exposure.origination_rating = Some("A".to_string());
        exposure.current_rating = Some("BBB".to_string());

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage1);
        assert!(result
            .triggers
            .iter()
            .all(|t| !matches!(t, StagingTrigger::PdDeltaAbsolute { .. })));
    }

    #[test]
    fn rating_pd_map_fires_pd_delta_across_ratings() {
        let map = RatingPdMap::new(IndexMap::from([
            (
                "A".to_string(),
                RawPdCurve::new(
                    "A",
                    vec![(0.0, 0.0), (1.0, 0.005), (5.0, 0.03), (10.0, 0.06)],
                )
                .unwrap(),
            ),
            (
                "BB".to_string(),
                RawPdCurve::new(
                    "BB",
                    vec![(0.0, 0.0), (1.0, 0.05), (5.0, 0.20), (10.0, 0.40)],
                )
                .unwrap(),
            ),
        ]));
        let config = StagingConfig::default();
        let mut exposure = base_exposure();
        exposure.origination_rating = Some("A".to_string());
        exposure.current_rating = Some("BB".to_string());

        let result = classify_stage(&exposure, &map, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage2);
        assert!(result
            .triggers
            .iter()
            .any(|t| matches!(t, StagingTrigger::PdDeltaAbsolute { .. })));
    }
}
