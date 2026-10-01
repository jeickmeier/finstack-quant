//! Registry-defaulted construction of the convenience Monte Carlo pricers.
//!
//! The host bindings expose `EuropeanPricer`, `PathDependentPricer` and an
//! American LSMC pricer whose every setting is optional. The defaults live in
//! the embedded Monte Carlo registry
//! ([`crate::monte_carlo::registry::ConvenienceDefaults`]); the functions here
//! are the one place that resolves an omitted setting to its registry value,
//! so Python and WebAssembly build identical pricers.
//!
//! # Examples
//!
//! ```
//! use finstack_quant_models::monte_carlo::convenience;
//!
//! let pricer = convenience::european_pricer(Some(1_000), Some(7), Some(false))
//!     .expect("valid path count");
//! let steps = convenience::european_num_steps(None).expect("registry default");
//! let currency = convenience::resolve_currency(None).expect("registry default");
//! let estimate = pricer
//!     .price_gbm_call(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, steps, currency)
//!     .expect("valid inputs");
//! assert!(estimate.mean.amount() > 0.0);
//! ```

use std::str::FromStr;

use finstack_quant_core::currency::Currency;
use finstack_quant_core::{Error, Result};

use super::pricer::basis::BasisKind;
use super::pricer::european::EuropeanPricer;
use super::pricer::lsmc::LsmcPricer;
use super::pricer::path_dependent::{PathDependentPricer, PathDependentPricerConfig};
use super::registry::{self, ConvenienceDefaults};

fn defaults() -> Result<&'static ConvenienceDefaults> {
    registry::embedded_defaults().map(|defaults| &defaults.convenience)
}

