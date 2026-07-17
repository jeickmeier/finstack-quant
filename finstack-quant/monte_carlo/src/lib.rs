#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
#![doc(test(attr(allow(clippy::expect_used))))]

//! Monte Carlo pricing infrastructure for derivative valuation and diagnostics.
//!
//! This crate combines stochastic-process definitions, discretization schemes,
//! random-number generators, payoffs, and pricing engines behind composable
//! traits. Most users start with [`engine::McEngine`] for generic simulations or
//! [`pricer::european::EuropeanPricer`] for a GBM-only entry point.
//!
//! # What Is Available
//!
//! The crate provides:
//!
//! - [`rng::philox::PhiloxRng`] for deterministic pseudo-random sampling
//! - [`rng::sobol::SobolRng`] for quasi-random Sobol generators
//! - [`process::gbm::GbmProcess`] and related Brownian / OU-style processes
//! - Heston, CIR, Hull-White, jump-diffusion, Bates, and Schwartz-Smith models
//! - [`discretization::exact::ExactGbm`] and other exact schemes
//! - Euler / Milstein / QE discretizations, path-dependent payoffs, LSMC, Greeks,
//!   control variates, and antithetic pairing (see [`variance_reduction`])
//! - deterministic seed helpers in [`crate::seed`]
//! - vanilla and exotic payoffs and the core [`traits`] / [`engine`] infrastructure
//!
//! Rayon-backed path simulation is compiled in by default. Parallel mode
//! requires an RNG that supports deterministic stream splitting, such as
//! [`rng::philox::PhiloxRng`].
//!
//! # Conventions
//!
//! Public APIs in this crate use the following conventions unless a module says
//! otherwise:
//!
//! - Rates, dividend yields, and volatilities are quoted in decimals, not basis points.
//! - Times and time-grid coordinates are year fractions.
//! - [`engine::McEngine::price`] expects a caller-supplied discount factor for the
//!   payoff horizon, typically `exp(-rT)` under a flat continuously compounded rate.
//! - [`traits::Payoff::value`] returns an undiscounted [`finstack_quant_core::money::Money`]
//!   amount in the requested currency; the engine applies the discount factor outside
//!   the payoff implementation.
//! - Captured path statistics such as percentiles and ranges are computed from the
//!   captured subset, not necessarily from the full Monte Carlo population.
//!
//! # Start Here
//!
//! - Use [`prelude`] for ergonomic imports in examples and downstream code.
//! - Read [`traits`] to understand the contracts shared by processes, schemes, and payoffs.
//! - Read [`engine`] for runtime constraints such as parallel RNG requirements and
//!   unsupported configuration combinations.
//! - Read [`rng`], [`process`], and [`discretization`] module docs for model- and
//!   scheme-specific assumptions.
//!
//! # Examples
//!
//! ```ignore
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_monte_carlo::prelude::*;
//!
//! let engine = McEngine::builder()
//!     .num_paths(50_000)
//!     .uniform_grid(1.0, 252)
//!     .build()
//!     .expect("valid Monte Carlo configuration");
//!
//! let rng = PhiloxRng::new(7);
//! let process = GbmProcess::with_params(0.03, 0.01, 0.20).unwrap();
//! let disc = ExactGbm::new();
//! let payoff = EuropeanCall::new(100.0, 1.0, 252);
//! let discount_factor = (-0.03_f64).exp();
//!
//! let result = engine
//!     .price(
//!         &rng,
//!         &process,
//!         &disc,
//!         &[100.0],
//!         &payoff,
//!         Currency::USD,
//!         discount_factor,
//!     )
//!     .expect("pricing should succeed");
//!
//! assert!(result.mean.amount().is_finite());
//! ```
//!
//! # References
//!
//! - Heston-style stochastic-volatility documentation should cite
//!   [`docs/REFERENCES.md#heston-1993`](../../docs/REFERENCES.md#heston-1993).
//! - Rate-discounting and option-pricing conventions should cite
//!   [`docs/REFERENCES.md#hull-options-futures`](../../docs/REFERENCES.md#hull-options-futures).
//! - Monte Carlo path generation and variance-reduction routines follow the
//!   standard references discussed in the relevant module docs, especially
//!   Glasserman (2003) and Andersen (2008).

// --- Simulation primitives ---
mod captured_path_stats;
pub mod discretization;
pub mod estimate;
mod gbm_paths;
mod indexed_spot_table;
pub mod online_stats;
pub mod paths;
pub mod process;
pub mod rng;
pub mod time_grid;
pub mod traits;

// --- Pricing infrastructure ---
pub mod barriers;
pub mod engine;
pub mod engine_fractional;
pub mod greeks;
pub mod payoff;
pub mod pricer;
pub mod registry;
pub mod results;
pub mod seed;
pub mod variance_reduction;

#[cfg(test)]
mod mc_process_params_serialization;

pub use gbm_paths::{simulate_gbm_paths, GbmPathConfig, GbmPathSummary};
pub use traits::{
    state_keys, Discretization, PathState, Payoff, ProportionalDiffusion, RandomStream, StateKey,
    StochasticProcess,
};

