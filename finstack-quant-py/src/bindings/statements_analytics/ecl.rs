//! Python bindings for Expected Credit Loss (ECL) / IFRS 9 / CECL.
//!
//! Exposes the simplified staging-and-measurement workflow:
//!
//! - `Exposure` — a wrapper of the Rust `Exposure` plus the two lifetime PDs
//!   the simplified SICR test compares.
//! - `Stage`, `StagingConfig`, `QualitativeFlags`, `StageResult` — typed
//!   staging inputs and output.
//! - `classify_stage` — IFRS 9 three-stage classification with audit trail.
//! - `compute_ecl` / `compute_ecl_weighted` — single-scenario and
//!   probability-weighted ECL returning the full `WeightedEclResult`
//!   (per-scenario `EclResult` with `EclBucket` rows).
//!
//! PD term structures are passed as ``[(time_years, cumulative_pd), ...]``
//! knots; a ``(0.0, 0.0)`` anchor is inserted when absent.

use crate::bindings::pandas_utils::{
    dict_to_dataframe, serde_rows_to_dataframe_with_schema, serde_to_py, ColumnSchema,
};
use crate::bindings::statements_analytics::{enum_convert, py_bool, py_opt_str};
use crate::errors::core_to_py;
use finstack_quant_statements_analytics::analysis as rust_ecl;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::{Deserialize, Serialize};

/// Column schema for `EclResult.to_dataframe`.
const BUCKET_COLUMNS: [ColumnSchema<'static>; 7] = [
    ("t_start", "float64"),
    ("t_end", "float64"),
    ("marginal_pd", "float64"),
    ("lgd", "float64"),
    ("ead", "float64"),
    ("discount_factor", "float64"),
    ("ecl", "float64"),
];

