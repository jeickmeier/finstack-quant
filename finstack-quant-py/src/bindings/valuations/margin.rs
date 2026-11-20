//! Standalone instrument initial margin bindings.

use crate::bindings::core::currency::extract_currency;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::extract::{extract_instrument, extract_market};
use crate::bindings::margin::PyImResult;
use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, value_error};
use finstack_quant_margin::SimmVersion;
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.valuations._margin", "calculate_simm")]
#[pyfunction]
#[pyo3(signature = (instruments, market, as_of, *, version=None, mpor_days=None, currency=None))]
fn calculate_simm(
    instruments: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    version: Option<&str>,
    mpor_days: Option<u32>,
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyImResult> {
    let instruments = instruments
        .try_iter()?
        .map(|item| {
            extract_instrument(&item?)?
                .into_parsed(None, None)
                .map_err(core_to_py)
        })
        .collect::<PyResult<Vec<_>>>()?;
    let market = extract_market(market.py(), market)?;
    let date = py_to_date(as_of)?;
    let version = version
        .map(str::parse::<SimmVersion>)
        .transpose()
        .map_err(value_error)?;
    let currency = currency.map(extract_currency).transpose()?;
    let result = as_of
        .py()
        .detach(|| {
            let instruments = instruments
                .iter()
                .map(|item| item.as_instrument())
                .collect::<Vec<_>>();
            finstack_quant_valuations::margin::calculate_simm(
                &instruments,
                &market,
                date,
                version,
                mpor_days,
                currency,
            )
        })
        .map_err(core_to_py)?;
    Ok(PyImResult::from_inner(result))
}

/// Register the standalone SIMM function.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(calculate_simm, module)?)
}
