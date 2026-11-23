//! High-level [`LsmcPricer`] facade for American and Bermudan options.

use super::engine::*;
use super::types::*;
use crate::monte_carlo::discretization::exact::ExactGbm;
use crate::monte_carlo::pricer::basis::BasisKind;
use crate::monte_carlo::process::gbm::GbmProcess;
use crate::monte_carlo::rng::philox::PhiloxRng;
use crate::monte_carlo::traits::{Discretization, RandomStream};
use crate::types::OptionMarketParams;
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::time_grid::TimeGrid;

pub(super) fn fill_gbm_row(
    disc: &ExactGbm,
    process: &GbmProcess,
    initial_spot: f64,
    time_grid: &TimeGrid,
    num_steps: usize,
    path_rng: &mut PhiloxRng,
    row: &mut [f64],
) -> finstack_quant_core::Result<()> {
    let mut state = [initial_spot];
    let mut z = [0.0];
    let mut work: [f64; 0] = [];
    row[0] = initial_spot;
    for step in 0..num_steps {
        let t = time_grid.time(step);
        let dt = time_grid.dt(step);
        path_rng.fill_std_normals(&mut z)?;
        disc.step(process, t, dt, &mut state, &z, &mut work)?;
        row[step + 1] = state[0];
    }
    Ok(())
}

pub(super) fn fill_gbm_antithetic_pair(
    disc: &ExactGbm,
    process: &GbmProcess,
    initial_spot: f64,
    time_grid: &TimeGrid,
    num_steps: usize,
    path_rng: &mut PhiloxRng,
    (primary, anti): (&mut [f64], &mut [f64]),
) -> finstack_quant_core::Result<()> {
    let mut state_p = [initial_spot];
    let mut state_a = [initial_spot];
    let mut z = [0.0];
    let mut z_anti = [0.0];
    let mut work: [f64; 0] = [];
    primary[0] = initial_spot;
    anti[0] = initial_spot;
    for step in 0..num_steps {
        let t = time_grid.time(step);
        let dt = time_grid.dt(step);
        path_rng.fill_std_normals(&mut z)?;
        z_anti[0] = -z[0];
        disc.step(process, t, dt, &mut state_p, &z, &mut work)?;
        disc.step(process, t, dt, &mut state_a, &z_anti, &mut work)?;
        primary[step + 1] = state_p[0];
        anti[step + 1] = state_a[0];
    }
    Ok(())
}

/// American LSMC pricer together with its default exercise grid and
/// regression basis.
///
/// [`LsmcEngine`] fixes the exercise grid at construction, while the host
/// pricing calls may override the step count, basis and basis degree per
/// call. This type keeps the construction-time choices and resolves each
/// call's overrides before pricing.
#[derive(Debug, Clone)]
pub struct LsmcPricer {
    pub(super) pricer: LsmcEngine,
    pub(super) num_steps: usize,
    pub(super) basis: BasisKind,
    pub(super) basis_degree: usize,
}

