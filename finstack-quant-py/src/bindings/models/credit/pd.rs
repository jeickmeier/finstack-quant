//! Python bindings for `finstack_quant_models::credit::pd` (calibration subset).

use finstack_quant_models::credit::pd::{
    apply_basel_irb_pd_floor as core_apply_basel_irb_pd_floor,
    central_tendency as core_central_tendency, pit_to_ttc as core_pit_to_ttc,
    ttc_to_pit as core_ttc_to_pit, MasterScale, MasterScaleGrade, MasterScaleResult, PdCycleParams,
    BASEL_IRB_PD_FLOOR,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use super::scoring::PyScoringResult;
use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    ColumnSchema,
};
use crate::errors::{core_to_py, pd_calibration_to_py, serde_json_to_py};

/// Column schema shared by `MasterScaleResult.to_dataframe` and
/// `MasterScale.map_pds`.
const RESULT_COLUMNS: &[ColumnSchema<'static>] = &[
    ("grade", "str"),
    ("grade_index", "int64"),
    ("input_pd", "float64"),
    ("central_pd", "float64"),
];

/// Convert a Point-in-Time PD to a Through-the-Cycle PD.
///
/// Uses the Merton-Vasicek single-factor model (Basel II IRB):
///
///   PD_TtC = Phi( Phi^{-1}(PD_PiT) * sqrt(1 - rho) + sqrt(rho) * z )
///
/// Parameters
/// ----------
/// pd_pit : float
///     Point-in-Time PD as a decimal in (0, 1).
/// asset_correlation : float
///     Asset correlation rho in (0, 1). Basel uses 0.12 - 0.24 for corporates.
/// cycle_index : float
///     Systematic risk factor z: 0 = average, < 0 = downturn, > 0 = benign.
///
/// Returns the Through-the-Cycle PD as a decimal.
///
/// Raises ``ValueError`` when ``pd_pit`` or ``asset_correlation`` is outside
/// (0, 1) or any input is non-finite.
#[pyfunction]
#[pyo3(text_signature = "(pd_pit, asset_correlation, cycle_index)")]
fn pit_to_ttc(pd_pit: f64, asset_correlation: f64, cycle_index: f64) -> PyResult<f64> {
    let params = PdCycleParams {
        asset_correlation,
        cycle_index,
    };
    core_pit_to_ttc(pd_pit, &params).map_err(pd_calibration_to_py)
}

/// Convert a Through-the-Cycle PD to a Point-in-Time PD.
///
/// Uses the Merton-Vasicek single-factor model (Basel II IRB):
///
///   PD_PiT = Phi( (Phi^{-1}(PD_TtC) - sqrt(rho) * z) / sqrt(1 - rho) )
///
/// Parameters
/// ----------
/// pd_ttc : float
///     Through-the-Cycle PD as a decimal in (0, 1).
/// asset_correlation : float
///     Asset correlation rho in (0, 1).
/// cycle_index : float
///     Systematic risk factor z: 0 = average, < 0 = downturn, > 0 = benign.
///
/// Returns the Point-in-Time PD as a decimal.
///
/// Raises ``ValueError`` when ``pd_ttc`` or ``asset_correlation`` is outside
/// (0, 1) or any input is non-finite.
#[pyfunction]
#[pyo3(text_signature = "(pd_ttc, asset_correlation, cycle_index)")]
fn ttc_to_pit(pd_ttc: f64, asset_correlation: f64, cycle_index: f64) -> PyResult<f64> {
    let params = PdCycleParams {
        asset_correlation,
        cycle_index,
    };
    core_ttc_to_pit(pd_ttc, &params).map_err(pd_calibration_to_py)
}

/// Calibrate a central tendency (long-run average PD) from annual default rates
/// using the arithmetic mean (the standard regulatory TtC approach per
/// Basel IRB / EBA GL/2017/16).
///
/// Zero-default years are valid observations and are included in the average.
///
/// Parameters
/// ----------
/// annual_default_rates : list[float]
///     Observed annual default rates as decimals in [0, 1]; at least one.
///
/// Returns the arithmetic mean in [0, 1].
///
/// Raises ``ValueError`` when the list is empty or any rate is non-finite or
/// outside [0, 1].
#[pyfunction]
#[pyo3(text_signature = "(annual_default_rates)")]
fn central_tendency(annual_default_rates: Vec<f64>) -> PyResult<f64> {
    core_central_tendency(&annual_default_rates).map_err(pd_calibration_to_py)
}

/// Apply the Basel IRB corporate PD floor: ``max(pd, BASEL_IRB_PD_FLOOR)``.
///
/// Parameters
/// ----------
/// pd : float
///     Probability of default as a decimal.
///
/// Returns the floored PD (``0.0003`` when ``pd`` is below 3 bp).
#[pyfunction]
#[pyo3(text_signature = "(pd)")]
fn apply_basel_irb_pd_floor(pd: f64) -> f64 {
    core_apply_basel_irb_pd_floor(pd)
}

