//! Python bindings for `finstack_quant_models::credit::recovery_waterfall`.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::core_to_py;
use finstack_quant_models::credit::recovery_waterfall::{
    self as waterfall, RecoveryAllocation, RecoveryClaim, RecoveryWaterfallResult,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// Column schema of `PyRecoveryWaterfallResult::to_arrow_table`, kept so a
/// claim-free waterfall still exports a table with the documented columns.
const ALLOCATION_COLUMNS: &[(&str, &str)] = &[
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
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, id, seniority, priority, principal, accrued, penalties, collateral_haircut, collateral_value=None)"
    )]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, pyo3::types::PyDict>>) -> PyResult<Self> {
        let inner: RecoveryClaim =
            crate::bindings::module_utils::fields_from_py(py, fields, "RecoveryClaim")?;
        Ok(Self { inner })
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

py_getters!(PyRecoveryClaim |s| {
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.id")]
    id -> String = s.inner.id.clone();
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.seniority")]
    seniority -> String = s.inner.seniority.clone();
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.priority")]
    priority -> u32 = s.inner.priority;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.principal")]
    principal -> f64 = s.inner.principal;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.accrued")]
    accrued -> f64 = s.inner.accrued;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.penalties")]
    penalties -> f64 = s.inner.penalties;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.collateral_value")]
    collateral_value -> Option<f64> = s.inner.collateral_value;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.collateral_haircut")]
    collateral_haircut -> f64 = s.inner.collateral_haircut;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryClaim.total_claim")]
    total_claim -> f64 = s.inner.total_claim();
});

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
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RecoveryAllocation", &self.inner)
    }
}

py_getters!(PyRecoveryAllocation |s| {
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.id")]
    id -> String = s.inner.id.clone();
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.seniority")]
    seniority -> String = s.inner.seniority.clone();
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.priority")]
    priority -> u32 = s.inner.priority;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.total_claim")]
    total_claim -> f64 = s.inner.total_claim;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.collateral_recovery")]
    collateral_recovery -> f64 = s.inner.collateral_recovery;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.general_recovery")]
    general_recovery -> f64 = s.inner.general_recovery;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.total_recovery")]
    total_recovery -> f64 = s.inner.total_recovery;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.recovery_rate")]
    recovery_rate -> f64 = s.inner.recovery_rate;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryAllocation.deficiency")]
    deficiency -> f64 = s.inner.deficiency;
});

impl PyRecoveryAllocation {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(std::slice::from_ref(&self.inner), ALLOCATION_COLUMNS)
    }
}

tabular_methods!(
    PyRecoveryAllocation,
    "RecoveryAllocation",
    module = "finstack_quant.models.credit.recovery_waterfall"
);
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
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RecoveryWaterfallResult", &self.inner)
    }
}

py_getters!(PyRecoveryWaterfallResult |s| {
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryWaterfallResult.total_distributed")]
    total_distributed -> f64 = s.inner.total_distributed;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryWaterfallResult.undistributed_estate")]
    undistributed_estate -> f64 = s.inner.undistributed_estate;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryWaterfallResult.apr_satisfied")]
    apr_satisfied -> bool = s.inner.apr_satisfied;
    #[doc = pydoc!("finstack_quant.models.credit.recovery_waterfall", "RecoveryWaterfallResult.allocations")]
    allocations -> Vec<PyRecoveryAllocation> = s
        .inner
        .allocations
        .iter()
        .cloned()
        .map(|inner| PyRecoveryAllocation { inner })
        .collect();
});

impl PyRecoveryWaterfallResult {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        // `RecoveryAllocation` derives Serialize and is already a flat record
        // of `f64`/`String`/`u32` fields whose serde names match the columns
        // above, so the rows go straight through the serde helper.
        serde_rows_to_arrow_table(&self.inner.allocations, ALLOCATION_COLUMNS)
    }
}

tabular_methods!(
    PyRecoveryWaterfallResult,
    "RecoveryWaterfallResult",
    module = "finstack_quant.models.credit.recovery_waterfall"
);
wire_methods!(
    PyRecoveryWaterfallResult,
    RecoveryWaterfallResult,
    "RecoveryWaterfallResult",
    module = "finstack_quant.models.credit.recovery_waterfall"
);

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
