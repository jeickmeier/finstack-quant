//! Process/scheme dispatch and path-simulation execution kernels.

use super::fractional::{FbmSpec, FractionalNoise};
use super::spec::*;
use crate::monte_carlo::discretization::cheyette_rough::CheyetteRoughEuler;
use crate::monte_carlo::discretization::euler::{EulerMaruyama, LogEuler};
use crate::monte_carlo::discretization::exact::{ExactGbm, ExactMultiGbm};
use crate::monte_carlo::discretization::exact_gbm_dividends::ExactGbmWithDividends;
use crate::monte_carlo::discretization::exact_hw1f::ExactHullWhite1F;
use crate::monte_carlo::discretization::lmm_predictor_corrector::LmmPredictorCorrector;
use crate::monte_carlo::discretization::milstein::Milstein;
use crate::monte_carlo::discretization::qe_cir::QeCir;
use crate::monte_carlo::discretization::qe_heston::QeHeston;
use crate::monte_carlo::discretization::rough_bergomi::RoughBergomiEuler;
use crate::monte_carlo::discretization::rough_heston::RoughHestonHybrid;
use crate::monte_carlo::discretization::schwartz_smith::ExactSchwartzSmith;
use crate::monte_carlo::engine::{
    MAX_CAPTURED_PATHS, McEngine, NoiseHook, build_correlation_factor, fill_shocks,
};
use crate::monte_carlo::process::brownian::{
    BrownianParams, BrownianProcess, MultiBrownianProcess,
};
use crate::monte_carlo::process::cheyette_rough::CheyetteRoughVolProcess;
use crate::monte_carlo::process::cir::{CirPlusPlusProcess, CirProcess};
use crate::monte_carlo::process::gbm::{GbmParams, GbmProcess, MultiGbmProcess};
use crate::monte_carlo::process::gbm_dividends::{Dividend, GbmWithDividends};
use crate::monte_carlo::process::heston::HestonProcess;
use crate::monte_carlo::process::lmm::LmmProcess;
use crate::monte_carlo::process::local_vol::{LocalVolParams, LocalVolProcess};
use crate::monte_carlo::process::multi_ou::MultiOuProcess;
use crate::monte_carlo::process::ou::HullWhite1FProcess;
use crate::monte_carlo::process::rough_bergomi::{RoughBergomiParams, RoughBergomiProcess};
use crate::monte_carlo::process::rough_heston::RoughHestonProcess;
use crate::monte_carlo::process::schwartz_smith::{SchwartzSmithParams, SchwartzSmithProcess};
use crate::monte_carlo::rng::philox::PhiloxRng;
use crate::monte_carlo::traits::{
    Discretization, ProcessMetadata, RandomStream, StochasticProcess,
};
use finstack_quant_core::{Error, Result};

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
/// * the scheme is not available for the process, or `fbm` is set for a
///   process that does not consume fractional noise;
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
///     fbm: None,
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
pub(super) fn dispatch(spec: &PathSimulationSpec, parallel: bool) -> Result<PathSummary> {
    let x0 = spec.initial_state.as_slice();
    let unsupported = || {
        Err(Error::Validation(format!(
            "scheme '{}' is not available for process '{}'",
            scheme_tag(spec.scheme),
            spec.process.tag()
        )))
    };
    if spec.fbm.is_some()
        && !matches!(
            spec.process,
            ProcessSpec::RoughBergomi(_) | ProcessSpec::CheyetteRough(_)
        )
    {
        return Err(Error::Validation(format!(
            "fbm applies only to processes 'rough_bergomi' and 'cheyette_rough', which consume \
             fractional noise; process '{}' does not",
            spec.process.tag()
        )));
    }
    match &spec.process {
        ProcessSpec::Gbm(p) => {
            let process = GbmProcess::new(GbmParams::new(p.r, p.q, p.sigma)?);
            require_domain(spec, &process, f64::MIN_POSITIVE)?;
            match spec.scheme {
                SchemeSpec::Default => {
                    simulate_with_noise(&process, &ExactGbm, spec, x0, parallel, None)
                }
                SchemeSpec::Euler => {
                    simulate_with_noise(&process, &EulerMaruyama, spec, x0, parallel, None)
                }
                SchemeSpec::LogEuler => {
                    simulate_with_noise(&process, &LogEuler, spec, x0, parallel, None)
                }
                SchemeSpec::Milstein => {
                    simulate_with_noise(&process, &Milstein, spec, x0, parallel, None)
                }
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
                    simulate_with_noise(&process, &ExactGbmWithDividends, spec, x0, parallel, None)
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
                SchemeSpec::Default => {
                    simulate_with_noise(&process, &ExactMultiGbm, spec, x0, parallel, None)
                }
                SchemeSpec::Euler => {
                    simulate_with_noise(&process, &EulerMaruyama, spec, x0, parallel, None)
                }
                SchemeSpec::LogEuler => {
                    simulate_with_noise(&process, &LogEuler, spec, x0, parallel, None)
                }
                SchemeSpec::Milstein => {
                    simulate_with_noise(&process, &Milstein, spec, x0, parallel, None)
                }
            }
        }
        ProcessSpec::Brownian(p) => {
            let process = BrownianProcess::new(BrownianParams::new(p.mu, p.sigma)?);
            match spec.scheme {
                SchemeSpec::Default | SchemeSpec::Euler => {
                    simulate_with_noise(&process, &EulerMaruyama, spec, x0, parallel, None)
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
                    simulate_with_noise(&process, &EulerMaruyama, spec, x0, parallel, None)
                }
                _ => unsupported(),
            }
        }
        ProcessSpec::MultiOu(p) => {
            p.validate()?;
            let process = MultiOuProcess::new(p.clone());
            match spec.scheme {
                SchemeSpec::Default | SchemeSpec::Euler => {
                    simulate_with_noise(&process, &EulerMaruyama, spec, x0, parallel, None)
                }
                _ => unsupported(),
            }
        }
        ProcessSpec::HullWhite1F(p) => {
            let process = HullWhite1FProcess::new(p.clone());
            match spec.scheme {
                SchemeSpec::Default => simulate_with_noise(
                    &process,
                    &ExactHullWhite1F::new(),
                    spec,
                    x0,
                    parallel,
                    None,
                ),
                SchemeSpec::Euler => {
                    simulate_with_noise(&process, &EulerMaruyama, spec, x0, parallel, None)
                }
                _ => unsupported(),
            }
        }
        ProcessSpec::Cir(p) => {
            let process =
                crate::monte_carlo::process::cir::CirParams::new(p.kappa, p.theta, p.sigma)
                    .map(CirProcess::new)?;
            require_domain(spec, &process, 0.0)?;
            match spec.scheme {
                SchemeSpec::Default => {
                    simulate_with_noise(&process, &QeCir::new(), spec, x0, parallel, None)
                }
                SchemeSpec::Euler => {
                    simulate_with_noise(&process, &EulerMaruyama, spec, x0, parallel, None)
                }
                _ => unsupported(),
            }
        }
        ProcessSpec::CirPlusPlus {
            params,
            shift_curve,
            shift_times,
        } => {
            let process = CirPlusPlusProcess::new(
                crate::monte_carlo::process::cir::CirParams::new(
                    params.kappa,
                    params.theta,
                    params.sigma,
                )
                .map(CirProcess::new)?,
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
                SchemeSpec::Default => {
                    simulate_with_noise(&process, &QeCir::new(), spec, &factor0, parallel, None)
                }
                SchemeSpec::Euler => {
                    simulate_with_noise(&process, &EulerMaruyama, spec, &factor0, parallel, None)
                }
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
            let process = crate::monte_carlo::process::heston::HestonProcessParams::new(
                p.r,
                p.q,
                p.model.kappa,
                p.model.theta,
                p.model.vol_of_vol,
                p.model.rho,
                p.model.v0,
            )
            .map(HestonProcess::new)?;
            let initial = heston_initial_state(spec, p.model.v0)?;
            match spec.scheme {
                SchemeSpec::Default => {
                    simulate_with_noise(&process, &QeHeston::new(), spec, &initial, parallel, None)
                }
                SchemeSpec::Euler => {
                    simulate_with_noise(&process, &EulerMaruyama, spec, &initial, parallel, None)
                }
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
                SchemeSpec::Default => simulate_with_noise(
                    &process,
                    &ExactSchwartzSmith::new(),
                    spec,
                    x0,
                    parallel,
                    None,
                ),
                SchemeSpec::Euler => {
                    simulate_with_noise(&process, &EulerMaruyama, spec, x0, parallel, None)
                }
                _ => unsupported(),
            }
        }
        ProcessSpec::LocalVol(p) => {
            let process = LocalVolProcess::new(LocalVolParams::new(p.r, p.q, p.surface.clone())?);
            require_domain(spec, &process, f64::MIN_POSITIVE)?;
            match spec.scheme {
                SchemeSpec::Default | SchemeSpec::LogEuler => {
                    simulate_with_noise(&process, &LogEuler, spec, x0, parallel, None)
                }
                SchemeSpec::Euler => {
                    simulate_with_noise(&process, &EulerMaruyama, spec, x0, parallel, None)
                }
                SchemeSpec::Milstein => unsupported(),
            }
        }
        ProcessSpec::Lmm(p) => {
            p.validate()?;
            let process = LmmProcess::new(p.clone());
            if x0.len() == p.initial_forwards.len()
                && x0
                    .iter()
                    .zip(&p.initial_forwards)
                    .any(|(x, forward)| x.to_bits() != forward.to_bits())
            {
                return Err(Error::Validation(format!(
                    "process 'lmm' needs initial_state to equal the initial_forwards parameter \
                     {:?}, got {x0:?}",
                    p.initial_forwards
                )));
            }
            match spec.scheme {
                SchemeSpec::Default => simulate_with_noise(
                    &process,
                    &LmmPredictorCorrector::new(),
                    spec,
                    x0,
                    parallel,
                    None,
                ),
                _ => unsupported(),
            }
        }
        ProcessSpec::RoughBergomi(p) => {
            let params = RoughBergomiParams::new(p.r, p.q, p.hurst, p.eta, p.rho, p.xi.clone())?;
            let process = RoughBergomiProcess::new(params);
            require_domain(spec, &process, f64::MIN_POSITIVE)?;
            match spec.scheme {
                SchemeSpec::Default => simulate_with_noise(
                    &process,
                    &RoughBergomiEuler::new(),
                    spec,
                    x0,
                    parallel,
                    Some((spec.fbm.unwrap_or_default(), p.hurst.get_value())),
                ),
                _ => unsupported(),
            }
        }
        ProcessSpec::RoughHeston(p) => {
            p.validate()?;
            let process = RoughHestonProcess::new(p.clone());
            let initial = heston_initial_state(spec, p.model.v0)?;
            if spec.scheme != SchemeSpec::Default {
                return unsupported();
            }
            // The scheme stores one kernel weight per pair of steps.
            let num_steps = spec.time_grid.num_steps();
            if num_steps
                .checked_mul(num_steps)
                .is_none_or(|weights| weights > MAX_STORED_VALUES)
            {
                return Err(Error::Validation(format!(
                    "process 'rough_heston' supports at most 8000 steps, got {num_steps}"
                )));
            }
            let grid = spec.time_grid.build()?;
            let scheme = RoughHestonHybrid::new(grid.times(), p.hurst.get_value())?;
            simulate_with_noise(&process, &scheme, spec, &initial, parallel, None)
        }
        ProcessSpec::CheyetteRough(p) => {
            p.validate()?;
            let process = CheyetteRoughVolProcess::new(p.clone());
            let mut summary = match spec.scheme {
                SchemeSpec::Default => simulate_with_noise(
                    &process,
                    &CheyetteRoughEuler::new(),
                    spec,
                    x0,
                    parallel,
                    Some((spec.fbm.unwrap_or_default(), p.hurst.get_value())),
                ),
                _ => unsupported(),
            }?;
            // The raw state, not the short rate `x + φ(t)` the process
            // metadata describes.
            summary.factor_names = vec!["x".to_string(), "y".to_string()];
            Ok(summary)
        }
    }
}

// Heston process parameters own the opening variance. Only spot is authored here.
pub(super) fn heston_initial_state(spec: &PathSimulationSpec, v0: f64) -> Result<[f64; 2]> {
    let tag = spec.process.tag();
    let [spot] = *spec.initial_state.as_slice() else {
        return Err(Error::Validation(format!(
            "process '{tag}' needs one initial_state value (spot); variance comes from v0"
        )));
    };
    if !spot.is_finite() || spot <= 0.0 {
        return Err(Error::Validation(format!(
            "process '{tag}' needs a finite strictly positive initial spot, got {spot}"
        )));
    }
    Ok([spot, v0])
}

/// Serialized tag of a scheme, used in error messages.
pub(super) fn scheme_tag(scheme: SchemeSpec) -> &'static str {
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
pub(super) fn require_domain<P: StochasticProcess>(
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
pub(super) struct Scratch {
    state: Vec<f64>,
    work: Vec<f64>,
    state_anti: Vec<f64>,
    work_anti: Vec<f64>,
    z: Vec<f64>,
    z_raw: Vec<f64>,
    /// Standard normals behind one path's fractional noise; empty without it.
    fbm_normals: Vec<f64>,
    /// One path's fractional increments, one per step; empty without them.
    fbm_increments: Vec<f64>,
}

/// Factor slot receiving the fractional increment of the step.
pub(super) const FBM_Z_INDEX: usize = 1;
/// Factor slot receiving the unit-variance normal that drives that increment.
pub(super) const DRIVE_Z_INDEX: usize = 2;

/// Simulate `spec.num_paths` streams of one concrete process and scheme.
///
/// Draws follow the generic engine exactly: stream `p` is the Philox
/// substream `p`, each step draws one vector of standard normals, the
/// engine-side Cholesky factor is applied unless the scheme correlates
/// internally, and an antithetic partner reuses the negated correlated draws.
///
/// `fractional` is the generator and Hurst exponent for a process that needs
/// injected noise. Such a stream follows
/// [`simulate_path_fractional`](crate::monte_carlo::engine::simulate_path_fractional)
/// as the pricers drive it: the generator's normals are drawn first, then each
/// step draws its vector of normals and has the fractional increment and its
/// driving normal written over factor slots 1 and 2. No engine-side
/// correlation is applied. The generators are linear in their normals, so the
/// antithetic partner still sees every Gaussian input negated.
pub(super) fn simulate_with_noise<P, D>(
    process: &P,
    scheme: &D,
    spec: &PathSimulationSpec,
    initial_state: &[f64],
    parallel: bool,
    fractional: Option<(FbmSpec, f64)>,
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
    if process.requires_injected_noise() != fractional.is_some()
        || (fractional.is_some() && process.num_factors() <= DRIVE_Z_INDEX)
    {
        return Err(Error::Validation(format!(
            "process '{}' and its noise source disagree on externally injected noise",
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
    let noise = fractional
        .map(|(fbm, hurst)| FractionalNoise::new(fbm, hurst, &spec.time_grid, &time_grid))
        .transpose()?;
    let noise = noise.as_ref();
    let correlation = match noise {
        Some(_) => None,
        None => build_correlation_factor(process, scheme)?,
    };
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
        fbm_normals: vec![0.0; noise.map_or(0, FractionalNoise::normals_len)],
        fbm_increments: vec![0.0; noise.map_or(0, |_| num_steps)],
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
        if let Some(noise) = noise {
            path_rng.fill_std_normals(&mut s.fbm_normals)?;
            noise.generate(&s.fbm_normals, &mut s.fbm_increments);
        }
        let hook = NoiseHook::Correlation(correlation.as_ref());
        for step in 0..num_steps {
            let (t, dt) = (time_grid.time(step), time_grid.dt(step));
            fill_shocks(&mut path_rng, &mut s.z, &mut s.z_raw, step, &hook)?;
            if let Some(noise) = noise {
                s.z[FBM_Z_INDEX] = s.fbm_increments[step];
                s.z[DRIVE_Z_INDEX] = noise.driving_normal(&s.fbm_normals, step);
            }
            scheme.step(process, t, dt, &mut s.state, &s.z, &mut s.work)?;
            record(primary, step + 1, &s.state, stream)?;
            if spec.antithetic {
                for z in &mut s.z {
                    *z = -*z;
                }
                scheme.step(process, t, dt, &mut s.state_anti, &s.z, &mut s.work_anti)?;
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

    let times = time_grid.times().to_vec();
    Ok(PathSummary {
        num_paths: spec.num_paths,
        num_simulated_paths,
        dim,
        times,
        factor_names,
        values,
    })
}
