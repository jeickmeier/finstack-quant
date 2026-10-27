//! Pricer bindings — European, Path-Dependent, LSMC.

use super::engine::extract_optional_currency;
use super::results::{PyLrmGreeks, PyMoneyEstimate};
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_models::monte_carlo::convenience;
use finstack_quant_models::monte_carlo::pricer::european::EuropeanPricer;
use finstack_quant_models::monte_carlo::pricer::lsmc::LsmcPricer;
use finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricer;
use finstack_quant_models::types::{OptionMarketParams, OptionType};
use pyo3::prelude::*;

/// Render a bool the way Python's `repr` does.
fn py_bool(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

#[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "EuropeanPricer")]
#[pyclass(
    name = "EuropeanPricer",
    module = "finstack_quant.models.monte_carlo",
    frozen
)]
pub struct PyEuropeanPricer {
    inner: EuropeanPricer,
}

#[pymethods]
impl PyEuropeanPricer {
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "EuropeanPricer.__init__")]
    #[new]
    #[pyo3(signature = (num_paths=None, seed=None, use_parallel=None))]
    fn new(
        num_paths: Option<usize>,
        seed: Option<u64>,
        use_parallel: Option<bool>,
    ) -> PyResult<Self> {
        convenience::european_pricer(num_paths, seed, use_parallel)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "EuropeanPricer.num_paths")]
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths()
    }
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "EuropeanPricer.seed")]
    #[getter]
    fn seed(&self) -> u64 {
        self.inner.seed()
    }
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "EuropeanPricer.use_parallel")]
    #[getter]
    fn use_parallel(&self) -> bool {
        self.inner.use_parallel()
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "EuropeanPricer.price_gbm")]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, is_call, num_steps=None, currency=None))]
    fn price_gbm(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        is_call: bool,
        num_steps: Option<usize>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyMoneyEstimate> {
        let ccy = extract_optional_currency(currency)?;
        let pricer = &self.inner;
        py.detach(|| {
            pricer.price_gbm(
                OptionType::from(is_call),
                spot,
                strike,
                rate,
                div_yield,
                vol,
                expiry,
                num_steps,
                ccy,
            )
        })
        .map(PyMoneyEstimate::from_inner)
        .map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "EuropeanPricer(num_paths={}, seed={}, use_parallel={})",
            self.inner.num_paths(),
            self.inner.seed(),
            py_bool(self.inner.use_parallel()),
        )
    }
}

#[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "PathDependentPricer")]
#[pyclass(
    name = "PathDependentPricer",
    module = "finstack_quant.models.monte_carlo",
    frozen
)]
pub struct PyPathDependentPricer {
    inner: PathDependentPricer,
}

#[pymethods]
impl PyPathDependentPricer {
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "PathDependentPricer.__init__")]
    #[new]
    #[pyo3(signature = (
        num_paths=None, seed=None, use_parallel=None,
        antithetic=None, use_sobol=None, use_brownian_bridge=None,
    ))]
    fn new(
        num_paths: Option<usize>,
        seed: Option<u64>,
        use_parallel: Option<bool>,
        antithetic: Option<bool>,
        use_sobol: Option<bool>,
        use_brownian_bridge: Option<bool>,
    ) -> PyResult<Self> {
        convenience::path_dependent_pricer(
            num_paths,
            seed,
            use_parallel,
            antithetic,
            use_sobol,
            use_brownian_bridge,
        )
        .map(|inner| Self { inner })
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "PathDependentPricer.price_asian")]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, is_call, num_steps=None, currency=None))]
    fn price_asian(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        is_call: bool,
        num_steps: Option<usize>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyMoneyEstimate> {
        let ccy = extract_optional_currency(currency)?;
        let pricer = &self.inner;
        py.detach(|| {
            pricer.price_asian(
                OptionType::from(is_call),
                spot,
                strike,
                rate,
                div_yield,
                vol,
                expiry,
                num_steps,
                ccy,
            )
        })
        .map(PyMoneyEstimate::from_inner)
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "PathDependentPricer.price_with_lrm_greeks")]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, is_call, num_steps=None, currency=None))]
    fn price_with_lrm_greeks(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        is_call: bool,
        num_steps: Option<usize>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyLrmGreeks> {
        let ccy = extract_optional_currency(currency)?;
        let pricer = &self.inner;
        py.detach(|| {
            pricer.price_with_lrm_greeks(
                OptionType::from(is_call),
                spot,
                strike,
                rate,
                div_yield,
                vol,
                expiry,
                num_steps,
                ccy,
            )
        })
        .map(PyLrmGreeks::from_inner)
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "PathDependentPricer.num_paths")]
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.config().num_paths
    }
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "PathDependentPricer.seed")]
    #[getter]
    fn seed(&self) -> u64 {
        self.inner.config().seed
    }
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "PathDependentPricer.use_parallel")]
    #[getter]
    fn use_parallel(&self) -> bool {
        self.inner.config().use_parallel
    }
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "PathDependentPricer.antithetic")]
    #[getter]
    fn antithetic(&self) -> bool {
        self.inner.config().antithetic
    }
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "PathDependentPricer.use_sobol")]
    #[getter]
    fn use_sobol(&self) -> bool {
        self.inner.config().use_sobol
    }
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "PathDependentPricer.use_brownian_bridge")]
    #[getter]
    fn use_brownian_bridge(&self) -> bool {
        self.inner.config().use_brownian_bridge
    }

    fn __repr__(&self) -> String {
        let config = self.inner.config();
        format!(
            "PathDependentPricer(num_paths={}, seed={}, use_parallel={}, antithetic={}, use_sobol={}, use_brownian_bridge={})",
            config.num_paths,
            config.seed,
            py_bool(config.use_parallel),
            py_bool(config.antithetic),
            py_bool(config.use_sobol),
            py_bool(config.use_brownian_bridge),
        )
    }
}

