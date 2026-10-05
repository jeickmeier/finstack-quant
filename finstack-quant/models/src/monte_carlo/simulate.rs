//! Path simulation for the Markov Monte Carlo processes behind one data-driven entry point.
//!
//! [`McEngine`] is generic over the
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
//!
//! Any other pairing is a validation error naming the process and the scheme.
//!
//! # References
//!
//! - Monte Carlo path simulation and antithetic variates:
//!   `docs/REFERENCES.md#glasserman-2004-monte-carlo`
//! - Quadratic-exponential scheme: `docs/REFERENCES.md#andersen-2008-heston-qe`
//! - Full-truncation Euler: `docs/REFERENCES.md#lord-koekkoek-vandijk-2010`
//! - Schwartz-Smith exact transition: `docs/REFERENCES.md#schwartz-smith-2000`

use crate::closed_form::heston::HestonPricingParams;
use crate::monte_carlo::discretization::euler::{EulerMaruyama, LogEuler};
use crate::monte_carlo::discretization::exact::{ExactGbm, ExactMultiGbm};
use crate::monte_carlo::discretization::exact_gbm_dividends::ExactGbmWithDividends;
use crate::monte_carlo::discretization::milstein::Milstein;
use crate::monte_carlo::discretization::{ExactHullWhite1F, ExactSchwartzSmith, QeCir, QeHeston};
use crate::monte_carlo::engine::{build_correlation_factor, McEngine, MAX_CAPTURED_PATHS};
use crate::monte_carlo::process::brownian::{BrownianProcess, MultiBrownianProcess};
use crate::monte_carlo::process::cir::{CirParams, CirPlusPlusProcess, CirProcess};
use crate::monte_carlo::process::gbm::MultiGbmProcess;
use crate::monte_carlo::process::gbm_dividends::{Dividend, GbmWithDividends};
use crate::monte_carlo::process::heston::HestonProcess;
use crate::monte_carlo::process::multi_ou::MultiOuProcess;
use crate::monte_carlo::process::ou::{HullWhite1FParams, HullWhite1FProcess};
use crate::monte_carlo::process::schwartz_smith::{SchwartzSmithParams, SchwartzSmithProcess};
use crate::monte_carlo::process::{
    BrownianParams, GbmParams, GbmProcess, MultiOuParams, ProcessMetadata,
};
use crate::monte_carlo::rng::philox::PhiloxRng;
use crate::monte_carlo::traits::{Discretization, RandomStream, StochasticProcess};
use crate::monte_carlo::TimeGrid;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// Upper bound on stored scalars: every path's states plus the shared time
/// grids. Checked before the time grid or any path buffer is allocated.
pub(crate) const MAX_STORED_VALUES: usize = 64_000_000;

