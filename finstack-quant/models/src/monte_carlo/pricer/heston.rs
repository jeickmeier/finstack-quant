//! Canonical Heston European Monte Carlo pricing entry points.
//!
//! These free functions own the `TimeGrid` + engine + QE-process + payoff
//! composition that the Python and WASM bindings expose as
//! `price_heston_call` / `price_heston_put`, so the pipeline — including its
//! registry-backed defaults for path count, seed, step count, currency, and
//! parallelism — is defined once, in Rust. Both hosts are thin delegations.
//!
//! Paths are generated with the Quadratic-Exponential (QE) discretization of
//! Andersen (2008), which stays stable when the Feller condition
//! (`2κθ ≥ σ_v²`, see [`crate::monte_carlo::process::heston::feller_condition`]) is
//! violated — the common case for equity calibrations.
//!
//! # Determinism
//!
//! The same seed reproduces the same price on any thread count; parallel and
//! serial execution are guaranteed identical. On `wasm32` targets parallelism
//! is forced off because no thread pool is available there.
//!
//! # References
//!
//! - Andersen, L. (2008). "Simple and Efficient Simulation of the Heston
//!   Stochastic Volatility Model." *Journal of Computational Finance*,
//!   11(3), 1-42. `docs/REFERENCES.md#andersen-2008-heston-qe`
//! - Heston, S. L. (1993). "A Closed-Form Solution for Options with
//!   Stochastic Volatility with Applications to Bond and Currency Options."
//!   *Review of Financial Studies*, 6(2), 327-343. `docs/REFERENCES.md#heston-1993`

use std::str::FromStr;

use crate::monte_carlo::discretization::QeHeston;
use crate::monte_carlo::engine::{McEngine, McEngineConfig};
use crate::monte_carlo::payoff::vanilla::{EuropeanCall, EuropeanPut};
use crate::monte_carlo::process::heston::HestonProcess;
use crate::monte_carlo::registry;
use crate::monte_carlo::results::MoneyEstimate;
use crate::monte_carlo::rng::philox::PhiloxRng;
use crate::monte_carlo::TimeGrid;
use finstack_quant_core::cashflow::flat_discount_factor;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::Result;

/// Price a European call under Heston stochastic volatility by Monte Carlo.
///
/// # Arguments
///
/// * `spot` - Current underlying price
/// * `strike` - Exercise price in the same units as `spot`
/// * `rate` - Continuously compounded risk-free rate (decimal, annualized)
/// * `div_yield` - Continuous dividend yield (decimal, annualized)
/// * `kappa` - Mean-reversion speed of the variance process
/// * `theta` - Long-run variance level (variance, not volatility)
/// * `vol_of_vol` - Volatility of variance
/// * `rho` - Spot/variance correlation in `[-1, 1]`
/// * `v0` - Initial instantaneous variance
/// * `expiry` - Time to expiry in years
/// * `num_paths` - Independent path estimators in `2..=10_000_000`; each
///   antithetic pair counts once. `None` uses the registry binding default.
/// * `seed` - RNG seed; `None` uses the registry binding default
/// * `num_steps` - Time steps per path; `None` uses the registry binding default
/// * `currency` - Currency stamped on the result; `None` uses the registry
///   binding default currency
///
/// # Returns
///
/// A discounted Monte Carlo [`MoneyEstimate`] with its standard error.
///
/// # Errors
///
/// Returns an error if the embedded registry defaults cannot be loaded, the
/// default currency code is invalid, the Heston parameters, expiry, step
/// count, path count, or discount factor fail validation, or a simulated
/// discounted payoff is non-finite.
///
/// # Examples
///
/// ```
/// use finstack_quant_models::monte_carlo::pricer::heston::price_heston_call;
///
/// let estimate = price_heston_call(
///     100.0, 100.0, 0.03, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04, 1.0,
///     Some(2_000), Some(42), Some(16), None,
/// )
/// .expect("pricing should succeed");
/// assert!(estimate.mean.amount() > 0.0);
/// ```
#[allow(clippy::too_many_arguments)]
pub fn price_heston_call(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    kappa: f64,
    theta: f64,
    vol_of_vol: f64,
    rho: f64,
    v0: f64,
    expiry: f64,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    currency: Option<Currency>,
) -> Result<MoneyEstimate> {
    price_heston_european(
        true, spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry, num_paths,
        seed, num_steps, currency,
    )
}

