//! Convenience pricer for European-style payoffs under GBM dynamics.
//!
//! This module wraps [`crate::monte_carlo::engine::McEngine`] for the common case of pricing
//! a European payoff under [`crate::monte_carlo::process::gbm::GbmProcess`] with
//! [`crate::monte_carlo::discretization::exact::ExactGbm`]. Use it when you want a compact
//! API and do not need custom process / discretization combinations.

use super::super::engine::{McEngine, McEngineConfig};
use super::super::results::MoneyEstimate;
use super::super::traits::Payoff;
use crate::monte_carlo::discretization::exact::ExactGbm;
use crate::monte_carlo::payoff::vanilla::{EuropeanCall, EuropeanPut};
use crate::monte_carlo::process::gbm::GbmProcess;
use crate::monte_carlo::rng::philox::PhiloxRng;
use crate::monte_carlo::TimeGrid;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::Result;

/// Compact GBM-only pricer for European-style contracts.
///
/// The pricer always uses exact GBM transitions and delegates the simulation
/// loop to [`crate::monte_carlo::engine::McEngine`]. Its simulation vocabulary is
/// deliberately a subset of [`McEngineConfig`] (`num_paths`, `seed`,
/// `use_parallel`) carried inline rather than through a separate config
/// struct, so there is one obvious way to describe a European run.
#[derive(Debug, Clone)]
pub struct EuropeanPricer {
    num_paths: usize,
    seed: u64,
    use_parallel: bool,
}

impl Default for EuropeanPricer {
    fn default() -> Self {
        let defaults = &crate::monte_carlo::registry::embedded_defaults_or_panic()
            .rust
            .european_pricer;
        Self {
            num_paths: defaults.num_paths,
            seed: defaults.seed,
            use_parallel: defaults.use_parallel,
        }
    }
}