/// Column schema for `WeightedEclResult.to_dataframe`.
const WEIGHTED_BUCKET_COLUMNS: [ColumnSchema<'static>; 9] = [
    ("scenario", "str"),
    ("weight", "float64"),
    ("t_start", "float64"),
    ("t_end", "float64"),
    ("marginal_pd", "float64"),
    ("lgd", "float64"),
    ("ead", "float64"),
    ("discount_factor", "float64"),
    ("ecl", "float64"),
];

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
    #[pyo3(signature = (
        watchlist=false,
        forbearance=false,
        adverse_conditions=false,
        custom=Vec::new(),
        bankruptcy=false,
        distressed_modification=false,
        cross_default=false,
        other_default_evidence=Vec::new(),
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        watchlist: bool,
        forbearance: bool,
        adverse_conditions: bool,
        custom: Vec<String>,
        bankruptcy: bool,
        distressed_modification: bool,
        cross_default: bool,
        other_default_evidence: Vec<String>,
    ) -> Self {
        Self {
            inner: rust_ecl::QualitativeFlags {
                watchlist,
                forbearance,
                adverse_conditions,
                custom,
                bankruptcy,
                distressed_modification,
                cross_default,
                other_default_evidence,
            },
        }
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

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "QualitativeFlags"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``QualitativeFlags`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid QualitativeFlags JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("QualitativeFlags", &self.inner)
    }
}

/// IFRS 9 staging policy: SICR thresholds, days-past-due backstops, qualitative
/// switches and curing windows.
///
/// Every parameter defaults to the canonical Rust ``StagingConfig::default()``
/// value, so ``StagingConfig()`` is the standard policy.
///
/// Parameters
/// ----------
/// pd_delta_absolute : float | None
///     Absolute lifetime-PD increase (decimal, ``0.01`` = 1pp) that fires the
///     Stage 2 SICR trigger.
/// pd_delta_relative : float | None
///     Relative lifetime-PD multiple (``2.0`` = PD doubled) that fires the
///     Stage 2 SICR trigger; ``inf`` disables it.
/// rating_downgrade_notches : int | None
///     Downgrade notches from origination that fire Stage 2; ``0`` disables
///     the trigger.
/// rating_scale_labels : list[str] | None
///     Ordered best-to-worst rating labels used to count notches; ``None``
///     uses the 10-state S&P/Fitch scale.
/// dpd_stage2_threshold : int | None
///     Days past due at or above which the Stage 2 backstop fires (default 30).
/// dpd_stage3_threshold : int | None
///     Days past due at or above which Stage 3 is forced (default 90).
/// qualitative_triggers_enabled : bool | None
///     Whether any active SICR flag fires Stage 2.
/// stage3_qualitative_triggers_enabled : bool | None
///     Whether default-evidence flags force Stage 3.
/// cure_periods_stage2_to_1 : int | None
///     Consecutive performing periods required to cure Stage 2 to Stage 1.
/// cure_periods_stage3_to_2 : int | None
///     Consecutive performing periods required to cure Stage 3 to Stage 2.
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
    #[pyo3(signature = (
        pd_delta_absolute=None,
        pd_delta_relative=None,
        rating_downgrade_notches=None,
        rating_scale_labels=None,
        dpd_stage2_threshold=None,
        dpd_stage3_threshold=None,
        qualitative_triggers_enabled=None,
        stage3_qualitative_triggers_enabled=None,
        cure_periods_stage2_to_1=None,
        cure_periods_stage3_to_2=None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        pd_delta_absolute: Option<f64>,
        pd_delta_relative: Option<f64>,
        rating_downgrade_notches: Option<u32>,
        rating_scale_labels: Option<Vec<String>>,
        dpd_stage2_threshold: Option<u32>,
        dpd_stage3_threshold: Option<u32>,
        qualitative_triggers_enabled: Option<bool>,
        stage3_qualitative_triggers_enabled: Option<bool>,
        cure_periods_stage2_to_1: Option<u32>,
        cure_periods_stage3_to_2: Option<u32>,
    ) -> Self {
        let defaults = rust_ecl::StagingConfig::default();
        Self {
            inner: rust_ecl::StagingConfig {
                pd_delta_absolute: pd_delta_absolute.unwrap_or(defaults.pd_delta_absolute),
                pd_delta_relative: pd_delta_relative.unwrap_or(defaults.pd_delta_relative),
                rating_downgrade_notches: rating_downgrade_notches
                    .unwrap_or(defaults.rating_downgrade_notches),
                rating_scale_labels: rating_scale_labels.or(defaults.rating_scale_labels),
                dpd_stage2_threshold: dpd_stage2_threshold.unwrap_or(defaults.dpd_stage2_threshold),
                dpd_stage3_threshold: dpd_stage3_threshold.unwrap_or(defaults.dpd_stage3_threshold),
                qualitative_triggers_enabled: qualitative_triggers_enabled
                    .unwrap_or(defaults.qualitative_triggers_enabled),
                stage3_qualitative_triggers_enabled: stage3_qualitative_triggers_enabled
                    .unwrap_or(defaults.stage3_qualitative_triggers_enabled),
                cure_periods_stage2_to_1: cure_periods_stage2_to_1
                    .unwrap_or(defaults.cure_periods_stage2_to_1),
                cure_periods_stage3_to_2: cure_periods_stage3_to_2
                    .unwrap_or(defaults.cure_periods_stage3_to_2),
            },
        }
    }

    /// Absolute lifetime-PD increase (decimal) that fires Stage 2.
    #[getter]
    fn pd_delta_absolute(&self) -> f64 {
        self.inner.pd_delta_absolute
    }

    /// Relative lifetime-PD multiple that fires Stage 2 (``inf`` = disabled).
    #[getter]
    fn pd_delta_relative(&self) -> f64 {
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
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "StagingConfig"))
    }

    /// Deserialize from canonical JSON (unknown fields are rejected).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``StagingConfig`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid StagingConfig JSON"))?;
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

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "StageResult"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``StageResult`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid StageResult JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
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

/// A single credit exposure at a reporting date.
///
/// Wraps the Rust ``Exposure`` and carries the two lifetime PDs the
/// simplified SICR test compares. ``classify_stage`` reads days past due,
/// qualitative flags, rating labels, previous stage and performing periods
/// (``ead``, ``lgd`` and ``eir`` do not affect staging); ``compute_ecl`` prices
/// ``ead + undrawn * ccf`` with ``lgd``, ``eir``, ``remaining_maturity`` and
/// any ``ead_schedule``.
///
/// Parameters
/// ----------
/// id : str
///     Unique identifier for the exposure.
/// ead : float
///     Drawn outstanding balance at the reporting date, in base currency.
/// lgd : float
///     Loss given default as a decimal fraction in ``[0, 1]``. For Stage 3,
///     this is the undiscounted loss fraction: allowance is current EAD less
///     discounted recovery ``(1 - lgd) * EAD``.
/// eir : float
///     Effective interest rate as a decimal annual rate, used as the IFRS 9
///     discount rate.
/// remaining_maturity : float
///     Remaining maturity in years.
/// current_pd : float
///     Current lifetime probability of default as a decimal in ``[0, 1]``.
/// origination_pd : float
///     Lifetime probability of default at initial recognition, decimal.
/// dpd : int
///     Days past due. Default ``0``.
/// undrawn : float
///     Undrawn commitment in the same currency as ``ead``. Default ``0.0``.
/// ccf : float
///     Credit-conversion factor applied to ``undrawn``, decimal in ``[0, 1]``.
///     Default ``0.75`` (Basel IRB revolver).
/// current_rating : str | None
///     Current rating label, used with ``origination_rating`` for the
///     rating-downgrade trigger. Default ``None``.
/// origination_rating : str | None
///     Rating label at initial recognition. Default ``None``.
/// qualitative_flags : QualitativeFlags | None
///     SICR and default-evidence flags. Default: no flags.
/// previous_stage : Stage | str | None
///     Stage assigned at the previous reporting date, enabling the curing
///     rules. Default ``None``.
/// consecutive_performing_periods : int
///     Performing periods since the last trigger, for curing. Default ``0``.
/// ead_schedule : list[tuple[float, float]] | None
///     Optional EAD amortisation profile as ``(time_years, ead)`` knots.
/// segments : list[str] | None
///     Portfolio segment keys. Default ``[]``.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import Exposure
/// >>> Exposure("loan", 1_000_000.0, 0.45, 0.06, 3.0, 0.02, 0.015).dpd
/// 0
#[pyclass(
    name = "Exposure",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyExposure {
    pub(crate) inner: rust_ecl::Exposure,
    /// Current lifetime PD (decimal) compared against ``origination_pd``.
    #[pyo3(get, set)]
    pub current_pd: f64,
    /// Lifetime PD at initial recognition (decimal).
    #[pyo3(get, set)]
    pub origination_pd: f64,
}

#[pymethods]
impl PyExposure {
    #[new]
    #[pyo3(signature = (
        id,
        ead,
        lgd,
        eir,
        remaining_maturity,
        current_pd,
        origination_pd,
        dpd=0,
        undrawn=0.0,
        ccf=rust_ecl::DEFAULT_REVOLVER_CCF,
        current_rating=None,
        origination_rating=None,
        qualitative_flags=None,
        previous_stage=None,
        consecutive_performing_periods=0,
        ead_schedule=None,
        segments=None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: String,
        ead: f64,
        lgd: f64,
        eir: f64,
        remaining_maturity: f64,
        current_pd: f64,
        origination_pd: f64,
        dpd: u32,
        undrawn: f64,
        ccf: f64,
        current_rating: Option<String>,
        origination_rating: Option<String>,
        qualitative_flags: Option<PyQualitativeFlags>,
        previous_stage: Option<&Bound<'_, PyAny>>,
        consecutive_performing_periods: u32,
        ead_schedule: Option<Vec<(f64, f64)>>,
        segments: Option<Vec<String>>,
    ) -> PyResult<Self> {
        let previous_stage = previous_stage.map(extract_stage).transpose()?;
        Ok(Self {
            inner: rust_ecl::Exposure {
                id,
                segments: segments.unwrap_or_default(),
                ead,
                undrawn,
                ccf,
                eir,
                remaining_maturity_years: remaining_maturity,
                lgd,
                days_past_due: dpd,
                current_rating,
                origination_rating,
                qualitative_flags: qualitative_flags
                    .map(|flags| flags.inner)
                    .unwrap_or_default(),
                consecutive_performing_periods,
                previous_stage,
                ead_schedule,
            },
            current_pd,
            origination_pd,
        })
    }

    /// Unique identifier for the exposure.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[setter]
    fn set_id(&mut self, id: String) {
        self.inner.id = id;
    }

    /// Drawn outstanding balance in base currency.
    #[getter]
    fn ead(&self) -> f64 {
        self.inner.ead
    }

    #[setter]
    fn set_ead(&mut self, ead: f64) {
        self.inner.ead = ead;
    }

    /// Undrawn commitment in the same currency as ``ead``.
    #[getter]
    fn undrawn(&self) -> f64 {
        self.inner.undrawn
    }

    #[setter]
    fn set_undrawn(&mut self, undrawn: f64) {
        self.inner.undrawn = undrawn;
    }

    /// Credit-conversion factor applied to ``undrawn`` (decimal in ``[0, 1]``).
    #[getter]
    fn ccf(&self) -> f64 {
        self.inner.ccf
    }

    #[setter]
    fn set_ccf(&mut self, ccf: f64) {
        self.inner.ccf = ccf;
    }

    /// Loss given default as a decimal fraction.
    #[getter]
    fn lgd(&self) -> f64 {
        self.inner.lgd
    }

    #[setter]
    fn set_lgd(&mut self, lgd: f64) {
        self.inner.lgd = lgd;
    }

    /// Effective interest rate as a decimal annual rate.
    #[getter]
    fn eir(&self) -> f64 {
        self.inner.eir
    }

    #[setter]
    fn set_eir(&mut self, eir: f64) {
        self.inner.eir = eir;
    }

    /// Remaining maturity in years.
    #[getter]
    fn remaining_maturity(&self) -> f64 {
        self.inner.remaining_maturity_years
    }

    #[setter]
    fn set_remaining_maturity(&mut self, years: f64) {
        self.inner.remaining_maturity_years = years;
    }

    /// Days past due.
    #[getter]
    fn dpd(&self) -> u32 {
        self.inner.days_past_due
    }

    #[setter]
    fn set_dpd(&mut self, dpd: u32) {
        self.inner.days_past_due = dpd;
    }

    /// Current rating label, or ``None``.
    #[getter]
    fn current_rating(&self) -> Option<&str> {
        self.inner.current_rating.as_deref()
    }

    #[setter]
    fn set_current_rating(&mut self, rating: Option<String>) {
        self.inner.current_rating = rating;
    }

    /// Rating label at initial recognition, or ``None``.
    #[getter]
    fn origination_rating(&self) -> Option<&str> {
        self.inner.origination_rating.as_deref()
    }

    #[setter]
    fn set_origination_rating(&mut self, rating: Option<String>) {
        self.inner.origination_rating = rating;
    }

    /// SICR and default-evidence flags.
    #[getter]
    fn qualitative_flags(&self) -> PyQualitativeFlags {
        PyQualitativeFlags {
            inner: self.inner.qualitative_flags.clone(),
        }
    }

    #[setter]
    fn set_qualitative_flags(&mut self, flags: PyQualitativeFlags) {
        self.inner.qualitative_flags = flags.inner;
    }

    /// Stage at the previous reporting date, or ``None``.
    #[getter]
    fn previous_stage(&self) -> PyResult<Option<PyStage>> {
        self.inner
            .previous_stage
            .map(PyStage::from_rust)
            .transpose()
    }

    #[setter]
    fn set_previous_stage(&mut self, stage: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.previous_stage = stage.map(extract_stage).transpose()?;
        Ok(())
    }

    /// Performing periods since the last trigger.
    #[getter]
    fn consecutive_performing_periods(&self) -> u32 {
        self.inner.consecutive_performing_periods
    }

    #[setter]
    fn set_consecutive_performing_periods(&mut self, periods: u32) {
        self.inner.consecutive_performing_periods = periods;
    }

    /// EAD amortisation profile as ``(time_years, ead)`` knots, or ``None``.
    #[getter]
    fn ead_schedule(&self) -> Option<Vec<(f64, f64)>> {
        self.inner.ead_schedule.clone()
    }

    #[setter]
    fn set_ead_schedule(&mut self, schedule: Option<Vec<(f64, f64)>>) {
        self.inner.ead_schedule = schedule;
    }

    /// Portfolio segment keys.
    #[getter]
    fn segments(&self) -> Vec<String> {
        self.inner.segments.clone()
    }

    #[setter]
    fn set_segments(&mut self, segments: Vec<String>) {
        self.inner.segments = segments;
    }

    /// Export the exposure as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``id``, ``ead``, ``undrawn``, ``ccf``, ``lgd``, ``eir``,
    /// ``remaining_maturity``, ``current_pd``, ``origination_pd``, ``dpd``,
    /// ``current_rating``, ``origination_rating``. Amounts are in the
    /// exposure's base currency; ``ccf``, ``lgd`` and the PDs are decimal
    /// fractions; ``eir`` is a decimal annual rate; ``remaining_maturity`` is
    /// in years; ``dpd`` is whole days.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("id", vec![self.inner.id.clone()])?;
        data.set_item("ead", vec![self.inner.ead])?;
        data.set_item("undrawn", vec![self.inner.undrawn])?;
        data.set_item("ccf", vec![self.inner.ccf])?;
        data.set_item("lgd", vec![self.inner.lgd])?;
        data.set_item("eir", vec![self.inner.eir])?;
        data.set_item(
            "remaining_maturity",
            vec![self.inner.remaining_maturity_years],
        )?;
        data.set_item("current_pd", vec![self.current_pd])?;
        data.set_item("origination_pd", vec![self.origination_pd])?;
        data.set_item("dpd", vec![self.inner.days_past_due])?;
        data.set_item("current_rating", vec![self.inner.current_rating.clone()])?;
        data.set_item(
            "origination_rating",
            vec![self.inner.origination_rating.clone()],
        )?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "Exposure(id='{}', ead={:.2}, undrawn={:.2}, ccf={:.2}, lgd={:.4}, \
             eir={:.4}, remaining_maturity={:.2}, current_pd={:.4}, origination_pd={:.4}, \
             dpd={}, current_rating={}, origination_rating={})",
            self.inner.id,
            self.inner.ead,
            self.inner.undrawn,
            self.inner.ccf,
            self.inner.lgd,
            self.inner.eir,
            self.inner.remaining_maturity_years,
            self.current_pd,
            self.origination_pd,
            self.inner.days_past_due,
            py_opt_str(self.inner.current_rating.as_deref()),
            py_opt_str(self.inner.origination_rating.as_deref()),
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`; returns `None` if the frame
    /// cannot be built so IPython falls back to `__repr__`.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// One integration bucket of an ECL calculation.
#[pyclass(
    name = "EclBucket",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyEclBucket {
    pub(crate) inner: rust_ecl::EclBucket,
}

#[pymethods]
impl PyEclBucket {
    /// Bucket start in years.
    #[getter]
    fn t_start(&self) -> f64 {
        self.inner.t_start
    }

    /// Bucket end in years.
    #[getter]
    fn t_end(&self) -> f64 {
        self.inner.t_end
    }

    /// Marginal default probability within the bucket (decimal).
    #[getter]
    fn marginal_pd(&self) -> f64 {
        self.inner.marginal_pd
    }

    /// Loss given default applied in the bucket (decimal).
    #[getter]
    fn lgd(&self) -> f64 {
        self.inner.lgd
    }

    /// Exposure at default in the bucket, in base currency.
    #[getter]
    fn ead(&self) -> f64 {
        self.inner.ead
    }

    /// Discount factor at the bucket midpoint, or at recovery for Stage 3.
    #[getter]
    fn discount_factor(&self) -> f64 {
        self.inner.discount_factor
    }

    /// Bucket ECL contribution in base currency.
    #[getter]
    fn ecl(&self) -> f64 {
        self.inner.ecl
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("EclBucket", &self.inner)
    }
}

/// ECL for one exposure under one PD scenario, with bucket detail.
#[pyclass(
    name = "EclResult",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyEclResult {
    pub(crate) inner: rust_ecl::EclResult,
}

#[pymethods]
impl PyEclResult {
    /// Exposure identifier.
    #[getter]
    fn exposure_id(&self) -> &str {
        &self.inner.exposure_id
    }

    /// Stage used for the measurement horizon.
    #[getter]
    fn stage(&self) -> PyResult<PyStage> {
        PyStage::from_rust(self.inner.stage)
    }

    /// Total ECL in the exposure's base currency.
    #[getter]
    fn ecl(&self) -> f64 {
        self.inner.ecl
    }

    /// Measurement horizon in years.
    #[getter]
    fn horizon(&self) -> f64 {
        self.inner.horizon
    }

    /// Bucket-level contributions in time order.
    #[getter]
    fn buckets(&self) -> Vec<PyEclBucket> {
        self.inner
            .buckets
            .iter()
            .cloned()
            .map(|inner| PyEclBucket { inner })
            .collect()
    }

    /// Result metadata (numeric mode, rounding context) as a dict.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Export the buckets as a pandas ``DataFrame``.
    ///
    /// Columns: ``t_start``, ``t_end`` (years), ``marginal_pd``, ``lgd``
    /// (decimals), ``ead``, ``ecl`` (base currency), ``discount_factor``.
    /// One row per bucket in time order.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.inner.buckets, &BUCKET_COLUMNS)
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "EclResult"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``EclResult`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid EclResult JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "EclResult(exposure_id='{}', stage=Stage.{:?}, ecl={}, horizon={}, buckets={})",
            self.inner.exposure_id,
            self.inner.stage,
            self.inner.ecl,
            self.inner.horizon,
            self.inner.buckets.len()
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Probability-weighted ECL across macro scenarios with the per-scenario
/// audit trail.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import Exposure, compute_ecl
/// >>> exp = Exposure("loan", 1_000_000.0, 0.45, 0.06, 3.0, 0.02, 0.015)
/// >>> result = compute_ecl(exp, [(1.0, 0.02), (3.0, 0.06)], stage="stage2")
/// >>> result.stage.value
/// 'stage2'
/// >>> list(result.to_dataframe().columns)[:2]
/// ['scenario', 'weight']
#[pyclass(
    name = "WeightedEclResult",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyWeightedEclResult {
    pub(crate) inner: rust_ecl::WeightedEclResult,
}

#[pymethods]
impl PyWeightedEclResult {
    /// Exposure identifier.
    #[getter]
    fn exposure_id(&self) -> &str {
        &self.inner.exposure_id
    }

    /// Stage used for the measurement horizon.
    #[getter]
    fn stage(&self) -> PyResult<PyStage> {
        PyStage::from_rust(self.inner.stage)
    }

    /// Probability-weighted ECL in the exposure's base currency.
    #[getter]
    fn ecl(&self) -> f64 {
        self.inner.ecl
    }

    /// Per-scenario ``(scenario_id, weight, EclResult)`` triples.
    #[getter]
    fn scenario_breakdown(&self) -> Vec<(String, f64, PyEclResult)> {
        self.inner
            .scenario_breakdown
            .iter()
            .map(|(id, weight, result)| {
                (
                    id.clone(),
                    *weight,
                    PyEclResult {
                        inner: result.clone(),
                    },
                )
            })
            .collect()
    }

    /// Result metadata (numeric mode, rounding context) as a dict.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Export the scenario x bucket audit trail as a pandas ``DataFrame``.
    ///
    /// Columns: ``scenario`` (scenario id), ``weight`` (decimal probability),
    /// ``t_start``, ``t_end`` (years), ``marginal_pd``, ``lgd`` (decimals),
    /// ``ead``, ``ecl`` (base currency, unweighted per scenario),
    /// ``discount_factor``. One row per (scenario, bucket).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .scenario_breakdown
            .iter()
            .flat_map(|(id, weight, result)| {
                result.buckets.iter().map(move |bucket| {
                    serde_json::json!({
                        "scenario": id,
                        "weight": weight,
                        "t_start": bucket.t_start,
                        "t_end": bucket.t_end,
                        "marginal_pd": bucket.marginal_pd,
                        "lgd": bucket.lgd,
                        "ead": bucket.ead,
                        "discount_factor": bucket.discount_factor,
                        "ecl": bucket.ecl,
                    })
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, &WEIGHTED_BUCKET_COLUMNS)
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "WeightedEclResult"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``WeightedEclResult`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid WeightedEclResult JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "WeightedEclResult(exposure_id='{}', stage=Stage.{:?}, ecl={}, scenarios={})",
            self.inner.exposure_id,
            self.inner.stage,
            self.inner.ecl,
            self.inner.scenario_breakdown.len()
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

fn staging_config(config: Option<&PyStagingConfig>) -> rust_ecl::StagingConfig {
    config.map_or_else(rust_ecl::StagingConfig::default, |c| c.inner.clone())
}

/// Classify an exposure into an IFRS 9 stage.
///
/// Runs the full Rust staging waterfall: the Stage 3 days-past-due backstop,
/// default-evidence flags, the absolute and relative PD-delta SICR tests
/// (``current_pd`` versus ``origination_pd``), the rating-downgrade notch test,
/// qualitative SICR flags, the Stage 2 days-past-due backstop and curing.
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
///     If PDs are outside [0, 1], or maturity or staging thresholds are invalid.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import Exposure, classify_stage
/// >>> exp = Exposure("loan", 1_000_000.0, 0.45, 0.06, 3.0, 0.02, 0.015, dpd=35)
/// >>> classify_stage(exp).stage.value
/// 'stage2'
#[pyfunction]
#[pyo3(signature = (exposure, config=None))]
fn classify_stage(
    exposure: &PyExposure,
    config: Option<&PyStagingConfig>,
) -> PyResult<PyStageResult> {
    let inner = rust_ecl::classify_exposure(
        &exposure.inner,
        exposure.current_pd,
        exposure.origination_pd,
        &staging_config(config),
    )
    .map_err(core_to_py)?;
    Ok(PyStageResult { inner })
}

fn resolve_stage(
    exposure: &PyExposure,
    stage: Option<&Bound<'_, PyAny>>,
) -> PyResult<rust_ecl::Stage> {
    match stage {
        Some(stage) => extract_stage(stage),
        None => Ok(classify_stage(exposure, None)?.inner.stage),
    }
}

/// Compute single-scenario ECL for one exposure.
///
/// The priced exposure at default is ``ead + undrawn * ccf``; ``lgd``, ``eir``,
/// ``remaining_maturity`` and any ``ead_schedule`` are read from the exposure.
///
/// Parameters
/// ----------
/// exposure : Exposure
///     The credit exposure to measure.
/// pd_schedule : list[tuple[float, float]]
///     Cumulative PD curve as ``[(time_years, cumulative_pd), ...]``, ascending
///     in time and non-decreasing in PD. A ``(0.0, 0.0)`` knot is inserted
///     when absent.
/// stage : Stage | str | None
///     Measurement stage (``Stage`` or serde name ``"stage1"``/``"stage2"``/
///     ``"stage3"``). ``None`` classifies the exposure first with the default
///     ``StagingConfig``.
/// bucket_width_years : float | None
///     Finite integration width of at least 0.0001 years (``0.25`` = quarterly); ``None`` uses
///     the canonical policy default.
/// stage3_time_to_recovery_years : float | None
///     Stage 3 discounting horizon to expected recovery, in years; ``None``
///     uses the canonical policy default.
///
/// Returns
/// -------
/// WeightedEclResult
///     ``ecl`` in base currency, the ``stage`` used, and a one-scenario
///     ``scenario_breakdown`` with bucket detail (``to_dataframe()``).
///
/// Raises
/// ------
/// ValueError
///     If ``stage`` is unknown, the PD or EAD schedule is invalid, or an
///     exposure input is outside its accepted range.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import Exposure, compute_ecl
/// >>> exp = Exposure("loan", 1_000_000.0, 0.45, 0.06, 3.0, 0.02, 0.015)
/// >>> compute_ecl(exp, [(1.0, 0.02), (3.0, 0.06)], stage="stage1").ecl > 0
/// True
#[pyfunction]
#[pyo3(signature = (exposure, pd_schedule, stage=None, bucket_width_years=None, stage3_time_to_recovery_years=None))]
fn compute_ecl(
    exposure: &PyExposure,
    pd_schedule: Vec<(f64, f64)>,
    stage: Option<&Bound<'_, PyAny>>,
    bucket_width_years: Option<f64>,
    stage3_time_to_recovery_years: Option<f64>,
) -> PyResult<PyWeightedEclResult> {
    let stage = resolve_stage(exposure, stage)?;
    let inner = rust_ecl::compute_ecl_for_exposure(
        &exposure.inner,
        stage,
        &[(1.0, pd_schedule)],
        bucket_width_years,
        stage3_time_to_recovery_years,
    )
    .map_err(core_to_py)?;
    Ok(PyWeightedEclResult { inner })
}

/// Compute probability-weighted ECL across macro scenarios.
///
/// Parameters
/// ----------
/// exposure : Exposure
///     The credit exposure to measure (EAD is ``ead + undrawn * ccf``).
/// scenarios : list[tuple[float, list[tuple[float, float]]]]
///     ``(weight, pd_schedule)`` pairs; weights must sum to ``1.0`` and each
///     schedule follows the ``compute_ecl`` conventions.
/// stage : Stage | str | None
///     Measurement stage; ``None`` classifies the exposure first with the
///     default ``StagingConfig``.
/// bucket_width_years : float | None
///     Finite integration width of at least 0.0001 years; ``None`` uses the canonical default.
/// stage3_time_to_recovery_years : float | None
///     Stage 3 discounting horizon in years; ``None`` uses the canonical default.
///
/// Returns
/// -------
/// WeightedEclResult
///     Probability-weighted ``ecl`` with one ``scenario_breakdown`` entry per
///     scenario.
///
/// Raises
/// ------
/// ValueError
///     If ``scenarios`` is empty, weights do not sum to ``1.0``, ``stage`` is
///     unknown, a schedule is invalid, or an exposure input is out of range.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import Exposure, compute_ecl_weighted
/// >>> exp = Exposure("loan", 1_000_000.0, 0.45, 0.06, 1.0, 0.02, 0.015)
/// >>> scenarios = [(0.7, [(1.0, 0.02)]), (0.3, [(1.0, 0.05)])]
/// >>> len(compute_ecl_weighted(exp, scenarios, stage="stage1").scenario_breakdown)
/// 2
#[pyfunction]
#[pyo3(signature = (exposure, scenarios, stage=None, bucket_width_years=None, stage3_time_to_recovery_years=None))]
fn compute_ecl_weighted(
    exposure: &PyExposure,
    scenarios: Vec<(f64, Vec<(f64, f64)>)>,
    stage: Option<&Bound<'_, PyAny>>,
    bucket_width_years: Option<f64>,
    stage3_time_to_recovery_years: Option<f64>,
) -> PyResult<PyWeightedEclResult> {
    let stage = resolve_stage(exposure, stage)?;
    let inner = rust_ecl::compute_ecl_for_exposure(
        &exposure.inner,
        stage,
        &scenarios,
        bucket_width_years,
        stage3_time_to_recovery_years,
    )
    .map_err(core_to_py)?;
    Ok(PyWeightedEclResult { inner })
}

/// Register ECL types and functions on the `statements_analytics` submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStage>()?;
    m.add_class::<PyQualitativeFlags>()?;
    m.add_class::<PyStagingConfig>()?;
    m.add_class::<PyStageResult>()?;
    m.add_class::<PyExposure>()?;
    m.add_class::<PyEclBucket>()?;
    m.add_class::<PyEclResult>()?;
    m.add_class::<PyWeightedEclResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(classify_stage, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(compute_ecl, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(compute_ecl_weighted, m)?)?;
    Ok(())
}