/// Stochastic process to simulate, with its parameters.
///
/// Serialized as an internally tagged object: `{"type": "gbm", "r": 0.03, ...}`.
/// Parameters are re-validated when [`simulate_paths`] runs, so a spec read
/// from JSON cannot bypass the range checks of the Rust constructors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProcessSpec {
    /// Geometric Brownian motion `dS = (r - q) S dt + σ S dW`.
    ///
    /// State: `[spot]`, which must start strictly positive.
    Gbm(GbmParams),
    /// Geometric Brownian motion with discrete dividends paid at fixed times.
    ///
    /// State: `[spot]`, which must start strictly positive.
    GbmWithDividends {
        /// Diffusion parameters between dividend dates. Set `q` to zero when
        /// every dividend is listed explicitly.
        params: GbmParams,
        /// Dividend schedule as `[time, dividend]` pairs. `time` is the
        /// ex-dividend time in years: strictly positive and distinct, in any
        /// order. A cash dividend is a non-negative amount in spot units and
        /// a proportional dividend a non-negative decimal fraction of spot.
        dividends: Vec<(f64, Dividend)>,
    },
    /// Correlated geometric Brownian motions, one per asset.
    ///
    /// State: `[spot_0, spot_1, ...]`, each starting strictly positive.
    MultiGbm {
        /// Per-asset parameters; the vector length is the state dimension.
        assets: Vec<GbmParams>,
        /// Row-major `n x n` correlation matrix of the driving Brownian
        /// motions; omitted means independent assets.
        #[serde(default)]
        correlation: Option<Vec<f64>>,
    },
    /// Arithmetic Brownian motion with drift `dX = μ dt + σ dW`.
    ///
    /// State: `[x]`.
    Brownian(BrownianParams),
    /// Correlated arithmetic Brownian motions.
    ///
    /// State: `[x_0, x_1, ...]`.
    MultiBrownian {
        /// Per-component drifts per year; the vector length is the state
        /// dimension.
        mus: Vec<f64>,
        /// Per-component diffusion scales per square-root year, non-negative.
        sigmas: Vec<f64>,
        /// Row-major `n x n` correlation matrix of the driving Brownian
        /// motions; omitted means independent components.
        #[serde(default)]
        correlation: Option<Vec<f64>>,
    },
    /// Correlated Ornstein-Uhlenbeck processes `dX_i = κ_i (θ_i - X_i) dt + σ_i dW_i`.
    ///
    /// State: `[x_0, x_1, ...]`.
    MultiOu(MultiOuParams),
    /// Hull-White one-factor short rate `dr = κ (θ(t) - r) dt + σ(t) dW`.
    ///
    /// State: `[short_rate]` as a decimal.
    #[serde(rename = "hull_white_1f")]
    HullWhite1F(HullWhite1FParams),
    /// Cox-Ingersoll-Ross square-root process `dv = κ (θ - v) dt + σ √v dW`.
    ///
    /// State: `[short_rate]` (or an intensity or variance), starting
    /// non-negative.
    Cir(CirParams),
    /// CIR++ short rate `r_t = x_t + φ(t)` with `x` a CIR factor and `φ` a
    /// piecewise-constant deterministic shift.
    ///
    /// State: `[short_rate]`. The initial state is the short rate `r_0`, not
    /// the factor: the factor starts at `r_0 - φ(0)`, which must be
    /// non-negative, and every reported value is the shifted rate.
    CirPlusPlus {
        /// Parameters of the unshifted CIR factor.
        params: CirParams,
        /// Shift values `φ` as decimal rates, one per entry of `shift_times`;
        /// each applies from its time until the next.
        shift_curve: Vec<f64>,
        /// Strictly increasing shift start times in years. Times before the
        /// first entry use the first shift value.
        shift_times: Vec<f64>,
    },
    /// Heston stochastic volatility.
    ///
    /// State: `[spot, variance]`; spot starts strictly positive, and the
    /// starting variance `initial_state[1]` must equal the `v0` parameter.
    Heston(HestonPricingParams),
    /// Schwartz-Smith two-factor commodity model.
    ///
    /// State: `[x, y]`, the short-term deviation and the long-term level of
    /// the log price; the spot is `exp(x + y)`.
    SchwartzSmith(SchwartzSmithParams),
}

impl ProcessSpec {
    /// Serialized tag of the variant, used in error messages.
    fn tag(&self) -> &'static str {
        match self {
            Self::Gbm(_) => "gbm",
            Self::GbmWithDividends { .. } => "gbm_with_dividends",
            Self::MultiGbm { .. } => "multi_gbm",
            Self::Brownian(_) => "brownian",
            Self::MultiBrownian { .. } => "multi_brownian",
            Self::MultiOu(_) => "multi_ou",
            Self::HullWhite1F(_) => "hull_white_1f",
            Self::Cir(_) => "cir",
            Self::CirPlusPlus { .. } => "cir_plus_plus",
            Self::Heston(_) => "heston",
            Self::SchwartzSmith(_) => "schwartz_smith",
        }
    }
}

/// Time-discretization scheme used to advance the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SchemeSpec {
    /// The process's canonical scheme: its exact transition where one exists,
    /// quadratic-exponential for square-root variance, Euler otherwise. See
    /// the [module table](self#supported-pairs).
    #[default]
    Default,
    /// Euler-Maruyama: `X += μ Δt + σ √Δt Z`. First-order weak convergence;
    /// available for every process except `gbm_with_dividends`.
    Euler,
    /// Euler on the log state, which keeps the state positive. Only for the
    /// proportional-diffusion processes `gbm` and `multi_gbm`.
    LogEuler,
    /// Milstein with the proportional-diffusion correction
    /// `½ σ² X (Z² - 1) Δt`. Only for `gbm` and `multi_gbm`.
    Milstein,
}

