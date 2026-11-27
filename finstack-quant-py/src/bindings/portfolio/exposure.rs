//! Typed portfolio and instrument path-exposure bindings.

use crate::bindings::core::config::PyFinstackConfig;
use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::extract::{extract_instrument, extract_portfolio_ref};
use crate::bindings::margin::PyExposureProfile;
use crate::bindings::margin::types::{PyCsaSpec, PyNettingSetId};
use crate::bindings::module_utils::{parse_currency, py_to_serde};
use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, portfolio_to_py};
use finstack_quant_portfolio::margin::{self, ExposureCashVm, ExposureSimulationConfig};
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.portfolio._exposure", "ExposureCashVm")]
#[pyclass(
    name = "ExposureCashVm",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyExposureCashVm {
    inner: ExposureCashVm,
}

#[pymethods]
impl PyExposureCashVm {
    #[new]
    #[pyo3(signature = (*, terms, opening_settled))]
    #[doc = pydoc!("finstack_quant.portfolio._exposure", "ExposureCashVm.__init__")]
    fn new(terms: &PyCsaSpec, opening_settled: &PyMoney) -> Self {
        Self {
            inner: ExposureCashVm {
                terms: terms.inner.clone(),
                opening_settled: opening_settled.inner,
            },
        }
    }
}

#[doc = pydoc!("finstack_quant.portfolio._exposure", "ExposureSimulationConfig")]
#[pyclass(
    name = "ExposureSimulationConfig",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyExposureSimulationConfig {
    inner: ExposureSimulationConfig,
}

#[pymethods]
impl PyExposureSimulationConfig {
    #[new]
    #[pyo3(signature = (*, netting_set, netting_source_id, position_ids, dates, factors, simulation, cash_vm=None))]
    #[doc = pydoc!("finstack_quant.portfolio._exposure", "ExposureSimulationConfig.__init__")]
    fn new(
        netting_set: &PyNettingSetId,
        netting_source_id: String,
        position_ids: Vec<String>,
        dates: Vec<Bound<'_, PyAny>>,
        factors: &Bound<'_, PyAny>,
        simulation: &Bound<'_, PyAny>,
        cash_vm: Option<&PyExposureCashVm>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: ExposureSimulationConfig {
                netting_set: netting_set.inner.clone(),
                netting_source_id,
                position_ids,
                dates: dates.iter().map(py_to_date).collect::<PyResult<Vec<_>>>()?,
                factors: py_to_serde(factors.py(), factors, "exposure factor mappings")?,
                simulation: py_to_serde(
                    simulation.py(),
                    simulation,
                    "path simulation specification",
                )?,
                cash_vm: cash_vm.map(|vm| vm.inner.clone()),
            },
        })
    }
}

#[pyfunction]
#[doc = pydoc!("finstack_quant.portfolio._exposure", "portfolio_exposure_profile")]
fn portfolio_exposure_profile(
    portfolio: &Bound<'_, PyAny>,
    market: PyRef<'_, PyMarketContext>,
    pricing: &PyFinstackConfig,
    spec: &PyExposureSimulationConfig,
) -> PyResult<PyExposureProfile> {
    let py = portfolio.py();
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let market = &market.inner;
    let portfolio: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market: &finstack_quant_core::market_data::context::MarketContext = market;
    let inner = py
        .detach(|| {
            margin::portfolio_exposure_profile(portfolio, market, &pricing.inner, &spec.inner)
        })
        .map_err(portfolio_to_py)?;
    Ok(PyExposureProfile::from_inner(inner))
}

#[pyfunction]
#[doc = pydoc!("finstack_quant.portfolio._exposure", "instrument_exposure_profile")]
fn instrument_exposure_profile(
    instrument: &Bound<'_, PyAny>,
    currency: &str,
    market: PyRef<'_, PyMarketContext>,
    pricing: &PyFinstackConfig,
    spec: &PyExposureSimulationConfig,
) -> PyResult<PyExposureProfile> {
    let py = market.py();
    let instrument = extract_instrument(instrument)?
        .into_parsed(None, None)
        .map_err(core_to_py)?;
    let market = &market.inner;
    let market: &finstack_quant_core::market_data::context::MarketContext = market;
    let currency = parse_currency(currency)?;
    let inner = py
        .detach(|| {
            margin::instrument_exposure_profile(
                instrument.as_instrument(),
                currency,
                market,
                &pricing.inner,
                &spec.inner,
            )
        })
        .map_err(portfolio_to_py)?;
    Ok(PyExposureProfile::from_inner(inner))
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyExposureSimulationConfig>()?;
    module.add_class::<PyExposureCashVm>()?;
    module.add_function(wrap_pyfunction!(portfolio_exposure_profile, module)?)?;
    module.add_function(wrap_pyfunction!(instrument_exposure_profile, module)?)?;
    Ok(())
}
