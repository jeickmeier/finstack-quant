//! IFRS 9 staging enum, flags, configuration, and classification bindings.

use super::model::PyExposure;
use crate::bindings::macros::wire_methods;
use crate::bindings::statements_analytics::{enum_convert, py_bool};
use crate::errors::core_to_py;
use finstack_quant_statements_analytics::analysis as rust_ecl;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::{Deserialize, Serialize};

/// IFRS 9 impairment stage.
///
/// ``Stage.Stage1`` measures a 12-month ECL; ``Stage.Stage2`` and
/// ``Stage.Stage3`` measure lifetime ECL (Stage 3 is credit-impaired).
/// ``value`` is the serde name (``"stage1"``, ``"stage2"``, ``"stage3"``)
/// accepted wherever a stage string is taken.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import Stage
/// >>> Stage.from_str("stage2").value
/// 'stage2'
/// >>> Stage.Stage2 == Stage.from_str("stage2")
/// True
#[pyclass(
    name = "Stage",
    module = "finstack_quant.statements_analytics",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PyStage {
    /// Performing: 12-month ECL.
    Stage1,
    /// Significant increase in credit risk: lifetime ECL.
    Stage2,
    /// Credit-impaired: lifetime ECL with PD = 1.
    Stage3,
}

#[pymethods]
impl PyStage {
    /// Parse the serde name (``"stage1"``, ``"stage2"`` or ``"stage3"``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not one of the three serde names.
    #[staticmethod]
    fn from_str(value: &str) -> PyResult<Self> {
        finstack_quant_core::wire::serde_parse::<rust_ecl::Stage>(value)
            .map_err(core_to_py)
            .and_then(|stage| enum_convert(&stage))
    }

    /// Serde name used in JSON and accepted by ``compute_ecl(stage=...)``.
    #[getter]
    fn value(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(self).map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "Stage.{}",
            match self {
                PyStage::Stage1 => "Stage1",
                PyStage::Stage2 => "Stage2",
                PyStage::Stage3 => "Stage3",
            }
        )
    }
}

impl PyStage {
    pub(crate) fn to_rust(self) -> PyResult<rust_ecl::Stage> {
        enum_convert(&self)
    }

    pub(crate) fn from_rust(stage: rust_ecl::Stage) -> PyResult<Self> {
        enum_convert(&stage)
    }
}

/// Extract a stage from a `Stage` object or its serde name.
pub(crate) fn extract_stage(obj: &Bound<'_, PyAny>) -> PyResult<rust_ecl::Stage> {
    if let Ok(stage) = obj.extract::<PyStage>() {
        return stage.to_rust();
    }
    let label: String = obj.extract().map_err(|_| {
        crate::errors::value_error(
            "stage must be a Stage or one of 'stage1', 'stage2', 'stage3'".to_string(),
        )
    })?;
    finstack_quant_core::wire::serde_parse(&label).map_err(core_to_py)
}

/// Qualitative SICR and default-evidence flags for staging (IFRS 9 B5.5.17 / B5.5.37).
///
/// Every parameter is keyword-only and optional; Rust reads them as the serde
/// struct and rejects unknown keywords (``ValueError``).
///
/// Parameters
/// ----------
/// watchlist : bool
///     Exposure is on an internal watchlist (SICR indicator). Default ``False``.
/// forbearance : bool
///     Forbearance measures were granted (SICR indicator). Default ``False``.
/// adverse_conditions : bool
///     Adverse business, financial or economic conditions (SICR indicator).
///     Default ``False``.
/// custom : list[str]
///     Additional caller-defined SICR flags; any non-empty entry counts as an
///     active flag. Default ``[]``.
/// bankruptcy : bool
///     Objective evidence of default: bankruptcy or similar proceedings.
///     Default ``False``.
/// distressed_modification : bool
///     Objective evidence of default: distressed restructuring. Default ``False``.
/// cross_default : bool
///     Objective evidence of default: cross-default triggered. Default ``False``.
/// other_default_evidence : list[str]
///     Additional caller-defined default-evidence flags. Default ``[]``.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import QualitativeFlags
/// >>> QualitativeFlags(watchlist=True).active_flags
/// ['watchlist']
#[pyclass(
    name = "QualitativeFlags",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyQualitativeFlags {
    pub(crate) inner: rust_ecl::QualitativeFlags,
}