fn parse_basis(name: &str) -> Result<BasisKind> {
    BasisKind::parse(name).map_err(Error::Validation)
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

/// Time-grid step count of a European convenience price.
///
/// # Arguments
///
/// * `num_steps` - Steps between `0` and expiry; `None` uses the registry
///   default.
///
/// # Errors
///
/// Returns an error if the embedded registry cannot be loaded.
pub fn european_num_steps(num_steps: Option<usize>) -> Result<usize> {
    Ok(num_steps.unwrap_or(defaults()?.european_pricer.num_steps))
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

/// Time-grid step count of a path-dependent convenience price.
///
/// # Arguments
///
/// * `num_steps` - Monitoring steps between `0` and expiry; `None` uses the
///   registry default.
///
/// # Errors
///
/// Returns an error if the embedded registry cannot be loaded.
pub fn path_dependent_num_steps(num_steps: Option<usize>) -> Result<usize> {
    Ok(num_steps.unwrap_or(defaults()?.path_dependent_pricer.num_steps))
}

/// American LSMC pricer together with its default exercise grid and
/// regression basis.
///
/// [`LsmcPricer`] fixes the exercise grid at construction, while the host
/// pricing calls may override the step count, basis and basis degree per
/// call. This type keeps the construction-time choices and resolves each
/// call's overrides through [`Self::call_config`].
#[derive(Debug, Clone)]
pub struct LsmcConvenience {
    pricer: LsmcPricer,
    num_steps: usize,
    basis: BasisKind,
    basis_degree: usize,
}

impl LsmcConvenience {
    /// Build the pricer, taking omitted settings from the registry.
    ///
    /// # Arguments
    ///
    /// * `num_paths` - Number of Monte Carlo paths; `None` uses the registry
    ///   default.
    /// * `seed` - Seed of the random streams; `None` uses the registry
    ///   default.
    /// * `use_parallel` - Whether paths run on the thread pool (ignored on
    ///   `wasm32`); `None` uses the registry default.
    /// * `num_steps` - Exercise dates between `0` and expiry; `None` uses the
    ///   registry default.
    /// * `basis` - Regression basis name accepted by `BasisKind::parse`
    ///   (such as `"laguerre"` or `"polynomial"`); `None` uses the registry
    ///   default.
    /// * `basis_degree` - Highest polynomial degree of the regression basis;
    ///   `None` uses the registry default.
    /// * `antithetic` - Whether antithetic variates are used; `None` uses the
    ///   registry default.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for an unknown basis name, and an error
    /// if the path or step count is zero.
    pub fn new(
        num_paths: Option<usize>,
        seed: Option<u64>,
        use_parallel: Option<bool>,
        num_steps: Option<usize>,
        basis: Option<&str>,
        basis_degree: Option<usize>,
        antithetic: Option<bool>,
    ) -> Result<Self> {
        let defaults = &defaults()?.lsmc;
        let basis = parse_basis(basis.unwrap_or(defaults.basis.as_str()))?;
        let num_steps = num_steps.unwrap_or(defaults.num_steps);
        let pricer = LsmcPricer::gbm_american(
            num_paths.unwrap_or(defaults.num_paths),
            num_steps,
            seed.unwrap_or(defaults.seed),
            use_parallel.unwrap_or(defaults.use_parallel),
            antithetic.unwrap_or(defaults.antithetic),
        )?;
        Ok(Self {
            pricer,
            num_steps,
            basis,
            basis_degree: basis_degree.unwrap_or(defaults.basis_degree),
        })
    }

    /// The pricer built at construction, on the default exercise grid.
    #[must_use]
    pub fn pricer(&self) -> &LsmcPricer {
        &self.pricer
    }

    /// Default number of exercise dates.
    #[must_use]
    pub fn num_steps(&self) -> usize {
        self.num_steps
    }

    /// Default regression basis.
    #[must_use]
    pub fn basis(&self) -> BasisKind {
        self.basis
    }

    /// Default highest polynomial degree of the regression basis.
    #[must_use]
    pub fn basis_degree(&self) -> usize {
        self.basis_degree
    }

    /// Resolve one pricing call's overrides against the construction-time
    /// defaults.
    ///
    /// # Arguments
    ///
    /// * `num_steps` - Exercise dates for this call; `None` keeps the
    ///   construction-time value.
    /// * `basis` - Regression basis name for this call; `None` keeps the
    ///   construction-time basis.
    /// * `basis_degree` - Basis degree for this call; `None` keeps the
    ///   construction-time degree.
    ///
    /// # Returns
    ///
    /// The pricer rebuilt on the resolved exercise grid, with the resolved
    /// step count, basis and basis degree to pass to its pricing methods.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for an unknown basis name, and an error
    /// if the step count is zero.
    pub fn call_config(
        &self,
        num_steps: Option<usize>,
        basis: Option<&str>,
        basis_degree: Option<usize>,
    ) -> Result<(LsmcPricer, usize, BasisKind, usize)> {
        let config = self.pricer.config();
        let num_steps = num_steps.unwrap_or(self.num_steps);
        let pricer = LsmcPricer::gbm_american(
            config.num_paths,
            num_steps,
            config.seed,
            config.use_parallel,
            config.antithetic,
        )?;
        let basis = match basis {
            Some(name) => parse_basis(name)?,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_settings_resolve_to_the_registry_defaults() {
        let defaults = defaults().expect("embedded registry");
        let european = european_pricer(None, None, None).expect("defaults are valid");
        assert_eq!(european.num_paths(), defaults.european_pricer.num_paths);
        assert_eq!(european.seed(), defaults.european_pricer.seed);
        assert_eq!(
            european_num_steps(None).expect("default"),
            defaults.european_pricer.num_steps
        );
        assert_eq!(european_num_steps(Some(7)).expect("explicit"), 7);

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

    #[test]
    fn lsmc_call_config_layers_call_overrides_over_construction() {
        let lsmc = LsmcConvenience::new(
            Some(500),
            Some(3),
            Some(false),
            Some(10),
            None,
            Some(2),
            None,
        )
        .expect("valid");
        assert_eq!(lsmc.num_steps(), 10);
        assert_eq!(lsmc.basis_degree(), 2);

        let (pricer, steps, basis, degree) = lsmc.call_config(None, None, None).expect("defaults");
        assert_eq!(pricer.config().num_paths, 500);
        assert_eq!((steps, basis, degree), (10, lsmc.basis(), 2));

        let (_, steps, _, degree) = lsmc
            .call_config(Some(20), None, Some(4))
            .expect("overrides");
        assert_eq!((steps, degree), (20, 4));

        assert!(lsmc.call_config(None, Some("not-a-basis"), None).is_err());
        assert!(LsmcConvenience::new(None, None, None, None, Some("nope"), None, None).is_err());
    }
}