/// Price a European put under Heston stochastic volatility by Monte Carlo.
///
/// Identical machinery to [`price_heston_call`] — QE discretization, same
/// registry-backed defaults, same determinism guarantee — with a put payoff.
///
/// # Arguments
///
/// * `spot` - Current underlying price
/// * `strike` - Exercise price in the same units as `spot`
/// * `rate` - Continuously compounded risk-free rate (decimal, annualized)
/// * `div_yield` - Continuous dividend yield (decimal, annualized)
/// * `kappa` - Mean-reversion speed of the variance process
/// * `theta` - Long-run variance level (variance, not volatility)
/// * `vol_of_vol` - Volatility of variance
/// * `rho` - Spot/variance correlation in `[-1, 1]`
/// * `v0` - Initial instantaneous variance
/// * `expiry` - Time to expiry in years
/// * `num_paths` - Independent path estimators in `2..=10_000_000`; each
///   antithetic pair counts once. `None` uses the registry binding default.
/// * `seed` - RNG seed; `None` uses the registry binding default
/// * `num_steps` - Time steps per path; `None` uses the registry binding default
/// * `currency` - Currency stamped on the result; `None` uses the registry
///   binding default currency
///
/// # Errors
///
/// Same failure modes as [`price_heston_call`].
#[allow(clippy::too_many_arguments)]
pub fn price_heston_put(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    kappa: f64,
    theta: f64,
    vol_of_vol: f64,
    rho: f64,
    v0: f64,
    expiry: f64,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    currency: Option<Currency>,
) -> Result<MoneyEstimate> {
    price_heston_european(
        false, spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry, num_paths,
        seed, num_steps, currency,
    )
}

/// Parse the registry's default currency code into a [`Currency`].
///
/// Shared by the canonical convenience pricers so an invalid registry value
/// surfaces as a core validation error rather than a raw parse error.
pub(crate) fn parse_registry_currency(code: &str) -> Result<Currency> {
    Currency::from_str(code).map_err(|err| {
        finstack_quant_core::Error::Validation(format!(
            "invalid registry default currency '{code}': {err}"
        ))
    })
}

/// Shared call/put composition behind the public Heston entry points.
#[allow(clippy::too_many_arguments)]
fn price_heston_european(
    is_call: bool,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    kappa: f64,
    theta: f64,
    vol_of_vol: f64,
    rho: f64,
    v0: f64,
    expiry: f64,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    currency: Option<Currency>,
) -> Result<MoneyEstimate> {
    let defaults = &registry::embedded_defaults()?.convenience;
    let pricer_defaults = &defaults.european_pricer;
    let num_paths = num_paths.unwrap_or(pricer_defaults.num_paths);
    let seed = seed.unwrap_or(pricer_defaults.seed);
    let num_steps = num_steps.unwrap_or(pricer_defaults.num_steps);
    let currency = match currency {
        Some(currency) => currency,
        None => parse_registry_currency(&defaults.default_currency)?,
    };
    // Serial ≡ parallel by the determinism invariant, so the flag only sets
    // throughput. On wasm32 no thread pool exists, so force serial there.
    #[cfg(target_arch = "wasm32")]
    let use_parallel = false;
    #[cfg(not(target_arch = "wasm32"))]
    let use_parallel = pricer_defaults.use_parallel;

    let time_grid = TimeGrid::uniform(expiry, num_steps)?;
    let engine = McEngine::new(McEngineConfig::new(num_paths, time_grid).parallel(use_parallel));
    let rng = PhiloxRng::new(seed);
    let process = HestonProcess::with_params(rate, div_yield, kappa, theta, vol_of_vol, rho, v0)?;
    let disc = QeHeston::new();
    let initial_state = [spot, v0];
    let discount_factor = flat_discount_factor(rate, expiry)?;

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

    fn atm_call(seed: u64) -> MoneyEstimate {
        price_heston_call(
            100.0,
            100.0,
            0.03,
            0.01,
            2.0,
            0.04,
            0.3,
            -0.7,
            0.04,
            1.0,
            Some(4_000),
            Some(seed),
            Some(16),
            None,
        )
        .expect("Heston call pricing should succeed")
    }

    #[test]
    fn heston_call_is_deterministic_and_positive() {
        let first = atm_call(42);
        let second = atm_call(42);
        assert_eq!(first.mean, second.mean);
        assert_eq!(first.stderr, second.stderr);
        assert!(first.mean.amount() > 0.0);
        assert!(first.stderr.is_finite() && first.stderr > 0.0);
    }

    #[test]
    fn heston_defaults_stamp_registry_currency() {
        let defaults = &registry::embedded_defaults()
            .expect("embedded defaults")
            .convenience;
        let expected = defaults.default_currency.parse().expect("valid currency");
        let estimate = atm_call(7);
        assert_eq!(estimate.mean.currency(), expected);
    }

    #[test]
    fn heston_put_call_have_distinct_values() {
        let call = atm_call(42);
        let put = price_heston_put(
            100.0,
            100.0,
            0.03,
            0.01,
            2.0,
            0.04,
            0.3,
            -0.7,
            0.04,
            1.0,
            Some(4_000),
            Some(42),
            Some(16),
            None,
        )
        .expect("Heston put pricing should succeed");
        assert!(put.mean.amount() > 0.0);
        assert_ne!(call.mean.amount(), put.mean.amount());
    }
}