/// Simulation dates, as year fractions from the simulation start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum TimeGridSpec {
    /// Equally spaced steps on `[0, expiry]`.
    Uniform {
        /// Finite positive horizon in years.
        expiry: f64,
        /// Positive number of steps.
        num_steps: usize,
    },
    /// Explicit simulation times.
    Times {
        /// Strictly increasing finite times in years, starting at exactly
        /// `0.0`, with at least two entries.
        times: Vec<f64>,
    },
}

impl TimeGridSpec {
    fn num_steps(&self) -> usize {
        match self {
            Self::Uniform { num_steps, .. } => *num_steps,
            Self::Times { times } => times.len().saturating_sub(1),
        }
    }

    fn build(&self) -> Result<TimeGrid> {
        match self {
            Self::Uniform { expiry, num_steps } => TimeGrid::uniform(*expiry, *num_steps),
            Self::Times { times } => TimeGrid::from_times(times.clone()),
        }
    }
}

/// Complete, serializable description of one path simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PathSimulationSpec {
    /// Process and parameters.
    pub process: ProcessSpec,
    /// Discretization scheme; defaults to the process's canonical scheme.
    #[serde(default)]
    pub scheme: SchemeSpec,
    /// State at time zero, in the layout documented on the [`ProcessSpec`]
    /// variant. Its length must equal the process dimension.
    pub initial_state: Vec<f64>,
    /// Simulation dates.
    pub time_grid: TimeGridSpec,
    /// Number of independent random streams, in `1..=100_000`. Each stream
    /// yields one path, or two when `antithetic` is set.
    pub num_paths: usize,
    /// Root seed of the Philox generator. The same spec always reproduces the
    /// same paths bit for bit.
    pub seed: u64,
    /// Emit an antithetic partner after each path, driven by the negated
    /// normal draws of the same stream.
    #[serde(default)]
    pub antithetic: bool,
}

/// Simulated paths on a shared time grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct PathSummary {
    /// Number of independent random streams requested.
    pub num_paths: usize,
    /// Number of stored paths: `num_paths`, or `2 * num_paths` with
    /// antithetic sampling, where stored paths `2k` and `2k + 1` are stream
    /// `k` and its antithetic partner.
    pub num_simulated_paths: usize,
    /// State dimension; equals `factor_names.len()`.
    pub dim: usize,
    /// Simulation times in years, starting at zero; length `num_steps + 1`.
    pub times: Vec<f64>,
    /// Name of each state component, in state-vector order.
    pub factor_names: Vec<String>,
    /// States in row-major `[path][time][factor]` order: the value of factor
    /// `f` on stored path `p` at `times[s]` is
    /// `values[(p * times.len() + s) * dim + f]`.
    pub values: Vec<f64>,
}

/// Simulate paths of the process and scheme selected by `spec`.
///
/// # Arguments
///
/// * `spec` - Process, scheme, initial state, time grid, stream count, seed
///   and antithetic flag. Paths are returned in stream order.
///
/// # Returns
///
/// Every simulated state, including the initial state at time zero.
///
/// # Errors
///
/// Returns a validation error when:
///
/// * a process parameter is out of range, or a correlation matrix is not a
///   valid correlation matrix of the process dimension;
/// * the scheme is not available for the process;
/// * `initial_state` has the wrong length, is non-finite, or lies outside the
///   process's state domain (for example a non-positive GBM spot);
/// * the time grid is invalid or crosses an event the scheme cannot integrate;
/// * `num_paths` is outside `1..=100_000`, or the output would exceed
///   64 million stored values;
/// * a simulated state is non-finite.
///
/// # Determinism
///
/// The output is a pure function of `spec`. Serial and parallel execution
/// return identical values because each stream owns its Philox substream.
///
/// # Examples
///
/// ```
/// use finstack_quant_models::monte_carlo::simulate::{
///     simulate_paths, PathSimulationSpec, ProcessSpec, SchemeSpec, TimeGridSpec,
/// };
/// use finstack_quant_models::monte_carlo::process::cir::CirParams;
///
/// let spec = PathSimulationSpec {
///     process: ProcessSpec::Cir(CirParams::new(0.5, 0.04, 0.1)?),
///     scheme: SchemeSpec::Default,
///     initial_state: vec![0.03],
///     time_grid: TimeGridSpec::Uniform { expiry: 2.0, num_steps: 24 },
///     num_paths: 100,
///     seed: 7,
///     antithetic: false,
/// };
/// let paths = simulate_paths(&spec)?;
///
/// assert_eq!(paths.times.len(), 25);
/// assert_eq!(paths.factor_names, ["short_rate"]);
/// assert_eq!(paths.values.len(), 100 * 25);
/// assert!(paths.values.iter().all(|rate| *rate >= 0.0));
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn simulate_paths(spec: &PathSimulationSpec) -> Result<PathSummary> {
    let parallel = crate::monte_carlo::registry::embedded_defaults()?
        .rust
        .engine
        .use_parallel;
    dispatch(spec, parallel)
}