#[pymethods]
impl PyQualitativeFlags {
    #[new]
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, watchlist=False, forbearance=False, adverse_conditions=False, custom=[], bankruptcy=False, distressed_modification=False, cross_default=False, other_default_evidence=[])"
    )]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let inner: rust_ecl::QualitativeFlags =
            crate::bindings::module_utils::fields_from_py(py, fields, "QualitativeFlags")?;
        Ok(Self { inner })
    }

    /// Internal watchlist flag (SICR indicator).
    #[getter]
    fn watchlist(&self) -> bool {
        self.inner.watchlist
    }

    /// Forbearance flag (SICR indicator).
    #[getter]
    fn forbearance(&self) -> bool {
        self.inner.forbearance
    }

    /// Adverse-conditions flag (SICR indicator).
    #[getter]
    fn adverse_conditions(&self) -> bool {
        self.inner.adverse_conditions
    }

    /// Caller-defined SICR flags.
    #[getter]
    fn custom(&self) -> Vec<String> {
        self.inner.custom.clone()
    }

    /// Bankruptcy flag (objective evidence of default).
    #[getter]
    fn bankruptcy(&self) -> bool {
        self.inner.bankruptcy
    }

    /// Distressed-modification flag (objective evidence of default).
    #[getter]
    fn distressed_modification(&self) -> bool {
        self.inner.distressed_modification
    }

    /// Cross-default flag (objective evidence of default).
    #[getter]
    fn cross_default(&self) -> bool {
        self.inner.cross_default
    }

    /// Caller-defined default-evidence flags.
    #[getter]
    fn other_default_evidence(&self) -> Vec<String> {
        self.inner.other_default_evidence.clone()
    }

    /// Names of the active SICR flags, in waterfall order.
    #[getter]
    fn active_flags(&self) -> Vec<String> {
        self.inner.active_flags()
    }

    /// Names of the active default-evidence flags, in waterfall order.
    #[getter]
    fn active_default_evidence(&self) -> Vec<String> {
        self.inner.active_default_evidence()
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("QualitativeFlags", &self.inner)
    }
}

wire_methods!(
    PyQualitativeFlags,
    rust_ecl::QualitativeFlags,
    "QualitativeFlags"
);

/// IFRS 9 staging policy: SICR thresholds, days-past-due backstops, qualitative
/// switches and curing windows.
///
/// Every parameter is keyword-only and read by Rust as the serde struct: an
/// omitted keyword takes the Rust ``StagingConfig::default()`` value, so
/// ``StagingConfig()`` is the standard policy. Explicit ``None`` disables the
/// relative PD trigger; the other fields do not take ``None``.
///
/// Parameters
/// ----------
/// pd_delta_absolute : float
///     Absolute remaining-window PD increase (decimal, ``0.01`` = 1pp) that fires the
///     Stage 2 SICR trigger. Default ``0.01``.
/// pd_delta_relative : float | None
///     Finite non-negative relative remaining-window PD multiple that fires
///     Stage 2. Omitted uses ``2.0`` (PD doubled); ``None`` disables the trigger.
/// rating_downgrade_notches : int
///     Downgrade notches from origination that fire Stage 2; ``0`` disables
///     the trigger. Default ``3``.
/// rating_scale_labels : list[str] | None
///     Ordered best-to-worst rating labels used to count notches; ``None``
///     uses the 10-state S&P/Fitch scale.
/// dpd_stage2_threshold : int
///     Days past due at or above which the Stage 2 backstop fires (default 30).
/// dpd_stage3_threshold : int
///     Days past due at or above which Stage 3 is forced (default 90).
/// qualitative_triggers_enabled : bool
///     Whether any active SICR flag fires Stage 2. Default ``True``.
/// stage3_qualitative_triggers_enabled : bool
///     Whether default-evidence flags force Stage 3. Default ``True``.
/// cure_periods_stage2_to_1 : int
///     Consecutive performing periods required to cure Stage 2 to Stage 1.
///     Default ``3``.
/// cure_periods_stage3_to_2 : int
///     Consecutive performing periods required to cure Stage 3 to Stage 2.
///     Default ``12``.
///
/// Raises
/// ------
/// ValueError
///     If the absolute or enabled relative threshold is negative or non-finite.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import StagingConfig
/// >>> StagingConfig(pd_delta_absolute=0.02).pd_delta_absolute
/// 0.02
#[pyclass(
    name = "StagingConfig",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyStagingConfig {
    pub(crate) inner: rust_ecl::StagingConfig,
}

