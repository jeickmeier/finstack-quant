//! Performing and lifetime ECL exposure, bucket, scenario, and calculation bindings.

use super::staging::{PyQualitativeFlags, PyStage, extract_stage};
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::{tabular_methods, wire_methods};
use crate::bindings::statements_analytics::py_opt_str;
use crate::bindings::table_builders::{TableBuilder, serde_rows_to_arrow_table};
use crate::errors::core_to_py;
use finstack_quant_statements_analytics::analysis as rust_ecl;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Column schema for [`PyEclResult`] tabular exports.
const BUCKET_COLUMNS: [(&str, &str); 7] = [
    ("t_start", "float64"),
    ("t_end", "float64"),
    ("marginal_pd", "float64"),
    ("lgd", "float64"),
    ("ead", "float64"),
    ("discount_factor", "float64"),
    ("ecl", "float64"),
];

/// Column schema for [`PyWeightedEclResult`] tabular exports.
const WEIGHTED_BUCKET_COLUMNS: [(&str, &str); 9] = [
    ("scenario", "str"),
    ("weight", "str"),
    ("t_start", "float64"),
    ("t_end", "float64"),
    ("marginal_pd", "float64"),
    ("lgd", "float64"),
    ("ead", "float64"),
    ("discount_factor", "float64"),
    ("ecl", "float64"),
];

/// A single credit exposure at a reporting date.
///
/// Wraps the Rust ``Exposure`` and carries the two remaining-window PDs the
/// simplified SICR test compares. ``classify_stage`` reads days past due,
/// qualitative flags, rating labels, previous stage and performing periods
/// (``ead``, ``lgd`` and ``eir`` do not affect staging); ``compute_ecl`` prices
/// ``ead + undrawn * ccf`` with ``lgd``, ``eir``, ``remaining_maturity_years`` and
/// any ``ead_schedule``.
///
/// Every parameter is keyword-only. ``current_pd`` and ``origination_pd`` are
/// held by the Python object; Rust reads the other keywords as its serde
/// ``Exposure``, supplying the defaults and rejecting unknown keywords. Keywords
/// and properties carry the Rust field names.
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
/// remaining_maturity_years : float
///     Remaining maturity in years.
/// current_pd : float
///     Current probability of default from reporting over
///     ``h = min(remaining_maturity_years, 30.0)`` years, as a decimal in ``[0, 1]``.
/// origination_pd : float
///     Initial-recognition expected PD for that same remaining window,
///     conditional on survival to the reporting date, as a decimal in ``[0, 1]``.
///     For original cumulative curve ``F``, elapsed years ``a`` and the capped
///     horizon ``h = min(remaining_maturity_years, 30.0)``, supply
///     ``(F(a + h) - F(a)) / (1 - F(a))``.
/// days_past_due : int
///     Days past due. Default ``0`` (performing).
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
/// qualitative_flags : QualitativeFlags
///     SICR and default-evidence flags. Default: no flags.
/// previous_stage : Stage | str | None
///     Stage assigned at the previous reporting date, enabling the curing
///     rules. Accepts a ``Stage`` or ``"stage1"``, ``"stage2"``, or
///     ``"stage3"``. Default ``None``.
/// consecutive_performing_periods : int
///     Performing periods since the last trigger, for curing. Default ``0``.
/// ead_schedule : list[tuple[float, float]] | None
///     Optional EAD amortisation profile as ``(time_years, ead)`` knots.
/// segments : list[str]
///     Portfolio segment keys. Default ``[]``.
///
/// Raises
/// ------
/// ValueError
///     If a required keyword is missing, a keyword is unknown, a value has
///     the wrong wire form, or ``previous_stage`` is neither a ``Stage`` nor a
///     recognized stage string. Financial inputs are validated by staging and
///     ECL calculations.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import Exposure
/// >>> Exposure(
/// ...     id="loan",
/// ...     ead=1_000_000.0,
/// ...     lgd=0.45,
/// ...     eir=0.06,
/// ...     remaining_maturity_years=3.0,
/// ...     current_pd=0.02,
/// ...     origination_pd=0.015,
/// ... ).days_past_due
/// 0
#[pyclass(
    name = "Exposure",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyExposure {
    pub(crate) inner: rust_ecl::Exposure,
    /// Current reporting-date SICR PD over min(remaining maturity, 30 years),
    /// decimal in [0, 1].
    #[pyo3(get, set)]
    pub current_pd: f64,
    /// Initial-recognition expected PD for the same capped remaining window,
    /// conditional on survival to the reporting date, decimal in [0, 1].
    #[pyo3(get, set)]
    pub origination_pd: f64,
}

