//! Core types for ECL / IFRS 9 / CECL computation.
//!
//! This module defines the fundamental data structures used throughout the ECL
//! framework:
//!
//! - [`Stage`] -- IFRS 9 impairment stage classification
//! - [`Exposure`] -- a single credit exposure at a reporting date
//! - [`QualitativeFlags`] -- qualitative SICR triggers
//! - [`PdTermStructure`] -- trait abstracting PD curve sources
//! - [`RawPdCurve`] -- user-supplied PD term structure with linear interpolation

use finstack_core::{Error, InputError, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Stage
// ---------------------------------------------------------------------------

/// IFRS 9 impairment stage for a credit exposure.
///
/// Under IFRS 9, financial instruments are classified into three stages that
/// determine the ECL measurement horizon:
///
/// - **Stage 1**: 12-month ECL (no significant increase in credit risk)
/// - **Stage 2**: Lifetime ECL (significant increase in credit risk detected)
/// - **Stage 3**: Lifetime ECL (credit-impaired, objective evidence of default)
///
/// # References
///
/// IFRS 9 Financial Instruments, Section 5.5 -- Impairment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stage {
    /// Performing -- 12-month ECL. No significant increase in credit risk
    /// since initial recognition.
    Stage1,
    /// Underperforming -- lifetime ECL. Significant increase in credit risk
    /// (SICR) detected but not yet credit-impaired.
    Stage2,
    /// Non-performing -- lifetime ECL. Credit-impaired (objective evidence of
    /// default: DPD > 90, restructuring, etc.).
    Stage3,
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stage::Stage1 => write!(f, "Stage 1"),
            Stage::Stage2 => write!(f, "Stage 2"),
            Stage::Stage3 => write!(f, "Stage 3"),
        }
    }
}

// ---------------------------------------------------------------------------
// Qualitative flags
// ---------------------------------------------------------------------------

/// Qualitative triggers for SICR detection (IFRS 9 B5.5.17) and
/// "unlikely-to-pay" evidence of default (IFRS 9 B5.5.37).
///
/// These flags represent non-quantitative indicators. SICR flags
/// (`watchlist`, `forbearance`, `adverse_conditions`, `custom`) may
/// trigger a Stage 2 classification. The `default_evidence` flags
/// (`bankruptcy`, `distressed_modification`, `cross_default`,
/// `other_default_evidence`) represent objective evidence of default
/// and should trigger Stage 3 independently of the 90-DPD backstop.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualitativeFlags {
    /// On internal watchlist.
    pub watchlist: bool,
    /// Forbearance or concession granted. Under IFRS 9 forbearance is
    /// at minimum a SICR trigger; distressed modifications (deep
    /// concessions such as haircut, maturity extension with NPV loss)
    /// are tracked separately under `distressed_modification` and
    /// trigger Stage 3.
    pub forbearance: bool,
    /// Significant adverse change in business or financial conditions.
    pub adverse_conditions: bool,
    /// Custom user-defined SICR flags (e.g., sector-specific triggers).
    pub custom: Vec<String>,

    // --- Objective evidence of default / "unlikely to pay" (Stage 3) ---
    /// Obligor has filed or is subject to bankruptcy / insolvency
    /// proceedings. Non-rebuttable Stage 3 trigger.
    pub bankruptcy: bool,
    /// Distressed restructuring / modification with material NPV loss
    /// to the lender. IFRS 9 B5.5.37(e) / EBA GL on default.
    pub distressed_modification: bool,
    /// Cross-default has been triggered on another obligation of the
    /// same obligor.
    pub cross_default: bool,
    /// Custom user-defined Stage 3 default-evidence flags.
    pub other_default_evidence: Vec<String>,
}

impl QualitativeFlags {
    /// Returns `true` if any SICR (Stage 2) qualitative flag is active.
    ///
    /// Note: this does not include the default-evidence flags. Use
    /// [`QualitativeFlags::has_default_evidence`] for Stage 3 triggers.
    pub fn any_active(&self) -> bool {
        self.watchlist || self.forbearance || self.adverse_conditions || !self.custom.is_empty()
    }