#[pymethods]
impl PyStagingConfig {
    #[new]
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, pd_delta_absolute=0.01, pd_delta_relative=2.0, rating_downgrade_notches=3, rating_scale_labels=None, dpd_stage2_threshold=30, dpd_stage3_threshold=90, qualitative_triggers_enabled=True, stage3_qualitative_triggers_enabled=True, cure_periods_stage2_to_1=3, cure_periods_stage3_to_2=12)"
    )]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let inner: rust_ecl::StagingConfig =
            crate::bindings::module_utils::fields_from_py(py, fields, "StagingConfig")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Absolute remaining-window PD increase (decimal) that fires Stage 2.
    #[getter]
    fn pd_delta_absolute(&self) -> f64 {
        self.inner.pd_delta_absolute
    }

    /// Relative remaining-window PD multiple that fires Stage 2; ``None`` disables it.
    #[getter]
    fn pd_delta_relative(&self) -> Option<f64> {
        self.inner.pd_delta_relative
    }

    /// Downgrade notches that fire Stage 2 (``0`` = disabled).
    #[getter]
    fn rating_downgrade_notches(&self) -> u32 {
        self.inner.rating_downgrade_notches
    }

    /// Ordered best-to-worst rating labels, or ``None`` for the default scale.
    #[getter]
    fn rating_scale_labels(&self) -> Option<Vec<String>> {
        self.inner.rating_scale_labels.clone()
    }

    /// Days past due at or above which the Stage 2 backstop fires.
    #[getter]
    fn dpd_stage2_threshold(&self) -> u32 {
        self.inner.dpd_stage2_threshold
    }

    /// Days past due at or above which Stage 3 is forced.
    #[getter]
    fn dpd_stage3_threshold(&self) -> u32 {
        self.inner.dpd_stage3_threshold
    }

    /// Whether active SICR flags fire Stage 2.
    #[getter]
    fn qualitative_triggers_enabled(&self) -> bool {
        self.inner.qualitative_triggers_enabled
    }

    /// Whether default-evidence flags force Stage 3.
    #[getter]
    fn stage3_qualitative_triggers_enabled(&self) -> bool {
        self.inner.stage3_qualitative_triggers_enabled
    }

    /// Performing periods required to cure Stage 2 to Stage 1.
    #[getter]
    fn cure_periods_stage2_to_1(&self) -> u32 {
        self.inner.cure_periods_stage2_to_1
    }

    /// Performing periods required to cure Stage 3 to Stage 2.
    #[getter]
    fn cure_periods_stage3_to_2(&self) -> u32 {
        self.inner.cure_periods_stage3_to_2
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        self.inner.validate().map_err(core_to_py)?;
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "StagingConfig"))
    }

    /// Deserialize from canonical JSON (unknown fields are rejected).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or a threshold is negative/non-finite.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_ecl::StagingConfig = serde_json::from_str(json)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid StagingConfig JSON"))?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("StagingConfig", &self.inner)
    }
}

