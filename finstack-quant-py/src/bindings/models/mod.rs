//! Python bindings for reusable quantitative model engines.

mod analytic;
pub mod correlation;
pub(crate) mod credit;
pub mod factor;
mod fourier;
mod liquidity;
pub mod monte_carlo;
pub mod rates;
mod volatility;
mod volatility_arbitrage;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `finstack_quant.models` domain.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = crate::bindings::module_utils::new_submodule(parent, "models")?;
    module.setattr(
        "__doc__",
        "Reusable analytical, Fourier, volatility, credit, correlation, rates, and Monte Carlo models.",
    )?;

    analytic::register(py, &module)?;
    fourier::register(py, &module)?;
    volatility::register(py, &module)?;
    monte_carlo::register(py, &module)?;
    credit::register(py, &module)?;
    factor::register(py, &module)?;
    liquidity::register(py, &module)?;
    correlation::register(py, &module)?;
    rates::register(py, &module)?;

    let all = PyList::new(
        py,
        [
            "BsGreeks",
            "asian_option_price",
            "bachelier_greeks",
            "bachelier_price",
            "barrier_call",
            "barrier_put",
            "black76_greeks",
            "black76_implied_vol",
            "black76_price",
            "black_shifted_price",
            "black_shifted_vega",
            "bs_cos_price",
            "bs_greeks",
            "bs_implied_vol",
            "bs_price",
            "correlation",
            "credit",
            "factor",
            "heston_price",
            "liquidity",
            "lookback_option_price",
            "merton_jump_cos_price",
            "monte_carlo",
            "quanto_option_price",
            "rates",
            "vanilla_expiry_payoff",
            "vg_cos_price",
            "volatility",
        ],
    )?;
    module.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &module,
        crate::bindings::module_utils::Exposure::Python,
    )?;
    Ok(())
}
