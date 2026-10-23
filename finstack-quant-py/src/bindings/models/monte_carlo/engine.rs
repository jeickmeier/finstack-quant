//! Path simulation, process helpers and canonical Monte Carlo convenience functions.

use super::results::{PyMoneyEstimate, PyPathSummary};
use crate::bindings::core::currency::extract_currency;
use crate::bindings::module_utils::py_to_json_value;
use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_core::currency::Currency;
use finstack_quant_models::monte_carlo::simulate::PathSimulationSpec;
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.models.monte_carlo", "simulate_paths")]
#[pyfunction]
#[pyo3(text_signature = "(spec)")]
fn simulate_paths(py: Python<'_>, spec: &Bound<'_, PyAny>) -> PyResult<PyPathSummary> {
    let value = py_to_json_value(py, spec, "PathSimulationSpec")?;
    py.detach(move || {
        let spec: PathSimulationSpec = serde_json::from_value(value)
            .map_err(|e| serde_json_to_py(e, "invalid PathSimulationSpec"))?;
        finstack_quant_models::monte_carlo::simulate::simulate_paths(&spec).map_err(core_to_py)
    })
    .map(PyPathSummary::from_inner)
}

#[doc = pydoc!("finstack_quant.models.monte_carlo", "heston_satisfies_feller")]
#[pyfunction]
fn heston_satisfies_feller(kappa: f64, theta: f64, vol_of_vol: f64) -> bool {
    finstack_quant_models::monte_carlo::process::heston::heston_satisfies_feller(
        kappa, theta, vol_of_vol,
    )
}

/// Extract an optional currency argument without applying any default.
///
/// Canonical entry points in the Monte Carlo crate own the registry default;
/// the binding only marshals an explicitly supplied currency.
pub(super) fn extract_optional_currency(
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<Currency>> {
    currency.map(extract_currency).transpose()
}

#[doc = pydoc!("finstack_quant.models.monte_carlo", "price_heston")]
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry, is_call, num_paths=None, seed=None, num_steps=None, currency=None))]
fn price_heston(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    kappa: f64,
    theta: f64,
    vol_of_vol: f64,
    rho: f64,
    v0: f64,
    expiry: f64,
    is_call: bool,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyMoneyEstimate> {
    use finstack_quant_models::monte_carlo::pricer::heston as canonical;
    use finstack_quant_models::types::OptionType;

    let ccy = extract_optional_currency(currency)?;
    py.detach(|| {
        canonical::price_heston(
            OptionType::from(is_call),
            spot,
            strike,
            rate,
            div_yield,
            kappa,
            theta,
            vol_of_vol,
            rho,
            v0,
            expiry,
            num_paths,
            seed,
            num_steps,
            ccy,
        )
    })
    .map(PyMoneyEstimate::from_inner)
    .map_err(core_to_py)
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(simulate_paths, m)?)?;
    m.add_function(wrap_pyfunction!(heston_satisfies_feller, m)?)?;
    m.add_function(wrap_pyfunction!(price_heston, m)?)?;
    Ok(())
}