/// Outcome of IFRS 9 stage classification with its trigger audit trail.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import StageResult
/// >>> StageResult.from_json('{"stage":"stage1","triggers":["no_trigger"],"cured":false}').stage.value
/// 'stage1'
#[pyclass(
    name = "StageResult",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyStageResult {
    pub(crate) inner: rust_ecl::StageResult,
}

#[pymethods]
impl PyStageResult {
    /// Assigned stage.
    #[getter]
    fn stage(&self) -> PyResult<PyStage> {
        PyStage::from_rust(self.inner.stage)
    }

    /// Ordered trigger audit trail rendered by the canonical Rust display
    /// (``["no_trigger"]`` for a clean Stage 1).
    #[getter]
    fn triggers(&self) -> Vec<String> {
        self.inner
            .triggers
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Whether the exposure was cured down from a higher previous stage.
    #[getter]
    fn cured(&self) -> bool {
        self.inner.cured
    }

    fn __repr__(&self) -> String {
        format!(
            "StageResult(stage=Stage.{:?}, triggers={:?}, cured={})",
            self.inner.stage,
            self.triggers(),
            py_bool(self.inner.cured)
        )
    }
}

wire_methods!(PyStageResult, rust_ecl::StageResult, "StageResult");

pub(super) fn staging_config(config: Option<&PyStagingConfig>) -> rust_ecl::StagingConfig {
    config.map_or_else(rust_ecl::StagingConfig::default, |c| c.inner.clone())
}

/// Classify an exposure into an IFRS 9 stage.
///
/// Runs the full Rust staging waterfall: the Stage 3 days-past-due backstop,
/// default-evidence flags, the absolute and relative PD-delta SICR tests
/// (``current_pd`` versus ``origination_pd``), the rating-downgrade notch test,
/// qualitative SICR flags, the Stage 2 days-past-due backstop and curing.
/// Both supplied PDs must cover ``min(remaining_maturity_years, 30.0)`` years from
/// reporting; the original PD must be conditional on survival to reporting.
/// The caller aligns these scalar risks to that window before classification.
///
/// Parameters
/// ----------
/// exposure : Exposure
///     The credit exposure; ``ead``, ``lgd`` and ``eir`` do not affect staging.
/// config : StagingConfig | None
///     Staging policy. ``None`` uses the canonical Rust defaults
///     (1pp absolute PD delta, 30/90 DPD backstops, qualitative triggers on).
///
/// Returns
/// -------
/// StageResult
///     Assigned ``stage`` plus the ordered ``triggers`` audit trail and the
///     ``cured`` flag.
///
/// Raises
/// ------
/// ValueError
///     If PDs are non-finite or outside [0, 1], maturity or staging thresholds
///     are invalid, or an enabled relative PD ratio is not finite. Zero
///     origination PD with positive current PD raises when PD-delta testing
///     runs; both zero PDs are stable. Stage 3 backstops skip PD-delta testing.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import Exposure, classify_stage
/// >>> exp = Exposure(
/// ...     id="loan",
/// ...     ead=1_000_000.0,
/// ...     lgd=0.45,
/// ...     eir=0.06,
/// ...     remaining_maturity_years=3.0,
/// ...     current_pd=0.02,
/// ...     origination_pd=0.015,
/// ...     days_past_due=35,
/// ... )
/// >>> classify_stage(exp).stage.value
/// 'stage2'
#[pyfunction]
#[pyo3(signature = (exposure, config=None))]
pub(super) fn classify_stage(
    exposure: &PyExposure,
    config: Option<&PyStagingConfig>,
) -> PyResult<PyStageResult> {
    let inner = rust_ecl::classify_stage(
        &exposure.inner,
        exposure.current_pd,
        exposure.origination_pd,
        &staging_config(config),
    )
    .map_err(core_to_py)?;
    Ok(PyStageResult { inner })
}