impl EuropeanPricer {
    /// Create a pricer with the given path count and defaults for the rest.
    ///
    /// Defaults are registry-backed seed and parallel settings (which quietly
    /// degrades to serial when the `parallel` feature is absent).
    ///
    /// # Arguments
    ///
    /// * `num_paths` - Independent Monte Carlo path estimators; must be positive.
    ///
    /// # Errors
    ///
    /// Returns a validation error when `num_paths` is zero.
    pub fn new(num_paths: usize) -> Result<Self> {
        if num_paths == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "EuropeanPricer num_paths must be positive".to_string(),
            ));
        }
        Ok(Self {
            num_paths,
            ..Self::default()
        })
    }

    /// Override the RNG seed.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Enable or disable parallel execution.
    ///
    /// If the crate is built without the `parallel` feature the underlying
    /// engine falls back to serial execution regardless of this flag.
    #[must_use]
    pub fn with_parallel(mut self, parallel: bool) -> Self {
        self.use_parallel = parallel;
        self
    }

    /// Requested number of Monte Carlo paths.
    pub fn num_paths(&self) -> usize {
        self.num_paths
    }

    /// Root RNG seed for deterministic replay.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Whether parallel execution was requested.
    pub fn use_parallel(&self) -> bool {
        self.use_parallel
    }

    /// Price a European-style payoff under GBM.
    ///
    /// # Arguments
    ///
    /// * `process` - GBM process supplying the risk-neutral drift and volatility.
    /// * `initial_spot` - Spot level at time `0`.
    /// * `time_to_maturity` - Maturity in years.
    /// * `num_steps` - Number of time-grid steps between `0` and maturity.
    /// * `payoff` - European-style payoff evaluated at `maturity_step = num_steps`.
    /// * `currency` - Currency for the returned estimate.
    /// * `discount_factor` - Present-value multiplier for the payoff horizon.
    ///   Build it with
    ///   [`finstack_quant_core::cashflow::flat_discount_factor`] when working
    ///   from a flat continuously compounded rate and a year fraction.
    ///
    /// # Returns
    ///
    /// A discounted Monte Carlo estimate in `currency`.
    ///
    /// # Errors
    ///
    /// Returns an error when the uniform time grid is invalid or when the
    /// underlying engine rejects the runtime configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_models::monte_carlo::payoff::vanilla::EuropeanCall;
    /// use finstack_quant_models::monte_carlo::pricer::european::EuropeanPricer;
    /// use finstack_quant_models::monte_carlo::process::gbm::GbmProcess;
    ///
    /// let pricer = EuropeanPricer::new(25_000)
    ///     .expect("positive path count")
    ///     .with_seed(19)
    ///     .with_parallel(false);
    /// let process = GbmProcess::with_params(0.03, 0.01, 0.20).unwrap();
    /// let payoff = EuropeanCall::new(100.0, 1.0, 252);
    /// let discount_factor = (-0.03_f64).exp();
    ///
    /// let result = pricer
    ///     .price(&process, 100.0, 1.0, 252, &payoff, Currency::USD, discount_factor)
    ///     .expect("pricing should succeed");
    ///
    /// assert!(result.mean.amount().is_finite());
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn price<P>(
        &self,
        process: &GbmProcess,
        initial_spot: f64,
        time_to_maturity: f64,
        num_steps: usize,
        payoff: &P,
        currency: Currency,
        discount_factor: f64,
    ) -> Result<MoneyEstimate>
    where
        P: Payoff,
    {
        let time_grid = TimeGrid::uniform(time_to_maturity, num_steps)?;
        // Serial ≡ parallel by the determinism invariant, so the flag only
        // sets throughput. On wasm32 no thread pool exists, so force serial
        // there regardless of the configured (registry-backed) default.
        #[cfg(target_arch = "wasm32")]
        let use_parallel = false;
        #[cfg(not(target_arch = "wasm32"))]
        let use_parallel = self.use_parallel;
        let engine_config = McEngineConfig::new(self.num_paths, time_grid).parallel(use_parallel);
        let engine = McEngine::new(engine_config);

        let rng = PhiloxRng::new(self.seed);
        let disc = ExactGbm::new();
        let initial_state = vec![initial_spot];

        engine.price(
            &rng,
            process,
            &disc,
            &initial_state,
            payoff,
            currency,
            discount_factor,
        )
    }

    /// Price a European call under risk-neutral GBM with flat continuous
    /// discounting `exp(-rT)`.
    ///
    /// This is a scalar-arg convenience for the common binding case where the
    /// caller supplies raw floats rather than pre-built `GbmProcess` / `EuropeanCall`
    /// instances.
    ///
    /// # Arguments
    ///
    /// * `spot` - Spot level at time `0`.
    /// * `strike` - Exercise price in the same units as `spot`.
    /// * `rate` - Continuously compounded risk-free rate (decimal, annualized).
    /// * `div_yield` - Continuous dividend yield (decimal, annualized).
    /// * `vol` - Annualized GBM volatility (decimal).
    /// * `expiry` - Time to expiry in years; also the uniform-grid horizon.
    /// * `num_steps` - Number of time-grid steps between `0` and `expiry`.
    /// * `currency` - Currency stamped on the returned estimate.
    ///
    /// # Errors
    ///
    /// Returns an error when GBM parameters fail validation, the uniform grid
    /// is invalid, or the underlying engine rejects the run.
    #[allow(clippy::too_many_arguments)]
    pub fn price_gbm_call(
        &self,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        num_steps: usize,
        currency: Currency,
    ) -> Result<MoneyEstimate> {
        crate::monte_carlo::require_positive_vol(vol)?;
        let process = GbmProcess::with_params(rate, div_yield, vol)?;
        let payoff = EuropeanCall::new(strike, 1.0, num_steps);
        let discount_factor = (-rate * expiry).exp();
        self.price(
            &process,
            spot,
            expiry,
            num_steps,
            &payoff,
            currency,
            discount_factor,
        )
    }

    /// Price a European put under risk-neutral GBM with flat continuous
    /// discounting `exp(-rT)`.
    ///
    /// # Arguments
    ///
    /// * `spot` - Spot level at time `0`.
    /// * `strike` - Exercise price in the same units as `spot`.
    /// * `rate` - Continuously compounded risk-free rate (decimal, annualized).
    /// * `div_yield` - Continuous dividend yield (decimal, annualized).
    /// * `vol` - Annualized GBM volatility (decimal).
    /// * `expiry` - Time to expiry in years; also the uniform-grid horizon.
    /// * `num_steps` - Number of time-grid steps between `0` and `expiry`.
    /// * `currency` - Currency stamped on the returned estimate.
    ///
    /// # Errors
    ///
    /// Returns an error when GBM parameters fail validation, the uniform grid
    /// is invalid, or the underlying engine rejects the run.
    #[allow(clippy::too_many_arguments)]
    pub fn price_gbm_put(
        &self,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        expiry: f64,
        num_steps: usize,
        currency: Currency,
    ) -> Result<MoneyEstimate> {
        crate::monte_carlo::require_positive_vol(vol)?;
        let process = GbmProcess::with_params(rate, div_yield, vol)?;
        let payoff = EuropeanPut::new(strike, 1.0, num_steps);
        let discount_factor = (-rate * expiry).exp();
        self.price(
            &process,
            spot,
            expiry,
            num_steps,
            &payoff,
            currency,
            discount_factor,
        )
    }
}