    /// Returns the names of all active SICR flags (for audit trail).
    pub fn active_flags(&self) -> Vec<String> {
        let mut flags = Vec::new();
        if self.watchlist {
            flags.push("watchlist".to_string());
        }
        if self.forbearance {
            flags.push("forbearance".to_string());
        }
        if self.adverse_conditions {
            flags.push("adverse_conditions".to_string());
        }
        for c in &self.custom {
            flags.push(c.clone());
        }
        flags
    }

    /// Returns `true` if any objective evidence of default (Stage 3) is
    /// flagged. Corresponds to IFRS 9 B5.5.37 "unlikely-to-pay"
    /// indicators.
    pub fn has_default_evidence(&self) -> bool {
        self.bankruptcy
            || self.distressed_modification
            || self.cross_default
            || !self.other_default_evidence.is_empty()
    }

    /// Returns the names of all active default-evidence flags.
    pub fn active_default_evidence(&self) -> Vec<String> {
        let mut flags = Vec::new();
        if self.bankruptcy {
            flags.push("bankruptcy".to_string());
        }
        if self.distressed_modification {
            flags.push("distressed_modification".to_string());
        }
        if self.cross_default {
            flags.push("cross_default".to_string());
        }
        for c in &self.other_default_evidence {
            flags.push(c.clone());
        }
        flags
    }
}

// ---------------------------------------------------------------------------
// Exposure
// ---------------------------------------------------------------------------

/// Sanity bound on `remaining_maturity_years`.
///
/// The ECL engines allocate one bucket per `bucket_width_years` over the
/// remaining maturity, so an unbounded maturity translates directly into an
/// unbounded bucket allocation. 100 years comfortably covers any real
/// instrument (including perpetuals modelled with a behavioural maturity).
pub const MAX_REMAINING_MATURITY_YEARS: f64 = 100.0;

/// A single credit exposure for ECL computation.
///
/// Represents one instrument or facility at a reporting date. All monetary
/// amounts are in the exposure's base currency (currency conversion is the
/// caller's responsibility).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exposure {
    /// Unique identifier for the exposure (instrument ID, facility ID, etc.).
    pub id: String,

    /// Segment keys for portfolio aggregation (product type, geography, rating
    /// bucket, etc.). Empty vec means "unclassified".
    pub segments: Vec<String>,

    /// Outstanding balance (drawn amount) at the reporting date.
    pub ead: f64,

    /// Effective interest rate (annualized, decimal). Used as the IFRS 9
    /// discount rate. Example: 0.05 = 5%.
    pub eir: f64,

    /// Remaining maturity in years from reporting date. For revolving
    /// facilities, use behavioural maturity.
    pub remaining_maturity_years: f64,

    /// Loss given default (decimal, 0..1). Can be point-in-time or
    /// downturn LGD depending on methodology.
    pub lgd: f64,

    /// Current days past due (DPD). Used for backstop staging triggers.
    pub days_past_due: u32,

    /// Current rating label (must match the `PdTermStructure` scale).
    /// `None` if the exposure is unrated.
    pub current_rating: Option<String>,

    /// Rating at origination (initial recognition). Used for SICR delta PD
    /// comparison. `None` disables the PD delta trigger.
    pub origination_rating: Option<String>,

    /// Qualitative flags that can trigger Stage 2 classification.
    pub qualitative_flags: QualitativeFlags,

    /// Number of consecutive performing periods since last Stage 2/3
    /// classification. Used for curing logic.
    pub consecutive_performing_periods: u32,

    /// Previous reporting period's stage (for migration tracking).
    pub previous_stage: Option<Stage>,

    /// Optional EAD amortization profile as `(time_years, ead)` knots.
    ///
    /// When present, the ECL engines evaluate EAD at each bucket midpoint by
    /// linear interpolation between knots with flat extrapolation at both
    /// ends. When `None`, the constant [`Exposure::ead`] is used for every
    /// bucket. Knot times must be strictly increasing, finite, and
    /// non-negative; EAD values must be finite and non-negative.
    #[serde(default)]
    pub ead_schedule: Option<Vec<(f64, f64)>>,
}

