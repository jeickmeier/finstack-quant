//! Python bindings for `finstack_quant_core::credit::recovery_waterfall`.

use crate::errors::core_to_py;
use finstack_quant_core::credit::recovery_waterfall::{
    self as waterfall, RecoveryAllocation, RecoveryClaim, RecoveryWaterfallResult,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// A claim participating in an absolute-priority recovery waterfall.
#[pyclass(
    name = "RecoveryClaim",
    module = "finstack_quant.core.credit.recovery_waterfall",
    frozen,
    from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyRecoveryClaim {
    pub(crate) inner: RecoveryClaim,
}

#[pymethods]
impl PyRecoveryClaim {
    /// Create a recovery claim.
    #[new]
    #[pyo3(signature = (id, seniority, priority, principal, accrued=0.0, penalties=0.0, collateral=None))]
    fn new(
        id: String,
        seniority: String,
        priority: u32,
        principal: f64,
        accrued: f64,
        penalties: f64,
        collateral: Option<(f64, f64)>,
    ) -> Self {
        let (collateral_value, collateral_haircut) = match collateral {
            Some((value, haircut)) => (Some(value), haircut),
            None => (None, 0.0),
        };
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

    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    #[getter]
    fn seniority(&self) -> String {
        self.inner.seniority.clone()
    }

    #[getter]
    fn priority(&self) -> u32 {
        self.inner.priority
    }

    #[getter]
    fn principal(&self) -> f64 {
        self.inner.principal
    }

    #[getter]
    fn accrued(&self) -> f64 {
        self.inner.accrued
    }

    #[getter]
    fn penalties(&self) -> f64 {
        self.inner.penalties
    }

    #[getter]
    fn collateral_value(&self) -> Option<f64> {
        self.inner.collateral_value
    }

    #[getter]
    fn collateral_haircut(&self) -> f64 {
        self.inner.collateral_haircut
    }

    #[getter]
    fn total_claim(&self) -> f64 {
        self.inner.total_claim()
    }

    fn __repr__(&self) -> String {
        format!(
            "RecoveryClaim(id='{}', seniority='{}', priority={}, total_claim={})",
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
    module = "finstack_quant.core.credit.recovery_waterfall",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyRecoveryAllocation {
    inner: RecoveryAllocation,
}

#[pymethods]
impl PyRecoveryAllocation {
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    #[getter]
    fn seniority(&self) -> String {
        self.inner.seniority.clone()
    }

    #[getter]
    fn priority(&self) -> u32 {
        self.inner.priority
    }

    #[getter]
    fn total_claim(&self) -> f64 {
        self.inner.total_claim
    }

    #[getter]
    fn collateral_recovery(&self) -> f64 {
        self.inner.collateral_recovery
    }

    #[getter]
    fn general_recovery(&self) -> f64 {
        self.inner.general_recovery
    }

    #[getter]
    fn total_recovery(&self) -> f64 {
        self.inner.total_recovery
    }

    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate
    }

    #[getter]
    fn deficiency(&self) -> f64 {
        self.inner.deficiency
    }
}

/// Result of allocating a distributable estate across claims.
#[pyclass(
    name = "RecoveryWaterfallResult",
    module = "finstack_quant.core.credit.recovery_waterfall",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyRecoveryWaterfallResult {
    inner: RecoveryWaterfallResult,
}

#[pymethods]
impl PyRecoveryWaterfallResult {
    #[getter]
    fn total_distributed(&self) -> f64 {
        self.inner.total_distributed
    }

    #[getter]
    fn undistributed_estate(&self) -> f64 {
        self.inner.undistributed_estate
    }

    #[getter]
    fn apr_satisfied(&self) -> bool {
        self.inner.apr_satisfied
    }

    #[getter]
    fn allocations(&self) -> Vec<PyRecoveryAllocation> {
        self.inner
            .allocations
            .iter()
            .cloned()
            .map(|inner| PyRecoveryAllocation { inner })
            .collect()
    }
}

/// Allocate an estate, inclusive of collateral, across recovery claims.
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

/// Build the `finstack_quant.core.credit.recovery_waterfall` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "recovery_waterfall")?;
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
            "RecoveryClaim",
            "RecoveryAllocation",
            "RecoveryWaterfallResult",
            "allocate_recovery",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "recovery_waterfall",
        "finstack_quant.core.credit",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;
    Ok(())
}