#[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "LsmcPricer")]
#[pyclass(
    name = "LsmcPricer",
    module = "finstack_quant.models.monte_carlo",
    frozen
)]
pub struct PyLsmcPricer {
    inner: LsmcPricer,
}

#[pymethods]
impl PyLsmcPricer {
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "LsmcPricer.__init__")]
    #[new]
    #[pyo3(signature = (
        num_paths=None,
        seed=None,
        use_parallel=None,
        num_steps=None,
        basis=None,
        basis_degree=None,
        antithetic=None,
    ))]
    fn new(
        num_paths: Option<usize>,
        seed: Option<u64>,
        use_parallel: Option<bool>,
        num_steps: Option<usize>,
        basis: Option<&str>,
        basis_degree: Option<usize>,
        antithetic: Option<bool>,
    ) -> PyResult<Self> {
        LsmcPricer::new(
            num_paths,
            seed,
            use_parallel,
            num_steps,
            basis,
            basis_degree,
            antithetic,
        )
        .map(|inner| Self { inner })
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "LsmcPricer.num_paths")]
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.pricer().config().num_paths
    }
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "LsmcPricer.seed")]
    #[getter]
    fn seed(&self) -> u64 {
        self.inner.pricer().config().seed
    }
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "LsmcPricer.use_parallel")]
    #[getter]
    fn use_parallel(&self) -> bool {
        self.inner.pricer().config().use_parallel
    }
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "LsmcPricer.antithetic")]
    #[getter]
    fn antithetic(&self) -> bool {
        self.inner.pricer().config().antithetic
    }
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "LsmcPricer.basis")]
    #[getter]
    fn basis(&self) -> &'static str {
        self.inner.basis().as_str()
    }
    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "LsmcPricer.basis_degree")]
    #[getter]
    fn basis_degree(&self) -> usize {
        self.inner.basis_degree()
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "LsmcPricer.price_american")]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, is_call, currency=None, num_steps=None, basis=None, basis_degree=None))]
    fn price_american(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        is_call: bool,
        currency: Option<&Bound<'_, PyAny>>,
        num_steps: Option<usize>,
        basis: Option<&str>,
        basis_degree: Option<usize>,
    ) -> PyResult<PyMoneyEstimate> {
        let currency = extract_optional_currency(currency)?;
        py.detach(|| {
            self.inner.price_american(
                &OptionMarketParams {
                    option_type: OptionType::from(is_call),
                    spot,
                    strike,
                    rate,
                    dividend_yield: div_yield,
                    volatility: vol,
                    time_to_expiry: expiry,
                },
                num_steps,
                currency,
                basis,
                basis_degree,
            )
        })
        .map(PyMoneyEstimate::from_inner)
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._pricers", "LsmcPricer.price_american_unbiased")]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        spot, strike, rate, div_yield, vol, expiry, is_call, pricing_seed, currency=None,
        num_steps=None, basis=None, basis_degree=None,
    ))]
    fn price_american_unbiased(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        is_call: bool,
        pricing_seed: u64,
        currency: Option<&Bound<'_, PyAny>>,
        num_steps: Option<usize>,
        basis: Option<&str>,
        basis_degree: Option<usize>,
    ) -> PyResult<PyMoneyEstimate> {
        let currency = extract_optional_currency(currency)?;
        py.detach(|| {
            self.inner.price_american_unbiased(
                &OptionMarketParams {
                    option_type: OptionType::from(is_call),
                    spot,
                    strike,
                    rate,
                    dividend_yield: div_yield,
                    volatility: vol,
                    time_to_expiry: expiry,
                },
                num_steps,
                currency,
                basis,
                basis_degree,
                pricing_seed,
            )
        })
        .map(PyMoneyEstimate::from_inner)
        .map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "LsmcPricer(num_paths={}, seed={}, use_parallel={}, antithetic={}, num_steps={}, basis='{}', basis_degree={})",
            self.inner.pricer().config().num_paths,
            self.inner.pricer().config().seed,
            py_bool(self.inner.pricer().config().use_parallel),
            py_bool(self.inner.pricer().config().antithetic),
            self.inner.num_steps(),
            self.inner.basis().as_str(),
            self.inner.basis_degree(),
        )
    }
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyEuropeanPricer>()?;
    m.add_class::<PyPathDependentPricer>()?;
    m.add_class::<PyLsmcPricer>()?;
    Ok(())
}
