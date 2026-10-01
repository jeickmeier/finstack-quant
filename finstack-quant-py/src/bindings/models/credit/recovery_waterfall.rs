//! Python bindings for `finstack_quant_models::credit::recovery_waterfall`.

use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    ColumnSchema,
};
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_models::credit::recovery_waterfall::{
    self as waterfall, RecoveryAllocation, RecoveryClaim, RecoveryWaterfallResult,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// Column schema of `PyRecoveryWaterfallResult::to_dataframe`, kept so a
/// claim-free waterfall still exports a frame with the documented columns.
const ALLOCATION_COLUMNS: &[ColumnSchema<'static>] = &[
    ("id", "str"),
    ("seniority", "str"),
    ("priority", "int64"),
    ("total_claim", "float64"),
    ("collateral_recovery", "float64"),
    ("general_recovery", "float64"),
    ("total_recovery", "float64"),
    ("recovery_rate", "float64"),
    ("deficiency", "float64"),
];

/// A claim participating in an absolute-priority recovery waterfall.
///
/// Monetary fields share the estate's currency; ``collateral_haircut`` is a
/// decimal fraction in [0, 1]. Validation happens in ``allocate_recovery``.
#[pyclass(
    name = "RecoveryClaim",
    module = "finstack_quant.models.credit.recovery_waterfall",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyRecoveryClaim {
    pub(crate) inner: RecoveryClaim,
}

#[pymethods]
impl PyRecoveryClaim {
    /// Create a recovery claim.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Stable identifier, unique after trimming whitespace.
    /// seniority : str
    ///     Free-form seniority class label carried through to the allocation.
    /// priority : int
    ///     Absolute-priority rank; lower ranks are paid first.
    /// principal : float
    ///     Principal outstanding (>= 0).
    /// accrued : float, default 0.0
    ///     Accrued but unpaid interest included in the claim.
    /// penalties : float, default 0.0
    ///     Penalties and fees included in the claim.
    /// collateral_value : float | None, default None
    ///     Gross value of collateral pledged to this claim, or ``None`` for an
    ///     unsecured claim.
    /// collateral_haircut : float, default 0.0
    ///     Haircut applied to ``collateral_value`` as a decimal in [0, 1].
    #[new]
    #[pyo3(signature = (id, seniority, priority, principal, accrued=0.0, penalties=0.0, collateral_value=None, collateral_haircut=0.0))]
    #[pyo3(
        text_signature = "(id, seniority, priority, principal, accrued=0.0, penalties=0.0, collateral_value=None, collateral_haircut=0.0)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: String,
        seniority: String,
        priority: u32,
        principal: f64,
        accrued: f64,
        penalties: f64,
        collateral_value: Option<f64>,
        collateral_haircut: f64,
    ) -> Self {
        Self {
            inner: RecoveryClaim {
                id,
                seniority,
                priority,
                principal,
                accrued,
                penalties,
                collateral_value,
                collateral_haircut,
            },
        }
    }

    /// Stable identifier for this claim.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Seniority class the claim sits in.
    #[getter]
    fn seniority(&self) -> String {
        self.inner.seniority.clone()
    }

    /// Absolute-priority rank; lower ranks are paid first.
    #[getter]
    fn priority(&self) -> u32 {
        self.inner.priority
    }

    /// Principal outstanding, before accrued interest and penalties.
    #[getter]
    fn principal(&self) -> f64 {
        self.inner.principal
    }

    /// Accrued but unpaid interest included in the claim.
    #[getter]
    fn accrued(&self) -> f64 {
        self.inner.accrued
    }

    /// Penalties and fees included in the claim.
    #[getter]
    fn penalties(&self) -> f64 {
        self.inner.penalties
    }

    /// Gross value of collateral pledged to this claim.
    #[getter]
    fn collateral_value(&self) -> Option<f64> {
        self.inner.collateral_value
    }

    /// Haircut applied to `collateral_value`, as a fraction in [0, 1].
    #[getter]
    fn collateral_haircut(&self) -> f64 {
        self.inner.collateral_haircut
    }

    /// Principal plus accrued interest and penalties.
    #[getter]
    fn total_claim(&self) -> f64 {
        self.inner.total_claim()
    }

    /// Deserialize from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid RecoveryClaim JSON"))?,
        })
    }

    /// Serialize to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "RecoveryClaim serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        format!(
            "RecoveryClaim(id={:?}, seniority={:?}, priority={}, total_claim={})",
            self.inner.id,
            self.inner.seniority,
            self.inner.priority,
            self.inner.total_claim()
        )
    }
}

/// Recovery allocated to one claim.
#[pyclass(
    name = "RecoveryAllocation",
    module = "finstack_quant.models.credit.recovery_waterfall",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyRecoveryAllocation {
    inner: RecoveryAllocation,
}

#[pymethods]
impl PyRecoveryAllocation {
    /// Stable identifier for this claim.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Seniority class the claim sits in.
    #[getter]
    fn seniority(&self) -> String {
        self.inner.seniority.clone()
    }

    /// Absolute-priority rank; lower ranks are paid first.
    #[getter]
    fn priority(&self) -> u32 {
        self.inner.priority
    }

    /// Principal plus accrued interest and penalties.
    #[getter]
    fn total_claim(&self) -> f64 {
        self.inner.total_claim
    }