/// Build the concrete process and scheme named by `spec` and run them.
fn dispatch(spec: &PathSimulationSpec, parallel: bool) -> Result<PathSummary> {
    let x0 = spec.initial_state.as_slice();
    let unsupported = || {
        Err(Error::Validation(format!(
            "scheme '{}' is not available for process '{}'",
            scheme_tag(spec.scheme),
            spec.process.tag()
        )))
    };
    match &spec.process {
        ProcessSpec::Gbm(p) => {
            let process = GbmProcess::new(GbmParams::new(p.r, p.q, p.sigma)?);
            require_domain(spec, &process, f64::MIN_POSITIVE)?;
            match spec.scheme {
                SchemeSpec::Default => simulate(&process, &ExactGbm, spec, x0, parallel),
                SchemeSpec::Euler => simulate(&process, &EulerMaruyama, spec, x0, parallel),
                SchemeSpec::LogEuler => simulate(&process, &LogEuler, spec, x0, parallel),
                SchemeSpec::Milstein => simulate(&process, &Milstein, spec, x0, parallel),
            }
        }
        ProcessSpec::GbmWithDividends { params, dividends } => {
            // A dividend at or before time zero would never be applied, and a
            // negative one would raise the spot.
            if let Some((time, dividend)) = dividends.iter().find(|(time, dividend)| {
                let (Dividend::Cash(size) | Dividend::Proportional(size)) = dividend;
                !(*time > 0.0 && size.is_finite() && *size >= 0.0)
            }) {
                return Err(Error::Validation(format!(
                    "process 'gbm_with_dividends' needs strictly positive dividend times and \
                     finite non-negative dividends, got {dividend:?} at time {time}"
                )));
            }
            let process = GbmWithDividends::new(
                GbmParams::new(params.r, params.q, params.sigma)?,
                dividends.clone(),
            )?;
            require_domain(spec, &process, f64::MIN_POSITIVE)?;
            match spec.scheme {
                SchemeSpec::Default => {
                    simulate(&process, &ExactGbmWithDividends, spec, x0, parallel)
                }
                _ => unsupported(),
            }
        }
        ProcessSpec::MultiGbm {
            assets,
            correlation,
        } => {
            let assets = assets
                .iter()
                .map(|p| GbmParams::new(p.r, p.q, p.sigma))
                .collect::<Result<Vec<_>>>()?;
            let process = MultiGbmProcess::new(assets, correlation.clone())?;
            require_domain(spec, &process, f64::MIN_POSITIVE)?;
            match spec.scheme {
                SchemeSpec::Default => simulate(&process, &ExactMultiGbm, spec, x0, parallel),
                SchemeSpec::Euler => simulate(&process, &EulerMaruyama, spec, x0, parallel),
                SchemeSpec::LogEuler => simulate(&process, &LogEuler, spec, x0, parallel),
                SchemeSpec::Milstein => simulate(&process, &Milstein, spec, x0, parallel),
            }
        }
        ProcessSpec::Brownian(p) => {
            let process = BrownianProcess::new(BrownianParams::new(p.mu, p.sigma)?);
            match spec.scheme {
                SchemeSpec::Default | SchemeSpec::Euler => {
                    simulate(&process, &EulerMaruyama, spec, x0, parallel)
                }
                _ => unsupported(),
            }
        }
        ProcessSpec::MultiBrownian {
            mus,
            sigmas,
            correlation,
        } => {
            let process =
                MultiBrownianProcess::new(mus.clone(), sigmas.clone(), correlation.clone())?;
            match spec.scheme {
                SchemeSpec::Default | SchemeSpec::Euler => {
                    simulate(&process, &EulerMaruyama, spec, x0, parallel)
                }
                _ => unsupported(),
            }
        }
        ProcessSpec::MultiOu(p) => {
            p.validate()?;
            let process = MultiOuProcess::new(p.clone());
            match spec.scheme {
                SchemeSpec::Default | SchemeSpec::Euler => {
                    simulate(&process, &EulerMaruyama, spec, x0, parallel)
                }
                _ => unsupported(),
            }
        }
        ProcessSpec::HullWhite1F(p) => {
            let process = HullWhite1FProcess::new(p.clone());
            match spec.scheme {
                SchemeSpec::Default => {
                    simulate(&process, &ExactHullWhite1F::new(), spec, x0, parallel)
                }
                SchemeSpec::Euler => simulate(&process, &EulerMaruyama, spec, x0, parallel),
                _ => unsupported(),
            }
        }
        ProcessSpec::Cir(p) => {
            let process = CirProcess::with_params(p.kappa, p.theta, p.sigma)?;
            require_domain(spec, &process, 0.0)?;
            match spec.scheme {
                SchemeSpec::Default => simulate(&process, &QeCir::new(), spec, x0, parallel),
                SchemeSpec::Euler => simulate(&process, &EulerMaruyama, spec, x0, parallel),
                _ => unsupported(),
            }
        }
        ProcessSpec::CirPlusPlus {
            params,
            shift_curve,
            shift_times,
        } => {
            let process = CirPlusPlusProcess::new(
                CirProcess::with_params(params.kappa, params.theta, params.sigma)?,
                shift_curve.clone(),
                shift_times.clone(),
            )?;
            // The spec carries the short rate; the scheme advances the
            // unshifted factor `x = r - φ(t)`.
            let factor0: Vec<f64> = x0.iter().map(|r| r - process.shift_at_time(0.0)).collect();
            if factor0.iter().any(|x| x.is_nan() || *x < 0.0) {
                return Err(Error::Validation(format!(
                    "process 'cir_plus_plus' needs an initial short rate of at least the \
                     initial shift {}, got {x0:?}",
                    process.shift_at_time(0.0)
                )));
            }
            let mut summary = match spec.scheme {
                SchemeSpec::Default => simulate(&process, &QeCir::new(), spec, &factor0, parallel),
                SchemeSpec::Euler => simulate(&process, &EulerMaruyama, spec, &factor0, parallel),
                _ => unsupported(),
            }?;
            let stride = summary.times.len();
            for path in summary.values.chunks_mut(stride) {
                // Time zero is the caller's short rate exactly, not a
                // subtract-then-add round trip of it.
                path[0] = x0[0];
                for (rate, time) in path.iter_mut().zip(&summary.times).skip(1) {
                    *rate = process.actual_rate(*rate, *time);
                }
            }
            summary.factor_names = vec!["short_rate".to_string()];
            Ok(summary)
        }
        ProcessSpec::Heston(p) => {
            let process =
                HestonProcess::with_params(p.r, p.q, p.kappa, p.theta, p.sigma_v, p.rho, p.v0)?;
            if let [spot, variance] = *x0 {
                if spot.is_nan() || spot <= 0.0 {
                    return Err(Error::Validation(format!(
                        "process 'heston' needs a strictly positive initial spot, got {spot}"
                    )));
                }
                if variance.to_bits() != p.v0.to_bits() {
                    return Err(Error::Validation(format!(
                        "process 'heston' needs initial_state[1] to equal the v0 parameter \
                         {}, got {variance}",
                        p.v0
                    )));
                }
            }
            match spec.scheme {
                SchemeSpec::Default => simulate(&process, &QeHeston::new(), spec, x0, parallel),
                SchemeSpec::Euler => simulate(&process, &EulerMaruyama, spec, x0, parallel),
                _ => unsupported(),
            }
        }
        ProcessSpec::SchwartzSmith(p) => {
            let params = SchwartzSmithParams::new(p.kappa, p.sigma_x, p.mu_y, p.sigma_y, p.rho_xy)?
                .with_lambda_x(p.lambda_x)?;
            // The process records its starting point; the length is checked
            // again, with a precise message, inside `simulate`.
            let process = SchwartzSmithProcess::new(
                params,
                x0.first().copied().unwrap_or(f64::NAN),
                x0.get(1).copied().unwrap_or(f64::NAN),
            );
            match spec.scheme {
                SchemeSpec::Default => {
                    let scheme = ExactSchwartzSmith::from_process(&process)?;
                    simulate(&process, &scheme, spec, x0, parallel)
                }
                SchemeSpec::Euler => simulate(&process, &EulerMaruyama, spec, x0, parallel),
                _ => unsupported(),
            }
        }
    }
}

