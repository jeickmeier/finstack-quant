//! Python bindings for Expected Credit Loss (ECL) / IFRS 9 / CECL.
//!
//! Exposes the minimum viable workflow:
//!
//! - [`PyExposure`] — a credit exposure at a reporting date.
//! - [`classify_stage`] — IFRS 9 three-stage classification with audit trail.
//! - [`compute_ecl`] — single-scenario ECL integrating marginal PD x LGD x EAD x DF.
//! - [`compute_ecl_weighted`] — probability-weighted ECL across macro scenarios.
//!
//! PD term structures are passed as ``Vec<(time_years, cumulative_pd)>`` knots
//! (wrapped by [`finstack_statements_analytics::analysis::RawPdCurve`]).

use crate::errors::display_to_py;
use finstack_statements_analytics::analysis as rust_ecl;
use pyo3::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a stage label (case-insensitive: "stage1"/"1", "stage2"/"2", "stage3"/"3").
fn parse_stage(s: &str) -> PyResult<rust_ecl::Stage> {
    let normalized: String = s
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect();
    match normalized.as_str() {
        "stage1" | "1" | "s1" => Ok(rust_ecl::Stage::Stage1),
        "stage2" | "2" | "s2" => Ok(rust_ecl::Stage::Stage2),
        "stage3" | "3" | "s3" => Ok(rust_ecl::Stage::Stage3),
        other => Err(crate::errors::value_error(format!(
            "unknown stage '{}' (expected one of: stage1/stage2/stage3 or 1/2/3)",
            other
        ))),
    }
}

/// Render a [`rust_ecl::StagingTrigger`] as a short human-readable reason.
fn trigger_reason(trigger: &rust_ecl::StagingTrigger) -> String {
    match trigger {
        rust_ecl::StagingTrigger::DpdStage3 { dpd, threshold } => {
            format!("dpd_stage3 (dpd={} > {})", dpd, threshold)
        }
        rust_ecl::StagingTrigger::DpdStage2 { dpd, threshold } => {
            format!("dpd_stage2 (dpd={} > {})", dpd, threshold)
        }
        rust_ecl::StagingTrigger::PdDeltaAbsolute { delta, threshold } => {
            format!("pd_delta_absolute (delta={:.4} > {:.4})", delta, threshold)
        }
        rust_ecl::StagingTrigger::PdDeltaRelative { ratio, threshold } => {
            format!(
                "pd_delta_relative (ratio={:.2}x > {:.2}x)",
                ratio, threshold
            )
        }
        rust_ecl::StagingTrigger::RatingDowngrade { notches, threshold } => {
            format!("rating_downgrade ({} >= {} notches)", notches, threshold)
        }
        rust_ecl::StagingTrigger::Qualitative { flag } => format!("qualitative:{}", flag),
        rust_ecl::StagingTrigger::Stage3Qualitative { flag } => {
            format!("stage3_qualitative:{}", flag)
        }
        rust_ecl::StagingTrigger::NoTrigger => "no_trigger".to_string(),
    }
}

// ---------------------------------------------------------------------------
// PyExposure
// ---------------------------------------------------------------------------

/// A single credit exposure at a reporting date.
///
/// Parameters
/// ----------
/// id : str
///     Unique identifier for the exposure.
/// ead : float
///     Exposure at default (drawn balance), in base currency.
/// lgd : float
///     Loss given default in decimal (0..1).
/// eir : float
///     Effective interest rate in decimal (used as IFRS 9 discount rate).
/// remaining_maturity : float
///     Remaining maturity in years.
/// current_pd : float
///     Current lifetime PD in decimal (0..1). Used as the BBB-rated curve value.
/// origination_pd : float
///     Lifetime PD at initial recognition, in decimal.
/// dpd : int
///     Current days past due.
#[pyclass(
    name = "Exposure",
    module = "finstack.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyExposure {
    #[pyo3(get, set)]
    pub id: String,
    #[pyo3(get, set)]
    pub ead: f64,
    #[pyo3(get, set)]
    pub lgd: f64,
    #[pyo3(get, set)]
    pub eir: f64,
    #[pyo3(get, set)]
    pub remaining_maturity: f64,
    #[pyo3(get, set)]
    pub current_pd: f64,
    #[pyo3(get, set)]
    pub origination_pd: f64,
    #[pyo3(get, set)]
    pub dpd: u32,
}

