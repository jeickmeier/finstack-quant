//! Convenience pricer for European-style payoffs under GBM dynamics.
//!
//! This module wraps [`crate::monte_carlo::engine::McEngine`] for the common case of pricing
//! a European payoff under [`crate::monte_carlo::process::gbm::GbmProcess`] with
//! [`crate::monte_carlo::discretization::exact::ExactGbm`]. Use it when you want a compact
//! API and do not need custom process / discretization combinations.

use super::super::engine::{McEngine, McEngineConfig, McPricingInputs};
use super::super::results::MoneyEstimate;
use super::super::traits::Payoff;
use crate::monte_carlo::discretization::exact::ExactGbm;
use crate::monte_carlo::payoff::vanilla::European;
use crate::monte_carlo::process::gbm::GbmProcess;
use crate::monte_carlo::rng::philox::PhiloxRng;
use crate::types::OptionMarketParams;
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::time_grid::TimeGrid;

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
    /// * `num_paths` - Independent Monte Carlo path estimators; at least two are
    ///   required to estimate sampling uncertainty.
    ///
    /// # Errors
    ///
    /// Returns a validation error when `num_paths` is less than two.
    pub fn new(num_paths: usize) -> Result<Self> {
        if num_paths < 2 {
            return Err(finstack_quant_core::Error::Validation(
                "EuropeanPricer requires at least two independent path estimators".to_string(),
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
    /// * `time_grid` - Simulation grid from `0` to maturity (in years), for
    ///   example [`TimeGrid::uniform`]`(maturity, num_steps)`.
    /// * `payoff` - European-style payoff evaluated at
    ///   `maturity_step = time_grid.num_steps()`.
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
    /// Returns an error when the underlying engine rejects the runtime
    /// configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::math::time_grid::TimeGrid;
    /// use finstack_quant_models::monte_carlo::payoff::vanilla::European;
    /// use finstack_quant_models::types::OptionType;
    /// use finstack_quant_models::monte_carlo::pricer::european::EuropeanPricer;
    /// use finstack_quant_models::monte_carlo::process::gbm::GbmProcess;
    ///
    /// let pricer = EuropeanPricer::new(25_000)
    ///     .expect("positive path count")
    ///     .with_seed(19)
    ///     .with_parallel(false);
    /// let process = finstack_quant_models::monte_carlo::process::gbm::GbmParams::new(0.03, 0.01, 0.20).map(GbmProcess::new).unwrap();
    /// let payoff = European::new(OptionType::Call, 100.0, 1.0, 252);
    /// let discount_factor = (-0.03_f64).exp();
    ///
    /// let grid = TimeGrid::uniform(1.0, 252).expect("valid grid");
    /// let result = pricer
    ///     .price(&process, 100.0, grid, &payoff, Currency::USD, discount_factor)
    ///     .expect("pricing should succeed");
    ///
    /// assert!(result.mean.amount().is_finite());
    /// ```
    pub fn price<P>(
        &self,
        process: &GbmProcess,
        initial_spot: f64,
        time_grid: TimeGrid,
        payoff: &P,
        currency: Currency,
        discount_factor: f64,
    ) -> Result<MoneyEstimate>
    where
        P: Payoff,
    {
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

        engine.price(McPricingInputs {
            rng: &rng,
            process,
            disc: &disc,
            initial_state: &initial_state,
            payoff,
            currency,
            discount_factor,
        })
    }

    /// Price a European option under risk-neutral GBM with flat continuous
    /// discounting `exp(-rT)`.
    ///
    /// This is a scalar-arg convenience for the common binding case where the
    /// caller supplies raw floats rather than pre-built `GbmProcess` / payoff
    /// instances.
    ///
    /// # Arguments
    ///
    /// * `params` - Call/put direction, spot at time `0`, strike in spot units,
    ///   continuously compounded decimal annual rate and dividend yield,
    ///   decimal annual GBM volatility, and time to expiry in years (also the
    ///   uniform-grid horizon); see [`OptionMarketParams`].
    /// * `num_steps` - Number of time-grid steps between `0` and expiry;
    ///   `None` uses the registry default `convenience.european_pricer.num_steps`.
    /// * `currency` - Currency stamped on the returned estimate; `None` uses the
    ///   registry default `convenience.default_currency`.
    ///
    /// # Errors
    ///
    /// Returns an error when the embedded registry defaults cannot be loaded,
    /// GBM parameters fail validation, the uniform grid is invalid, or the
    /// underlying engine rejects the run.
    pub fn price_gbm(
        &self,
        params: &OptionMarketParams,
        num_steps: Option<usize>,
        currency: Option<Currency>,
    ) -> Result<MoneyEstimate> {
        let &OptionMarketParams {
            option_type,
            spot,
            strike,
            rate,
            dividend_yield: div_yield,
            volatility: vol,
            time_to_expiry: expiry,
        } = params;
        let currency = crate::monte_carlo::convenience::resolve_currency(currency)?;
        let num_steps = num_steps.unwrap_or(
            crate::monte_carlo::registry::embedded_defaults()?
                .convenience
                .european_pricer
                .num_steps,
        );
        crate::monte_carlo::require_positive_vol(vol)?;
        let process = crate::monte_carlo::process::gbm::GbmParams::new(rate, div_yield, vol)
            .map(GbmProcess::new)?;
        let discount_factor = (-rate * expiry).exp();
        let payoff = European::new(option_type, strike, 1.0, num_steps);
        let time_grid = TimeGrid::uniform(expiry, num_steps)?;
        self.price(
            &process,
            spot,
            time_grid,
            &payoff,
            currency,
            discount_factor,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monte_carlo::payoff::vanilla::European;
    use crate::monte_carlo::process::gbm::GbmParams;
    use crate::types::OptionType;

    fn grid(t_max: f64, num_steps: usize) -> TimeGrid {
        TimeGrid::uniform(t_max, num_steps).expect("valid grid")
    }

    #[test]
    fn european_pricer_requires_two_independent_estimators() {
        for num_paths in [0, 1] {
            assert!(EuropeanPricer::new(num_paths).is_err());
        }
        let pricer = EuropeanPricer::new(2)
            .expect("two independent estimators")
            .with_parallel(false);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let call = European::new(OptionType::Call, 100.0, 1.0, 1);
        let estimate = pricer
            .price(&gbm, 100.0, grid(1.0, 1), &call, Currency::USD, 0.95)
            .expect("minimum-size estimate");
        assert_eq!(estimate.num_paths, 2);
        assert!(estimate.stderr.is_finite());
        assert!(estimate.ci_95.0.amount().is_finite());
        assert!(estimate.ci_95.1.amount().is_finite());
    }

    #[test]
    fn test_european_pricer_basic() {
        let pricer = EuropeanPricer::new(1000)
            .expect("positive path count")
            .with_seed(42)
            .with_parallel(false);

        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let call = European::new(OptionType::Call, 100.0, 1.0, 10);

        let result = pricer
            .price(&gbm, 100.0, grid(1.0, 10), &call, Currency::USD, 0.95)
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
        let call = European::new(OptionType::Call, 100.0, 1.0, 252);

        let result = pricer
            .price(&gbm, 100.0, grid(1.0, 252), &call, Currency::USD, 1.0)
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
        let call = European::new(OptionType::Call, 50.0, 1.0, 100);

        let result = pricer
            .price(&gbm, 100.0, grid(1.0, 100), &call, Currency::USD, 1.0)
            .expect("should succeed");

        assert!((result.mean.amount() - 50.0).abs() < 5.0);
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
            let option_type = crate::types::OptionType::from(case.is_call);
            let result = pricer
                .price_gbm(
                    &OptionMarketParams {
                        option_type,
                        spot: case.spot,
                        strike: case.strike,
                        rate: case.rate,
                        dividend_yield: case.div_yield,
                        volatility: case.vol,
                        time_to_expiry: case.expiry,
                    },
                    Some(1),
                    Some(Currency::USD),
                )
                .expect("pricing should succeed");
            let expected = crate::closed_form::bs_price(
                case.spot,
                case.strike,
                case.rate,
                case.div_yield,
                case.vol,
                case.expiry,
                option_type,
            )
            .expect("valid Black-Scholes inputs");

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
