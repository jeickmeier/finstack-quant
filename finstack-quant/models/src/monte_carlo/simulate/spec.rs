//! Serializable Monte Carlo path simulation specifications and summaries.

use super::fractional::FbmSpec;
use crate::monte_carlo::process::brownian::BrownianParams;
use crate::monte_carlo::process::cheyette_rough::CheyetteRoughVolParams;
use crate::monte_carlo::process::cir::CirParams;
use crate::monte_carlo::process::gbm::GbmParams;
use crate::monte_carlo::process::gbm_dividends::Dividend;
use crate::monte_carlo::process::heston::HestonProcessParams;
use crate::monte_carlo::process::lmm::LmmParams;
use crate::monte_carlo::process::local_vol::LocalVolParams;
use crate::monte_carlo::process::multi_ou::MultiOuParams;
use crate::monte_carlo::process::ou::HullWhite1FParams;
use crate::monte_carlo::process::rough_bergomi::RoughBergomiParams;
use crate::monte_carlo::process::rough_heston::RoughHestonParams;
use crate::monte_carlo::process::schwartz_smith::SchwartzSmithParams;
use finstack_quant_core::Result;
use finstack_quant_core::math::time_grid::TimeGrid;
use serde::{Deserialize, Serialize};

/// Upper bound on stored scalars: every path's states plus the shared time
/// grids. Checked before the time grid or any path buffer is allocated.
pub(crate) const MAX_STORED_VALUES: usize = 64_000_000;

/// Stochastic process to simulate, with its parameters.
///
/// Serialized as an internally tagged object: `{"type": "gbm", "r": 0.03, ...}`.
/// Parameters are re-validated when [`simulate_paths`](super::simulate_paths) runs, so a spec read
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
    /// Supply only the strictly positive opening spot in `initial_state`.
    /// The process derives variance from its `v0` parameter and returns states
    /// as `[spot, variance]`.
    Heston(HestonProcessParams),
    /// Schwartz-Smith two-factor commodity model.
    ///
    /// State: `[x, y]`, the short-term deviation and the long-term level of
    /// the log price; the spot is `exp(x + y)`.
    SchwartzSmith(SchwartzSmithParams),
    /// Dupire local volatility `dS = (r - q) S dt + σ_loc(t, S) S dW`, with
    /// `σ_loc` read from a local volatility surface at the simulation time
    /// and the current spot.
    ///
    /// State: `[spot]`, which must start strictly positive. The default
    /// scheme is `log_euler`; its discretization error falls with the step
    /// size, so use a fine time grid (about 100 steps per year).
    LocalVol(LocalVolParams),
    /// LIBOR market model: displaced-lognormal forward rates under the
    /// terminal measure, with piecewise-constant factor loadings.
    ///
    /// State: `[forward_0, forward_1, ...]`, simple forward rates as decimals,
    /// one per accrual period. `initial_state` must equal the
    /// `initial_forwards` parameter. A forward stops moving at its fixing
    /// date, and the time grid must contain every fixing date and volatility
    /// breakpoint inside the horizon.
    Lmm(LmmParams),
    /// Rough Bergomi stochastic volatility: lognormal variance driven by a
    /// Riemann-Liouville Volterra process, around the forward variance curve
    /// `xi`.
    ///
    /// State: `[spot]`, which must start strictly positive. The variance is a
    /// functional of the driver's history and is not part of the state.
    /// Consumes fractional noise; see [`FbmSpec`].
    RoughBergomi(RoughBergomiParams),
    /// Rough Heston stochastic volatility: a Volterra square-root variance
    /// with a power-law kernel, `hurst` in `(0, 0.5)`.
    ///
    /// Supply only the strictly positive opening spot in `initial_state`.
    /// The process derives variance from its `v0` parameter and returns states
    /// as `[spot, variance]`.
    /// At most 8,000 steps.
    RoughHeston(RoughHestonParams),
    /// Cheyette short-rate model with rough stochastic volatility.
    ///
    /// `sigma_base` holds point or ending-interval amplitudes in decimal rate
    /// units per square-root model year. For example, a flat 50 bp amplitude
    /// is `{"interpolation":"linear","amplitudes":[[0.0,0.005]]}`.
    ///
    /// State: `[x, y]`, the deviation of the short rate from the initial
    /// forward curve and the accumulated variance; both normally start at
    /// zero. The short rate at time `t` is `x + φ(t)`, with `φ` the initial
    /// forward curve in the parameters. Consumes fractional noise; see
    /// [`FbmSpec`].
    CheyetteRough(CheyetteRoughVolParams),
}

impl ProcessSpec {
    /// Serialized tag of the variant, used in error messages.
    pub(super) fn tag(&self) -> &'static str {
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
            Self::LocalVol(_) => "local_vol",
            Self::Lmm(_) => "lmm",
            Self::RoughBergomi(_) => "rough_bergomi",
            Self::RoughHeston(_) => "rough_heston",
            Self::CheyetteRough(_) => "cheyette_rough",
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
    /// the [module table](super#supported-pairs).
    #[default]
    Default,
    /// Euler-Maruyama: `X += μ Δt + σ √Δt Z`. First-order weak convergence;
    /// available for every process except `gbm_with_dividends`, `lmm`,
    /// `rough_bergomi`, `rough_heston` and `cheyette_rough`.
    Euler,
    /// Euler on the log state, which keeps the state positive. Only for
    /// `gbm`, `multi_gbm` and `local_vol`.
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
    pub(super) fn num_steps(&self) -> usize {
        match self {
            Self::Uniform { num_steps, .. } => *num_steps,
            Self::Times { times } => times.len().saturating_sub(1),
        }
    }

    pub(super) fn build(&self) -> Result<TimeGrid> {
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
    /// variant. Heston and rough Heston take only spot here and use their
    /// own `v0` for the starting variance.
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
    /// Fractional-noise generator for `rough_bergomi` and `cheyette_rough`;
    /// omitted selects `volterra`, the generator the pricers use. Setting it
    /// for any other process is a validation error.
    #[serde(default)]
    pub fbm: Option<FbmSpec>,
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