/// Prelude for convenient imports of the main Monte Carlo entry points.
///
/// Use this module when you want the crate's common engine, process, payoff, and pricer
/// types without spelling their full paths.
pub mod prelude {
    // --- Core traits and infrastructure ---
    pub use super::estimate::Estimate;
    pub use super::gbm_paths::{simulate_gbm_paths, GbmPathConfig, GbmPathSummary};
    pub use super::online_stats::{required_samples, OnlineCovariance, OnlineStats};
    pub use super::paths::{
        CashflowType, PathDataset, PathPoint, PathSamplingMethod, ProcessParams, SimulatedPath,
    };
    pub use super::time_grid::TimeGrid;
    pub use super::traits::{Discretization, PathState, RandomStream, StochasticProcess};

    // --- RNG ---
    pub use super::rng::philox::PhiloxRng;
    pub use super::rng::sobol::SobolRng;

    // --- Processes ---
    pub use super::process::bates::{BatesParams, BatesProcess};
    pub use super::process::brownian::{BrownianParams, BrownianProcess, MultiBrownianProcess};
    pub use super::process::cir::{CirParams, CirPlusPlusProcess, CirProcess};
    pub use super::process::correlation::{apply_correlation, cholesky_decomposition};
    pub use super::process::gbm::{GbmParams, GbmProcess, MultiGbmProcess};
    pub use super::process::heston::{HestonParams, HestonProcess};
    pub use super::process::jump_diffusion::{MertonJumpParams, MertonJumpProcess};
    pub use super::process::multi_ou::MultiOuParams;
    pub use super::process::ou::{HullWhite1FParams, HullWhite1FProcess};
    pub use super::process::schwartz_smith::{SchwartzSmithParams, SchwartzSmithProcess};

    // --- Discretization schemes ---
    //
    // Route everything through the `discretization` module's own re-exports
    // (see `src/discretization/mod.rs`) so there is one canonical public path
    // per scheme. The prelude is a curated list on top of that.
    pub use super::discretization::{
        CheyetteRoughEuler, EulerMaruyama, ExactGbm, ExactHullWhite1F, ExactMultiGbm,
        ExactMultiGbmCorrelated, ExactSchwartzSmith, JumpEuler, Milstein, QeBates, QeCir, QeHeston,
        RoughBergomiEuler, RoughHestonHybrid,
    };

    // --- Engine and configuration ---
    pub use super::engine::{
        McEngine, McEngineBuilder, McEngineConfig, PathCaptureConfig, PathCaptureMode,
    };
    pub use super::engine_fractional::simulate_path_fractional;

    // --- Fractional noise ---
    pub use super::rng::fbm::{create_fbm_generator, FractionalNoiseGenerator};
    pub use super::rng::volterra::RiemannLiouvilleVolterra;
    // --- Pricing results ---
    pub use super::results::{MoneyEstimate, MonteCarloResult};
    pub use super::traits::Payoff;

    // --- Payoffs ---
    pub use super::payoff::asian::{
        geometric_asian_call_closed_form, AsianCall, AsianPut, AveragingMethod,
    };
    pub use super::payoff::barrier::{BarrierOptionPayoff, BarrierType};
    pub use super::payoff::basket::{
        margrabe_exchange_option, BasketCall, BasketPut, BasketType, ExchangeOption,
    };
    pub use super::payoff::vanilla::{Digital, EuropeanCall, EuropeanPut, Forward};

    // --- Pricers ---
    pub use super::pricer::basis::{LaguerreBasis, PolynomialBasis};
    pub use super::pricer::european::EuropeanPricer;
    pub use super::pricer::lsmc::{
        AmericanCall, AmericanPut, ExercisePolicy, LsmcConfig, LsmcPricer,
    };
    pub use super::pricer::path_dependent::{PathDependentPricer, PathDependentPricerConfig};

    // --- Greeks ---
    pub use super::greeks::finite_diff::{
        finite_diff_delta, finite_diff_delta_crn, finite_diff_gamma, finite_diff_gamma_crn,
    };

    // --- Variance reduction ---
    pub use super::variance_reduction::control_variate::{black_scholes_call, black_scholes_put};
}

#[cfg(test)]
mod gbm_path_summary_tests {
    use super::{simulate_gbm_paths, GbmPathConfig};

    #[test]
    fn gbm_path_summary_is_deterministic_and_shaped() {
        let config = GbmPathConfig::new(100.0, 0.05, 0.01, 0.2, 1.0, 4, 3).with_seed(42);
        let first = simulate_gbm_paths(&config).expect("GBM paths should simulate");
        let second = simulate_gbm_paths(&config).expect("same GBM paths should simulate");

        assert_eq!(first, second);
        assert_eq!(first.num_paths, 3);
        assert_eq!(first.num_simulated_paths, 3);
        assert_eq!(first.times.len(), 5);
        assert_eq!(first.paths.len(), 3);
        assert!(first.paths.iter().all(|path| path.len() == 5));
        assert!(first.paths.iter().all(|path| path[0] == 100.0));
    }

    #[test]
    fn gbm_path_capture_rejects_antithetic_pairing() {
        let config = GbmPathConfig::new(100.0, 0.05, 0.0, 0.2, 1.0, 4, 3).with_antithetic(true);
        let error =
            simulate_gbm_paths(&config).expect_err("path capture and antithetic must be rejected");
        assert!(error.to_string().contains("antithetic"));
    }
}