/// Remove one of the two staging PDs from the constructor keywords.
///
/// ``current_pd`` and ``origination_pd`` belong to the Python ``Exposure``
/// (they feed ``classify_stage``), not to the Rust ``Exposure`` the remaining
/// keywords deserialize into.
fn take_staging_pd(fields: &Bound<'_, PyDict>, name: &str) -> PyResult<f64> {
    let value = fields.get_item(name)?.ok_or_else(|| {
        crate::errors::value_error(format!("invalid Exposure: missing field `{name}`"))
    })?;
    fields.del_item(name)?;
    value.extract()
}

#[pymethods]
impl PyExposure {
    #[new]
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, id, ead, lgd, eir, remaining_maturity_years, current_pd, origination_pd, days_past_due=0, undrawn=0.0, ccf=0.75, current_rating=None, origination_rating=None, qualitative_flags=..., previous_stage=None, consecutive_performing_periods=0, ead_schedule=None, segments=[])"
    )]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let fields = match fields {
            Some(fields) => fields.copy()?,
            None => PyDict::new(py),
        };
        let current_pd = take_staging_pd(&fields, "current_pd")?;
        let origination_pd = take_staging_pd(&fields, "origination_pd")?;
        let inner: rust_ecl::Exposure =
            crate::bindings::module_utils::fields_from_py(py, Some(&fields), "Exposure")?;
        Ok(Self {
            inner,
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
    fn remaining_maturity_years(&self) -> f64 {
        self.inner.remaining_maturity_years
    }

    #[setter]
    fn set_remaining_maturity_years(&mut self, years: f64) {
        self.inner.remaining_maturity_years = years;
    }

    /// Days past due.
    #[getter]
    fn days_past_due(&self) -> u32 {
        self.inner.days_past_due
    }

    #[setter]
    fn set_days_past_due(&mut self, days_past_due: u32) {
        self.inner.days_past_due = days_past_due;
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

    fn __repr__(&self) -> String {
        format!(
            "Exposure(id='{}', ead={:.2}, undrawn={:.2}, ccf={:.2}, lgd={:.4}, \
             eir={:.4}, remaining_maturity_years={:.2}, current_pd={:.4}, origination_pd={:.4}, \
             days_past_due={}, current_rating={}, origination_rating={})",
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
}

impl PyExposure {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .utf8("id", vec![self.inner.id.clone()])?
            .f64("ead", vec![self.inner.ead])?
            .f64("undrawn", vec![self.inner.undrawn])?
            .f64("ccf", vec![self.inner.ccf])?
            .f64("lgd", vec![self.inner.lgd])?
            .f64("eir", vec![self.inner.eir])?
            .f64(
                "remaining_maturity_years",
                vec![self.inner.remaining_maturity_years],
            )?
            .f64("current_pd", vec![self.current_pd])?
            .f64("origination_pd", vec![self.origination_pd])?
            .i64("days_past_due", vec![i64::from(self.inner.days_past_due)])?
            .nullable_utf8("current_rating", vec![self.inner.current_rating.clone()])?
            .nullable_utf8(
                "origination_rating",
                vec![self.inner.origination_rating.clone()],
            )?
            .build()
    }
}

tabular_methods!(PyExposure);

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
}

wire_methods!(PyEclResult, rust_ecl::EclResult, "EclResult");

impl PyEclResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(&self.inner.buckets, &BUCKET_COLUMNS)
    }
}

tabular_methods!(PyEclResult);

#[pyclass(
    name = "EclScenario",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.model", "EclScenario")]
#[derive(Clone)]
pub(super) struct PyEclScenario {
    inner: rust_ecl::EclScenario,
}

