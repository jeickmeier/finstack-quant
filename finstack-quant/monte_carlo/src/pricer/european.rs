//! Convenience pricer for European-style payoffs under GBM dynamics.
//!
//! This module wraps [`crate::engine::McEngine`] for the common case of pricing
//! a European payoff under [`crate::process::gbm::GbmProcess`] with
//! [`crate::discretization::exact::ExactGbm`]. Use it when you want a compact
//! API and do not need custom process / discretization combinations.

use super::super::engine::{McEngine, McEngineConfig};
use super::super::results::MoneyEstimate;
use super::super::traits::Payoff;
use crate::discretization::exact::ExactGbm;
use crate::payoff::vanilla::{EuropeanCall, EuropeanPut};
use crate::process::gbm::GbmProcess;
use crate::rng::philox::PhiloxRng;
use crate::time_grid::TimeGrid;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::Result;

/// Compact GBM-only pricer for European-style contracts.
///
/// The pricer always uses exact GBM transitions and delegates the simulation
/// loop to [`crate::engine::McEngine`]. Its simulation vocabulary is
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
        let defaults = &crate::registry::embedded_defaults_or_panic()
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
    pub fn new(num_paths: usize) -> Self {
        Self {
            num_paths,
            ..Self::default()
        }
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
    /// ```ignore
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_monte_carlo::payoff::vanilla::EuropeanCall;
    /// use finstack_quant_monte_carlo::pricer::european::EuropeanPricer;
    /// use finstack_quant_monte_carlo::process::gbm::GbmProcess;
    ///
    /// let pricer = EuropeanPricer::new(25_000)
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
        let engine_config =
            McEngineConfig::new(self.num_paths, time_grid).parallel(self.use_parallel);
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
    #[allow(clippy::too_many_arguments)]
    pub fn price_gbm_call(
        &self,
        spot: f64,
        strike: f64,
        rate: f64,
        dividend_yield: f64,
        volatility: f64,
        expiry: f64,
        num_steps: usize,
        currency: Currency,
    ) -> Result<MoneyEstimate> {
        let process = GbmProcess::with_params(rate, dividend_yield, volatility)?;
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
    #[allow(clippy::too_many_arguments)]
    pub fn price_gbm_put(
        &self,
        spot: f64,
        strike: f64,
        rate: f64,
        dividend_yield: f64,
        volatility: f64,
        expiry: f64,
        num_steps: usize,
        currency: Currency,
    ) -> Result<MoneyEstimate> {
        let process = GbmProcess::with_params(rate, dividend_yield, volatility)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payoff::vanilla::EuropeanCall;
    use crate::process::gbm::GbmParams;

    #[test]
    fn test_european_pricer_basic() {
        let pricer = EuropeanPricer::new(1000).with_seed(42).with_parallel(false);

        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let call = EuropeanCall::new(100.0, 1.0, 10);

        let result = pricer
            .price(&gbm, 100.0, 1.0, 10, &call, Currency::USD, 0.95)
            .expect("should succeed");

        // Should get a reasonable option value
        assert!(result.mean.amount() > 0.0);
        assert!(result.mean.amount() < 50.0); // Sanity check
        assert_eq!(result.num_paths, 1000);
    }

    #[test]
    fn test_european_pricer_atm_call() {
        let pricer = EuropeanPricer::new(10000)
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
            .with_seed(42)
            .with_parallel(false);

        let gbm = GbmProcess::new(GbmParams::new(0.0, 0.0, 0.01).unwrap());
        let call = EuropeanCall::new(50.0, 1.0, 100);

        let result = pricer
            .price(&gbm, 100.0, 1.0, 100, &call, Currency::USD, 1.0)
            .expect("should succeed");

        // Should be close to intrinsic value of 50
        assert!((result.mean.amount() - 50.0).abs() < 5.0);
    }
}
