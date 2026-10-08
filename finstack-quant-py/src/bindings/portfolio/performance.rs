//! Python bindings for portfolio performance measurement.
//!
//! The functions accept JSON inputs matching the Rust `serde` shapes and
//! delegate all calculations to `finstack_quant_portfolio::performance`.
//! `twrr_linked` returns a typed wrapper; `to_json` provides its canonical
//! JSON wire string.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bindings::pandas_utils::serde_object_to_single_row_dataframe_with_schema;
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
    pub(crate) inner: finstack_quant_portfolio::LinkedReturn,
}

crate::bindings::macros::wire_methods!(
    PyLinkedReturn,
    finstack_quant_portfolio::LinkedReturn,
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

    /// Number of sub-periods linked.
    #[getter]
    fn num_periods(&self) -> usize {
        self.inner.num_periods
    }

    /// Single-row :class:`pandas.DataFrame` view of the linked return.
    ///
    /// Columns: ``cumulative``, ``annualised``, ``num_periods``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &["cumulative", "annualised", "num_periods"],
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "LinkedReturn(cumulative={}, annualised={}, num_periods={})",
            self.inner.cumulative, self.inner.annualised, self.inner.num_periods,
        )
    }
}

/// Compute a Modified-Dietz TWRR sub-period return.
///
/// Parameters
/// ----------
/// period : str | dict
///     ``TwrrPeriod`` as JSON or a dict: ``beginning_market_value``,
///     ``ending_market_value`` and optional ``cashflows: [{amount,
///     fraction_of_period_remaining}]`` (omitted means no flows). A positive
///     ``amount`` is a contribution into the portfolio; the fraction, in
///     ``[0, 1]``, is the share of the period remaining after the flow.
///     Unknown keys are rejected. The same input the WASM
///     ``twrrModifiedDietz`` takes.
///
/// Returns
/// -------
/// float
///     Sub-period return as a decimal fraction.
///
/// Raises
/// ------
/// ValueError
///     When the return is undefined (non-positive adjusted denominator, a
///     cashflow weight outside ``[0, 1]``) or the period is malformed or has
///     unknown keys.
#[pyfunction]
#[pyo3(text_signature = "(period)")]
fn twrr_modified_dietz(py: Python<'_>, period: &Bound<'_, PyAny>) -> PyResult<f64> {
    let json = crate::bindings::extract::extract_records_json(py, period, "period")?;
    let period: finstack_quant_portfolio::TwrrPeriod = serde_json::from_str(&json)
        .map_err(|err| serde_json_to_py(err, "invalid TWRR period JSON"))?;
    py.detach(move || finstack_quant_portfolio::twrr_modified_dietz(&period).map_err(core_to_py))
}

/// Parse the returns and run the canonical geometric linking.
fn run_twrr_linked(
    py: Python<'_>,
    returns_json: &str,
    horizon_years: f64,
) -> PyResult<finstack_quant_portfolio::LinkedReturn> {
    let returns_json = returns_json.to_owned();
    py.detach(move || {
        let returns: Vec<f64> = serde_json::from_str(&returns_json)
            .map_err(|err| serde_json_to_py(err, "invalid TWRR returns JSON"))?;
        finstack_quant_portfolio::twrr_linked(&returns, horizon_years).map_err(core_to_py)
    })
}

/// Geometrically link TWRR sub-period returns.
///
/// Parameters
/// ----------
/// returns_json : str | dict | list | pandas.DataFrame
///     JSON array of sub-period returns as decimal fractions.
/// horizon_years : float
///     Full elapsed horizon in 365-day calendar years; values below one skip
///     annualization (``annualised`` then mirrors ``cumulative``).
///
/// Returns
/// -------
/// LinkedReturn
///     Typed result with ``cumulative``, ``annualised`` and ``num_periods``.
///     Use :meth:`to_json` for the raw wire string.
///
/// Raises
/// ------
/// ValueError
///     When any sub-period return is non-finite or at most -1, or the
///     compounded growth factor is non-positive.
#[pyfunction]
#[pyo3(text_signature = "(returns_json, horizon_years)")]
fn twrr_linked(
    py: Python<'_>,
    returns_json: &Bound<'_, PyAny>,
    horizon_years: f64,
) -> PyResult<PyLinkedReturn> {
    let returns_json = crate::bindings::extract::extract_records_json(py, returns_json, "returns")?;
    let returns_json: &str = &returns_json;
    Ok(PyLinkedReturn {
        inner: run_twrr_linked(py, returns_json, horizon_years)?,
    })
}
/// Compute the money-weighted return (XIRR, Act/365F) from dated cashflows.
///
/// Binds Rust ``mwr_xirr``.
///
/// Parameters
/// ----------
/// cashflows : str | list[tuple[date, float]] | list[dict] | pandas.DataFrame
///     Dated flows from the investor's cash account: contributions negative,
///     terminal value / distributions positive. Accepts ``(date, amount)``
///     pairs (dates as ``datetime.date`` or ISO strings), JSON-shaped dicts
///     with ``date`` and ``amount`` keys, a DataFrame with those columns, or
///     the canonical JSON array string. Dates are sorted and equal-date
///     flows netted; the remaining nonzero flows must change sign exactly once.
///
/// Returns
/// -------
/// float
///     Unique annualised internal rate of return as a finite decimal greater
///     than -1, using Act/365F year fractions.
///
/// Raises
/// ------
/// ValueError
///     If the flows are malformed, have fewer than two nonzero net dates,
///     do not have exactly one net sign change, or no sufficiently accurate
///     finite return greater than -1 can be found. Nonconventional cashflows
///     are rejected even when a numerical solver could find one of their roots.
/// RuntimeError
///     If the numerical solver fails to converge within the valid return bracket.
#[pyfunction]
#[pyo3(text_signature = "(cashflows)")]
fn mwr_xirr(py: Python<'_>, cashflows: &Bound<'_, PyAny>) -> PyResult<f64> {
    let pairs = extract_dated_pairs(cashflows)?;
    let flows: Vec<finstack_quant_portfolio::DatedCashflow> = match pairs {
        Some(pairs) => pairs
            .into_iter()
            .map(|(date, amount)| finstack_quant_portfolio::DatedCashflow { date, amount })
            .collect(),
        None => {
            let json = crate::bindings::extract::extract_records_json(py, cashflows, "cashflows")?;
            serde_json::from_str(&json)
                .map_err(|err| serde_json_to_py(err, "invalid MWR cashflows JSON"))?
        }
    };
    py.detach(move || finstack_quant_portfolio::mwr_xirr(&flows).map_err(core_to_py))
}

/// Interpret a sequence of ``(date, amount)`` 2-tuples; ``None`` when the
/// input is not in that shape (so the caller can fall back to JSON records).
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
