//! Python bindings for `finstack_quant_models::credit::recovery_waterfall`.

use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    ColumnSchema,
};
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
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

#[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim")]
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
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.__init__")]
    #[new]
    #[pyo3(signature = (id, seniority, priority, principal, accrued, penalties, collateral_haircut, collateral_value=None))]
    #[pyo3(
        text_signature = "(id, seniority, priority, principal, accrued, penalties, collateral_haircut, collateral_value=None)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: String,
        seniority: String,
        priority: u32,
        principal: f64,
        accrued: f64,
        penalties: f64,
        collateral_haircut: f64,
        collateral_value: Option<f64>,
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

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.id")]
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.seniority")]
    #[getter]
    fn seniority(&self) -> String {
        self.inner.seniority.clone()
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.priority")]
    #[getter]
    fn priority(&self) -> u32 {
        self.inner.priority
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.principal")]
    #[getter]
    fn principal(&self) -> f64 {
        self.inner.principal
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.accrued")]
    #[getter]
    fn accrued(&self) -> f64 {
        self.inner.accrued
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.penalties")]
    #[getter]
    fn penalties(&self) -> f64 {
        self.inner.penalties
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.collateral_value")]
    #[getter]
    fn collateral_value(&self) -> Option<f64> {
        self.inner.collateral_value
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.collateral_haircut")]
    #[getter]
    fn collateral_haircut(&self) -> f64 {
        self.inner.collateral_haircut
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.total_claim")]
    #[getter]
    fn total_claim(&self) -> f64 {
        self.inner.total_claim()
    }

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

wire_methods!(
    PyRecoveryClaim,
    RecoveryClaim,
    "RecoveryClaim",
    module = "finstack_quant.models.credit.recovery_waterfall"
);

#[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation")]
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
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.id")]
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.seniority")]
    #[getter]
    fn seniority(&self) -> String {
        self.inner.seniority.clone()
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.priority")]
    #[getter]
    fn priority(&self) -> u32 {
        self.inner.priority
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.total_claim")]
    #[getter]
    fn total_claim(&self) -> f64 {
        self.inner.total_claim
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.collateral_recovery")]
    #[getter]
    fn collateral_recovery(&self) -> f64 {
        self.inner.collateral_recovery
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.general_recovery")]
    #[getter]
    fn general_recovery(&self) -> f64 {
        self.inner.general_recovery
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.total_recovery")]
    #[getter]
    fn total_recovery(&self) -> f64 {
        self.inner.total_recovery
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.recovery_rate")]
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.deficiency")]
    #[getter]
    fn deficiency(&self) -> f64 {
        self.inner.deficiency
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let columns: Vec<&str> = ALLOCATION_COLUMNS.iter().map(|(name, _)| *name).collect();
        serde_object_to_single_row_dataframe_with_schema(py, &self.inner, &columns)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RecoveryAllocation", &self.inner)
    }
}

wire_methods!(
    PyRecoveryAllocation,
    RecoveryAllocation,
    "RecoveryAllocation",
    module = "finstack_quant.models.credit.recovery_waterfall"
);

#[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryWaterfallResult")]
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
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryWaterfallResult.total_distributed")]
    #[getter]
    fn total_distributed(&self) -> f64 {
        self.inner.total_distributed
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryWaterfallResult.undistributed_estate")]
    #[getter]
    fn undistributed_estate(&self) -> f64 {
        self.inner.undistributed_estate
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryWaterfallResult.apr_satisfied")]
    #[getter]
    fn apr_satisfied(&self) -> bool {
        self.inner.apr_satisfied
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryWaterfallResult.allocations")]
    #[getter]
    fn allocations(&self) -> Vec<PyRecoveryAllocation> {
        self.inner
            .allocations
            .iter()
            .cloned()
            .map(|inner| PyRecoveryAllocation { inner })
            .collect()
    }

    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryWaterfallResult.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // `RecoveryAllocation` derives Serialize and is already a flat record
        // of `f64`/`String`/`u32` fields whose serde names match the columns
        // above, so the rows go straight through the serde helper.
        serde_rows_to_dataframe_with_schema(py, &self.inner.allocations, ALLOCATION_COLUMNS)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RecoveryWaterfallResult", &self.inner)
    }
}

wire_methods!(
    PyRecoveryWaterfallResult,
    RecoveryWaterfallResult,
    "RecoveryWaterfallResult",
    module = "finstack_quant.models.credit.recovery_waterfall"
);
impl_repr_html_via_dataframe!(PyRecoveryWaterfallResult);

#[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "allocate_recovery")]
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
        pydoc!(
            "finstack_quant.models.credit.recovery_waterfall",
            "__module__"
        ),
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