impl Exposure {
    /// Validate that the exposure's numeric invariants hold before it enters
    /// the ECL engine.
    ///
    /// The ECL engine assumes:
    ///
    /// - `ead >= 0` (signed EAD is not a modelled concept here)
    /// - `eir` is finite and non-negative (discount factors must be well-defined)
    /// - `lgd` ∈ \[0, 1\]
    /// - `remaining_maturity_years >= 0` and finite
    ///
    /// Running ECL on violating inputs would either produce silently wrong
    /// numbers (negative EAD flips the sign of the ECL, out-of-range LGD
    /// distorts severity) or NaN/Inf via the discounting step.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` with a descriptive message if any invariant
    /// is violated.
    pub fn validate(&self) -> Result<()> {
        if !self.ead.is_finite() || self.ead < 0.0 {
            return Err(Error::Validation(format!(
                "Exposure '{}': EAD must be finite and non-negative (got {})",
                self.id, self.ead
            )));
        }
        if !self.eir.is_finite() || self.eir < 0.0 {
            return Err(Error::Validation(format!(
                "Exposure '{}': EIR must be finite and non-negative (got {})",
                self.id, self.eir
            )));
        }
        if !self.lgd.is_finite() || !(0.0..=1.0).contains(&self.lgd) {
            return Err(Error::Validation(format!(
                "Exposure '{}': LGD must be finite and in [0, 1] (got {})",
                self.id, self.lgd
            )));
        }
        if !self.remaining_maturity_years.is_finite() || self.remaining_maturity_years < 0.0 {
            return Err(Error::Validation(format!(
                "Exposure '{}': remaining maturity must be finite and non-negative (got {})",
                self.id, self.remaining_maturity_years
            )));
        }
        if self.remaining_maturity_years > MAX_REMAINING_MATURITY_YEARS {
            return Err(Error::Validation(format!(
                "Exposure '{}': remaining maturity must be <= {MAX_REMAINING_MATURITY_YEARS} \
                 years (got {})",
                self.id, self.remaining_maturity_years
            )));
        }
        if let Some(schedule) = &self.ead_schedule {
            if schedule.is_empty() {
                return Err(Error::Validation(format!(
                    "Exposure '{}': ead_schedule must not be empty when present",
                    self.id
                )));
            }
            let mut prev_t = f64::NEG_INFINITY;
            for &(t, ead) in schedule {
                if !t.is_finite() || t < 0.0 {
                    return Err(Error::Validation(format!(
                        "Exposure '{}': ead_schedule times must be finite and non-negative \
                         (got {t})",
                        self.id
                    )));
                }
                if t <= prev_t {
                    return Err(Error::Validation(format!(
                        "Exposure '{}': ead_schedule times must be strictly increasing \
                         (got {t} after {prev_t})",
                        self.id
                    )));
                }
                if !ead.is_finite() || ead < 0.0 {
                    return Err(Error::Validation(format!(
                        "Exposure '{}': ead_schedule EAD values must be finite and \
                         non-negative (got {ead} at t={t})",
                        self.id
                    )));
                }
                prev_t = t;
            }
        }
        Ok(())
    }

    /// EAD at time `t` (years from the reporting date).
    ///
    /// Uses linear interpolation over [`Exposure::ead_schedule`] with flat
    /// extrapolation at both ends when a schedule is present; otherwise
    /// returns the constant [`Exposure::ead`].
    pub(crate) fn ead_at(&self, t: f64) -> f64 {
        match &self.ead_schedule {
            Some(schedule) if !schedule.is_empty() => interp_linear(schedule, t),
            _ => self.ead,
        }
    }
}

// ---------------------------------------------------------------------------
// PD Term Structure trait
// ---------------------------------------------------------------------------

