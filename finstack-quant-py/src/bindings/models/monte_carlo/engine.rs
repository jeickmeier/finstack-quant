//! Path simulation, process helpers and canonical Monte Carlo convenience functions.

use super::results::{PyMoneyEstimate, PyPathSummary};
use crate::bindings::core::currency::extract_currency;
use crate::bindings::module_utils::{opt_spec_from_py, py_to_json_value, spec_from_py};
use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_core::currency::Currency;
use finstack_quant_models::monte_carlo::simulate::PathSimulationSpec;
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.models.monte_carlo._engine", "simulate_paths")]
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

#[doc = pydoc!("finstack_quant.models.monte_carlo._engine", "heston_satisfies_feller")]
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

#[doc = pydoc!("finstack_quant.models.monte_carlo._engine", "price_heston")]
#[pyfunction]
#[pyo3(signature = (spot, strike, expiry, params, is_call, settings=None))]
fn price_heston(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    expiry: f64,
    params: &Bound<'_, PyAny>,
    is_call: bool,
    settings: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyMoneyEstimate> {
    use finstack_quant_models::closed_form::HestonPricingParams;
    use finstack_quant_models::monte_carlo::pricer::heston as canonical;
    use finstack_quant_models::types::OptionType;

    let params: HestonPricingParams = spec_from_py(py, params, "HestonPricingParams")?;
    let settings: canonical::HestonMcSettings =
        opt_spec_from_py(py, settings, "HestonMcSettings")?.unwrap_or_default();
    py.detach(|| {
        canonical::price_heston(
            spot,
            strike,
            expiry,
            &params,
            OptionType::from(is_call),
            &settings,
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
