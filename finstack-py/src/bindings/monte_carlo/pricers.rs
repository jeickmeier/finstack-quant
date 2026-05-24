//! Pricer bindings — European, Path-Dependent, LSMC.

use super::engine::resolve_currency;
use super::results::PyMonteCarloResult;
use crate::errors::core_to_py;
use finstack_core::currency::Currency;
use finstack_monte_carlo::pricer::basis::{build_lsmc_basis, BasisKind, LsmcBasis};
use finstack_monte_carlo::pricer::european::EuropeanPricer;
use finstack_monte_carlo::pricer::lsmc::{LsmcConfig, LsmcPricer};
use finstack_monte_carlo::process::gbm::GbmProcess;
use finstack_monte_carlo::registry::{embedded_defaults, PythonBindingDefaults};
use pyo3::prelude::*;

fn py_mc_defaults() -> PyResult<&'static PythonBindingDefaults> {
    embedded_defaults()
        .map(|defaults| &defaults.python_bindings)
        .map_err(core_to_py)
}

/// Convenience pricer for European options under GBM dynamics.
#[pyclass(name = "EuropeanPricer", module = "finstack.monte_carlo", frozen)]
pub struct PyEuropeanPricer {
    num_paths: usize,
    seed: u64,
    use_parallel: bool,
}

#[pymethods]
impl PyEuropeanPricer {
    #[new]
    #[pyo3(signature = (num_paths=None, seed=None, use_parallel=None))]
    fn new(
        num_paths: Option<usize>,
        seed: Option<u64>,
        use_parallel: Option<bool>,
    ) -> PyResult<Self> {
        let defaults = &py_mc_defaults()?.european_pricer;
        Ok(Self {
            num_paths: num_paths.unwrap_or(defaults.num_paths),
            seed: seed.unwrap_or(defaults.seed),
            use_parallel: use_parallel.unwrap_or(defaults.use_parallel),
        })
    }

    #[getter]
    fn num_paths(&self) -> usize {
        self.num_paths
    }
    #[getter]
    fn seed(&self) -> u64 {
        self.seed
    }
    #[getter]
    fn use_parallel(&self) -> bool {
        self.use_parallel
    }

    /// Price a European call option under GBM.
    ///
    /// Releases the GIL during the Monte Carlo run so other Python threads
    /// can make progress.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, num_steps=None, currency=None))]
    fn price_call(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        num_steps: Option<usize>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyMonteCarloResult> {
        let ccy = resolve_currency(currency)?;
        let num_steps = num_steps.unwrap_or(py_mc_defaults()?.european_pricer.num_steps);
        let pricer = self.build_pricer();
        py.detach(|| {
            pricer.price_gbm_call(spot, strike, rate, div_yield, vol, expiry, num_steps, ccy)
        })
        .map(PyMonteCarloResult::from_inner)
        .map_err(core_to_py)
    }

    /// Price a European put option under GBM.
    ///
    /// Releases the GIL during the Monte Carlo run so other Python threads
    /// can make progress.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, num_steps=None, currency=None))]
    fn price_put(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        num_steps: Option<usize>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyMonteCarloResult> {
        let ccy = resolve_currency(currency)?;
        let num_steps = num_steps.unwrap_or(py_mc_defaults()?.european_pricer.num_steps);
        let pricer = self.build_pricer();
        py.detach(|| {
            pricer.price_gbm_put(spot, strike, rate, div_yield, vol, expiry, num_steps, ccy)
        })
        .map(PyMonteCarloResult::from_inner)
        .map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "EuropeanPricer(num_paths={}, seed={}, use_parallel={})",
            self.num_paths, self.seed, self.use_parallel,
        )
    }
}

impl PyEuropeanPricer {
    fn build_pricer(&self) -> EuropeanPricer {
        EuropeanPricer::new(self.num_paths)
            .with_seed(self.seed)
            .with_parallel(self.use_parallel)
    }
}

// ---------------------------------------------------------------------------
// Path-dependent pricer
// ---------------------------------------------------------------------------

/// Path-dependent Monte Carlo pricer for exotic payoffs (Asian, barrier, etc.).
#[pyclass(name = "PathDependentPricer", module = "finstack.monte_carlo", frozen)]
pub struct PyPathDependentPricer {
    num_paths: usize,
    seed: u64,
    use_parallel: bool,
}

#[pymethods]
impl PyPathDependentPricer {
    #[new]
    #[pyo3(signature = (num_paths=None, seed=None, use_parallel=None))]
    fn new(
        num_paths: Option<usize>,
        seed: Option<u64>,
        use_parallel: Option<bool>,
    ) -> PyResult<Self> {
        let defaults = &py_mc_defaults()?.path_dependent_pricer;
        Ok(Self {
            num_paths: num_paths.unwrap_or(defaults.num_paths),
            seed: seed.unwrap_or(defaults.seed),
            use_parallel: use_parallel.unwrap_or(defaults.use_parallel),
        })
    }

