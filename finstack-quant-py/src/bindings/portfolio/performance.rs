//! Python bindings for portfolio performance measurement.
//!
//! The functions accept JSON inputs matching the Rust `serde` shapes and
//! delegate all calculations to `finstack_quant_portfolio::performance`.
//! `twrr_linked` returns a typed wrapper; `to_json` provides its canonical
//! JSON wire string.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::tabular_methods;
use crate::bindings::table_builders::serde_object_to_single_row_arrow_table;
use crate::errors::{core_to_py, serde_json_to_py};

/// Result of geometrically linking TWRR sub-period returns.
///
/// Returned by :func:`twrr_linked`.
#[pyclass(
    name = "LinkedReturn",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyLinkedReturn {
    pub(crate) inner: finstack_quant_portfolio::performance::LinkedReturn,
}

crate::bindings::macros::wire_methods!(
    PyLinkedReturn,
    finstack_quant_portfolio::performance::LinkedReturn,
    "LinkedReturn"
);

#[pymethods]
impl PyLinkedReturn {
    /// Cumulative return over the full horizon.
    #[getter]
    fn cumulative(&self) -> f64 {
        self.inner.cumulative
    }

    /// Annualised return; mirrors ``cumulative`` for horizons below one year.
    #[getter]
    fn annualised(&self) -> f64 {
        self.inner.annualised
    }

    /// Whether ``annualised`` was annualised: true for horizons of at least one year.
    #[getter]
    fn is_annualised(&self) -> bool {
        self.inner.is_annualised
    }

    /// Number of sub-periods linked.
    #[getter]
    fn num_periods(&self) -> usize {
        self.inner.num_periods
    }

    fn __repr__(&self) -> String {
        format!(
            "LinkedReturn(cumulative={}, annualised={}, is_annualised={}, num_periods={})",
            self.inner.cumulative,
            self.inner.annualised,
            self.inner.is_annualised,
            self.inner.num_periods,
        )
    }
}

impl PyLinkedReturn {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_object_to_single_row_arrow_table(
            &self.inner,
            &[
                ("cumulative", "float64"),
                ("annualised", "float64"),
                ("is_annualised", "bool"),
                ("num_periods", "int64"),
            ],
        )
    }
}