/// Abstraction over PD term structure sources.
///
/// Implementations extract marginal default probabilities for time buckets.
/// The library provides [`RawPdCurve`] for user-supplied PD vectors; external
/// implementations can wrap `HazardCurve` or `TransitionMatrix`.
///
/// # Contract
///
/// - `cumulative_pd` must return values in \[0, 1\].
/// - `cumulative_pd` must be monotonically non-decreasing in `t`.
/// - `marginal_pd` must return values in \[0, 1\].
pub trait PdTermStructure: Send + Sync {
    /// Cumulative probability of default by time `t` (in years) for the
    /// given rating state. Returns a value in \[0, 1\].
    fn cumulative_pd(&self, rating: &str, t: f64) -> Result<f64>;

    /// Marginal (forward) PD for the interval (t1, t2\], conditional on
    /// survival to t1. Default implementation derives from cumulative PD.
    ///
    /// When `S(t1) <= 0` (already in the absorbing default state at `t1`),
    /// the conditional probability of *additional* default in `(t1, t2]` is
    /// **0**, not 1. Returning 1 here would double-count an already-defaulted
    /// obligor in every subsequent ECL bucket.
    fn marginal_pd(&self, rating: &str, t1: f64, t2: f64) -> Result<f64> {
        let s1 = 1.0 - self.cumulative_pd(rating, t1)?;
        let s2 = 1.0 - self.cumulative_pd(rating, t2)?;
        if s1 <= 0.0 {
            return Ok(0.0);
        }
        Ok(1.0 - s2 / s1)
    }
}

// ---------------------------------------------------------------------------
// RawPdCurve
// ---------------------------------------------------------------------------

/// Raw user-supplied PD term structure with linear interpolation.
///
/// Use this when you have a discrete set of cumulative PD observations
/// (e.g., from internal rating model output) rather than a parametric
/// hazard curve or transition matrix.
///
/// # Interpolation
///
/// - Knots must be sorted by time and monotonically increasing in PD.
/// - For `t` before the first knot, cumulative PD is 0.
/// - For `t` after the last knot, cumulative PD is flat-extrapolated.
/// - Between knots, linear interpolation is applied.
///
/// # Deserialization
///
/// Deserialization is validating: JSON inputs go through the same invariant
/// checks as [`RawPdCurve::new`] (first knot at `(0.0, 0.0)`, strictly
/// increasing times, monotone PDs in `[0, 1]`) and unknown fields are
/// rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "RawPdCurveData")]
pub struct RawPdCurve {
    /// Rating label this curve applies to.
    pub rating: String,
    /// (time_years, cumulative_pd) knots, sorted by time, monotonically increasing.
    pub knots: Vec<(f64, f64)>,
}

/// Serde shadow type that routes `RawPdCurve` deserialization through
/// [`RawPdCurve::new`] so JSON inputs cannot bypass the curve invariants.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPdCurveData {
    rating: String,
    knots: Vec<(f64, f64)>,
}

impl TryFrom<RawPdCurveData> for RawPdCurve {
    type Error = Error;

    fn try_from(data: RawPdCurveData) -> Result<Self> {
        RawPdCurve::new(data.rating, data.knots)
    }
}

