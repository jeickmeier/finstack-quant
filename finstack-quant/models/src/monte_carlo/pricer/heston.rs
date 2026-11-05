//! Canonical Heston European Monte Carlo pricing entry points.
//!
//! These free functions own the `TimeGrid` + engine + QE-process + payoff
//! composition that the Python and WASM bindings expose as
//! `price_heston` (Python) / `priceHeston` (WASM), so the pipeline — including its
//! registry-backed defaults for path count, seed, step count, currency, and
//! parallelism — is defined once, in Rust. Both hosts are thin delegations.
//!
//! Paths are generated with the Quadratic-Exponential (QE) discretization of
//! Andersen (2008), which stays stable when the Feller condition
//! (`2κθ ≥ σ_v²`, see [`crate::monte_carlo::process::heston::heston_satisfies_feller`]) is
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

use crate::closed_form::HestonPricingParams;
use crate::monte_carlo::discretization::qe_heston::QeHeston;
use crate::monte_carlo::engine::{McEngine, McEngineConfig, McPricingInputs};
use crate::monte_carlo::payoff::vanilla::European;
use crate::monte_carlo::process::heston::HestonProcess;
use crate::monte_carlo::registry;
use crate::monte_carlo::results::MoneyEstimate;
use crate::monte_carlo::rng::philox::PhiloxRng;
use crate::types::OptionType;
use finstack_quant_core::Result;
use finstack_quant_core::cashflow::flat_discount_factor;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::time_grid::TimeGrid;

/// Monte Carlo run settings of [`price_heston`].
///
/// Every field is optional; `None` takes the registry binding default
/// (`convenience.european_pricer` and `convenience.default_currency`), so
/// `HestonMcSettings::default()` reproduces the host defaults.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HestonMcSettings {
    /// Independent path estimators in `2..=10_000_000`; each antithetic pair
    /// counts once.
    pub num_paths: Option<usize>,
    /// Root seed of the Philox path streams.
    pub seed: Option<u64>,
    /// Uniform time steps per path between `0` and expiry.
    pub num_steps: Option<usize>,
    /// Currency stamped on the result.
    pub currency: Option<Currency>,
}

/// Price a European option under Heston stochastic volatility by Monte Carlo.
///
/// Mirrors the Fourier entry point
/// [`crate::closed_form::heston_price`]: same market and model inputs, with
/// Monte Carlo settings in place of the Fourier settings.
///
/// # Arguments
///
/// * `spot` - Current underlying price
/// * `strike` - Exercise price in the same units as `spot`
/// * `time` - Time to expiry in years
/// * `params` - Continuously compounded decimal annual rate `r` and dividend
///   yield `q`, plus the Heston model (`v0` initial variance, `kappa` mean
///   reversion, `theta` long-run variance, `sigma_v` vol of variance, `rho`
///   spot/variance correlation in `[-1, 1]`); see [`HestonPricingParams`].
/// * `option_type` - Call or put payoff direction
/// * `settings` - Path count, seed, step count and result currency; each
///   `None` uses the registry binding default (see [`HestonMcSettings`]).
///
/// # Returns
///
/// A discounted Monte Carlo [`MoneyEstimate`] with its standard error.
///
/// # Errors
///
/// Returns an error if the embedded registry defaults cannot be loaded, the
/// default currency code is invalid, the expiry, step count, path count, or
/// discount factor fail validation, or a simulated discounted payoff is
/// non-finite.
///
/// # Examples
///
/// ```
/// use finstack_quant_models::closed_form::HestonPricingParams;
/// use finstack_quant_models::monte_carlo::pricer::heston::{price_heston, HestonMcSettings};
/// use finstack_quant_models::types::OptionType;
///
/// let params = HestonPricingParams::new(0.03, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04)
///     .expect("valid Heston parameters");
/// let settings = HestonMcSettings {
///     num_paths: Some(2_000),
///     seed: Some(42),
///     num_steps: Some(16),
///     currency: None,
/// };
/// let estimate = price_heston(100.0, 100.0, 1.0, &params, OptionType::Call, &settings)
///     .expect("pricing should succeed");
/// assert!(estimate.mean.amount() > 0.0);
/// ```
pub fn price_heston(
    spot: f64,
    strike: f64,
    time: f64,
    params: &HestonPricingParams,
    option_type: OptionType,
    settings: &HestonMcSettings,
) -> Result<MoneyEstimate> {
    let expiry = time;
    let defaults = &registry::embedded_defaults()?.convenience;
    let pricer_defaults = &defaults.european_pricer;
    let num_paths = settings.num_paths.unwrap_or(pricer_defaults.num_paths);
    let seed = settings.seed.unwrap_or(pricer_defaults.seed);
    let num_steps = settings.num_steps.unwrap_or(pricer_defaults.num_steps);
    let currency = crate::monte_carlo::convenience::resolve_currency(settings.currency)?;
    // Serial ≡ parallel by the determinism invariant, so the flag only sets
    // throughput. On wasm32 no thread pool exists, so force serial there.
    #[cfg(target_arch = "wasm32")]
    let use_parallel = false;
    #[cfg(not(target_arch = "wasm32"))]
    let use_parallel = pricer_defaults.use_parallel;

    let time_grid = TimeGrid::uniform(expiry, num_steps)?;
    let engine = McEngine::new(McEngineConfig::new(num_paths, time_grid).parallel(use_parallel));
    let rng = PhiloxRng::new(seed);
    let process = HestonProcess::new(*params);
    let disc = QeHeston::new();
    let initial_state = [spot, params.model.v0];
    let discount_factor = flat_discount_factor(params.r, expiry)?;

    let payoff = European::new(option_type, strike, 1.0, num_steps);
    engine.price(McPricingInputs {
        rng: &rng,
        process: &process,
        disc: &disc,
        initial_state: &initial_state,
        payoff: &payoff,
        currency,
        discount_factor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> HestonPricingParams {
        HestonPricingParams::new(0.03, 0.01, 2.0, 0.04, 0.3, -0.7, 0.04)
            .expect("valid Heston parameters")
    }

    fn settings(seed: u64) -> HestonMcSettings {
        HestonMcSettings {
            num_paths: Some(4_000),
            seed: Some(seed),
            num_steps: Some(16),
            currency: None,
        }
    }

    fn atm_call(seed: u64) -> MoneyEstimate {
        price_heston(
            100.0,
            100.0,
            1.0,
            &params(),
            OptionType::Call,
            &settings(seed),
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
        let put = price_heston(100.0, 100.0, 1.0, &params(), OptionType::Put, &settings(42))
            .expect("Heston put pricing should succeed");
        assert!(put.mean.amount() > 0.0);
        assert_ne!(call.mean.amount(), put.mean.amount());
    }
}
