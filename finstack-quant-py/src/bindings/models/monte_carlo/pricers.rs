//! Pricer bindings — European, Path-Dependent, LSMC.

use super::engine::{py_mc_defaults, resolve_currency};
use super::results::PyMoneyEstimate;
use crate::errors::core_to_py;
use finstack_quant_models::monte_carlo::pricer::basis::BasisKind;
use finstack_quant_models::monte_carlo::pricer::european::EuropeanPricer;
use finstack_quant_models::monte_carlo::pricer::lsmc::LsmcPricer;
use finstack_quant_models::monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use pyo3::prelude::*;

/// Render a bool the way Python's `repr` does.
fn py_bool(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

/// Convenience pricer for European options under GBM dynamics.
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
    /// Create a European pricer with at least two independent path estimators.
    ///
    /// Parameters
    /// ----------
    /// num_paths : int, optional
    ///     Independent path estimators in ``[2, 10_000_000]``. ``None`` uses
    ///     the registry default; the upper limit is checked when pricing.
    /// seed : int, optional
    ///     Deterministic Philox seed; ``None`` uses the registry default.
    /// use_parallel : bool, optional
    ///     Whether to use the rayon pool; ``None`` uses the registry default.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the path count is less than two or embedded defaults cannot be loaded.
    #[new]
    #[pyo3(signature = (num_paths=None, seed=None, use_parallel=None))]
    fn new(
        num_paths: Option<usize>,
        seed: Option<u64>,
        use_parallel: Option<bool>,
    ) -> PyResult<Self> {
        let defaults = &py_mc_defaults()?.european_pricer;
        let inner = EuropeanPricer::new(num_paths.unwrap_or(defaults.num_paths))
            .map_err(core_to_py)?
            .with_seed(seed.unwrap_or(defaults.seed))
            .with_parallel(use_parallel.unwrap_or(defaults.use_parallel));
        Ok(Self { inner })
    }

    /// Independent Monte Carlo path count used by this pricer.
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths()
    }
    /// Seed value used for path generation.
    #[getter]
    fn seed(&self) -> u64 {
        self.inner.seed()
    }
    /// Whether path generation runs on the rayon pool.
    #[getter]
    fn use_parallel(&self) -> bool {
        self.inner.use_parallel()
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
    ) -> PyResult<PyMoneyEstimate> {
        let ccy = resolve_currency(currency)?;
        let num_steps = num_steps.unwrap_or(py_mc_defaults()?.european_pricer.num_steps);
        let pricer = &self.inner;
        py.detach(|| {
            pricer.price_gbm_call(spot, strike, rate, div_yield, vol, expiry, num_steps, ccy)
        })
        .map(PyMoneyEstimate::from_inner)
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
    ) -> PyResult<PyMoneyEstimate> {
        let ccy = resolve_currency(currency)?;
        let num_steps = num_steps.unwrap_or(py_mc_defaults()?.european_pricer.num_steps);
        let pricer = &self.inner;
        py.detach(|| {
            pricer.price_gbm_put(spot, strike, rate, div_yield, vol, expiry, num_steps, ccy)
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

/// Path-dependent Monte Carlo pricer for exotic payoffs (Asian, barrier, etc.).
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
    /// Create a path-dependent pricer.
    ///
    /// Parameters
    /// ----------
    /// num_paths : int, optional
    ///     Path budget of at least two. Without Sobol, counts independent
    ///     estimators, with each antithetic pair counted once; with Sobol,
    ///     counts points across independent scrambles. Defaults to the registry value.
    /// seed : int, optional
    ///     Root Philox seed. Defaults to the registry value.
    /// use_parallel : bool, optional
    ///     Run path generation on the rayon pool. Defaults to the registry
    ///     value. Incompatible with ``use_sobol=True``.
    /// antithetic : bool, optional
    ///     Pair each path with its sign-flipped counterpart. Defaults to the
    ///     registry value.
    /// use_sobol : bool, optional
    ///     Drive paths from a Sobol quasi-random sequence instead of Philox.
    ///     Enabling Sobol also switches on the Brownian bridge unless
    ///     ``use_brownian_bridge`` is passed explicitly. Defaults to the
    ///     registry value.
    /// use_brownian_bridge : bool, optional
    ///     Brownian-bridge path construction (only meaningful with Sobol).
    ///     Defaults to the registry value, or ``True`` when ``use_sobol`` is
    ///     enabled here.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the path budget is less than two, embedded defaults cannot be loaded, or the
    ///     configuration is inconsistent (``use_sobol`` with ``use_parallel``).
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
        let defaults = &py_mc_defaults()?.path_dependent_pricer;
        let mut config = PathDependentPricerConfig::new(num_paths.unwrap_or(defaults.num_paths))
            .with_seed(seed.unwrap_or(defaults.seed))
            .with_parallel(use_parallel.unwrap_or(defaults.use_parallel));
        if let Some(antithetic) = antithetic {
            config = config.with_antithetic(antithetic);
        }
        if let Some(use_sobol) = use_sobol {
            config = config.with_sobol(use_sobol);
        }
        if let Some(bridge) = use_brownian_bridge {
            config = config.with_brownian_bridge(bridge);
        }
        config.validate().map_err(core_to_py)?;
        Ok(Self {
            inner: PathDependentPricer::new(config),
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
    ) -> PyResult<PyMoneyEstimate> {
        let ccy = resolve_currency(currency)?;
        let num_steps = num_steps.unwrap_or(py_mc_defaults()?.path_dependent_pricer.num_steps);
        let pricer = &self.inner;
        py.detach(|| {
            pricer.price_gbm_asian_call(spot, strike, rate, div_yield, vol, expiry, num_steps, ccy)
        })
        .map(PyMoneyEstimate::from_inner)
        .map_err(core_to_py)
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
    ) -> PyResult<PyMoneyEstimate> {
        let ccy = resolve_currency(currency)?;
        let num_steps = num_steps.unwrap_or(py_mc_defaults()?.path_dependent_pricer.num_steps);
        let pricer = &self.inner;
        py.detach(|| {
            pricer.price_gbm_asian_put(spot, strike, rate, div_yield, vol, expiry, num_steps, ccy)
        })
        .map(PyMoneyEstimate::from_inner)
        .map_err(core_to_py)
    }

    /// Independent Monte Carlo path count used by this pricer.
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.config().num_paths
    }
    /// Seed value used for path generation.
    #[getter]
    fn seed(&self) -> u64 {
        self.inner.config().seed
    }
    /// Whether path generation runs on the rayon pool.
    #[getter]
    fn use_parallel(&self) -> bool {
        self.inner.config().use_parallel
    }
    /// Whether each path is paired with its sign-flipped counterpart.
    #[getter]
    fn antithetic(&self) -> bool {
        self.inner.config().antithetic
    }
    /// Whether paths are driven by a Sobol quasi-random sequence.
    #[getter]
    fn use_sobol(&self) -> bool {
        self.inner.config().use_sobol
    }
    /// Whether Brownian-bridge construction is enabled (Sobol only).
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

/// Longstaff-Schwartz Monte Carlo pricer for American options.
///
/// Independent paths must be in ``[1, 10_000_000]`` and time steps in
/// ``[1, 100_000]``. Each pricing pass retains at most ``64_000_000`` spot
/// values: ``num_paths * (2 if antithetic else 1) * (num_steps + 1)``,
/// including time zero and both rows of each antithetic pair. Construction
/// checks storage without partners; pricing checks the selected pairing
/// before allocating paths.
#[pyclass(
    name = "LsmcPricer",
    module = "finstack_quant.models.monte_carlo",
    frozen
)]
pub struct PyLsmcPricer {
    inner: LsmcPricer,
    num_steps: usize,
    basis: BasisKind,
    basis_degree: usize,
}

#[pymethods]
impl PyLsmcPricer {
    /// Configure the path generation and regression basis for early exercise.
    ///
    /// Parameters
    /// ----------
    /// num_paths : int, optional
    ///     Independent path count in ``[2, 10_000_000]``. ``None`` uses the
    ///     registry default; subject to the retained-storage limit above.
    /// seed : int, optional
    ///     Deterministic Philox seed; ``None`` uses the registry default.
    /// use_parallel : bool, optional
    ///     Whether path generation uses the rayon pool; ``None`` uses the
    ///     registry default.
    /// num_steps : int, optional
    ///     Time-grid steps and exercise dates in ``[1, 100_000]``. ``None``
    ///     uses the registry default; subject to the retained-storage limit.
    /// basis : str, optional
    ///     ``"laguerre"``, ``"polynomial"``, or ``"normalized_polynomial"``;
    ///     ``None`` selects the registry default.
    /// basis_degree : int, optional
    ///     Positive regression degree, validated when pricing; Laguerre
    ///     supports degrees one through four. ``None`` uses the registry default.
    /// antithetic : bool, optional
    ///     Pair each path with its sign-flipped shocks. ``None`` uses the
    ///     registry default; both rows count toward retained storage at pricing.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the basis family is unknown, the path or step count is outside
    ///     the documented range, or paths without antithetic partners exceed
    ///     ``64_000_000`` retained spot values including time zero.
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
        let defaults = &py_mc_defaults()?.lsmc;
        let basis = BasisKind::parse(basis.unwrap_or(defaults.basis.as_str()))
            .map_err(crate::errors::value_error)?;
        let num_steps = num_steps.unwrap_or(defaults.num_steps);
        let inner = LsmcPricer::gbm_american(
            num_paths.unwrap_or(defaults.num_paths),
            num_steps,
            seed.unwrap_or(defaults.seed),
            use_parallel.unwrap_or(defaults.use_parallel),
            antithetic.unwrap_or(defaults.antithetic),
        )
        .map_err(core_to_py)?;
        Ok(Self {
            inner,
            num_steps,
            basis,
            basis_degree: basis_degree.unwrap_or(defaults.basis_degree),
        })
    }

    /// Independent Monte Carlo path count used by this pricer.
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.config().num_paths
    }
    /// Seed value used for path generation.
    #[getter]
    fn seed(&self) -> u64 {
        self.inner.config().seed
    }
    /// Whether path generation runs on the rayon pool.
    #[getter]
    fn use_parallel(&self) -> bool {
        self.inner.config().use_parallel
    }
    /// Whether each path is paired with its sign-flipped counterpart.
    #[getter]
    fn antithetic(&self) -> bool {
        self.inner.config().antithetic
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
    ///
    /// Parameters
    /// ----------
    /// spot, strike, rate, div_yield, vol, expiry : float
    ///     GBM market inputs; ``rate`` / ``div_yield`` are continuously
    ///     compounded decimals, ``vol`` is a positive decimal, ``expiry`` is in
    ///     years.
    /// currency : Currency or str, optional
    ///     Currency stamped on the estimate; defaults to the registry value.
    /// num_steps : int, optional
    ///     Per-call exercise-grid override in ``[1, 100_000]``; defaults to
    ///     the instance ``num_steps``. Subject to the retained-storage limit.
    /// basis : str, optional
    ///     Per-call override of the regression basis family; defaults to the
    ///     instance ``basis``.
    /// basis_degree : int, optional
    ///     Per-call override of the basis degree; defaults to the instance
    ///     ``basis_degree``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the requested grid or path count exceeds the documented range,
    ///     or retained spots exceed ``64_000_000`` values including time zero
    ///     and both rows of each antithetic pair; also raised for invalid market
    ///     inputs, currency, basis family, or basis degree.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, currency=None, num_steps=None, basis=None, basis_degree=None))]
    fn price_american_put(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        currency: Option<&Bound<'_, PyAny>>,
        num_steps: Option<usize>,
        basis: Option<&str>,
        basis_degree: Option<usize>,
    ) -> PyResult<PyMoneyEstimate> {
        let currency = resolve_currency(currency)?;
        let (pricer, num_steps, basis, basis_degree) =
            self.call_config(num_steps, basis, basis_degree)?;
        py.detach(|| {
            pricer.price_gbm_american_put(
                spot,
                strike,
                rate,
                div_yield,
                vol,
                expiry,
                num_steps,
                currency,
                basis,
                basis_degree,
            )
        })
        .map(PyMoneyEstimate::from_inner)
        .map_err(core_to_py)
    }

    /// Price an American call under GBM dynamics.
    ///
    /// Releases the GIL during the Monte Carlo run. Accepts the same per-call
    /// ``num_steps`` / ``basis`` / ``basis_degree`` overrides as
    /// ``price_american_put``.
    ///
    /// Raises ``ValueError`` for the same workload limits: at most ``100_000``
    /// steps, ``10_000_000`` independent paths, and ``64_000_000`` retained spot
    /// values including time zero and both rows of each antithetic pair.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, currency=None, num_steps=None, basis=None, basis_degree=None))]
    fn price_american_call(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        currency: Option<&Bound<'_, PyAny>>,
        num_steps: Option<usize>,
        basis: Option<&str>,
        basis_degree: Option<usize>,
    ) -> PyResult<PyMoneyEstimate> {
        let currency = resolve_currency(currency)?;
        let (pricer, num_steps, basis, basis_degree) =
            self.call_config(num_steps, basis, basis_degree)?;
        py.detach(|| {
            pricer.price_gbm_american_call(
                spot,
                strike,
                rate,
                div_yield,
                vol,
                expiry,
                num_steps,
                currency,
                basis,
                basis_degree,
            )
        })
        .map(PyMoneyEstimate::from_inner)
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
    /// Releases the GIL during both Monte Carlo passes. Accepts the same
    /// per-call ``num_steps`` / ``basis`` / ``basis_degree`` overrides as
    /// ``price_american_put``.
    ///
    /// Each pass applies the same workload limits and raises ``ValueError``
    /// above ``100_000`` steps, ``10_000_000`` independent paths, or
    /// ``64_000_000`` retained spot values including time zero and both rows
    /// of each antithetic pair.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        spot, strike, rate, div_yield, vol, expiry, pricing_seed, currency=None,
        num_steps=None, basis=None, basis_degree=None,
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
        currency: Option<&Bound<'_, PyAny>>,
        num_steps: Option<usize>,
        basis: Option<&str>,
        basis_degree: Option<usize>,
    ) -> PyResult<PyMoneyEstimate> {
        let currency = resolve_currency(currency)?;
        let (pricer, num_steps, basis, basis_degree) =
            self.call_config(num_steps, basis, basis_degree)?;
        py.detach(|| {
            pricer.price_gbm_american_put_unbiased(
                spot,
                strike,
                rate,
                div_yield,
                vol,
                expiry,
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

    /// Two-pass unbiased American call price.
    ///
    /// See ``price_american_put_unbiased`` for the bias-mitigation rationale
    /// and the meaning of ``pricing_seed``; the same per-call overrides apply.
    /// Each pass raises ``ValueError`` for the same workload limits, including
    /// ``64_000_000`` retained spot values with time zero and antithetic rows.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        spot, strike, rate, div_yield, vol, expiry, pricing_seed, currency=None,
        num_steps=None, basis=None, basis_degree=None,
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
        currency: Option<&Bound<'_, PyAny>>,
        num_steps: Option<usize>,
        basis: Option<&str>,
        basis_degree: Option<usize>,
    ) -> PyResult<PyMoneyEstimate> {
        let currency = resolve_currency(currency)?;
        let (pricer, num_steps, basis, basis_degree) =
            self.call_config(num_steps, basis, basis_degree)?;
        py.detach(|| {
            pricer.price_gbm_american_call_unbiased(
                spot,
                strike,
                rate,
                div_yield,
                vol,
                expiry,
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
            self.inner.config().num_paths,
            self.inner.config().seed,
            py_bool(self.inner.config().use_parallel),
            py_bool(self.inner.config().antithetic),
            self.num_steps,
            self.basis.as_str(),
            self.basis_degree,
        )
    }
}

impl PyLsmcPricer {
    /// Resolve per-call overrides against the instance configuration.
    ///
    /// A `num_steps` override rebuilds the LSMC configuration (the exercise
    /// grid is part of it); every other setting is carried over unchanged.
    fn call_config(
        &self,
        num_steps: Option<usize>,
        basis: Option<&str>,
        basis_degree: Option<usize>,
    ) -> PyResult<(LsmcPricer, usize, BasisKind, usize)> {
        let config = self.inner.config();
        let num_steps = num_steps.unwrap_or(self.num_steps);
        let pricer = LsmcPricer::gbm_american(
            config.num_paths,
            num_steps,
            config.seed,
            config.use_parallel,
            config.antithetic,
        )
        .map_err(core_to_py)?;
        let basis = match basis {
            Some(name) => BasisKind::parse(name).map_err(crate::errors::value_error)?,
            None => self.basis,
        };
        Ok((
            pricer,
            num_steps,
            basis,
            basis_degree.unwrap_or(self.basis_degree),
        ))
    }
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyEuropeanPricer>()?;
    m.add_class::<PyPathDependentPricer>()?;
    m.add_class::<PyLsmcPricer>()?;
    Ok(())
}