/// Serialized tag of a scheme, used in error messages.
fn scheme_tag(scheme: SchemeSpec) -> &'static str {
    match scheme {
        SchemeSpec::Default => "default",
        SchemeSpec::Euler => "euler",
        SchemeSpec::LogEuler => "log_euler",
        SchemeSpec::Milstein => "milstein",
    }
}

/// Reject an initial state with a component below `lower`.
///
/// `lower` is `f64::MIN_POSITIVE` for strictly positive states and `0.0` for
/// non-negative ones. A wrong length is left to [`simulate`], which reports
/// it against the process dimension.
fn require_domain<P: StochasticProcess>(
    spec: &PathSimulationSpec,
    process: &P,
    lower: f64,
) -> Result<()> {
    let x0 = &spec.initial_state;
    if x0.len() == process.dim() && x0.iter().any(|x| x.is_nan() || *x < lower) {
        return Err(Error::Validation(format!(
            "process '{}' needs every initial state component to be {}, got {x0:?}",
            spec.process.tag(),
            if lower > 0.0 {
                "strictly positive"
            } else {
                "non-negative"
            }
        )));
    }
    Ok(())
}

/// Per-worker scratch buffers for one stream (and its antithetic partner).
struct Scratch {
    state: Vec<f64>,
    work: Vec<f64>,
    state_anti: Vec<f64>,
    work_anti: Vec<f64>,
    z: Vec<f64>,
    z_raw: Vec<f64>,
}