    /// Amount recovered from pledged collateral.
    #[getter]
    fn collateral_recovery(&self) -> f64 {
        self.inner.collateral_recovery
    }

    /// Amount recovered from the general estate.
    #[getter]
    fn general_recovery(&self) -> f64 {
        self.inner.general_recovery
    }

    /// Collateral plus general recovery.
    #[getter]
    fn total_recovery(&self) -> f64 {
        self.inner.total_recovery
    }

    /// `total_recovery / total_claim`, as a fraction in [0, 1].
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate
    }

    /// Unrecovered claim: `total_claim - total_recovery`, floored at zero.
    #[getter]
    fn deficiency(&self) -> f64 {
        self.inner.deficiency
    }

    /// Deserialize from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid RecoveryAllocation JSON"))?,
        })
    }

    /// Serialize to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "RecoveryAllocation serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Export as a single-row pandas ``DataFrame`` with the same columns as
    /// ``RecoveryWaterfallResult.to_dataframe``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let columns: Vec<&str> = ALLOCATION_COLUMNS.iter().map(|(name, _)| *name).collect();
        serde_object_to_single_row_dataframe_with_schema(py, &self.inner, &columns)
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RecoveryAllocation", &self.inner)
    }
}

/// Result of allocating a distributable estate across claims.
#[pyclass(
    name = "RecoveryWaterfallResult",
    module = "finstack_quant.models.credit.recovery_waterfall",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyRecoveryWaterfallResult {
    inner: RecoveryWaterfallResult,
}

#[pymethods]
impl PyRecoveryWaterfallResult {
    /// Deserialize a waterfall result from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid RecoveryWaterfallResult JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this result to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "RecoveryWaterfallResult serialization failed"))
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Sum of every claim's `total_recovery`.
    #[getter]
    fn total_distributed(&self) -> f64 {
        self.inner.total_distributed
    }

    /// Estate value left after all claims are satisfied.
    #[getter]
    fn undistributed_estate(&self) -> f64 {
        self.inner.undistributed_estate
    }

    /// Whether the run respected absolute priority end to end.
    #[getter]
    fn apr_satisfied(&self) -> bool {
        self.inner.apr_satisfied
    }

    /// Per-claim allocations, in absolute-priority order.
    #[getter]
    fn allocations(&self) -> Vec<PyRecoveryAllocation> {
        self.inner
            .allocations
            .iter()
            .cloned()
            .map(|inner| PyRecoveryAllocation { inner })
            .collect()
    }

    /// Export the per-claim allocations as a pandas ``DataFrame``.
    ///
    /// Columns: ``id``, ``seniority``, ``priority``, ``total_claim``,
    /// ``collateral_recovery``, ``general_recovery``, ``total_recovery``,
    /// ``recovery_rate``, ``deficiency``.
    ///
    /// One row per claim — the natural grain of a waterfall. Rows keep the
    /// Rust ordering (ascending ``priority``, then original claim order), so
    /// repeated exports of the same result are byte-identical. The
    /// estate-level fields (:attr:`total_distributed`,
    /// :attr:`undistributed_estate`, :attr:`apr_satisfied`) are deliberately
    /// not repeated on every row; read them from the result object.
    ///
    /// A waterfall with no claims yields a zero-row frame that still carries
    /// the columns above.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // `RecoveryAllocation` derives Serialize and is already a flat record
        // of `f64`/`String`/`u32` fields whose serde names match the columns
        // above, so the rows go straight through the serde helper.
        serde_rows_to_dataframe_with_schema(py, &self.inner.allocations, ALLOCATION_COLUMNS)
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

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RecoveryWaterfallResult", &self.inner)
    }
}

/// Allocate an estate, inclusive of collateral, across recovery claims.
///
/// Parameters
/// ----------
/// estate_value : float
///     Total distributable estate including pledged collateral (>= 0).
/// claims : list[RecoveryClaim]
///     Claims to satisfy; collateral is applied first, then the remaining
///     estate by ascending ``priority``.
///
/// Returns a ``RecoveryWaterfallResult``.
///
/// Raises ``ValueError`` for negative amounts, haircuts outside [0, 1], or
/// duplicate claim ids after trimming.
#[pyfunction]
#[pyo3(text_signature = "(estate_value, claims)")]
fn allocate_recovery(
    py: Python<'_>,
    estate_value: f64,
    claims: Vec<PyRecoveryClaim>,
) -> PyResult<PyRecoveryWaterfallResult> {
    let claims = claims
        .into_iter()
        .map(|claim| claim.inner)
        .collect::<Vec<_>>();
    py.detach(|| waterfall::allocate_recovery(estate_value, &claims))
        .map(|inner| PyRecoveryWaterfallResult { inner })
        .map_err(core_to_py)
}

/// Build the `finstack_quant.models.credit.recovery_waterfall` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "recovery_waterfall")?;
    m.setattr(
        "__doc__",
        "Absolute-priority recovery allocation with estate-inclusive collateral.",
    )?;

    m.add_class::<PyRecoveryClaim>()?;
    m.add_class::<PyRecoveryAllocation>()?;
    m.add_class::<PyRecoveryWaterfallResult>()?;
    m.add_function(wrap_pyfunction!(allocate_recovery, &m)?)?;

    let all = PyList::new(
        py,
        [
            "RecoveryAllocation",
            "RecoveryClaim",
            "RecoveryWaterfallResult",
            "allocate_recovery",
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
