//! Spec-type bindings for `finstack_quant_cashflows::builder`.

use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::builder::{FeeAccrualBasis, FeeBase, FeeSpec, ScheduleParams};
use finstack_quant_core::dates::BusinessDayConvention;
use pyo3::prelude::*;

use super::*;
use crate::bindings::core::dates::calendar::{
    extract_business_day_convention, PyBusinessDayConvention,
};
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::{extract_tenor, PyTenor};
use crate::bindings::core::money::{decimal_to_py, PyMoney};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::repr_support::enum_repr_from_serde as enum_repr;
use crate::errors::core_to_py;

#[doc = pydoc!("finstack_quant.cashflows.builder", "FeeAccrualBasis")]
#[pyclass(
    name = "FeeAccrualBasis",
    module = "finstack_quant.cashflows.builder",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyFeeAccrualBasis {
    /// Inner fee accrual basis.
    pub(crate) inner: FeeAccrualBasis,
}

#[pymethods]
impl PyFeeAccrualBasis {
    /// Sample the outstanding balance at accrual-period start (default).
    #[classattr]
    const POINT_IN_TIME: PyFeeAccrualBasis = PyFeeAccrualBasis {
        inner: FeeAccrualBasis::PointInTime,
    };
    /// Time-weighted average outstanding over the accrual period.
    #[classattr]
    const TIME_WEIGHTED_AVERAGE: PyFeeAccrualBasis = PyFeeAccrualBasis {
        inner: FeeAccrualBasis::TimeWeightedAverage,
    };

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("FeeAccrualBasis", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.cashflows.builder", "FeeBase")]
#[pyclass(
    name = "FeeBase",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFeeBase {
    /// Inner fee base.
    pub(crate) inner: FeeBase,
}

wire_methods!(
    PyFeeBase,
    FeeBase,
    "FeeBase",
    stub = "finstack_quant.cashflows.builder"
);

#[pymethods]
impl PyFeeBase {
    /// Fee accrues on the drawn outstanding balance.
    #[classattr]
    const DRAWN: Self = Self {
        inner: FeeBase::Drawn,
    };

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeBase.undrawn")]
    #[staticmethod]
    #[pyo3(text_signature = "(commitment)")]
    fn undrawn(commitment: PyMoney) -> Self {
        Self {
            inner: FeeBase::Undrawn {
                commitment: commitment.inner,
            },
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeBase.kind")]
    #[getter]
    fn kind(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(&self.inner).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeBase.commitment")]
    #[getter]
    fn commitment(&self) -> Option<PyMoney> {
        match self.inner {
            FeeBase::Undrawn { commitment } => Some(PyMoney::from_inner(commitment)),
            FeeBase::Drawn => None,
        }
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("FeeBase", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.cashflows.builder", "FeeSpec")]
#[pyclass(
    name = "FeeSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFeeSpec {
    /// Inner fee specification.
    pub(crate) inner: FeeSpec,
}

wire_methods!(
    PyFeeSpec,
    FeeSpec,
    "FeeSpec",
    stub = "finstack_quant.cashflows.builder"
);

#[pymethods]
impl PyFeeSpec {
    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeSpec.fixed")]
    #[staticmethod]
    #[pyo3(text_signature = "(date, amount)")]
    fn fixed(date: &Bound<'_, PyAny>, amount: PyMoney) -> PyResult<Self> {
        Ok(Self {
            inner: FeeSpec::Fixed {
                date: py_to_date(date)?,
                amount: amount.inner,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeSpec.periodic_bp")]
    #[staticmethod]
    #[pyo3(
        signature = (base, bp, frequency, day_count, calendar_id, business_day_convention=None, stub=None, accrual_basis=None),
        text_signature = "(base, bp, frequency, day_count, calendar_id, business_day_convention=None, stub=None, accrual_basis=None)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn periodic_bp(
        base: PyRef<'_, PyFeeBase>,
        bp: &Bound<'_, PyAny>,
        frequency: &Bound<'_, PyAny>,
        day_count: PyRef<'_, PyDayCount>,
        calendar_id: &str,
        business_day_convention: Option<&Bound<'_, PyAny>>,
        stub: Option<PyRef<'_, PyStubKind>>,
        accrual_basis: Option<PyRef<'_, PyFeeAccrualBasis>>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: FeeSpec::PeriodicBp {
                base: base.inner.clone(),
                bp: decimal_from_any(bp)?,
                frequency: extract_tenor(frequency)?,
                day_count: day_count.inner,
                business_day_convention: match business_day_convention {
                    Some(obj) => extract_business_day_convention(obj)?,
                    None => BusinessDayConvention::default(),
                },
                calendar_id: calendar_id.to_string(),
                stub: stub.map_or(ScheduleParams::DEFAULT_STUB, |s| s.inner),
                accrual_basis: accrual_basis.map(|a| a.inner.clone()).unwrap_or_default(),
            },
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeSpec.kind")]
    #[getter]
    fn kind(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(&self.inner).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeSpec.date")]
    #[getter]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner {
            FeeSpec::Fixed { date, .. } => date_to_py(py, date).map(Some),
            FeeSpec::PeriodicBp { .. } => Ok(None),
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeSpec.amount")]
    #[getter]
    fn amount(&self) -> Option<PyMoney> {
        match self.inner {
            FeeSpec::Fixed { amount, .. } => Some(PyMoney::from_inner(amount)),
            FeeSpec::PeriodicBp { .. } => None,
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeSpec.base")]
    #[getter]
    fn base(&self) -> Option<PyFeeBase> {
        match &self.inner {
            FeeSpec::PeriodicBp { base, .. } => Some(PyFeeBase {
                inner: base.clone(),
            }),
            FeeSpec::Fixed { .. } => None,
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeSpec.bp")]
    #[getter]
    fn bp<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner {
            FeeSpec::PeriodicBp { bp, .. } => decimal_to_py(py, bp).map(Some),
            FeeSpec::Fixed { .. } => Ok(None),
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeSpec.frequency")]
    #[getter]
    fn frequency(&self) -> Option<PyTenor> {
        match self.inner {
            FeeSpec::PeriodicBp { frequency, .. } => Some(PyTenor::from_inner(frequency)),
            FeeSpec::Fixed { .. } => None,
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeSpec.day_count")]
    #[getter]
    fn day_count(&self) -> Option<PyDayCount> {
        match self.inner {
            FeeSpec::PeriodicBp { day_count, .. } => Some(PyDayCount::from_inner(day_count)),
            FeeSpec::Fixed { .. } => None,
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeSpec.business_day_convention")]
    #[getter]
    fn business_day_convention(&self) -> Option<PyBusinessDayConvention> {
        match self.inner {
            FeeSpec::PeriodicBp {
                business_day_convention,
                ..
            } => Some(PyBusinessDayConvention::from_inner(business_day_convention)),
            FeeSpec::Fixed { .. } => None,
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeSpec.calendar_id")]
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        match &self.inner {
            FeeSpec::PeriodicBp { calendar_id, .. } => Some(calendar_id.clone()),
            FeeSpec::Fixed { .. } => None,
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeSpec.stub")]
    #[getter]
    fn stub(&self) -> Option<PyStubKind> {
        match self.inner {
            FeeSpec::PeriodicBp { stub, .. } => Some(PyStubKind::from_inner(stub)),
            FeeSpec::Fixed { .. } => None,
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FeeSpec.accrual_basis")]
    #[getter]
    fn accrual_basis(&self) -> Option<PyFeeAccrualBasis> {
        match &self.inner {
            FeeSpec::PeriodicBp { accrual_basis, .. } => Some(PyFeeAccrualBasis {
                inner: accrual_basis.clone(),
            }),
            FeeSpec::Fixed { .. } => None,
        }
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("FeeSpec", &self.inner)
    }
}
