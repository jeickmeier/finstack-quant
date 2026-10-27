//! Typed facility terms: `TermOutSpec` and `AmortizationEvent`, the
//! revolving-period end and the early-amortization events of an
//! `AssetBackedFacility`.

use pyo3::prelude::*;

use crate::bindings::date_utils::{date_to_py, py_to_date};
use finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::{
    AmortizationEvent, TermOutSpec,
};

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms", "TermOutSpec")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "TermOutSpec",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyTermOutSpec {
    /// Inner canonical Rust term-out.
    pub(crate) inner: TermOutSpec,
}

crate::bindings::macros::wire_methods!(
    PyTermOutSpec,
    TermOutSpec,
    "TermOutSpec",
    module = "finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms",
    to_dict
);

#[pymethods]
impl PyTermOutSpec {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms", "TermOutSpec.__init__")]
    #[new]
    #[pyo3(text_signature = "(months)")]
    fn new(months: u32) -> Self {
        Self {
            inner: TermOutSpec { months },
        }
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("TermOutSpec(months={})", self.inner.months)
    }
}

crate::bindings::macros::py_getters!(PyTermOutSpec |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms", "TermOutSpec.months")]
    months -> u32 = s.inner.months;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms", "AmortizationEvent")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "AmortizationEvent",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyAmortizationEvent {
    /// Inner canonical Rust event.
    pub(crate) inner: AmortizationEvent,
}

crate::bindings::macros::wire_methods!(
    PyAmortizationEvent,
    AmortizationEvent,
    "AmortizationEvent",
    module = "finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms",
    to_dict
);

#[pymethods]
impl PyAmortizationEvent {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms", "AmortizationEvent.date")]
    #[staticmethod]
    #[pyo3(text_signature = "(date)")]
    fn date(date: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: AmortizationEvent::Date {
                date: py_to_date(date)?,
            },
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms", "AmortizationEvent.cumulative_loss")]
    #[staticmethod]
    #[pyo3(text_signature = "(max_cumulative_loss)")]
    fn cumulative_loss(max_cumulative_loss: f64) -> PyResult<Self> {
        let inner = AmortizationEvent::CumulativeLoss {
            max_cumulative_loss,
        };
        inner.validate().map_err(crate::errors::core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms", "AmortizationEvent.excess_spread")]
    #[staticmethod]
    #[pyo3(text_signature = "(min_excess_spread_3m)")]
    fn excess_spread(min_excess_spread_3m: f64) -> PyResult<Self> {
        let inner = AmortizationEvent::ExcessSpread {
            min_excess_spread_3m,
        };
        inner.validate().map_err(crate::errors::core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms", "AmortizationEvent.kind")]
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            AmortizationEvent::Date { .. } => "date",
            AmortizationEvent::CumulativeLoss { .. } => "cumulative_loss",
            AmortizationEvent::ExcessSpread { .. } => "excess_spread",
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms", "AmortizationEvent.date_value")]
    #[getter]
    fn date_value<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner {
            AmortizationEvent::Date { date } => date_to_py(py, date).map(Some),
            _ => Ok(None),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms", "AmortizationEvent.max_cumulative_loss")]
    #[getter]
    fn max_cumulative_loss(&self) -> Option<f64> {
        match self.inner {
            AmortizationEvent::CumulativeLoss {
                max_cumulative_loss,
            } => Some(max_cumulative_loss),
            _ => None,
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms", "AmortizationEvent.min_excess_spread_3m")]
    #[getter]
    fn min_excess_spread_3m(&self) -> Option<f64> {
        match self.inner {
            AmortizationEvent::ExcessSpread {
                min_excess_spread_3m,
            } => Some(min_excess_spread_3m),
            _ => None,
        }
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("AmortizationEvent(kind={:?})", self.kind())
    }
}