/// Simulate `spec.num_paths` streams of one concrete process and scheme.
///
/// Draws follow the generic engine exactly: stream `p` is the Philox
/// substream `p`, each step draws one vector of standard normals, the
/// engine-side Cholesky factor is applied unless the scheme correlates
/// internally, and an antithetic partner reuses the negated correlated draws.
fn simulate<P, D>(
    process: &P,
    scheme: &D,
    spec: &PathSimulationSpec,
    initial_state: &[f64],
    parallel: bool,
) -> Result<PathSummary>
where
    P: StochasticProcess + ProcessMetadata,
    D: Discretization<P> + Clone,
{
    let dim = process.dim();
    let factor_names = process.metadata().factor_names;
    if dim == 0 || factor_names.len() != dim {
        return Err(Error::Validation(format!(
            "process '{}' reports dimension {dim} but {} factor names",
            spec.process.tag(),
            factor_names.len()
        )));
    }
    if process.requires_injected_noise() {
        return Err(Error::Validation(format!(
            "process '{}' needs externally injected noise and cannot be driven by \
             independent normal draws",
            spec.process.tag()
        )));
    }
    if initial_state.len() != dim || initial_state.iter().any(|x| !x.is_finite()) {
        return Err(Error::Validation(format!(
            "initial_state must hold {dim} finite values for process '{}', got {:?}",
            spec.process.tag(),
            spec.initial_state
        )));
    }
    McEngine::validate_scheme_pairing(process, scheme)?;

    if spec.num_paths == 0 || spec.num_paths > MAX_CAPTURED_PATHS {
        return Err(Error::Validation(format!(
            "num_paths must be in 1..={MAX_CAPTURED_PATHS}, got {}",
            spec.num_paths
        )));
    }
    let legs = if spec.antithetic { 2 } else { 1 };
    let num_simulated_paths = spec.num_paths * legs;
    let num_steps = spec.time_grid.num_steps();
    if num_steps == 0 {
        return Err(Error::Validation(
            "time_grid must contain at least one simulation step".to_string(),
        ));
    }
    let too_large = || {
        Error::Validation(format!(
            "simulated paths exceed {MAX_STORED_VALUES} stored values"
        ))
    };
    let stride = num_steps.checked_add(1).ok_or_else(too_large)?;
    // Paths plus the shared time-grid storage, bounded before either is
    // allocated.
    num_simulated_paths
        .checked_mul(dim)
        .and_then(|rows| rows.checked_add(3))
        .and_then(|rows| rows.checked_mul(stride))
        .filter(|&values| values <= MAX_STORED_VALUES)
        .ok_or_else(too_large)?;

    let time_grid = spec.time_grid.build()?;
    process.validate_time_grid(&time_grid)?;
    let mut scheme = scheme.clone();
    scheme.prepare(process, &time_grid);
    let scheme = &scheme;
    let correlation = build_correlation_factor(process, scheme)?;
    let rng = PhiloxRng::new(spec.seed);

    let path_len = stride * dim;
    let num_factors = process.num_factors();
    let work_size = scheme.work_size(process);
    let new_scratch = || Scratch {
        state: vec![0.0; dim],
        work: vec![0.0; work_size],
        state_anti: vec![0.0; dim * (legs - 1)],
        work_anti: vec![0.0; work_size * (legs - 1)],
        z: vec![0.0; num_factors],
        z_raw: vec![
            0.0;
            if correlation.is_some() {
                num_factors
            } else {
                0
            }
        ],
    };
    let record = |path: &mut [f64], step: usize, state: &[f64], stream: usize| -> Result<()> {
        if let Some(factor) = state.iter().position(|x| !x.is_finite()) {
            return Err(Error::Validation(format!(
                "non-finite {} on stream {stream} at step {step}",
                factor_names[factor]
            )));
        }
        path[step * dim..(step + 1) * dim].copy_from_slice(state);
        Ok(())
    };
    // `block` holds the stream's path followed by its antithetic partner.
    let fill = |s: &mut Scratch, (stream, block): (usize, &mut [f64])| -> Result<()> {
        let mut path_rng = rng.substream(stream as u64);
        let (primary, mirrored) = block.split_at_mut(path_len);
        s.state.copy_from_slice(initial_state);
        s.work.fill(0.0);
        primary[..dim].copy_from_slice(initial_state);
        if spec.antithetic {
            s.state_anti.copy_from_slice(initial_state);
            s.work_anti.fill(0.0);
            mirrored[..dim].copy_from_slice(initial_state);
        }
        for step in 0..num_steps {
            let (t, dt) = (time_grid.time(step), time_grid.dt(step));
            match &correlation {
                Some(factor) => {
                    path_rng.fill_std_normals(&mut s.z_raw);
                    factor.apply(&s.z_raw, &mut s.z).map_err(|error| {
                        Error::Validation(format!("failed to correlate shocks: {error}"))
                    })?;
                }
                None => path_rng.fill_std_normals(&mut s.z),
            }
            scheme.step(process, t, dt, &mut s.state, &s.z, &mut s.work);
            record(primary, step + 1, &s.state, stream)?;
            if spec.antithetic {
                for z in &mut s.z {
                    *z = -*z;
                }
                scheme.step(process, t, dt, &mut s.state_anti, &s.z, &mut s.work_anti);
                record(mirrored, step + 1, &s.state_anti, stream)?;
            }
        }
        Ok(())
    };

    let mut values = vec![0.0; num_simulated_paths * path_len];
    let block_len = legs * path_len;
    #[cfg(not(target_arch = "wasm32"))]
    if parallel {
        use rayon::prelude::*;
        values
            .par_chunks_mut(block_len)
            .enumerate()
            .try_for_each_init(new_scratch, fill)?;
    } else {
        let mut scratch = new_scratch();
        values
            .chunks_mut(block_len)
            .enumerate()
            .try_for_each(|block| fill(&mut scratch, block))?;
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = parallel;
        let mut scratch = new_scratch();
        values
            .chunks_mut(block_len)
            .enumerate()
            .try_for_each(|block| fill(&mut scratch, block))?;
    }

    // `time + dt` rather than the stored knots: this is the time the engine
    // stamps on each captured point.
    let times = std::iter::once(0.0)
        .chain((0..num_steps).map(|step| time_grid.time(step) + time_grid.dt(step)))
        .collect();
    Ok(PathSummary {
        num_paths: spec.num_paths,
        num_simulated_paths,
        dim,
        times,
        factor_names,
        values,
    })
}