impl RawPdCurve {
    /// Create a new `RawPdCurve`, validating the `PdTermStructure` contract:
    ///
    /// 1. First knot anchored at `(0.0, 0.0)` so cumulative PD is 0 at t=0
    ///    (matches the module-level contract and the "PD is 0 before first
    ///    knot" guarantee).
    /// 2. Knot times strictly increasing.
    /// 3. Cumulative PDs monotonically non-decreasing and in `[0, 1]`.
    pub fn new(rating: impl Into<String>, knots: Vec<(f64, f64)>) -> Result<Self> {
        if knots.len() < 2 {
            return Err(InputError::TooFewPoints.into());
        }
        let (t0, pd0) = knots[0];
        if t0 != 0.0 || pd0 != 0.0 {
            return Err(Error::Validation(format!(
                "RawPdCurve: first knot must be (0.0, 0.0), got ({t0}, {pd0})"
            )));
        }
        for window in knots.windows(2) {
            let (t_prev, pd_prev) = window[0];
            let (t_curr, pd_curr) = window[1];
            if t_curr <= t_prev {
                return Err(InputError::NonMonotonicKnots.into());
            }
            if pd_curr < pd_prev {
                return Err(Error::Validation(format!(
                    "RawPdCurve: cumulative PD must be non-decreasing, \
                     got ({t_prev}, {pd_prev}) -> ({t_curr}, {pd_curr})"
                )));
            }
            if !(0.0..=1.0).contains(&pd_curr) {
                return Err(Error::Validation(format!(
                    "RawPdCurve: cumulative PD must be in [0, 1], got {pd_curr} at t={t_curr}"
                )));
            }
        }
        Ok(Self {
            rating: rating.into(),
            knots,
        })
    }

    /// Anchor a cumulative-PD knot vector at `(0.0, 0.0)`.
    ///
    /// Prepends the `(0.0, 0.0)` knot required by [`RawPdCurve::new`] when the
    /// caller-supplied schedule starts after `t = 0`; schedules that already
    /// include a `t = 0` knot are returned unchanged. This is the canonical
    /// convenience for host bindings that accept user PD schedules without
    /// forcing callers to spell out the zero anchor.
    #[must_use]
    pub fn anchor_knots(knots: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
        match knots.first() {
            Some(&(t0, _)) if t0 > 0.0 => {
                let mut anchored = Vec::with_capacity(knots.len() + 1);
                anchored.push((0.0, 0.0));
                anchored.extend(knots);
                anchored
            }
            _ => knots,
        }
    }
}

impl PdTermStructure for RawPdCurve {
    fn cumulative_pd(&self, rating: &str, t: f64) -> Result<f64> {
        if rating != self.rating {
            return Err(Error::Validation(format!(
                "RawPdCurve is for rating '{}', got '{}'",
                self.rating, rating
            )));
        }
        Ok(interp_linear(&self.knots, t))
    }
}