/// Price a European call under GBM on a caller-built engine.
///
/// Unlike [`EuropeanPricer::price_gbm_call`], the payoff horizon is the
/// engine's own grid `t_max` — not a separately supplied expiry — and the
/// engine's antithetic/parallel configuration is honored. This is the
/// canonical composition behind the host-binding
/// `McEngine.price_european_call` method; both hosts delegate here rather
/// than assembling process, discretization, and payoff themselves.
///
/// # Arguments
///
/// * `engine` - Caller-built engine whose grid defines the payoff horizon
/// * `seed` - RNG seed for deterministic replay
/// * `spot` - Spot level at time `0`
/// * `strike` - Exercise price in the same units as `spot`
/// * `rate` - Continuously compounded risk-free rate (decimal, annualized)
/// * `div_yield` - Continuous dividend yield (decimal, annualized)
/// * `vol` - Annualized volatility (decimal)
/// * `currency` - Currency stamped on the result; `None` uses the registry
///   binding default currency
///
/// # Errors
///
/// Returns an error if the registry defaults cannot be loaded when `currency`
/// is `None`, the GBM parameters or discount factor fail validation, or the
/// engine rejects the run.
///
/// # Examples
///
/// ```
/// use finstack_quant_models::monte_carlo::engine::{McEngine, McEngineConfig};
/// use finstack_quant_models::monte_carlo::pricer::european::price_engine_gbm_call;
/// use finstack_quant_models::monte_carlo::TimeGrid;
///
/// let grid = TimeGrid::uniform(1.0, 16).unwrap();
/// let engine = McEngine::new(McEngineConfig::new(2_000, grid).parallel(false));
/// let estimate = price_engine_gbm_call(&engine, 42, 100.0, 100.0, 0.03, 0.0, 0.2, None)
///     .expect("pricing should succeed");
/// assert!(estimate.mean.amount() > 0.0);
/// ```
#[allow(clippy::too_many_arguments)]
pub fn price_engine_gbm_call(
    engine: &McEngine,
    seed: u64,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    currency: Option<Currency>,
) -> Result<MoneyEstimate> {
    price_engine_gbm(
        engine, seed, true, spot, strike, rate, div_yield, vol, currency,
    )
}

/// Price a European put under GBM on a caller-built engine.
///
/// Put counterpart of [`price_engine_gbm_call`]; see it for horizon and
/// default-resolution semantics.
///
/// # Arguments
///
/// * `engine` - Caller-built engine whose grid defines the payoff horizon
/// * `seed` - RNG seed for deterministic replay
/// * `spot` - Spot level at time `0`
/// * `strike` - Exercise price in the same units as `spot`
/// * `rate` - Continuously compounded risk-free rate (decimal, annualized)
/// * `div_yield` - Continuous dividend yield (decimal, annualized)
/// * `vol` - Annualized volatility (decimal)
/// * `currency` - Currency stamped on the result; `None` uses the registry
///   binding default currency
///
/// # Errors
///
/// Same failure modes as [`price_engine_gbm_call`].
#[allow(clippy::too_many_arguments)]
pub fn price_engine_gbm_put(
    engine: &McEngine,
    seed: u64,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    currency: Option<Currency>,
) -> Result<MoneyEstimate> {
    price_engine_gbm(
        engine, seed, false, spot, strike, rate, div_yield, vol, currency,
    )
}