#[pymethods]
impl PyEclScenario {
    #[new]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.model", "EclScenario.__init__")]
    #[pyo3(signature = (*, id, weight, pd_schedule=None, lgd_override=None))]
    fn new(
        id: String,
        weight: &Bound<'_, PyAny>,
        pd_schedule: Option<Vec<(f64, f64)>>,
        lgd_override: Option<f64>,
    ) -> PyResult<Self> {
        let weight = crate::bindings::core::money::extract_exact_decimal(
            weight,
            "weight must be a decimal probability",
        )?;
        Ok(Self {
            inner: rust_ecl::EclScenario {
                id,
                weight: finstack_quant_core::wire::DecimalWire(weight),
                pd_schedule,
                lgd_override,
            },
        })
    }
}

#[pyclass(
    name = "EclScenarioOutcome",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.model", "EclScenarioOutcome")]
#[derive(Clone)]
pub(super) struct PyEclScenarioOutcome {
    inner: rust_ecl::EclScenarioOutcome,
}

#[pymethods]
impl PyEclScenarioOutcome {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.model", "EclScenarioOutcome.id")]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.model", "EclScenarioOutcome.weight")]
    fn weight<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::money::decimal_to_py(py, self.inner.weight.0)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.model", "EclScenarioOutcome.lgd_override")]
    fn lgd_override(&self) -> Option<f64> {
        self.inner.lgd_override
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.model", "EclScenarioOutcome.status")]
    fn status(&self) -> &str {
        match &self.inner.measurement {
            rust_ecl::EclScenarioMeasurement::Measured { .. } => "measured",
            rust_ecl::EclScenarioMeasurement::Unavailable { .. } => "unavailable",
            rust_ecl::EclScenarioMeasurement::ZeroWeight => "zero_weight",
        }
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.model", "EclScenarioOutcome.result")]
    fn result(&self) -> Option<PyEclResult> {
        match &self.inner.measurement {
            rust_ecl::EclScenarioMeasurement::Measured { result, .. } => Some(PyEclResult {
                inner: result.as_ref().clone(),
            }),
            _ => None,
        }
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.model", "EclScenarioOutcome.weighted_allowance")]
    fn weighted_allowance(&self) -> Option<f64> {
        match &self.inner.measurement {
            rust_ecl::EclScenarioMeasurement::Measured {
                weighted_allowance, ..
            } => Some(*weighted_allowance),
            rust_ecl::EclScenarioMeasurement::ZeroWeight => Some(0.0),
            rust_ecl::EclScenarioMeasurement::Unavailable { .. } => None,
        }
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.model", "EclScenarioOutcome.reason")]
    fn reason(&self) -> Option<&str> {
        match &self.inner.measurement {
            rust_ecl::EclScenarioMeasurement::Unavailable { reason } => Some(reason),
            _ => None,
        }
    }
}

/// Probability-weighted ECL across macro scenarios with the per-scenario
/// audit trail.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import Exposure, compute_ecl
/// >>> exp = Exposure(
/// ...     id="loan",
/// ...     ead=1_000_000.0,
/// ...     lgd=0.45,
/// ...     eir=0.06,
/// ...     remaining_maturity_years=3.0,
/// ...     current_pd=0.02,
/// ...     origination_pd=0.015,
/// ... )
/// >>> result = compute_ecl(exp, [(1.0, 0.02), (3.0, 0.06)], stage="stage2")
/// >>> result.stage.value
/// 'stage2'
/// >>> list(result.to_pandas().columns)[:2]
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

    /// Probability-weighted ECL in exposure currency, or None when evidence is incomplete.
    #[getter]
    fn ecl(&self) -> Option<f64> {
        self.inner.ecl
    }

    /// Named scenarios, exact weights, and allowance availability in request order.
    #[getter]
    fn scenario_breakdown(&self) -> Vec<PyEclScenarioOutcome> {
        self.inner
            .scenario_breakdown
            .iter()
            .cloned()
            .map(|inner| PyEclScenarioOutcome { inner })
            .collect()
    }

    /// Result metadata (numeric mode, rounding context) as a dict.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    fn __repr__(&self) -> String {
        format!(
            "WeightedEclResult(exposure_id='{}', stage=Stage.{:?}, ecl={:?}, scenarios={})",
            self.inner.exposure_id,
            self.inner.stage,
            self.inner.ecl,
            self.inner.scenario_breakdown.len()
        )
    }
}

wire_methods!(
    PyWeightedEclResult,
    rust_ecl::WeightedEclResult,
    "WeightedEclResult"
);

impl PyWeightedEclResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .scenario_breakdown
            .iter()
            .filter_map(|scenario| match &scenario.measurement {
                rust_ecl::EclScenarioMeasurement::Measured { result, .. } => {
                    Some((scenario, result))
                }
                _ => None,
            })
            .flat_map(|(scenario, result)| {
                result.buckets.iter().map(move |bucket| {
                    serde_json::json!({
                        "scenario": scenario.id,
                        "weight": scenario.weight,
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
        serde_rows_to_arrow_table(&rows, &WEIGHTED_BUCKET_COLUMNS)
    }
}

tabular_methods!(PyWeightedEclResult);

/// Compute single-scenario ECL for one exposure.
///
/// The priced exposure at default is ``ead + undrawn * ccf``; ``lgd``, ``eir``,
/// ``remaining_maturity_years`` and any ``ead_schedule`` are read from the exposure.
///
/// Parameters
/// ----------
/// exposure : Exposure
///     The credit exposure to measure.
/// pd_schedule : list[tuple[float, float]]
///     Cumulative PD curve as ``[(time_years, cumulative_pd), ...]``, ascending
///     in time and non-decreasing in PD. A ``(0.0, 0.0)`` knot is inserted
///     when absent.
/// stage : Stage | str
///     Measurement stage (``Stage`` or serde name ``"stage1"`` or ``"stage2"``). Required: classify first with ``classify_stage`` under
///     the desk's ``StagingConfig`` and pass ``result.stage``. For Stage 3 use ``compute_impaired_ecl``.
/// bucket_width_years : float | None
///     Finite integration width of at least 0.0001 years (``0.25`` = quarterly); ``None`` uses
///     the canonical policy default.
///
/// Returns
/// -------
/// WeightedEclResult
///     ``ecl`` in base currency, the ``stage`` used, and a one-scenario
///     ``scenario_breakdown`` with bucket detail (``to_pandas()`` / ``to_arrow()``).
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
/// >>> exp = Exposure(
/// ...     id="loan",
/// ...     ead=1_000_000.0,
/// ...     lgd=0.45,
/// ...     eir=0.06,
/// ...     remaining_maturity_years=3.0,
/// ...     current_pd=0.02,
/// ...     origination_pd=0.015,
/// ... )
/// >>> compute_ecl(exp, [(1.0, 0.02), (3.0, 0.06)], stage="stage1").ecl > 0
/// True
#[pyfunction]
#[pyo3(signature = (exposure, pd_schedule, stage, bucket_width_years=None))]
pub(super) fn compute_ecl(
    py: Python<'_>,
    exposure: &PyExposure,
    pd_schedule: Vec<(f64, f64)>,
    stage: &Bound<'_, PyAny>,
    bucket_width_years: Option<f64>,
) -> PyResult<PyWeightedEclResult> {
    let stage = extract_stage(stage)?;
    let exposure = exposure.inner.clone();
    let inner = py
        .detach(move || rust_ecl::compute_ecl(&exposure, stage, pd_schedule, bucket_width_years))
        .map_err(core_to_py)?;
    Ok(PyWeightedEclResult { inner })
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.model", "compute_ecl_weighted")]
#[pyfunction]
#[pyo3(signature = (exposure, scenarios, stage, bucket_width_years=None))]
pub(super) fn compute_ecl_weighted(
    py: Python<'_>,
    exposure: &PyExposure,
    scenarios: Vec<PyEclScenario>,
    stage: &Bound<'_, PyAny>,
    bucket_width_years: Option<f64>,
) -> PyResult<PyWeightedEclResult> {
    let stage = extract_stage(stage)?;
    let exposure = exposure.inner.clone();
    let scenarios = scenarios
        .into_iter()
        .map(|scenario| scenario.inner)
        .collect::<Vec<_>>();
    let inner = py
        .detach(move || {
            rust_ecl::compute_ecl_weighted(&exposure, stage, &scenarios, bucket_width_years)
        })
        .map_err(core_to_py)?;
    Ok(PyWeightedEclResult { inner })
}