/// One PD band in a rating master scale.
///
/// ``upper_pd`` is the inclusive upper bound of the band and ``central_pd`` the
/// representative PD assigned to anything mapped into it; both are decimals.
#[pyclass(
    module = "finstack_quant.models.credit.pd",
    name = "MasterScaleGrade",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyMasterScaleGrade {
    inner: MasterScaleGrade,
}

#[pymethods]
impl PyMasterScaleGrade {
    /// Construct one probability-of-default band on a master scale.
    ///
    /// Parameters
    /// ----------
    /// label : str
    ///     Grade label (e.g. ``"BBB"``).
    /// upper_pd : float
    ///     Inclusive upper PD bound of the band, a decimal in (0, 1].
    /// central_pd : float
    ///     Representative PD assigned to the band, a decimal in (0, 1).
    ///
    /// Validation happens when the grade is placed in a ``MasterScale``.
    #[new]
    #[pyo3(text_signature = "(label, upper_pd, central_pd)")]
    fn new(label: String, upper_pd: f64, central_pd: f64) -> Self {
        Self {
            inner: MasterScaleGrade {
                label,
                upper_pd,
                central_pd,
            },
        }
    }

    /// Grade label (e.g. ``"BBB"``).
    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }

    /// Inclusive upper PD bound of the band.
    #[getter]
    fn upper_pd(&self) -> f64 {
        self.inner.upper_pd
    }

    /// Representative PD assigned to anything falling in the band.
    #[getter]
    fn central_pd(&self) -> f64 {
        self.inner.central_pd
    }

    /// Deserialize a grade from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid MasterScaleGrade JSON"))?,
        })
    }

    /// Serialize this grade to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "MasterScaleGrade serialization failed"))
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("MasterScaleGrade", &self.inner)
    }
}

/// Result of mapping a PD onto a master scale.
#[pyclass(
    module = "finstack_quant.models.credit.pd",
    name = "MasterScaleResult",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyMasterScaleResult {
    inner: MasterScaleResult,
}

#[pymethods]
impl PyMasterScaleResult {
    /// Deserialize a mapped-grade result from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid MasterScaleResult JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this result to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "MasterScaleResult serialization failed"))
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Label of the grade the PD mapped into.
    #[getter]
    fn grade(&self) -> &str {
        &self.inner.grade
    }

    /// Central PD of the assigned grade — the notched value.
    #[getter]
    fn central_pd(&self) -> f64 {
        self.inner.central_pd
    }

    /// The PD that was mapped, before notching.
    #[getter]
    fn input_pd(&self) -> f64 {
        self.inner.input_pd
    }

    /// Zero-based index of the assigned grade in the scale.
    #[getter]
    fn grade_index(&self) -> usize {
        self.inner.grade_index
    }

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``grade``, ``grade_index``, ``input_pd``, ``central_pd``.
    ///
    /// One mapping is one flat record, so a one-row frame is the right shape;
    /// use ``MasterScale.map_pds`` for a whole obligor-level grading table.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let columns: Vec<&str> = RESULT_COLUMNS.iter().map(|(name, _)| *name).collect();
        serde_object_to_single_row_dataframe_with_schema(py, &self.inner, &columns)
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("MasterScaleResult", &self.inner)
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Ordered PD bands mapping a continuous PD onto discrete rating grades.
///
/// Bands must be strictly increasing in ``upper_pd``, and each grade's
/// ``central_pd`` must fall inside its own band. PDs are decimals in [0, 1].
#[pyclass(
    module = "finstack_quant.models.credit.pd",
    name = "MasterScale",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyMasterScale {
    inner: MasterScale,
}

#[pymethods]
impl PyMasterScale {
    /// Build a master scale from ordered grades.
    ///
    /// Parameters
    /// ----------
    /// grades : list[MasterScaleGrade]
    ///     Bands in ascending ``upper_pd`` order, strongest grade first.
    ///
    /// Raises ``ValueError`` when the list is empty, a PD is outside its valid
    /// range, or the bands are not strictly ascending.
    #[new]
    #[pyo3(text_signature = "(grades)")]
    fn new(grades: Vec<PyMasterScaleGrade>) -> PyResult<Self> {
        let grades = grades.into_iter().map(|g| g.inner).collect();
        MasterScale::new(grades)
            .map(|inner| Self { inner })
            .map_err(pd_calibration_to_py)
    }