/// Shared call/put composition behind the engine-based GBM entry points.
#[allow(clippy::too_many_arguments)]
fn price_engine_gbm(
    engine: &McEngine,
    seed: u64,
    is_call: bool,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    currency: Option<Currency>,
) -> Result<MoneyEstimate> {
    use finstack_quant_core::cashflow::flat_discount_factor;

    let currency = match currency {
        Some(currency) => currency,
        None => {
            let defaults = &crate::monte_carlo::registry::embedded_defaults()?.convenience;
            super::heston::parse_registry_currency(&defaults.default_currency)?
        }
    };
    let t_max = engine.config().time_grid.t_max();
    let num_steps = engine.config().time_grid.num_steps();
    let rng = PhiloxRng::new(seed);
    let process = GbmProcess::with_params(rate, div_yield, vol)?;
    let disc = ExactGbm::new();
    let initial_state = [spot];
    // The payoff horizon is the grid's own t_max: this entry point takes a
    // caller-built engine whose grid defines the horizon.
    let discount_factor = flat_discount_factor(rate, t_max)?;

    if is_call {
        let payoff = EuropeanCall::new(strike, 1.0, num_steps);
        engine.price(
            &rng,
            &process,
            &disc,
            &initial_state,
            &payoff,
            currency,
            discount_factor,
        )
    } else {
        let payoff = EuropeanPut::new(strike, 1.0, num_steps);
        engine.price(
            &rng,
            &process,
            &disc,
            &initial_state,
            &payoff,
            currency,
            discount_factor,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::closed_form::{black_scholes_spot_call, black_scholes_spot_put};
    use crate::monte_carlo::payoff::vanilla::EuropeanCall;
    use crate::monte_carlo::process::gbm::GbmParams;

    #[test]
    fn test_european_pricer_basic() {
        let pricer = EuropeanPricer::new(1000)
            .expect("positive path count")
            .with_seed(42)
            .with_parallel(false);

        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let call = EuropeanCall::new(100.0, 1.0, 10);

        let result = pricer
            .price(&gbm, 100.0, 1.0, 10, &call, Currency::USD, 0.95)
            .expect("should succeed");

        assert!(result.mean.amount() > 0.0);
        assert!(result.mean.amount() < 50.0); // Sanity check
        assert_eq!(result.num_paths, 1000);
    }

    #[test]
    fn test_european_pricer_atm_call() {
        let pricer = EuropeanPricer::new(10000)
            .expect("positive path count")
            .with_seed(42)
            .with_parallel(false);

        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.02, 0.2).unwrap());
        let call = EuropeanCall::new(100.0, 1.0, 252);

        let result = pricer
            .price(&gbm, 100.0, 1.0, 252, &call, Currency::USD, 1.0)
            .expect("should succeed");

        // ATM call with σ=20%, T=1y should have positive value
        assert!(result.mean.amount() > 5.0);
    }

    #[test]
    fn test_european_pricer_deep_itm() {
        let pricer = EuropeanPricer::new(10000)
            .expect("positive path count")
            .with_seed(42)
            .with_parallel(false);

        let gbm = GbmProcess::new(GbmParams::new(0.0, 0.0, 0.01).unwrap());
        let call = EuropeanCall::new(50.0, 1.0, 100);

        let result = pricer
            .price(&gbm, 100.0, 1.0, 100, &call, Currency::USD, 1.0)
            .expect("should succeed");

        assert!((result.mean.amount() - 50.0).abs() < 5.0);
    }

    #[test]
    fn engine_gbm_helpers_are_deterministic_and_stamp_default_currency() {
        let grid = TimeGrid::uniform(1.0, 8).unwrap();
        let engine = McEngine::new(McEngineConfig::new(2_000, grid).parallel(false));

        let first = price_engine_gbm_call(&engine, 42, 100.0, 100.0, 0.03, 0.0, 0.2, None)
            .expect("engine GBM call should price");
        let second = price_engine_gbm_call(&engine, 42, 100.0, 100.0, 0.03, 0.0, 0.2, None)
            .expect("engine GBM call should price again");
        assert_eq!(first.mean, second.mean);
        assert_eq!(first.stderr, second.stderr);
        assert!(first.mean.amount() > 0.0);

        let expected: Currency = crate::monte_carlo::registry::embedded_defaults()
            .expect("embedded defaults")
            .convenience
            .default_currency
            .parse()
            .unwrap();
        assert_eq!(first.mean.currency(), expected);

        let put = price_engine_gbm_put(&engine, 42, 100.0, 100.0, 0.03, 0.0, 0.2, None)
            .expect("engine GBM put should price");
        assert!(put.mean.amount() > 0.0);
        assert_ne!(first.mean.amount(), put.mean.amount());
    }

    #[test]
    fn test_philox_exact_gbm_european_prices_match_black_scholes() {
        struct Case {
            name: &'static str,
            is_call: bool,
            spot: f64,
            strike: f64,
            rate: f64,
            div_yield: f64,
            vol: f64,
            expiry: f64,
        }

        let cases = [
            Case {
                name: "atm_call_with_dividend",
                is_call: true,
                spot: 100.0,
                strike: 100.0,
                rate: 0.05,
                div_yield: 0.02,
                vol: 0.20,
                expiry: 1.0,
            },
            Case {
                name: "atm_put_with_dividend",
                is_call: false,
                spot: 100.0,
                strike: 100.0,
                rate: 0.05,
                div_yield: 0.02,
                vol: 0.20,
                expiry: 1.0,
            },
            Case {
                name: "short_dated_otm_call",
                is_call: true,
                spot: 80.0,
                strike: 100.0,
                rate: 0.01,
                div_yield: 0.0,
                vol: 0.30,
                expiry: 0.25,
            },
            Case {
                name: "long_dated_itm_put",
                is_call: false,
                spot: 80.0,
                strike: 100.0,
                rate: 0.03,
                div_yield: 0.01,
                vol: 0.35,
                expiry: 3.0,
            },
        ];

        let pricer = EuropeanPricer::new(50_000)
            .expect("positive path count")
            .with_seed(42)
            .with_parallel(false);

        for case in cases {
            let (result, expected) = if case.is_call {
                (
                    pricer
                        .price_gbm_call(
                            case.spot,
                            case.strike,
                            case.rate,
                            case.div_yield,
                            case.vol,
                            case.expiry,
                            1,
                            Currency::USD,
                        )
                        .expect("call pricing should succeed"),
                    black_scholes_spot_call(
                        case.spot,
                        case.strike,
                        case.rate,
                        case.div_yield,
                        case.vol,
                        case.expiry,
                    ),
                )
            } else {
                (
                    pricer
                        .price_gbm_put(
                            case.spot,
                            case.strike,
                            case.rate,
                            case.div_yield,
                            case.vol,
                            case.expiry,
                            1,
                            Currency::USD,
                        )
                        .expect("put pricing should succeed"),
                    black_scholes_spot_put(
                        case.spot,
                        case.strike,
                        case.rate,
                        case.div_yield,
                        case.vol,
                        case.expiry,
                    ),
                )
            };

            assert!(
                result.stderr.is_finite() && result.stderr > 0.0,
                "{}: expected a finite positive standard error, got {}",
                case.name,
                result.stderr
            );
            let error = (result.mean.amount() - expected).abs();
            let tolerance = 6.0 * result.stderr;
            assert!(
                error <= tolerance,
                "{}: MC={} BS={} error={} exceeds 6*stderr={}",
                case.name,
                result.mean.amount(),
                expected,
                error,
                tolerance
            );
        }
    }
}
