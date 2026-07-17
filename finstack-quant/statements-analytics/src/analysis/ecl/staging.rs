//! IFRS 9 stage classification engine.
//!
//! Implements the waterfall logic for assigning exposures to impairment stages
//! based on quantitative triggers (PD delta, DPD backstops) and qualitative
//! flags (watchlist, forbearance, adverse conditions).
//!
//! The classification waterfall is:
//! 1. Stage 3 DPD backstop: DPD > `dpd_stage3_threshold` (absolute, non-rebuttable)
//! 2. Stage 3 qualitative default evidence (IFRS 9 B5.5.37 "unlikely to
//!    pay"): bankruptcy / distressed modification / cross-default / user-
//!    supplied default-evidence flags.
//! 3. Stage 2 quantitative: PD delta exceeds thresholds (absolute OR relative)
//! 4. Stage 2 qualitative SICR flags (IFRS 9 B5.5.17)
//! 5. Stage 2 DPD backstop: DPD > `dpd_stage2_threshold` (rebuttable presumption)
//! 6. Curing: If previously Stage 2/3 and now meets cure criteria, allow step-down
//! 7. Default: Stage 1
//!
//! # Comparator convention
//!
//! IFRS 9 B5.5.37 phrases the backstops as "contractual payments are
//! *more than* 30 / 90 days past due". This module follows that literal
//! wording: a trigger fires when `days_past_due > threshold` (equivalently
//! DPD >= threshold + 1). Callers who require the alternative ">=" reading
//! (e.g. some Basel IRB implementations) should set
//! `dpd_stage2_threshold = 29` and `dpd_stage3_threshold = 89`.
//!
//! # References
//!
//! - IFRS 9 Financial Instruments, Section 5.5 -- Impairment
//! - IFRS 9 B5.5.17 -- Qualitative indicators of SICR
//! - IFRS 9 B5.5.19 -- 30 days past due rebuttable presumption
//! - IFRS 9 B5.5.37 -- 90 days past due default presumption

use finstack_quant_core::Result;
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

// ---------------------------------------------------------------------------
// Staging trigger (audit trail)
// ---------------------------------------------------------------------------

/// Reason for stage assignment (audit trail).
///
/// Each variant captures the specific values that triggered the classification,
/// enabling regulatory reporting and model governance review.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Rating downgraded by N or more notches.
    ///
    /// **Not yet implemented**: [`classify_stage`] never emits this trigger
    /// because the staging engine has no rating-scale notion to count
    /// notches with. The variant is retained for serde wire stability and
    /// for callers that construct audit trails externally.
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

// ---------------------------------------------------------------------------
// Stage result
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Staging configuration
// ---------------------------------------------------------------------------

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

    /// Rating downgrade notches that trigger Stage 2.
    /// Example: 3 means a 3-notch downgrade from origination triggers SICR.
    ///
    /// **Not yet implemented**: [`classify_stage`] does not evaluate this
    /// threshold (it has no rating-scale notion to count notches with), so
    /// the [`StagingTrigger::RatingDowngrade`] trigger never fires. The
    /// field is retained for serde wire stability; SICR on rating migration
    /// is captured indirectly via the PD-delta triggers, which compare the
    /// PD curves of the origination and current ratings.
    pub rating_downgrade_notches: u32,

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

impl Default for StagingConfig {
    fn default() -> Self {
        super::policy::default_staging_config()
    }
}

// ---------------------------------------------------------------------------
// Classification logic
// ---------------------------------------------------------------------------

/// Classify an exposure into an IFRS 9 stage.
///
/// The classification waterfall evaluates triggers in priority order:
///
/// 1. **Stage 3 backstop**: DPD exceeds `dpd_stage3_threshold` (non-rebuttable)
/// 2. **Stage 2 quantitative**: PD delta exceeds absolute OR relative threshold
/// 3. **Stage 2 qualitative**: Any qualitative flag is active (if enabled)
/// 4. **Stage 2 DPD backstop**: DPD exceeds `dpd_stage2_threshold`
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
/// current ratings are present and the SICR comparison requires cumulative
/// probabilities.
pub fn classify_stage(
    exposure: &Exposure,
    pd_source: &dyn PdTermStructure,
    config: &StagingConfig,
) -> Result<StageResult> {
    let mut triggers = Vec::new();

    // 1. Stage 3 DPD backstop (non-rebuttable)
    if exposure.days_past_due > config.dpd_stage3_threshold {
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

    // 3. Stage 2 quantitative: PD delta (only if both ratings available)
    if let (Some(orig_rating), Some(curr_rating)) =
        (&exposure.origination_rating, &exposure.current_rating)
    {
        let horizon = exposure
            .remaining_maturity_years
            .min(MAX_SICR_HORIZON_YEARS);
        let orig_pd = pd_source.cumulative_pd(orig_rating, horizon)?;
        let curr_pd = pd_source.cumulative_pd(curr_rating, horizon)?;

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

    // 4. Stage 2 qualitative SICR flags
    if config.qualitative_triggers_enabled && exposure.qualitative_flags.any_active() {
        for flag_name in exposure.qualitative_flags.active_flags() {
            triggers.push(StagingTrigger::Qualitative { flag: flag_name });
        }
    }

    // 5. Stage 2 DPD backstop
    if exposure.days_past_due > config.dpd_stage2_threshold {
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
    use crate::analysis::ecl::types::{QualitativeFlags, RawPdCurve};

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
                    return Ok(super::super::types::interp_linear(knots, t));
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
        }
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
        exposure.days_past_due = 91;

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage3);
        assert!(matches!(
            result.triggers[0],
            StagingTrigger::DpdStage3 {
                dpd: 91,
                threshold: 90
            }
        ));
    }

    #[test]
    fn test_stage2_dpd_backstop() {
        let curve = make_pd_curve();
        let config = StagingConfig::default();
        let mut exposure = base_exposure();
        exposure.days_past_due = 35;

        let result = classify_stage(&exposure, &curve, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage2);
        assert!(result
            .triggers
            .iter()
            .any(|t| matches!(t, StagingTrigger::DpdStage2 { .. })));
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
}
