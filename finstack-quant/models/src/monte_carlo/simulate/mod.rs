//! Path simulation for the Monte Carlo processes behind one data-driven entry point.
//!
//! [`McEngine`](crate::monte_carlo::engine::McEngine) is generic over the
//! process and the discretization scheme, so a host language cannot name the
//! pair it wants. [`simulate_paths`] closes that gap: a [`ProcessSpec`] and a
//! [`SchemeSpec`] select the concrete pair inside Rust, and the result is a
//! compact [`PathSummary`] holding every simulated state on a shared time grid.
//!
//! # Conventions
//!
//! - Rates, yields and volatilities are annualized decimals; times are year
//!   fractions measured from the simulation start.
//! - Correlation matrices are row-major `n x n` with unit diagonal.
//! - Path `p` (zero-based, before antithetic mirroring) draws from the Philox
//!   substream `p` of the root `seed`, so the output does not depend on thread
//!   count or on how many paths are requested.
//!
//! # Supported pairs
//!
//! | Process | `default` scheme | Other schemes |
//! | --- | --- | --- |
//! | `gbm` | exact lognormal | `euler`, `log_euler`, `milstein` |
//! | `gbm_with_dividends` | exact lognormal with dividend jumps | none |
//! | `multi_gbm` | exact lognormal | `euler`, `log_euler`, `milstein` |
//! | `brownian`, `multi_brownian` | Euler (exact for constant coefficients) | `euler` |
//! | `multi_ou` | Euler | `euler` |
//! | `hull_white_1f` | exact Gaussian transition | `euler` |
//! | `cir`, `cir_plus_plus` | quadratic-exponential | `euler` (full truncation) |
//! | `heston` | quadratic-exponential | `euler` (biased; research use) |
//! | `schwartz_smith` | exact Gaussian transition | `euler` |
//! | `local_vol` | log-Euler, volatility frozen at the step start | `log_euler`, `euler` |
//! | `lmm` | predictor-corrector under the terminal measure | none |
//! | `rough_bergomi` | left-point log-Euler on injected fractional noise | none |
//! | `rough_heston` | hybrid Volterra kernel scheme | none |
//! | `cheyette_rough` | left-point Euler on injected fractional noise | none |
//!
//! Any other pairing is a validation error naming the process and the scheme.
//!
//! # Path-dependent processes
//!
//! The last four rows are not Markov in the reported state: `lmm` freezes
//! each forward at its fixing date, and the rough models carry the history of
//! their volatility driver. `rough_bergomi` and `cheyette_rough` consume
//! fractional noise generated for the whole path before its first step;
//! [`FbmSpec`] selects the generator, and
//! [`PathSimulationSpec::fbm`] is rejected for every other process. Their
//! cost per path grows with the square of the number of steps.
//!
//! # References
//!
//! - Monte Carlo path simulation and antithetic variates:
//!   `docs/REFERENCES.md#glasserman-2004-monte-carlo`
//! - Quadratic-exponential scheme: `docs/REFERENCES.md#andersen-2008-heston-qe`
//! - Full-truncation Euler: `docs/REFERENCES.md#lord-koekkoek-vandijk-2010`
//! - Schwartz-Smith exact transition: `docs/REFERENCES.md#schwartz-smith-2000`
//! - LIBOR market model: `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
//! - Rough Bergomi: `docs/REFERENCES.md#bayer-friz-gatheral-2016`
//! - Hybrid scheme for the Volterra driver:
//!   `docs/REFERENCES.md#bennedsen-lunde-pakkanen-2017`

mod fractional;
#[cfg(test)]
#[path = "tests/non_markov.rs"]
mod non_markov_tests;
mod runner;
mod spec;
#[cfg(test)]
mod tests;

pub use fractional::FbmSpec;
pub use runner::simulate_paths;
pub(crate) use spec::MAX_STORED_VALUES;
pub use spec::{PathSimulationSpec, PathSummary, ProcessSpec, SchemeSpec, TimeGridSpec};
