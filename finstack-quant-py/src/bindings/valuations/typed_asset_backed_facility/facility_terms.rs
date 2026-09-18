//! Typed facility terms: `TermOutSpec` and `AmortizationEvent`, the
//! revolving-period end and the early-amortization events of an
//! `AssetBackedFacility`.

use pyo3::prelude::*;

use crate::bindings::date_utils::{date_to_py, extract_date};
use finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::{
    AmortizationEvent, TermOutSpec,
};

/// Term-out: months after the revolving period ends over which the drawn
/// balance is repaid.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import TermOutSpec
/// >>> TermOutSpec(24).months
/// 24
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

sc_wire_methods!(PyTermOutSpec, TermOutSpec, "TermOutSpec");

#[pymethods]
impl PyTermOutSpec {
    /// Construct a term-out.
    ///
    /// Parameters
    /// ----------
    /// months : int
    ///     Months after the revolving period (or an earlier amortization
    ///     event) by which the line is repaid.
    #[new]
    #[pyo3(text_signature = "(months)")]
    fn new(months: u32) -> Self {
        Self {
            inner: TermOutSpec { months },
        }
    }

    /// Term-out length in months.
    #[getter]
    fn months(&self) -> u32 {
        self.inner.months
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("TermOutSpec(months={})", self.inner.months)
    }
}

/// An event that ends a facility's revolving period early.
///
/// Built through :meth:`date` (a fixed date), :meth:`cumulative_loss` (a
/// cumulative-loss threshold in percent of the original collateral) or
/// :meth:`excess_spread` (a trailing three-month excess-spread floor).
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.valuations.instruments import AmortizationEvent
/// >>> AmortizationEvent.cumulative_loss(4.0).max_pct
/// 4.0
/// >>> AmortizationEvent.date(datetime.date(2025, 1, 15)).kind
/// 'date'
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

sc_wire_methods!(PyAmortizationEvent, AmortizationEvent, "AmortizationEvent");

#[pymethods]
impl PyAmortizationEvent {
    /// The revolving period ends on a fixed date.
    ///
    /// Parameters
    /// ----------
    /// date : datetime.date
    ///     Date the event fires (the first payment date at or after it).
    ///
    /// Returns
    /// -------
    /// AmortizationEvent
    ///     The dated event.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``date`` is not a valid date.
    ///
    /// Examples
    /// --------
    /// >>> import datetime
    /// >>> from finstack_quant.valuations.instruments import AmortizationEvent
    /// >>> AmortizationEvent.date(datetime.date(2025, 1, 15)).date
    /// datetime.date(2025, 1, 15)
    #[staticmethod]
    #[pyo3(text_signature = "(date)")]
    fn date(date: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: AmortizationEvent::Date {
                date: extract_date(date)?,
            },
        })
    }

    /// The revolving period ends once cumulative losses reach a threshold.
    ///
    /// Parameters
    /// ----------
    /// max_pct : float
    ///     Cumulative net loss as a percent of the original collateral
    ///     balance (``4.0`` = 4%).
    ///
    /// Returns
    /// -------
    /// AmortizationEvent
    ///     The loss event.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``max_pct`` is negative or not finite.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import AmortizationEvent
    /// >>> AmortizationEvent.cumulative_loss(4.0).kind
    /// 'cumulative_loss'
    #[staticmethod]
    #[pyo3(text_signature = "(max_pct)")]
    fn cumulative_loss(max_pct: f64) -> PyResult<Self> {
        if !max_pct.is_finite() || max_pct < 0.0 {
            return Err(crate::errors::value_error(format!(
                "cumulative loss max_pct ({max_pct}) must be a finite non-negative percent"
            )));
        }
        Ok(Self {
            inner: AmortizationEvent::CumulativeLoss { max_pct },
        })
    }

    /// The revolving period ends once trailing excess spread falls below a
    /// floor.
    ///
    /// Parameters
    /// ----------
    /// min_3m : float
    ///     Minimum trailing three-month annualized excess spread as a
    ///     decimal (``0.01`` = 1%).
    ///
    /// Returns
    /// -------
    /// AmortizationEvent
    ///     The excess-spread event.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``min_3m`` is not finite.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import AmortizationEvent
    /// >>> AmortizationEvent.excess_spread(0.01).min_3m
    /// 0.01
    #[staticmethod]
    #[pyo3(text_signature = "(min_3m)")]
    fn excess_spread(min_3m: f64) -> PyResult<Self> {
        if !min_3m.is_finite() {
            return Err(crate::errors::value_error(format!(
                "excess spread min_3m ({min_3m}) must be finite"
            )));
        }
        Ok(Self {
            inner: AmortizationEvent::ExcessSpread { min_3m },
        })
    }

    /// ``"date"``, ``"cumulative_loss"`` or ``"excess_spread"``.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            AmortizationEvent::Date { .. } => "date",
            AmortizationEvent::CumulativeLoss { .. } => "cumulative_loss",
            AmortizationEvent::ExcessSpread { .. } => "excess_spread",
        }
    }

    /// Event date as ``datetime.date`` for a dated event, else ``None``.
    #[getter]
    fn date_value<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner {
            AmortizationEvent::Date { date } => date_to_py(py, date).map(Some),
            _ => Ok(None),
        }
    }

    /// Cumulative-loss threshold in percent, else ``None``.
    #[getter]
    fn max_pct(&self) -> Option<f64> {
        match self.inner {
            AmortizationEvent::CumulativeLoss { max_pct } => Some(max_pct),
            _ => None,
        }
    }

    /// Excess-spread floor (decimal), else ``None``.
    #[getter]
    fn min_3m(&self) -> Option<f64> {
        match self.inner {
            AmortizationEvent::ExcessSpread { min_3m } => Some(min_3m),
            _ => None,
        }
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("AmortizationEvent(kind={:?})", self.kind())
    }
}