    /// Library PD-band assumptions using S&P-style labels.
    ///
    /// The labels resemble S&P notation as a reporting convention only;
    /// neither the boundaries nor the central PDs are agency-published
    /// statistics.
    ///
    /// Raises ``ValueError`` if the embedded credit registry is invalid.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn sp_assumptions() -> PyResult<Self> {
        MasterScale::sp_assumptions()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Library PD-band assumptions using Moody's-style labels.
    ///
    /// As with ``sp_assumptions``, the labels are a reporting convention
    /// rather than an agency calibration.
    ///
    /// Raises ``ValueError`` if the embedded credit registry is invalid.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn moodys_assumptions() -> PyResult<Self> {
        MasterScale::moodys_assumptions()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Load a master scale by ID from the embedded credit registry.
    ///
    /// Parameters
    /// ----------
    /// scale_id : str
    ///     Registry identifier of the scale.
    ///
    /// Raises ``KeyError`` when the id is unknown and ``ValueError`` when the
    /// registry is invalid.
    #[staticmethod]
    #[pyo3(text_signature = "(scale_id)")]
    fn from_registry_id(scale_id: &str) -> PyResult<Self> {
        MasterScale::from_registry_id(scale_id)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Map a PD onto its rating grade.
    ///
    /// Parameters
    /// ----------
    /// pd : float
    ///     Probability of default as a decimal in [0, 1].
    ///
    /// Raises ``ValueError`` when ``pd`` is non-finite or outside [0, 1].
    #[pyo3(text_signature = "($self, pd)")]
    fn map_pd(&self, pd: f64) -> PyResult<PyMasterScaleResult> {
        let result = self.inner.map_pd(pd).map_err(pd_calibration_to_py)?;
        Ok(PyMasterScaleResult { inner: result })
    }

    /// Map several PDs and return one grading table.
    ///
    /// Parameters
    /// ----------
    /// pds : list[float]
    ///     Probabilities of default as decimals in [0, 1].
    ///
    /// Returns a pandas ``DataFrame`` with columns ``grade``, ``grade_index``,
    /// ``input_pd``, ``central_pd``; one row per input, in input order (an
    /// empty input yields a zero-row frame with the same columns).
    ///
    /// Raises ``ValueError`` when any PD is non-finite or outside [0, 1].
    #[pyo3(text_signature = "($self, pds)")]
    fn map_pds<'py>(&self, py: Python<'py>, pds: Vec<f64>) -> PyResult<Bound<'py, PyAny>> {
        let rows = pds
            .iter()
            .map(|pd| self.inner.map_pd(*pd))
            .collect::<Result<Vec<_>, _>>()
            .map_err(pd_calibration_to_py)?;
        serde_rows_to_dataframe_with_schema(py, &rows, RESULT_COLUMNS)
    }

    /// Map a scoring result's implied PD onto its rating grade.
    ///
    /// Parameters
    /// ----------
    /// result : ScoringResult
    ///     Output of a ``models.credit.scoring`` model that carries an
    ///     ``implied_pd`` (Ohlson, Zmijewski).
    ///
    /// Raises ``ValueError`` when the result has no implied PD (Altman
    /// family) or the PD is non-finite.
    #[pyo3(text_signature = "($self, result)")]
    fn map_score(&self, result: &PyScoringResult) -> PyResult<PyMasterScaleResult> {
        self.inner
            .map_score(&result.inner)
            .map(|inner| PyMasterScaleResult { inner })
            .map_err(pd_calibration_to_py)
    }

    /// Number of grades in the scale.
    #[getter]
    fn n_grades(&self) -> usize {
        self.inner.n_grades()
    }

    /// The scale's grades, in ascending PD order.
    #[getter]
    fn grades(&self) -> Vec<PyMasterScaleGrade> {
        self.inner
            .grades()
            .iter()
            .cloned()
            .map(|inner| PyMasterScaleGrade { inner })
            .collect()
    }

    /// Export the bands as a pandas ``DataFrame``.
    ///
    /// Columns: ``label``, ``upper_pd``, ``central_pd``; one row per grade in
    /// ascending PD order.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(
            py,
            self.inner.grades(),
            &[
                ("label", "str"),
                ("upper_pd", "float64"),
                ("central_pd", "float64"),
            ],
        )
    }

    /// Deserialize a master scale from canonical JSON (re-validated on load).
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid MasterScale JSON"))?,
        })
    }

    /// Serialize this scale to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "MasterScale serialization failed"))
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __len__(&self) -> usize {
        self.inner.n_grades()
    }

    fn __repr__(&self) -> String {
        let labels: Vec<String> = self
            .inner
            .grades()
            .iter()
            .map(|g| format!("{:?}", g.label))
            .collect();
        format!(
            "MasterScale(n_grades={}, labels=[{}])",
            self.inner.n_grades(),
            labels.join(", ")
        )
    }
}

/// Build the `finstack_quant.models.credit.pd` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "pd")?;
    m.setattr(
        "__doc__",
        "Probability of default: PiT/TtC conversion (Merton-Vasicek), central-tendency calibration, Basel IRB floor, and rating master scales.",
    )?;

    m.add("BASEL_IRB_PD_FLOOR", BASEL_IRB_PD_FLOOR)?;
    m.add_function(wrap_pyfunction!(apply_basel_irb_pd_floor, &m)?)?;
    m.add_function(wrap_pyfunction!(pit_to_ttc, &m)?)?;
    m.add_function(wrap_pyfunction!(ttc_to_pit, &m)?)?;
    m.add_function(wrap_pyfunction!(central_tendency, &m)?)?;
    m.add_class::<PyMasterScale>()?;
    m.add_class::<PyMasterScaleGrade>()?;
    m.add_class::<PyMasterScaleResult>()?;

    let all = PyList::new(
        py,
        [
            "BASEL_IRB_PD_FLOOR",
            "MasterScale",
            "MasterScaleGrade",
            "MasterScaleResult",
            "apply_basel_irb_pd_floor",
            "central_tendency",
            "pit_to_ttc",
            "ttc_to_pit",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