tabular_methods!(PyLinkedReturn);

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._performance", "twrr_modified_dietz")]
#[pyfunction]
#[pyo3(text_signature = "(period)")]
fn twrr_modified_dietz(py: Python<'_>, period: &Bound<'_, PyAny>) -> PyResult<f64> {
    let json = crate::bindings::extract::extract_records_json(py, period, "period")?;
    let period: finstack_quant_portfolio::performance::TwrrPeriod = serde_json::from_str(&json)
        .map_err(|err| serde_json_to_py(err, "invalid TWRR period JSON"))?;
    py.detach(move || {
        finstack_quant_portfolio::performance::twrr_modified_dietz(&period).map_err(core_to_py)
    })
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._performance", "twrr_linked")]
#[pyfunction]
#[pyo3(text_signature = "(returns, horizon_years)")]
fn twrr_linked(
    py: Python<'_>,
    returns: &Bound<'_, PyAny>,
    horizon_years: f64,
) -> PyResult<PyLinkedReturn> {
    let returns: Vec<f64> = if crate::bindings::dataframe_utils::is_dataframe_like(returns)? {
        let Some((flat, _nrows, ncols)) =
            crate::bindings::dataframe_utils::extract_2d_f64(returns, "returns")?
        else {
            return Err(crate::errors::value_error(
                "returns DataFrame could not be extracted as float64 values",
            ));
        };
        if ncols != 1 {
            return Err(crate::errors::value_error(format!(
                "twrr_linked expects a single-column DataFrame of returns, got {ncols} columns"
            )));
        }
        flat
    } else {
        crate::bindings::dataframe_utils::extract_f64_vec(returns, "returns")?
    };
    Ok(PyLinkedReturn {
        inner: py.detach(move || {
            finstack_quant_portfolio::performance::twrr_linked(&returns, horizon_years)
                .map_err(core_to_py)
        })?,
    })
}
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._performance", "mwr_xirr")]
#[pyfunction]
#[pyo3(text_signature = "(cashflows)")]
fn mwr_xirr(py: Python<'_>, cashflows: &Bound<'_, PyAny>) -> PyResult<f64> {
    let flows: Vec<finstack_quant_portfolio::performance::DatedCashflow> =
        if crate::bindings::dataframe_utils::is_pandas_series(cashflows)? {
            let index = cashflows.getattr("index")?.call_method0("tolist")?;
            let dates_py: Vec<Bound<'_, PyAny>> = index.extract()?;
            let amounts =
                crate::bindings::dataframe_utils::extract_float64_column(cashflows, "cashflows")?;
            dates_py
                .iter()
                .zip(amounts)
                .map(|(d, amount)| {
                    Ok(finstack_quant_portfolio::performance::DatedCashflow {
                        date: crate::bindings::date_utils::py_to_date(d)?,
                        amount,
                    })
                })
                .collect::<PyResult<Vec<_>>>()?
        } else if crate::bindings::dataframe_utils::is_pandas_dataframe(cashflows)?
            && !cashflows.getattr("columns")?.contains("date")?
            && cashflows.getattr("columns")?.contains("amount")?
        {
            let dates_py: Vec<Bound<'_, PyAny>> = cashflows
                .getattr("index")?
                .call_method0("tolist")?
                .extract()?;
            let amounts = crate::bindings::dataframe_utils::extract_float64_column(
                &cashflows.get_item("amount")?,
                "amount",
            )?;
            dates_py
                .iter()
                .zip(amounts)
                .map(|(d, amount)| {
                    Ok(finstack_quant_portfolio::performance::DatedCashflow {
                        date: crate::bindings::date_utils::py_to_date(d)?,
                        amount,
                    })
                })
                .collect::<PyResult<Vec<_>>>()?
        } else if let Some(pairs) = extract_dated_pairs(cashflows)? {
            pairs
                .into_iter()
                .map(
                    |(date, amount)| finstack_quant_portfolio::performance::DatedCashflow {
                        date,
                        amount,
                    },
                )
                .collect()
        } else {
            let json = crate::bindings::extract::extract_records_json(py, cashflows, "cashflows")?;
            serde_json::from_str(&json)
                .map_err(|err| serde_json_to_py(err, "invalid MWR cashflows JSON"))?
        };
    py.detach(move || finstack_quant_portfolio::performance::mwr_xirr(&flows).map_err(core_to_py))
}

/// Interpret a sequence of ``(date, amount)`` 2-tuples; ``None`` when the
/// input is not in that shape (so the caller can fall back to records).
fn extract_dated_pairs(
    obj: &Bound<'_, PyAny>,
) -> PyResult<Option<Vec<(finstack_quant_core::dates::Date, f64)>>> {
    if obj.extract::<String>().is_ok() || obj.hasattr("columns")? {
        return Ok(None);
    }
    let Ok(iter) = obj.try_iter() else {
        return Ok(None);
    };
    let mut pairs = Vec::new();
    for item in iter {
        let item = item?;
        let Ok((date, amount)) = item.extract::<(Bound<'_, PyAny>, f64)>() else {
            return Ok(None);
        };
        pairs.push((crate::bindings::date_utils::py_to_date(&date)?, amount));
    }
    Ok(Some(pairs))
}

/// Register performance measurement functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLinkedReturn>()?;
    m.add_function(wrap_pyfunction!(twrr_modified_dietz, m)?)?;
    m.add_function(wrap_pyfunction!(twrr_linked, m)?)?;
    m.add_function(wrap_pyfunction!(mwr_xirr, m)?)?;
    Ok(())
}
