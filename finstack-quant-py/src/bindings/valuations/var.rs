//! Portfolio historical VaR / expected shortfall by full revaluation.
//!
//! Binds `finstack_quant_valuations::metrics::risk::var_calculator::calculate_var`
//! and its typed `VarResult`. The per-instrument `hvar` metric reprices one
//! instrument; this entry point reprices a whole list under every scenario and
//! takes the quantile of the summed P&L, so diversification is preserved.

use super::pricing::PyMarketHistory;
use crate::bindings::extract::{extract_instrument, extract_market, InstrumentArg};
use crate::bindings::module_utils::{opt_spec_from_py, spec_from_py};
use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::errors::{core_to_py, display_to_py};
use finstack_quant_valuations::metrics::risk::var_calculator::{
    calculate_var as rust_calculate_var, VarConfig, VarResult,
};
use finstack_quant_valuations::metrics::risk::MarketHistory;
use finstack_quant_valuations::pricer::PricingDispatch;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Coerce ``MarketHistory | dict | str`` into the Rust history.
fn extract_history(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<MarketHistory> {
    if let Ok(typed) = obj.cast::<PyMarketHistory>() {
        return Ok(typed.borrow().inner.clone());
    }
    spec_from_py(py, obj, "MarketHistory")
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "calculate_var")]
#[pyfunction]
#[pyo3(signature = (instruments, market, history, as_of, config=None, model="default"))]
#[pyo3(text_signature = "(instruments, market, history, as_of, config=None, model='default')")]
fn calculate_var(
    py: Python<'_>,
    instruments: Vec<Bound<'_, PyAny>>,
    market: &Bound<'_, PyAny>,
    history: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    config: Option<&Bound<'_, PyAny>>,
    model: &str,
) -> PyResult<PyVarResult> {
    let instruments = instruments
        .iter()
        .map(extract_instrument)
        .collect::<PyResult<Vec<InstrumentArg>>>()?;
    let history = extract_history(py, history)?;
    let config: VarConfig = opt_spec_from_py(py, config, "VarConfig")?.unwrap_or_default();
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
    let model = model.to_owned();
    py.detach(move || {
        let parsed = instruments
            .into_iter()
            .map(|instrument| instrument.into_parsed(None))
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;
        let refs: Vec<_> = parsed.iter().map(|p| p.as_instrument()).collect();
        rust_calculate_var(
            &refs,
            &market,
            &history,
            as_of,
            &config,
            PricingDispatch::from_model(
                &model,
                finstack_quant_valuations::pricer::STANDARD.shared_pricers(),
            )?,
            finstack_quant_calibration::recalibration::pricing_options().recalibration_provider,
        )
    })
    .map(|inner| PyVarResult { inner })
    .map_err(core_to_py)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "VarResult")]
#[pyclass(
    name = "VarResult",
    module = "finstack_quant.valuations.instruments",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyVarResult {
    pub(crate) inner: VarResult,
}

#[pymethods]
impl PyVarResult {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "VarResult.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        let series = py.import("pandas")?.getattr("Series")?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("dtype", "float64")?;
        data.set_item(
            "pnl",
            series.call((self.inner.pnl_distribution.clone(),), Some(&kwargs))?,
        )?;
        dict_to_dataframe(py, &data, None)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "VarResult.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid VarResult JSON"))?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "VarResult.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "VarResult.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "VarResult(var={}, expected_shortfall={}, num_scenarios={}, confidence_level={})",
            self.inner.var,
            self.inner.expected_shortfall,
            self.inner.num_scenarios,
            self.inner.confidence_level
        )
    }
}

crate::bindings::macros::py_getters!(PyVarResult |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "VarResult.var")]
    var -> f64 = s.inner.var;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "VarResult.expected_shortfall")]
    expected_shortfall -> f64 = s.inner.expected_shortfall;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "VarResult.pnl_distribution")]
    pnl_distribution -> Vec<f64> = s.inner.pnl_distribution.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "VarResult.num_scenarios")]
    num_scenarios -> usize = s.inner.num_scenarios;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "VarResult.confidence_level")]
    confidence_level -> f64 = s.inner.confidence_level;
});

/// Register the VaR entry point and result type on the instruments submodule.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyVarResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(calculate_var, m)?)?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["VarResult", "calculate_var"];
