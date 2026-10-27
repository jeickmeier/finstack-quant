//! Spec-type bindings for `finstack_quant_cashflows::builder`.

use crate::bindings::cashflows::aggregation::extract_dated_flows;
use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::builder::{AmortizationSpec, Notional};
use pyo3::prelude::*;

use super::DateMoneyRows;
use super::*;
use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::currency::PyCurrency;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::repr_support::enum_repr_from_serde as enum_repr;
use crate::errors::core_to_py;

#[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "AmortizationSpec")]
#[pyclass(
    name = "AmortizationSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq, Hash)]
pub struct PyAmortizationSpec {
    /// Inner amortization rule.
    pub(crate) inner: AmortizationSpec,
}

wire_methods!(
    PyAmortizationSpec,
    AmortizationSpec,
    "AmortizationSpec",
    module = "finstack_quant.cashflows.builder._specs.amortization"
);

#[pymethods]
impl PyAmortizationSpec {
    /// No amortization (bullet).
    #[classattr]
    const NONE: Self = Self {
        inner: AmortizationSpec::None,
    };

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "AmortizationSpec.linear_to")]
    #[staticmethod]
    #[pyo3(text_signature = "(final_notional)")]
    fn linear_to(final_notional: PyMoney) -> Self {
        Self {
            inner: AmortizationSpec::LinearTo {
                final_notional: final_notional.inner,
            },
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "AmortizationSpec.step_remaining")]
    #[staticmethod]
    #[pyo3(text_signature = "(schedule)")]
    fn step_remaining(schedule: Vec<(Bound<'_, PyAny>, PyMoney)>) -> PyResult<Self> {
        Ok(Self {
            inner: AmortizationSpec::StepRemaining {
                schedule: extract_dated_flows(schedule)?,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "AmortizationSpec.percent_of_original_per_period")]
    #[staticmethod]
    #[pyo3(text_signature = "(pct)")]
    fn percent_of_original_per_period(pct: f64) -> Self {
        Self {
            inner: AmortizationSpec::PercentOfOriginalPerPeriod { pct },
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "AmortizationSpec.percent_of_remaining_per_period")]
    #[staticmethod]
    #[pyo3(text_signature = "(pct)")]
    fn percent_of_remaining_per_period(pct: f64) -> Self {
        Self {
            inner: AmortizationSpec::PercentOfRemainingPerPeriod { pct },
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "AmortizationSpec.linear_between")]
    #[staticmethod]
    #[pyo3(text_signature = "(start, end)")]
    fn linear_between(start: &Bound<'_, PyAny>, end: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: AmortizationSpec::LinearBetween {
                start: py_to_date(start)?,
                end: py_to_date(end)?,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "AmortizationSpec.custom_principal")]
    #[staticmethod]
    #[pyo3(text_signature = "(items)")]
    fn custom_principal(items: Vec<(Bound<'_, PyAny>, PyMoney)>) -> PyResult<Self> {
        Ok(Self {
            inner: AmortizationSpec::CustomPrincipal {
                items: extract_dated_flows(items)?,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "AmortizationSpec.kind")]
    #[getter]
    fn kind(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(&self.inner).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "AmortizationSpec.final_notional")]
    #[getter]
    fn final_notional(&self) -> Option<PyMoney> {
        match self.inner {
            AmortizationSpec::LinearTo { final_notional } => {
                Some(PyMoney::from_inner(final_notional))
            }
            _ => None,
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "AmortizationSpec.schedule")]
    #[getter]
    fn schedule<'py>(&self, py: Python<'py>) -> PyResult<Option<DateMoneyRows<'py>>> {
        match &self.inner {
            AmortizationSpec::StepRemaining { schedule } => {
                date_money_to_py(py, schedule).map(Some)
            }
            _ => Ok(None),
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "AmortizationSpec.pct")]
    #[getter]
    fn pct(&self) -> Option<f64> {
        match self.inner {
            AmortizationSpec::PercentOfOriginalPerPeriod { pct }
            | AmortizationSpec::PercentOfRemainingPerPeriod { pct } => Some(pct),
            _ => None,
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "AmortizationSpec.window")]
    #[getter]
    fn window<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
        match self.inner {
            AmortizationSpec::LinearBetween { start, end } => {
                Ok(Some((date_to_py(py, start)?, date_to_py(py, end)?)))
            }
            _ => Ok(None),
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "AmortizationSpec.items")]
    #[getter]
    fn items<'py>(&self, py: Python<'py>) -> PyResult<Option<DateMoneyRows<'py>>> {
        match &self.inner {
            AmortizationSpec::CustomPrincipal { items } => date_money_to_py(py, items).map(Some),
            _ => Ok(None),
        }
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("AmortizationSpec", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "Notional")]
#[pyclass(
    name = "Notional",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyNotional {
    /// Inner notional with amortization rule.
    pub(crate) inner: Notional,
}

impl PyNotional {
    /// Build from an existing Rust [`Notional`].
    pub(crate) fn from_inner(inner: Notional) -> Self {
        Self { inner }
    }
}

wire_methods!(
    PyNotional,
    Notional,
    "Notional",
    module = "finstack_quant.cashflows.builder._specs.amortization",
    validate
);

#[pymethods]
impl PyNotional {
    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "Notional.__init__")]
    #[new]
    #[pyo3(signature = (initial, amort=None), text_signature = "(initial, amort=None)")]
    fn new(initial: PyMoney, amort: Option<PyRef<'_, PyAmortizationSpec>>) -> PyResult<Self> {
        let inner = Notional {
            initial: initial.inner,
            amort: amort.map(|a| a.inner.clone()).unwrap_or_default(),
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "Notional.par")]
    #[staticmethod]
    #[pyo3(text_signature = "(amount, currency)")]
    fn par(amount: f64, currency: &Bound<'_, PyAny>) -> PyResult<Self> {
        let inner = Notional::par(amount, extract_currency(currency)?)
            .map_err(crate::errors::core_to_py)?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "Notional.initial")]
    #[getter]
    fn initial(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.initial)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "Notional.amort")]
    #[getter]
    fn amort(&self) -> PyAmortizationSpec {
        PyAmortizationSpec {
            inner: self.inner.amort.clone(),
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "Notional.currency")]
    #[pyo3(text_signature = "(self)")]
    fn currency(&self, py: Python<'_>) -> PyResult<Py<PyCurrency>> {
        Py::new(py, PyCurrency::from_inner(self.inner.currency()))
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.amortization", "Notional.validate")]
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        format!(
            "Notional(initial={} {}, amort={})",
            self.inner.initial.amount(),
            self.inner.initial.currency(),
            enum_repr("AmortizationSpec", &self.inner.amort)
        )
    }
}