    /// Price an Asian call under GBM dynamics.
    ///
    /// Releases the GIL during the Monte Carlo run.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, num_steps=None, currency=None))]
    fn price_asian_call(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        num_steps: Option<usize>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyMonteCarloResult> {
        use finstack_monte_carlo::payoff::asian::{
            default_fixing_steps, AsianCall, AveragingMethod,
        };
        let ccy = resolve_currency(currency)?;
        let num_steps = num_steps.unwrap_or(py_mc_defaults()?.path_dependent_pricer.num_steps);
        let fixing_steps = default_fixing_steps(num_steps);
        let payoff = AsianCall::new(strike, 1.0, AveragingMethod::Arithmetic, fixing_steps);
        let df = (-rate * expiry).exp();
        self.run_gbm(
            py, &payoff, spot, rate, div_yield, vol, expiry, num_steps, ccy, df,
        )
    }

    /// Price an Asian put under GBM dynamics.
    ///
    /// Releases the GIL during the Monte Carlo run.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, num_steps=None, currency=None))]
    fn price_asian_put(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        num_steps: Option<usize>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyMonteCarloResult> {
        use finstack_monte_carlo::payoff::asian::{
            default_fixing_steps, AsianPut, AveragingMethod,
        };
        let ccy = resolve_currency(currency)?;
        let num_steps = num_steps.unwrap_or(py_mc_defaults()?.path_dependent_pricer.num_steps);
        let fixing_steps = default_fixing_steps(num_steps);
        let payoff = AsianPut::new(strike, 1.0, AveragingMethod::Arithmetic, fixing_steps);
        let df = (-rate * expiry).exp();
        self.run_gbm(
            py, &payoff, spot, rate, div_yield, vol, expiry, num_steps, ccy, df,
        )
    }

    #[getter]
    fn num_paths(&self) -> usize {
        self.num_paths
    }
    #[getter]
    fn seed(&self) -> u64 {
        self.seed
    }

    fn __repr__(&self) -> String {
        format!(
            "PathDependentPricer(paths={}, seed={}, parallel={})",
            self.num_paths, self.seed, self.use_parallel,
        )
    }
}

impl PyPathDependentPricer {
    #[allow(clippy::too_many_arguments)]
    fn run_gbm(
        &self,
        py: Python<'_>,
        payoff: &impl finstack_monte_carlo::traits::Payoff,
        spot: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        num_steps: usize,
        currency: finstack_core::currency::Currency,
        discount_factor: f64,
    ) -> PyResult<PyMonteCarloResult> {
        use finstack_monte_carlo::pricer::path_dependent::{
            PathDependentPricer, PathDependentPricerConfig,
        };

        let config = PathDependentPricerConfig::new(self.num_paths)
            .with_seed(self.seed)
            .with_parallel(self.use_parallel);
        let pricer = PathDependentPricer::new(config);
        let process = GbmProcess::with_params(rate, div_yield, vol).map_err(core_to_py)?;
        py.detach(|| {
            pricer.price(
                &process,
                spot,
                expiry,
                num_steps,
                payoff,
                currency,
                discount_factor,
            )
        })
        .map(PyMonteCarloResult::from_inner)
        .map_err(core_to_py)
    }
}

// ---------------------------------------------------------------------------
// LSMC pricer
// ---------------------------------------------------------------------------

/// Longstaff-Schwartz Monte Carlo pricer for American options.
#[pyclass(name = "LsmcPricer", module = "finstack.monte_carlo", frozen)]
pub struct PyLsmcPricer {
    num_paths: usize,
    seed: u64,
    use_parallel: bool,
    basis: BasisKind,
    basis_degree: usize,
}

struct PyLsmcRun {
    pricer: LsmcPricer,
    process: GbmProcess,
    basis: LsmcBasis,
    currency: Currency,
    num_steps: usize,
}

impl PyLsmcPricer {
    fn build_basis(&self, strike: f64) -> PyResult<LsmcBasis> {
        let to_py = |e: String| crate::errors::value_error(e);
        build_lsmc_basis(self.basis, self.basis_degree, strike).map_err(to_py)
    }

    fn build_config(&self, num_steps: usize) -> PyResult<LsmcConfig> {
        let exercise_dates: Vec<usize> = (1..=num_steps).collect();
        LsmcConfig::new(self.num_paths, exercise_dates, num_steps)
            .map_err(core_to_py)
            .map(|config| config.with_seed(self.seed).with_parallel(self.use_parallel))
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_gbm_run(
        &self,
        rate: f64,
        div_yield: f64,
        vol: f64,
        strike: f64,
        num_steps: Option<usize>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyLsmcRun> {
        let currency = resolve_currency(currency)?;
        let num_steps = num_steps.unwrap_or(py_mc_defaults()?.lsmc.num_steps);
        let config = self.build_config(num_steps)?;
        let process = GbmProcess::with_params(rate, div_yield, vol).map_err(core_to_py)?;
        let basis = self.build_basis(strike)?;

        Ok(PyLsmcRun {
            pricer: LsmcPricer::new(config),
            process,
            basis,
            currency,
            num_steps,
        })
    }
}

#[pymethods]
impl PyLsmcPricer {
    #[new]
    #[pyo3(signature = (
        num_paths=None,
        seed=None,
        use_parallel=None,
        basis=None,
        basis_degree=None,
    ))]
    fn new(
        num_paths: Option<usize>,
        seed: Option<u64>,
        use_parallel: Option<bool>,
        basis: Option<&str>,
        basis_degree: Option<usize>,
    ) -> PyResult<Self> {
        let defaults = &py_mc_defaults()?.lsmc;
        let basis = BasisKind::parse(basis.unwrap_or(defaults.basis.as_str()))
            .map_err(crate::errors::value_error)?;
        let basis_degree = basis_degree.unwrap_or(defaults.basis_degree);
        if basis_degree == 0 {
            return Err(crate::errors::value_error(
                "basis_degree must be a positive integer",
            ));
        }
        Ok(Self {
            num_paths: num_paths.unwrap_or(defaults.num_paths),
            seed: seed.unwrap_or(defaults.seed),
            use_parallel: use_parallel.unwrap_or(defaults.use_parallel),
            basis,
            basis_degree,
        })
    }

    #[getter]
    fn num_paths(&self) -> usize {
        self.num_paths
    }
    #[getter]
    fn seed(&self) -> u64 {
        self.seed
    }
    #[getter]
    fn use_parallel(&self) -> bool {
        self.use_parallel
    }
    #[getter]
    fn basis(&self) -> &'static str {
        self.basis.as_str()
    }
    #[getter]
    fn basis_degree(&self) -> usize {
        self.basis_degree
    }