/// Linear interpolation with flat extrapolation on a sorted knot vector.
pub(crate) fn interp_linear(knots: &[(f64, f64)], t: f64) -> f64 {
    if knots.is_empty() {
        return 0.0;
    }
    if t <= knots[0].0 {
        return knots[0].1;
    }
    if t >= knots[knots.len() - 1].0 {
        return knots[knots.len() - 1].1;
    }
    // Binary search for the right interval
    let idx = knots.partition_point(|k| k.0 < t);
    if idx == 0 {
        return knots[0].1;
    }
    let (t0, y0) = knots[idx - 1];
    let (t1, y1) = knots[idx];
    // Guard against a zero-width interval. `RawPdCurve::new` (and the
    // validating deserializer) reject duplicate knot times, but the struct
    // has public fields, so a curve built by struct literal can still
    // carry `t1 == t0`. Without this guard the interpolation weight
    // would be `0.0 / 0.0 = NaN` and poison every downstream ECL figure.
    if t1 <= t0 {
        return y1;
    }
    let w = (t - t0) / (t1 - t0);
    y0 + w * (y1 - y0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_display() {
        assert_eq!(format!("{}", Stage::Stage1), "Stage 1");
        assert_eq!(format!("{}", Stage::Stage2), "Stage 2");
        assert_eq!(format!("{}", Stage::Stage3), "Stage 3");
    }

    #[test]
    fn test_qualitative_flags_none_active() {
        let flags = QualitativeFlags::default();
        assert!(!flags.any_active());
        assert!(flags.active_flags().is_empty());
    }

    #[test]
    fn test_qualitative_flags_some_active() {
        let flags = QualitativeFlags {
            watchlist: true,
            custom: vec!["sector_stress".to_string()],
            ..QualitativeFlags::default()
        };
        assert!(flags.any_active());
        let active = flags.active_flags();
        assert_eq!(active.len(), 2);
        assert!(active.contains(&"watchlist".to_string()));
        assert!(active.contains(&"sector_stress".to_string()));
    }

    #[test]
    fn test_raw_pd_curve_validates_knots() {
        // Too few points
        assert!(RawPdCurve::new("BBB", vec![(0.0, 0.0)]).is_err());
        // First knot must be (0, 0)
        assert!(RawPdCurve::new("BBB", vec![(1.0, 0.02), (2.0, 0.04)]).is_err());
        assert!(RawPdCurve::new("BBB", vec![(0.0, 0.01), (1.0, 0.02)]).is_err());
        // Non-monotonic times
        assert!(RawPdCurve::new("BBB", vec![(0.0, 0.0), (2.0, 0.02), (1.0, 0.04)]).is_err());
        // Non-monotonic cumulative PD
        assert!(RawPdCurve::new("BBB", vec![(0.0, 0.0), (1.0, 0.05), (2.0, 0.03)]).is_err());
        // Out-of-range PD
        assert!(RawPdCurve::new("BBB", vec![(0.0, 0.0), (1.0, 1.5)]).is_err());
        // Valid
        assert!(RawPdCurve::new("BBB", vec![(0.0, 0.0), (1.0, 0.02), (2.0, 0.04)]).is_ok());
    }

    #[test]
    fn test_raw_pd_curve_deserialization_validates() {
        // Valid JSON round-trips through RawPdCurve::new.
        let ok: std::result::Result<RawPdCurve, _> =
            serde_json::from_str(r#"{"rating":"BBB","knots":[[0.0,0.0],[1.0,0.02]]}"#);
        assert!(ok.is_ok());

        // Invalid knots cannot bypass new() via JSON.
        let bad: std::result::Result<RawPdCurve, _> =
            serde_json::from_str(r#"{"rating":"BBB","knots":[[0.0,0.0],[1.0,1.5]]}"#);
        assert!(bad.is_err());

        // Unknown fields are rejected.
        let unknown: std::result::Result<RawPdCurve, _> =
            serde_json::from_str(r#"{"rating":"BBB","knots":[[0.0,0.0],[1.0,0.02]],"extra":1}"#);
        assert!(unknown.is_err());
    }

    #[test]
    fn test_exposure_deserialization_strict_and_defaults() {
        let json = r#"{
            "id": "E1", "segments": [], "ead": 100.0, "eir": 0.05,
            "remaining_maturity_years": 2.0, "lgd": 0.4, "days_past_due": 0,
            "current_rating": null, "origination_rating": null,
            "qualitative_flags": {
                "watchlist": false, "forbearance": false, "adverse_conditions": false,
                "custom": [], "bankruptcy": false, "distressed_modification": false,
                "cross_default": false, "other_default_evidence": []
            },
            "consecutive_performing_periods": 0, "previous_stage": null
        }"#;
        // ead_schedule is optional (serde default) for backwards compatibility.
        let exposure: Exposure = serde_json::from_str(json).unwrap();
        assert!(exposure.ead_schedule.is_none());

        // Unknown fields are rejected.
        let unknown = json.replacen("\"id\": \"E1\",", "\"id\": \"E1\", \"bogus\": 1,", 1);
        let result: std::result::Result<Exposure, _> = serde_json::from_str(&unknown);
        assert!(result.is_err());
    }

    #[test]
    fn test_exposure_validate_maturity_bound_and_ead_schedule() {
        let mut exposure: Exposure = Exposure {
            id: "E1".to_string(),
            segments: vec![],
            ead: 100.0,
            eir: 0.05,
            remaining_maturity_years: 2.0,
            lgd: 0.4,
            days_past_due: 0,
            current_rating: None,
            origination_rating: None,
            qualitative_flags: QualitativeFlags::default(),
            consecutive_performing_periods: 0,
            previous_stage: None,
            ead_schedule: None,
        };
        assert!(exposure.validate().is_ok());

        exposure.remaining_maturity_years = MAX_REMAINING_MATURITY_YEARS + 1.0;
        assert!(exposure.validate().is_err());
        exposure.remaining_maturity_years = 2.0;

        // Valid schedule + interpolation behaviour.
        exposure.ead_schedule = Some(vec![(0.0, 100.0), (2.0, 0.0)]);
        assert!(exposure.validate().is_ok());
        assert!((exposure.ead_at(1.0) - 50.0).abs() < 1e-12);
        assert!((exposure.ead_at(5.0) - 0.0).abs() < 1e-12); // flat extrapolation

        // Non-increasing times rejected.
        exposure.ead_schedule = Some(vec![(1.0, 100.0), (1.0, 50.0)]);
        assert!(exposure.validate().is_err());

        // Negative EAD rejected.
        exposure.ead_schedule = Some(vec![(0.0, -1.0)]);
        assert!(exposure.validate().is_err());

        // Empty schedule rejected.
        exposure.ead_schedule = Some(vec![]);
        assert!(exposure.validate().is_err());
    }

    #[test]
    fn test_interp_linear() {
        let knots = vec![(1.0, 0.02), (2.0, 0.05), (5.0, 0.12)];
        // Before first knot -> first value
        assert!((interp_linear(&knots, 0.5) - 0.02).abs() < 1e-10);
        // At first knot
        assert!((interp_linear(&knots, 1.0) - 0.02).abs() < 1e-10);
        // Midpoint of first segment
        assert!((interp_linear(&knots, 1.5) - 0.035).abs() < 1e-10);
        // At second knot
        assert!((interp_linear(&knots, 2.0) - 0.05).abs() < 1e-10);
        // After last knot -> flat extrapolation
        assert!((interp_linear(&knots, 10.0) - 0.12).abs() < 1e-10);
    }

    #[test]
    fn test_raw_pd_curve_cumulative_pd() {
        let curve = RawPdCurve::new(
            "BBB",
            vec![(0.0, 0.0), (1.0, 0.02), (2.0, 0.05), (5.0, 0.12)],
        )
        .ok();
        let curve = curve.as_ref().unwrap(); // ok in test
        assert!((curve.cumulative_pd("BBB", 1.5).unwrap() - 0.035).abs() < 1e-10);
        // Wrong rating
        assert!(curve.cumulative_pd("AA", 1.0).is_err());
    }

    #[test]
    fn test_raw_pd_curve_marginal_pd() {
        let curve = RawPdCurve::new("BBB", vec![(0.0, 0.0), (1.0, 0.02), (2.0, 0.05)]).ok();
        let curve = curve.as_ref().unwrap();
        // Marginal PD from 0 to 1: cumulative goes from 0 to 0.02
        // survival(0) = 1.0, survival(1) = 0.98
        // marginal = 1 - 0.98/1.0 = 0.02
        let mpd = curve.marginal_pd("BBB", 0.0, 1.0).unwrap();
        assert!((mpd - 0.02).abs() < 1e-10);

        // Marginal PD from 1 to 2: survival(1) = 0.98, survival(2) = 0.95
        // marginal = 1 - 0.95/0.98 = 0.030612...
        let mpd = curve.marginal_pd("BBB", 1.0, 2.0).unwrap();
        let expected = 1.0 - 0.95 / 0.98;
        assert!((mpd - expected).abs() < 1e-10);
    }

    #[test]
    fn test_marginal_pd_absorbing_state() {
        // Curve hits full default at t=1. Marginal PD for any subsequent
        // interval must be 0 (already defaulted), not 1 (double-counting).
        let curve = RawPdCurve::new("DEFAULT", vec![(0.0, 0.0), (1.0, 1.0), (2.0, 1.0)]).unwrap();
        assert_eq!(curve.marginal_pd("DEFAULT", 1.0, 2.0).unwrap(), 0.0);
        assert_eq!(curve.marginal_pd("DEFAULT", 1.5, 2.0).unwrap(), 0.0);
    }
}
