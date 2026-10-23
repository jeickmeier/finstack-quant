//! Registry-defaulted construction of the convenience Monte Carlo pricers.
//!
//! The host bindings expose `EuropeanPricer`, `PathDependentPricer` and an
//! American LSMC pricer whose every setting is optional. The defaults live in
//! the embedded Monte Carlo registry
//! ([`crate::monte_carlo::registry::ConvenienceDefaults`]); the functions here
//! resolve omitted construction settings to their registry values, and the
//! pricing entry points (`EuropeanPricer::price_gbm`,
//! `PathDependentPricer::price_asian` / `price_with_lrm_greeks`) take their
//! step count and currency as `Option`s resolved the same way, so Python and
//! WebAssembly build identical pricers.
//!
//! # Examples
//!
//! ```
//! use finstack_quant_models::monte_carlo::convenience;
//! use finstack_quant_models::types::OptionType;
//!
//! let pricer = convenience::european_pricer(Some(1_000), Some(7), Some(false))
//!     .expect("valid path count");
//! // `None` step count and currency resolve to the registry defaults.
//! let estimate = pricer
//!     .price_gbm(OptionType::Call, 100.0, 100.0, 0.05, 0.0, 0.2, 1.0, None, None)
//!     .expect("valid inputs");
//! assert!(estimate.mean.amount() > 0.0);
//! ```

use std::str::FromStr;

use finstack_quant_core::currency::Currency;
use finstack_quant_core::{Error, Result};

use super::pricer::european::EuropeanPricer;
use super::pricer::path_dependent::{PathDependentPricer, PathDependentPricerConfig};
use super::registry::{self, ConvenienceDefaults};

fn defaults() -> Result<&'static ConvenienceDefaults> {
    registry::embedded_defaults().map(|defaults| &defaults.convenience)
}

/// Resolve the currency stamped on a convenience estimate.
///
/// # Arguments
///
/// * `currency` - Currency chosen by the caller; `None` selects the registry
///   default currency (`convenience.default_currency`).
///
/// # Errors
///
/// Returns [`Error::Validation`] if the registry default is not a valid
/// currency code.
pub fn resolve_currency(currency: Option<Currency>) -> Result<Currency> {
    match currency {
        Some(currency) => Ok(currency),
        None => {
            let code = &defaults()?.default_currency;
            Currency::from_str(code).map_err(|err| {
                Error::Validation(format!("invalid registry default currency '{code}': {err}"))
            })
        }
    }
}

/// Build a GBM European pricer, taking omitted settings from the registry.
///
/// # Arguments
///
/// * `num_paths` - Number of Monte Carlo paths; `None` uses the registry
///   default.
/// * `seed` - Seed of the path-indexed random streams; `None` uses the
///   registry default.
/// * `use_parallel` - Whether paths run on the thread pool (ignored on
///   `wasm32`); `None` uses the registry default.
///
/// # Errors
///
/// Returns an error if the path count is zero.
pub fn european_pricer(
    num_paths: Option<usize>,
    seed: Option<u64>,
    use_parallel: Option<bool>,
) -> Result<EuropeanPricer> {
    let defaults = &defaults()?.european_pricer;
    Ok(
        EuropeanPricer::new(num_paths.unwrap_or(defaults.num_paths))?
            .with_seed(seed.unwrap_or(defaults.seed))
            .with_parallel(use_parallel.unwrap_or(defaults.use_parallel)),
    )
}

/// Build a path-dependent (Asian) pricer, taking omitted settings from the
/// registry and from [`PathDependentPricerConfig`].
///
/// # Arguments
///
/// * `num_paths` - Number of Monte Carlo paths; `None` uses the registry
///   default.
/// * `seed` - Seed of the random streams; `None` uses the registry default.
/// * `use_parallel` - Whether paths run on the thread pool (ignored on
///   `wasm32`); `None` uses the registry default.
/// * `antithetic` - Whether antithetic variates are used; `None` keeps the
///   configuration default.
/// * `use_sobol` - Whether Sobol quasi-random numbers replace the
///   pseudo-random stream; `None` keeps the configuration default.
/// * `use_brownian_bridge` - Whether paths are built by Brownian bridge;
///   `None` keeps the configuration default.
///
/// # Errors
///
/// Returns an error if the resulting configuration is invalid (for example a
/// zero path count, or Sobol combined with antithetic variates).
pub fn path_dependent_pricer(
    num_paths: Option<usize>,
    seed: Option<u64>,
    use_parallel: Option<bool>,
    antithetic: Option<bool>,
    use_sobol: Option<bool>,
    use_brownian_bridge: Option<bool>,
) -> Result<PathDependentPricer> {
    let defaults = &defaults()?.path_dependent_pricer;
    let mut config = PathDependentPricerConfig::new(num_paths.unwrap_or(defaults.num_paths))
        .with_seed(seed.unwrap_or(defaults.seed))
        .with_parallel(use_parallel.unwrap_or(defaults.use_parallel));
    if let Some(antithetic) = antithetic {
        config = config.with_antithetic(antithetic);
    }
    if let Some(use_sobol) = use_sobol {
        config = config.with_sobol(use_sobol);
    }
    if let Some(use_brownian_bridge) = use_brownian_bridge {
        config = config.with_brownian_bridge(use_brownian_bridge);
    }
    config.validate()?;
    Ok(PathDependentPricer::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_settings_resolve_to_the_registry_defaults() {
        let defaults = defaults().expect("embedded registry");
        let european = european_pricer(None, None, None).expect("defaults are valid");
        assert_eq!(european.num_paths(), defaults.european_pricer.num_paths);
        assert_eq!(european.seed(), defaults.european_pricer.seed);

        // A default pricer with the default LRM step count stays under the
        // likelihood-ratio path-capture cap.
        assert!(
            defaults.path_dependent_pricer.num_paths * (defaults.greeks.lrm_num_steps + 1)
                <= super::super::pricer::path_dependent::MAX_LRM_CAPTURED_POINTS
        );

        let asian =
            path_dependent_pricer(None, None, None, None, None, None).expect("defaults are valid");
        assert_eq!(
            asian.config().num_paths,
            defaults.path_dependent_pricer.num_paths
        );
        assert_eq!(
            resolve_currency(None).expect("default").to_string(),
            defaults.default_currency
        );
        assert_eq!(
            resolve_currency(Some(Currency::EUR)).expect("explicit"),
            Currency::EUR
        );
    }

    #[test]
    fn explicit_settings_override_the_registry() {
        let european = european_pricer(Some(123), Some(9), Some(false)).expect("valid");
        assert_eq!(european.num_paths(), 123);
        assert_eq!(european.seed(), 9);
        assert!(!european.use_parallel());
        assert!(european_pricer(Some(0), None, None).is_err());
    }
}