impl LsmcPricer {
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
    /// Returns [`finstack_quant_core::Error::Validation`] for an unknown basis name, and an error
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
        let defaults = &super::super::super::registry::embedded_defaults()?
            .convenience
            .lsmc;
        let basis = BasisKind::parse(basis.unwrap_or(defaults.basis.as_str()))?;
        let num_steps = num_steps.unwrap_or(defaults.num_steps);
        let pricer = LsmcEngine::gbm_american(
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
    pub fn pricer(&self) -> &LsmcEngine {
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

    /// Price an American option under GBM with the binding convenience pipeline.
    ///
    /// Builds the GBM process, Laguerre/polynomial basis, unit-notional
    /// exercise payoff, and every-step exercise schedule, then runs [`LsmcEngine::price`].
    /// Host bindings must delegate here rather than assembling those pieces.
    ///
    /// # Arguments
    ///
    /// * `params` - Call/put direction, positive spot/strike, decimal annual rate,
    ///   carry and volatility, and expiry in years; see [`OptionMarketParams`].
    /// * `num_steps` - Time-grid steps between `0` and `expiry`; `None` keeps the construction-time grid.
    /// * `currency` - Currency stamped on the returned estimate; `None` uses the
    ///   registry default `convenience.default_currency`.
    /// * `basis` - Regression basis name (`laguerre`, `polynomial`, or `normalized_polynomial`); `None` keeps the construction-time family.
    /// * `basis_degree` - Optional basis degree; `None` keeps the construction-time degree. It must be positive (`laguerre` also
    ///   requires the degree in `[1, 4]`).
    ///
    /// # Errors
    ///
    /// Returns an error when the registry default currency cannot be resolved,
    /// the strike, GBM parameters, basis, path count, step count, or
    /// discounting inputs fail validation, or the run fails.
    pub fn price_american(
        &self,
        params: &OptionMarketParams,
        num_steps: Option<usize>,
        currency: Option<Currency>,
        basis: Option<&str>,
        basis_degree: Option<usize>,
    ) -> Result<LsmcResult> {
        let &OptionMarketParams {
            option_type,
            spot,
            strike,
            rate,
            dividend_yield: div_yield,
            volatility: vol,
            time_to_expiry: expiry,
        } = params;
        let (pricer, num_steps, basis, basis_degree) =
            self.call_config(num_steps, basis, basis_degree)?;
        let currency = crate::monte_carlo::convenience::resolve_currency(currency)?;
        let exercise = AmericanExercise::new(option_type, strike)?;
        crate::monte_carlo::require_positive_vol(vol)?;
        let process = crate::monte_carlo::process::gbm::GbmParams::new(rate, div_yield, vol)
            .map(GbmProcess::new)?;
        let basis = lsmc_basis(basis, basis_degree, strike)?;
        let inputs = LsmcInputs {
            process: &process,
            initial_spot: spot,
            time_to_maturity: expiry,
            num_steps,
            exercise: &exercise,
            basis: &basis,
            discount_rate: rate,
        };
        pricer.price_internal(&inputs, currency, true)
    }

    /// Two-pass unbiased American option price under GBM.
    ///
    /// Fits the exercise policy on the configured training seed and prices on
    /// an independent `pricing_seed` path set via [`LsmcEngine::price_unbiased`].
    ///
    /// # Arguments
    ///
    /// * `params` - Call/put direction, positive spot/strike, decimal annual rate,
    ///   carry and volatility, and expiry in years; see [`OptionMarketParams`].
    /// * `num_steps` - Time-grid steps between `0` and `expiry`; `None` keeps the construction-time grid.
    /// * `currency` - Currency stamped on the returned estimate; `None` uses the
    ///   registry default `convenience.default_currency`.
    /// * `basis` - Regression basis name (`laguerre`, `polynomial`, or `normalized_polynomial`); `None` keeps the construction-time family.
    /// * `basis_degree` - Optional basis degree; `None` keeps the construction-time degree. It must be positive (`laguerre` also
    ///   requires the degree in `[1, 4]`).
    /// * `pricing_seed` - Seed for the out-of-sample pricing paths; must differ
    ///   from the configured training seed.
    ///
    /// # Errors
    ///
    /// Returns an error when inputs fail validation, `pricing_seed` matches the
    /// training seed, or either Monte Carlo pass fails.
    pub fn price_american_unbiased(
        &self,
        params: &OptionMarketParams,
        num_steps: Option<usize>,
        currency: Option<Currency>,
        basis: Option<&str>,
        basis_degree: Option<usize>,
        pricing_seed: u64,
    ) -> Result<LsmcResult> {
        let &OptionMarketParams {
            option_type,
            spot,
            strike,
            rate,
            dividend_yield: div_yield,
            volatility: vol,
            time_to_expiry: expiry,
        } = params;
        let (pricer, num_steps, basis, basis_degree) =
            self.call_config(num_steps, basis, basis_degree)?;
        let currency = crate::monte_carlo::convenience::resolve_currency(currency)?;
        let exercise = AmericanExercise::new(option_type, strike)?;
        crate::monte_carlo::require_positive_vol(vol)?;
        let process = crate::monte_carlo::process::gbm::GbmParams::new(rate, div_yield, vol)
            .map(GbmProcess::new)?;
        let basis = lsmc_basis(basis, basis_degree, strike)?;
        let inputs = LsmcInputs {
            process: &process,
            initial_spot: spot,
            time_to_maturity: expiry,
            num_steps,
            exercise: &exercise,
            basis: &basis,
            discount_rate: rate,
        };
        pricer.price_unbiased_internal(&inputs, currency, pricing_seed, true)
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
    /// Returns [`finstack_quant_core::Error::Validation`] for an unknown basis name, and an error
    /// if the step count is zero.
    pub(super) fn call_config(
        &self,
        num_steps: Option<usize>,
        basis: Option<&str>,
        basis_degree: Option<usize>,
    ) -> Result<(LsmcEngine, usize, BasisKind, usize)> {
        let (pricer, num_steps) = match num_steps {
            Some(steps) => {
                let config = self.pricer.config();
                (
                    LsmcEngine::gbm_american(
                        config.num_paths,
                        steps,
                        config.seed,
                        config.use_parallel,
                        config.antithetic,
                    )?,
                    steps,
                )
            }
            None => (self.pricer.clone(), self.num_steps),
        };
        let basis = match basis {
            Some(name) => BasisKind::parse(name)?,
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