#[pymethods]
impl PyExposure {
    #[new]
    #[pyo3(signature = (id, ead, lgd, eir, remaining_maturity, current_pd, origination_pd, dpd=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: String,
        ead: f64,
        lgd: f64,
        eir: f64,
        remaining_maturity: f64,
        current_pd: f64,
        origination_pd: f64,
        dpd: Option<u32>,
    ) -> Self {
        Self {
            id,
            ead,
            lgd,
            eir,
            remaining_maturity,
            current_pd,
            origination_pd,
            dpd: dpd.unwrap_or_else(rust_ecl::binding_default_exposure_dpd),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Exposure(id='{}', ead={:.2}, lgd={:.4}, eir={:.4}, maturity={:.2}y, \
             current_pd={:.4}, origination_pd={:.4}, dpd={})",
            self.id,
            self.ead,
            self.lgd,
            self.eir,
            self.remaining_maturity,
            self.current_pd,
            self.origination_pd,
            self.dpd,
        )
    }
}

impl PyExposure {
    /// Build the underlying [`rust_ecl::Exposure`] for binding internals.
    ///
    /// Uses synthetic rating labels ("current"/"origination") so the caller
    /// can supply lifetime PDs directly without constructing full rating
    /// curves. A flat PD curve carrying the caller-supplied PD at the
    /// remaining-maturity horizon is used for SICR evaluation.
    fn to_rust(&self) -> rust_ecl::Exposure {
        rust_ecl::Exposure {
            id: self.id.clone(),
            segments: vec![],
            ead: self.ead,
            eir: self.eir,
            remaining_maturity_years: self.remaining_maturity,
            lgd: self.lgd,
            days_past_due: self.dpd,
            current_rating: Some("current".to_string()),
            origination_rating: Some("origination".to_string()),
            qualitative_flags: rust_ecl::QualitativeFlags::default(),
            consecutive_performing_periods: 0,
            previous_stage: None,
            ead_schedule: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Staging
// ---------------------------------------------------------------------------

/// A lightweight two-rating PD source wrapping the current/origination PDs
/// attached to a [`PyExposure`] for SICR comparison.
struct FlatPdSource {
    current_pd: f64,
    origination_pd: f64,
}

impl rust_ecl::PdTermStructure for FlatPdSource {
    fn cumulative_pd(&self, rating: &str, _t: f64) -> finstack_core::Result<f64> {
        match rating {
            "current" => Ok(self.current_pd),
            "origination" => Ok(self.origination_pd),
            other => Err(finstack_core::Error::Validation(format!(
                "FlatPdSource: unknown rating '{}'",
                other
            ))),
        }
    }
}

/// Classify an exposure into an IFRS 9 stage.
///
/// Parameters
/// ----------
/// exposure : Exposure
///     The credit exposure.
/// pd_delta_stage2 : float
///     Absolute PD increase threshold (e.g. ``0.01`` = 1pp) for SICR.
/// dpd_30_trigger : bool
///     When ``True``, DPD > 30 is used as a Stage 2 backstop (IFRS 9 B5.5.19).
/// dpd_90_trigger : bool
///     When ``True``, DPD > 90 forces Stage 3 (non-rebuttable backstop).
///
/// Returns
/// -------
/// tuple[str, str]
///     ``(stage, trigger_reason)``. Stage is one of ``"Stage 1"``,
///     ``"Stage 2"``, ``"Stage 3"``. The trigger reason describes the first
///     trigger that fired (or ``"no_trigger"`` for a clean Stage 1).
#[pyfunction]
#[pyo3(signature = (exposure, pd_delta_stage2=None, dpd_30_trigger=None, dpd_90_trigger=None))]
fn classify_stage(
    exposure: &PyExposure,
    pd_delta_stage2: Option<f64>,
    dpd_30_trigger: Option<bool>,
    dpd_90_trigger: Option<bool>,
) -> PyResult<(String, String)> {
    let rust_exp = exposure.to_rust();
    let pd_source = FlatPdSource {
        current_pd: exposure.current_pd,
        origination_pd: exposure.origination_pd,
    };
    let dpd_30_trigger =
        dpd_30_trigger.unwrap_or_else(rust_ecl::binding_default_classify_stage_dpd_30_trigger);
    let dpd_90_trigger =
        dpd_90_trigger.unwrap_or_else(rust_ecl::binding_default_classify_stage_dpd_90_trigger);
    let default_staging = rust_ecl::default_staging_config();

    let staging_config = rust_ecl::StagingConfig {
        pd_delta_absolute: pd_delta_stage2
            .unwrap_or_else(rust_ecl::binding_default_classify_stage_pd_delta_absolute),
        // Disable relative PD trigger by setting ratio threshold effectively out of reach.
        pd_delta_relative: f64::INFINITY,
        rating_downgrade_notches: u32::MAX,
        dpd_stage2_threshold: if dpd_30_trigger {
            default_staging.dpd_stage2_threshold
        } else {
            u32::MAX
        },
        dpd_stage3_threshold: if dpd_90_trigger {
            default_staging.dpd_stage3_threshold
        } else {
            u32::MAX
        },
        qualitative_triggers_enabled: false,
        stage3_qualitative_triggers_enabled: false,
        cure_periods_stage2_to_1: rust_ecl::binding_default_cure_periods_stage2_to_1(),
        cure_periods_stage3_to_2: rust_ecl::binding_default_cure_periods_stage3_to_2(),
    };

    let result =
        rust_ecl::classify_stage(&rust_exp, &pd_source, &staging_config).map_err(display_to_py)?;

    let reason = result
        .triggers
        .first()
        .map(trigger_reason)
        .unwrap_or_else(|| "no_trigger".to_string());

    Ok((result.stage.to_string(), reason))
}

// ---------------------------------------------------------------------------
// ECL computation
// ---------------------------------------------------------------------------

fn build_pd_curve(pd_schedule: Vec<(f64, f64)>) -> PyResult<rust_ecl::RawPdCurve> {
    // Anchor the curve at (0.0, 0.0) via the canonical Rust helper so
    // marginal PDs are well-defined from the reporting date.
    let knots = rust_ecl::RawPdCurve::anchor_knots(pd_schedule);
    rust_ecl::RawPdCurve::new("scenario", knots).map_err(display_to_py)
}

fn build_ecl_config(
    bucket_width_years: f64,
    stage3_time_to_recovery_years: Option<f64>,
) -> PyResult<rust_ecl::EclConfig> {
    let mut builder = rust_ecl::EclConfigBuilder::new().bucket_width(bucket_width_years);
    if let Some(years) = stage3_time_to_recovery_years {
        builder = builder.stage3_time_to_recovery(years);
    }
    builder.build().map_err(display_to_py)
}

fn cap_maturity(exposure: &rust_ecl::Exposure, max_horizon_years: f64) -> rust_ecl::Exposure {
    rust_ecl::Exposure {
        remaining_maturity_years: exposure.remaining_maturity_years.min(max_horizon_years),
        ..exposure.clone()
    }
}

/// Compute single-scenario ECL for one exposure.
///
/// Parameters
/// ----------
/// ead : float
///     Exposure at default.
/// pd_schedule : list[tuple[float, float]]
///     Cumulative PD curve as ``[(time_years, cumulative_pd), ...]``,
///     sorted ascending in time and monotonically non-decreasing in PD.
///     A ``(0.0, 0.0)`` knot is inserted automatically if not present.
/// lgd : float
///     Loss given default (decimal).
/// eir : float
///     Effective interest rate (decimal). Used for discounting.
/// max_horizon_years : float
///     Remaining maturity cap for the integration.
/// bucket_width_years : float
///     Width of each time bucket (e.g. ``0.25`` for quarterly).
/// stage : str
///     ``"stage1"`` (12-month ECL) or ``"stage2"``/``"stage3"`` (lifetime ECL).
/// ead_schedule : list[tuple[float, float]] | None
///     Optional EAD amortization profile as ``[(time_years, ead), ...]``
///     knots; linear interpolation with flat extrapolation.
/// stage3_time_to_recovery_years : float | None
///     Stage 3 discounting horizon to expected recovery, in years.
///
/// Returns
/// -------
/// float
///     ECL amount in the exposure's base currency.
#[pyfunction]
#[pyo3(signature = (ead, pd_schedule, lgd, eir, max_horizon_years, bucket_width_years=None, stage="stage1", ead_schedule=None, stage3_time_to_recovery_years=None))]
#[allow(clippy::too_many_arguments)]
fn compute_ecl(
    ead: f64,
    pd_schedule: Vec<(f64, f64)>,
    lgd: f64,
    eir: f64,
    max_horizon_years: f64,
    bucket_width_years: Option<f64>,
    stage: &str,
    ead_schedule: Option<Vec<(f64, f64)>>,
    stage3_time_to_recovery_years: Option<f64>,
) -> PyResult<f64> {
    let stage = parse_stage(stage)?;
    let curve = build_pd_curve(pd_schedule)?;
    let config = build_ecl_config(
        bucket_width_years.unwrap_or_else(rust_ecl::binding_default_compute_ecl_bucket_width_years),
        stage3_time_to_recovery_years,
    )?;

    let exposure = rust_ecl::Exposure {
        id: "single".to_string(),
        segments: vec![],
        ead,
        eir,
        remaining_maturity_years: max_horizon_years,
        lgd,
        days_past_due: 0,
        current_rating: Some("scenario".to_string()),
        origination_rating: Some("scenario".to_string()),
        qualitative_flags: rust_ecl::QualitativeFlags::default(),
        consecutive_performing_periods: 0,
        previous_stage: None,
        ead_schedule,
    };
    let exposure = cap_maturity(&exposure, max_horizon_years);

    let result =
        rust_ecl::compute_ecl_single(&exposure, stage, &curve, &config).map_err(display_to_py)?;
    Ok(result.ecl)
}

/// Compute probability-weighted ECL across macro scenarios.
///
/// Parameters
/// ----------
/// ead : float
///     Exposure at default.
/// scenarios : list[tuple[float, list[tuple[float, float]]]]
///     List of ``(weight, pd_schedule)`` pairs. Weights must sum to 1.0.
///     A ``(0.0, 0.0)`` knot is inserted automatically into each schedule
///     if not present (same convention as ``compute_ecl``).
/// lgd : float
///     Loss given default (decimal).
/// eir : float
///     Effective interest rate (decimal).
/// max_horizon : float
///     Remaining maturity cap.
/// stage : str
///     ``"stage1"``, ``"stage2"``, or ``"stage3"``.
/// ead_schedule : list[tuple[float, float]] | None
///     Optional EAD amortization profile as ``[(time_years, ead), ...]``
///     knots; linear interpolation with flat extrapolation.
/// stage3_time_to_recovery_years : float | None
///     Stage 3 discounting horizon to expected recovery, in years.
///
/// Returns
/// -------
/// float
///     Probability-weighted ECL in the exposure's base currency.
#[pyfunction]
#[pyo3(signature = (ead, scenarios, lgd, eir, max_horizon, stage="stage1", ead_schedule=None, stage3_time_to_recovery_years=None))]
#[allow(clippy::too_many_arguments)]
fn compute_ecl_weighted(
    ead: f64,
    scenarios: Vec<(f64, Vec<(f64, f64)>)>,
    lgd: f64,
    eir: f64,
    max_horizon: f64,
    stage: &str,
    ead_schedule: Option<Vec<(f64, f64)>>,
    stage3_time_to_recovery_years: Option<f64>,
) -> PyResult<f64> {
    let stage = parse_stage(stage)?;

    let config = build_ecl_config(
        rust_ecl::binding_default_compute_ecl_bucket_width_years(),
        stage3_time_to_recovery_years,
    )?;
    // Anchor each scenario PD schedule at (0.0, 0.0) via the same canonical
    // helper used by `compute_ecl`, so both entry points accept identical
    // curve inputs.
    let scenarios: Vec<(f64, Vec<(f64, f64)>)> = scenarios
        .into_iter()
        .map(|(weight, schedule)| (weight, rust_ecl::RawPdCurve::anchor_knots(schedule)))
        .collect();
    let exposure = rust_ecl::Exposure {
        id: "weighted".to_string(),
        segments: vec![],
        ead,
        eir,
        remaining_maturity_years: max_horizon,
        lgd,
        days_past_due: 0,
        current_rating: Some("scenario".to_string()),
        origination_rating: Some("scenario".to_string()),
        qualitative_flags: rust_ecl::QualitativeFlags::default(),
        consecutive_performing_periods: 0,
        previous_stage: None,
        ead_schedule,
    };

    let result =
        rust_ecl::compute_ecl_weighted_from_schedules(&exposure, stage, &scenarios, &config)
            .map_err(display_to_py)?;
    Ok(result.ecl)
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register ECL types and functions on the `statements_analytics` submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyExposure>()?;
    m.add_function(pyo3::wrap_pyfunction!(classify_stage, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(compute_ecl, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(compute_ecl_weighted, m)?)?;
    Ok(())
}
