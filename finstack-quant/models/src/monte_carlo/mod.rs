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

//! Monte Carlo simulation, pricing, and diagnostics for quantitative finance.
//!
//! # Entry points
//!
//! - [`engine::McEngine`] composes a [`traits::RandomStream`],
//!   [`traits::StochasticProcess`], [`traits::Discretization`], and
//!   [`traits::Payoff`] into a generic simulation.
//! - [`pricer`] provides higher-level European, path-dependent, and LSMC workflows.
//! - [`simulate_gbm_paths`] returns compact captured GBM paths for plotting and
//!   diagnostics.
//!
//! # Module map
//!
//! - Models and simulation inputs: [`rng`], [`process`], and [`discretization`].
//! - Engine contracts and execution: [`traits`], [`TimeGrid`], [`engine`], and
//!   [`engine_fractional`].
//! - Products and analytics: [`payoff`], [`pricer`], [`barriers`], [`greeks`], and
//!   [`variance_reduction`].
//! - Results and diagnostics: [`estimate`], [`OnlineStats`], [`paths`], and [`results`].
//! - Runtime defaults and reproducibility: [`registry`] and [`seed`].
//!
//! # Conventions
//!
//! Unless a module documents otherwise:
//!
//! - Rates, dividend yields, and volatilities are decimals; times and time-grid
//!   coordinates are year fractions.
//! - [`traits::Payoff::value`] returns an undiscounted
//!   [`finstack_quant_core::money::Money`]; [`engine::McEngine::price`] applies
//!   the caller-supplied discount factor.
//! - Captured-path statistics such as percentiles and ranges describe the
//!   captured subset, not necessarily the full Monte Carlo population.
//!
//! See [`engine`] for parallel-execution and configuration constraints. Model,
//! scheme, and estimator assumptions and references live in their leaf modules.
//!
//! # Quick start
//!
//! ```
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_models::monte_carlo::payoff::vanilla::EuropeanCall;
//! use finstack_quant_models::monte_carlo::pricer::european::EuropeanPricer;
//! use finstack_quant_models::monte_carlo::process::gbm::GbmProcess;
//!
//! let pricer = EuropeanPricer::new(25_000)
//!     .expect("positive path count")
//!     .with_seed(19)
//!     .with_parallel(false);
//! let process = GbmProcess::with_params(0.03, 0.01, 0.20).unwrap();
//! let payoff = EuropeanCall::new(100.0, 1.0, 252);
//! let result = pricer
//!     .price(&process, 100.0, 1.0, 252, &payoff, Currency::USD, (-0.03_f64).exp())
//!     .expect("pricing should succeed");
//! assert!(result.mean.amount().is_finite());
//! ```
//!
//! # References
//!
//! - Monte Carlo methods: `docs/REFERENCES.md#glasserman-2004-monte-carlo`
//! - GBM dynamics: `docs/REFERENCES.md#black-scholes-1973`
//! - Online mean/variance: `docs/REFERENCES.md#welford-1962`

mod captured_path_stats;
pub mod discretization;
pub mod estimate;
mod gbm_paths;
mod indexed_spot_table;
pub mod paths;
pub mod process;
pub mod rng;
pub mod traits;

pub mod barriers;
pub mod convenience;
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

pub use finstack_quant_core::math::stats::{required_samples, OnlineCovariance, OnlineStats};
pub use finstack_quant_core::math::time_grid::TimeGrid;
pub use gbm_paths::{simulate_gbm_paths, GbmPathConfig, GbmPathSummary};
pub use traits::{
    state_keys, Discretization, PathState, Payoff, ProportionalDiffusion, RandomStream, StateKey,
    StochasticProcess,
};

/// Reject a non-finite or non-positive volatility before a convenience pricer runs.
///
/// The generic GBM process accepts `sigma == 0` (a deterministic forward), but
/// the host-facing convenience entry points require a strictly positive
/// volatility so a sign slip or a zero placeholder surfaces as a validation
/// error instead of a silent degenerate price.
///
/// # Arguments
///
/// * `vol` - Annualized lognormal volatility as a decimal; must be finite and
///   strictly positive.
pub(crate) fn require_positive_vol(vol: f64) -> finstack_quant_core::Result<()> {
    if !vol.is_finite() || vol <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Monte Carlo convenience pricers require a finite, strictly positive volatility, got {vol}"
        )));
    }
    Ok(())
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

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`. Without it those samples
/// are dead text and rot silently on any API change.
#[cfg(doctest)]
#[doc = include_str!("../../README.md")]
struct ReadmeDoctests;
