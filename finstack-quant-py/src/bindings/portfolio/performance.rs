//! Python bindings for portfolio performance measurement.
//!
//! The functions accept JSON inputs matching the Rust `serde` shapes and
//! delegate all calculations to `finstack_quant_portfolio::performance`.
//! `twrr_linked` returns a typed wrapper; `twrr_linked_json` keeps the exact
//! JSON wire string.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bindings::pandas_utils::serde_object_to_single_row_dataframe_with_schema;
use crate::errors::{core_to_py, display_to_py, serde_json_to_py};

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

    /// Serialize to a compact JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_portfolio::LinkedReturn =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "LinkedReturn(cumulative={}, annualised={}, num_periods={})",
            self.inner.cumulative, self.inner.annualised, self.inner.num_periods,
        )
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Compute a Modified-Dietz TWRR sub-period return.
///
/// Parameters
/// ----------
/// period : str | dict | None
///     Complete ``TwrrPeriod`` (``beginning_market_value``,
///     ``ending_market_value``, ``cashflows: [{amount,
///     fraction_of_period_remaining}]``) as JSON or a dict. Omit it to build
///     the period from the keyword arguments instead.
/// beginning_market_value : float | None
///     PV at period start (used when ``period`` is omitted).
/// ending_market_value : float | None
///     PV at period end (used when ``period`` is omitted).
/// cashflows : list[tuple[float, float]] | None
///     External flows as ``(amount, fraction_of_period_remaining)`` pairs:
///     positive amount = contribution into the portfolio, fraction in
///     ``[0, 1]`` weighting the flow by time remaining. Defaults to none.
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
///     cashflow weight outside ``[0, 1]``), the inputs are malformed, or
///     neither ``period`` nor both market values are supplied.
#[pyfunction]
#[pyo3(
    signature = (period=None, *, beginning_market_value=None, ending_market_value=None, cashflows=None),
    text_signature = "(period=None, *, beginning_market_value=None, ending_market_value=None, cashflows=None)"
)]
fn twrr_modified_dietz(
    py: Python<'_>,
    period: Option<&Bound<'_, PyAny>>,
    beginning_market_value: Option<f64>,
    ending_market_value: Option<f64>,
    cashflows: Option<Vec<(f64, f64)>>,
) -> PyResult<f64> {
    let period: finstack_quant_portfolio::TwrrPeriod = match period {
        Some(obj) => {
            let json = crate::bindings::extract::extract_records_json(py, obj, "period")?;
            serde_json::from_str(&json)
                .map_err(|err| serde_json_to_py(err, "invalid TWRR period JSON"))?
        }
        None => {
            let (Some(beginning_market_value), Some(ending_market_value)) =
                (beginning_market_value, ending_market_value)
            else {
                return Err(crate::errors::value_error(
                    "twrr_modified_dietz requires either `period` or both \
                     `beginning_market_value` and `ending_market_value`",
                ));
            };
            finstack_quant_portfolio::TwrrPeriod {
                beginning_market_value,
                ending_market_value,
                cashflows: cashflows
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(amount, fraction_of_period_remaining)| {
                        finstack_quant_portfolio::performance::DietzFlow {
                            amount,
                            fraction_of_period_remaining,
                        }
                    })
                    .collect(),
            }
        }
    };
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
///     Use :func:`twrr_linked_json` for the raw wire string.
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

/// Geometrically link TWRR sub-period returns and return wire JSON.
///
/// Wire twin of :func:`twrr_linked`; same inputs, JSON-string output.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``LinkedReturn``.
#[pyfunction]
#[pyo3(text_signature = "(returns_json, horizon_years)")]
fn twrr_linked_json(
    py: Python<'_>,
    returns_json: &Bound<'_, PyAny>,
    horizon_years: f64,
) -> PyResult<String> {
    let returns_json = crate::bindings::extract::extract_records_json(py, returns_json, "returns")?;
    let returns_json: &str = &returns_json;
    let result = run_twrr_linked(py, returns_json, horizon_years)?;
    serde_json::to_string(&result).map_err(|err| serde_json_to_py(err, "serialize linked return"))
}

/// Compute the money-weighted return (XIRR, Act/365F) from dated cashflows.
///
/// Parameters
/// ----------
/// cashflows : str | list[tuple[date, float]] | list[dict] | pandas.DataFrame
///     Dated flows from the investor's cash account: contributions negative,
///     terminal value / distributions positive. Accepts ``(date, amount)``
///     pairs (dates as ``datetime.date`` or ISO strings), JSON-shaped dicts
///     with ``date`` and ``amount`` keys, a DataFrame with those columns, or
///     the canonical JSON array string.
///
/// Returns
/// -------
/// float
///     Annualised internal rate of return as a decimal fraction.
///
/// Raises
/// ------
/// ValueError
///     If the flows are malformed, all one sign, or no root is found.
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
    py.detach(move || finstack_quant_portfolio::mwr_xirr_from_cashflows(&flows).map_err(core_to_py))
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
        pairs.push((crate::bindings::date_utils::extract_date(&date)?, amount));
    }
    Ok(Some(pairs))
}

/// Register performance measurement functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLinkedReturn>()?;
    m.add_function(wrap_pyfunction!(twrr_modified_dietz, m)?)?;
    m.add_function(wrap_pyfunction!(twrr_linked, m)?)?;
    m.add_function(wrap_pyfunction!(twrr_linked_json, m)?)?;
    m.add_function(wrap_pyfunction!(mwr_xirr, m)?)?;
    Ok(())
}