    /// Price an American put under GBM dynamics.
    ///
    /// Releases the GIL during the Monte Carlo run.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, num_steps=None, currency=None))]
    fn price_american_put(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        num_steps: Option<usize>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyMonteCarloResult> {
        use finstack_monte_carlo::pricer::lsmc::AmericanPut;

        let exercise = AmericanPut::new(strike).map_err(core_to_py)?;
        let run = self.prepare_gbm_run(rate, div_yield, vol, strike, num_steps, currency)?;
        py.detach(|| {
            run.pricer.price(
                &run.process,
                spot,
                expiry,
                run.num_steps,
                &exercise,
                &run.basis,
                run.currency,
                rate,
            )
        })
        .map(PyMonteCarloResult::from_inner)
        .map_err(core_to_py)
    }

    /// Price an American call under GBM dynamics.
    ///
    /// Releases the GIL during the Monte Carlo run.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, num_steps=None, currency=None))]
    fn price_american_call(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        num_steps: Option<usize>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyMonteCarloResult> {
        use finstack_monte_carlo::pricer::lsmc::AmericanCall;

        let exercise = AmericanCall::new(strike).map_err(core_to_py)?;
        let run = self.prepare_gbm_run(rate, div_yield, vol, strike, num_steps, currency)?;
        py.detach(|| {
            run.pricer.price(
                &run.process,
                spot,
                expiry,
                run.num_steps,
                &exercise,
                &run.basis,
                run.currency,
                rate,
            )
        })
        .map(PyMonteCarloResult::from_inner)
        .map_err(core_to_py)
    }

    /// Two-pass unbiased American put price (training fit + out-of-sample pricing).
    ///
    /// Mitigates the in-sample upward bias of single-pass LSMC: the regression
    /// is fit on a training path set seeded by the pricer's `seed`, and the
    /// frozen exercise policy is replayed against an *independent* path set
    /// seeded by `pricing_seed`. `pricing_seed` must differ from `seed`;
    /// matching seeds reintroduce the bias and are rejected.
    ///
    /// Releases the GIL during both Monte Carlo passes.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        spot, strike, rate, div_yield, vol, expiry, pricing_seed,
        num_steps=None, currency=None,
    ))]
    fn price_american_put_unbiased(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        pricing_seed: u64,
        num_steps: Option<usize>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyMonteCarloResult> {
        use finstack_monte_carlo::pricer::lsmc::AmericanPut;

        let exercise = AmericanPut::new(strike).map_err(core_to_py)?;
        let run = self.prepare_gbm_run(rate, div_yield, vol, strike, num_steps, currency)?;
        py.detach(|| {
            run.pricer.price_unbiased(
                &run.process,
                spot,
                expiry,
                run.num_steps,
                &exercise,
                &run.basis,
                run.currency,
                rate,
                pricing_seed,
            )
        })
        .map(PyMonteCarloResult::from_inner)
        .map_err(core_to_py)
    }

    /// Two-pass unbiased American call price.
    ///
    /// See [`Self::price_american_put_unbiased`] for the bias-mitigation
    /// rationale and the meaning of `pricing_seed`.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        spot, strike, rate, div_yield, vol, expiry, pricing_seed,
        num_steps=None, currency=None,
    ))]
    fn price_american_call_unbiased(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        pricing_seed: u64,
        num_steps: Option<usize>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyMonteCarloResult> {
        use finstack_monte_carlo::pricer::lsmc::AmericanCall;

        let exercise = AmericanCall::new(strike).map_err(core_to_py)?;
        let run = self.prepare_gbm_run(rate, div_yield, vol, strike, num_steps, currency)?;
        py.detach(|| {
            run.pricer.price_unbiased(
                &run.process,
                spot,
                expiry,
                run.num_steps,
                &exercise,
                &run.basis,
                run.currency,
                rate,
                pricing_seed,
            )
        })
        .map(PyMonteCarloResult::from_inner)
        .map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "LsmcPricer(paths={}, seed={}, use_parallel={}, basis={}, basis_degree={})",
            self.num_paths,
            self.seed,
            self.use_parallel,
            self.basis.as_str(),
            self.basis_degree,
        )
    }
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyEuropeanPricer>()?;
    m.add_class::<PyPathDependentPricer>()?;
    m.add_class::<PyLsmcPricer>()?;
    Ok(())
}
