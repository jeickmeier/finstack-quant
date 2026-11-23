//! LSMC configuration, exercise policy, and path-matrix storage types.

use super::super::super::results::MoneyEstimate;
use crate::monte_carlo::pricer::basis::BasisFunctions;
use crate::monte_carlo::process::gbm::GbmProcess;
use crate::types::OptionType;
use finstack_quant_core::Result;
use finstack_quant_core::math::solver::BrentSolver;

pub(super) const MAX_LSMC_PATH_VALUES: usize = 64_000_000;
pub(super) const MAX_LSMC_STEPS: usize = 100_000;

pub(super) fn checked_path_values(
    num_paths: usize,
    num_steps: usize,
    antithetic: bool,
) -> Result<usize> {
    if !(2..=crate::monte_carlo::engine::MAX_NUM_PATHS).contains(&num_paths) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "LSMC num_paths must be in 2..={} independent estimators, got {num_paths}",
            crate::monte_carlo::engine::MAX_NUM_PATHS
        )));
    }
    if num_steps == 0 || num_steps > MAX_LSMC_STEPS {
        return Err(finstack_quant_core::Error::Validation(format!(
            "LSMC num_steps must be in 1..={MAX_LSMC_STEPS}, got {num_steps}"
        )));
    }
    let values = num_paths
        .checked_mul(if antithetic { 2 } else { 1 })
        .and_then(|rows| {
            num_steps
                .checked_add(1)
                .and_then(|stride| rows.checked_mul(stride))
        })
        .filter(|&values| values <= MAX_LSMC_PATH_VALUES)
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "LSMC retained path storage exceeds {MAX_LSMC_PATH_VALUES} values \
             (num_paths={num_paths}, num_steps={num_steps}, antithetic={antithetic})"
            ))
        })?;
    Ok(values)
}

/// A frozen LSMC exercise policy fit on one path set, applicable to another.
///
/// Captures the per-exercise-date least-squares regression coefficients used to
/// approximate continuation values. Apply it via [`LsmcEngine::price_with_policy`](super::LsmcEngine::price_with_policy)
/// to a fresh, independent path set to recover an *out-of-sample* American option
/// price free of the in-sample regression bias.
///
/// Build one with [`LsmcEngine::fit_exercise_policy`](super::LsmcEngine::fit_exercise_policy).
#[derive(Debug, Clone)]
pub struct ExercisePolicy {
    /// Whether the training paths selected immediate exercise at time zero.
    /// Pricing paths replay this decision without comparing their sample mean
    /// with intrinsic value, preserving independence from policy fitting.
    pub exercise_at_start: bool,
    /// Per-exercise-step regression coefficients in (step, coefficients) pairs,
    /// sorted by ascending step for forward replay. Only dates strictly inside
    /// `(0, num_steps)` are stored; terminal exercise is always applied.
    pub coefficients_by_date: Vec<(usize, Vec<f64>)>,
    /// Number of basis functions used during fitting; the same basis must be
    /// passed to [`LsmcEngine::price_with_policy`](super::LsmcEngine::price_with_policy).
    pub num_basis: usize,
    /// Number of simulation steps in the training run; the pricing run must
    /// agree.
    pub num_steps: usize,
}

/// Continuation-regression diagnostics for one interior exercise date.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExerciseBoundaryPoint {
    /// Index on the uniform simulation grid, excluding inception and expiry.
    pub step: usize,
    /// Years from simulation inception.
    pub time: f64,
    /// Strike-adjacent exercise boundary within the observed in-the-money spots.
    /// Absent if a crossing cannot be established or the fit was skipped.
    pub critical_spot: Option<f64>,
    /// Number of training paths in the money before this regression.
    pub itm_paths: usize,
    /// Number of those paths selected for immediate exercise at this step by
    /// the backward regression, before any decision at an earlier step.
    pub exercised_paths: usize,
    /// Fitted weights in basis order, empty when the regression was skipped.
    pub continuation_coeffs: Vec<f64>,
}

/// Option price estimate and interior exercise-boundary diagnostics.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LsmcResult {
    /// Price in the specified currency, with sampling error and confidence interval.
    pub estimate: MoneyEstimate,
    /// Diagnostics from the training paths, ordered by ascending step.
    pub exercise_boundary: Vec<ExerciseBoundaryPoint>,
}

pub(super) struct TrainingResult {
    pub(super) values: Vec<f64>,
    pub(super) policy: ExercisePolicy,
    pub(super) exercise_boundary: Vec<ExerciseBoundaryPoint>,
}

pub(super) fn critical_spot<B: BasisFunctions + ?Sized>(
    spots: &mut [f64],
    exercise: &AmericanExercise,
    basis: &B,
    coefficients: &[f64],
    basis_vals: &mut [f64],
) -> Option<f64> {
    spots.sort_unstable_by(f64::total_cmp);
    let mut gap = |spot| {
        basis.evaluate(spot, basis_vals);
        let continuation: f64 = coefficients
            .iter()
            .zip(basis_vals.iter())
            .map(|(a, b)| a * b)
            .sum();
        exercise.exercise_value(spot) - continuation
    };
    // The in-the-money region is one-sided. Traverse from the strike so
    // the first bracket is the strike-adjacent one even with multiple roots.
    let ordered = (0..spots.len()).map(|i| match exercise.option_type {
        OptionType::Call => spots[i],
        OptionType::Put => spots[spots.len() - 1 - i],
    });
    let mut previous: Option<(f64, f64)> = None;
    for spot in ordered {
        let value = gap(spot);
        if !value.is_finite() {
            previous = None;
            continue;
        }
        if value == 0.0 {
            return Some(spot);
        }
        if let Some((prior_spot, prior_value)) = previous
            && value.is_sign_positive() != prior_value.is_sign_positive()
        {
            let tolerance = (exercise.strike * 1e-10).max(f64::EPSILON);
            return BrentSolver::new(tolerance, 100)
                .ok()?
                .solve_in_bracket(|x| Ok(gap(x)), prior_spot, spot)
                .ok();
        }
        previous = Some((spot, value));
    }
    None
}

/// Simulation and exercise inputs of one GBM LSMC run.
///
/// Shared by [`LsmcEngine::price`](super::LsmcEngine::price), [`LsmcEngine::fit_exercise_policy`](super::LsmcEngine::fit_exercise_policy),
/// [`LsmcEngine::price_with_policy`](super::LsmcEngine::price_with_policy) and [`LsmcEngine::price_unbiased`](super::LsmcEngine::price_unbiased); the
/// two passes of the unbiased workflow reuse one value.
pub struct LsmcInputs<'a, B: ?Sized> {
    /// GBM process driving the simulated spot paths.
    pub process: &'a GbmProcess,
    /// Positive, finite initial spot level in payoff price units.
    pub initial_spot: f64,
    /// Remaining maturity in years on the model time axis; the paths use a
    /// uniform grid of `num_steps` intervals over it.
    pub time_to_maturity: f64,
    /// Positive number of uniform time steps; also the last exercise date.
    pub num_steps: usize,
    /// Immediate exercise payoff (call or put intrinsic at a strike).
    pub exercise: &'a AmericanExercise,
    /// Regression basis used to approximate continuation values.
    pub basis: &'a B,
    /// Flat continuously compounded annual discount rate as a decimal,
    /// applied as `exp(-discount_rate * t)`.
    pub discount_rate: f64,
}

impl<B: ?Sized> Clone for LsmcInputs<'_, B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<B: ?Sized> Copy for LsmcInputs<'_, B> {}

#[derive(Clone, Copy)]
pub(super) struct PolicyTiming {
    pub(super) discount_rate: f64,
    pub(super) time_to_maturity: f64,
    pub(super) num_steps: usize,
}

/// Row-major flat storage for simulated spot paths.
///
/// Spot of path `i` at step `s` lives at `data[i * stride + s]`, with
/// `stride = num_steps + 1`. Backing all paths with a single allocation (rather
/// than a `Vec<Vec<f64>>`) removes one heap allocation per path and keeps the
/// backward-induction reads inside one contiguous buffer instead of chasing a
/// pointer per path.
pub(super) struct PathMatrix {
    pub(super) data: Vec<f64>,
    pub(super) stride: usize,
}

impl PathMatrix {
    /// Number of stored paths.
    #[inline]
    pub(super) fn num_paths(&self) -> usize {
        self.data.len().checked_div(self.stride).unwrap_or(0)
    }

    /// Borrow the full spot trajectory of path `path` (length `stride`).
    #[inline]
    pub(super) fn row(&self, path: usize) -> &[f64] {
        let base = path * self.stride;
        &self.data[base..base + self.stride]
    }

    /// Build a matrix from per-path row vectors (test helper).
    #[cfg(test)]
    pub(super) fn from_rows(rows: &[Vec<f64>]) -> Self {
        let stride = rows.first().map_or(0, Vec::len);
        let mut data = Vec::with_capacity(rows.len() * stride);
        for row in rows {
            data.extend_from_slice(row);
        }
        Self { data, stride }
    }
}

/// American call or put immediate-exercise payoff.
#[derive(Debug, Clone)]
pub struct AmericanExercise {
    /// Payoff direction: call pays `max(spot - strike, 0)`, put pays
    /// `max(strike - spot, 0)`
    pub option_type: OptionType,
    /// Strike price
    pub strike: f64,
}

impl AmericanExercise {
    /// Create a validated American exercise payoff with a finite, strictly
    /// positive strike.
    ///
    /// The payoff is `max(spot - strike, 0)` for a call and
    /// `max(strike - spot, 0)` for a put, in the same scalar unit as the
    /// simulated spot. This constructor does not attach currency, discounting,
    /// or exercise-date conventions; the LSMC pricer supplies those context
    /// inputs.
    ///
    /// # Arguments
    ///
    /// * `option_type` - Call or put exercise payoff.
    /// * `strike` - Exercise price in the simulated spot's units; finite and
    ///   strictly positive.
    ///
    /// # Errors
    ///
    /// Returns an error if `strike` is non-finite or `strike <= 0`.
    pub fn new(option_type: OptionType, strike: f64) -> finstack_quant_core::Result<Self> {
        if !strike.is_finite() || strike <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "strike must be finite and positive".to_string(),
            ));
        }
        Ok(Self {
            option_type,
            strike,
        })
    }

    /// Value of exercising immediately at `spot`.
    ///
    /// # Arguments
    ///
    /// * `spot` - Simulated underlying level in the strike's units.
    #[must_use]
    pub fn exercise_value(&self, spot: f64) -> f64 {
        self.option_type.intrinsic(spot, self.strike)
    }
}

/// LSMC configuration.
#[derive(Debug, Clone)]
pub struct LsmcConfig {
    /// Independent path estimators; at least two are required for sampling
    /// uncertainty. Each estimator averages two physical paths when antithetic.
    pub num_paths: usize,
    /// Random seed
    pub seed: u64,
    /// Exercise dates as step indices; include `0` to permit valuation-date exercise.
    pub exercise_dates: Vec<usize>,
    /// Use parallel execution
    pub use_parallel: bool,
    /// Pair each path with its antithetic counterpart (`Z` and `-Z`)
    pub antithetic: bool,
}

impl LsmcConfig {
    /// Create a validated LSMC configuration.
    ///
    /// Verifies that `num_paths >= 2`, `exercise_dates` is non-empty with
    /// non-negative step indices, `num_steps > 0`, and every date satisfies
    /// `0 <= date <= num_steps`. Dates are sorted and de-duplicated: a
    /// duplicate exercise date would run the same-step regression twice with
    /// already-exercised cashflows, corrupting the second pass.
    ///
    /// An index of `num_steps` corresponds to the terminal exercise. Note
    /// that the pricer **always** applies terminal exercise at `num_steps`
    /// (American boundary condition) whether or not it is listed — a
    /// Bermudan whose last exercise right ends strictly before maturity is
    /// not representable; set `num_steps` to the last exercise step instead.
    /// Immediate exercise at valuation is permitted only when `exercise_dates`
    /// contains `0`. In-sample pricing then floors at intrinsic; two-pass
    /// pricing freezes the time-zero decision using the training paths.
    ///
    /// Antithetic pairing (`Z` and `-Z` from the same draws) is taken from the
    /// registry default unless overridden with [`Self::with_antithetic`].
    ///
    /// # Errors
    ///
    /// Returns an error if `num_paths` is outside `2..=10_000_000`, no exercise
    /// date is supplied, `num_steps` is outside `1..=100_000`, any date exceeds
    /// `num_steps`, or retained paths exceed 64 million spot values (including
    /// time zero). Pricing revalidates this limit with the selected antithetic
    /// partners included. Duplicate dates are
    /// accepted but removed after sorting, and the registry supplies the
    /// default seed, parallel-execution, and antithetic settings.
    ///
    /// # Arguments
    ///
    /// * `num_paths` - Number of independent Monte Carlo estimators; at least
    ///   two are required to estimate sampling uncertainty.
    /// * `exercise_dates` - Non-empty exercise step indices in `0..=num_steps`;
    ///   include `0` to permit valuation-date exercise. Terminal payoff at
    ///   maturity remains the boundary condition even when omitted here.
    /// * `num_steps` - Positive number of uniform simulation intervals to maturity.
    pub fn new(
        num_paths: usize,
        exercise_dates: Vec<usize>,
        num_steps: usize,
    ) -> finstack_quant_core::Result<Self> {
        if num_paths < 2 {
            return Err(finstack_quant_core::Error::Validation(
                "num_paths must provide at least two independent estimators".to_string(),
            ));
        }
        if exercise_dates.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "exercise_dates must have at least one element".to_string(),
            ));
        }
        if num_steps == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "num_steps must be positive".to_string(),
            ));
        }
        if let Some(&bad) = exercise_dates.iter().find(|&&d| d > num_steps) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "exercise_dates contain {bad} which exceeds num_steps={num_steps}; each date \
                 must satisfy 0 <= date <= num_steps"
            )));
        }
        let mut exercise_dates = exercise_dates;
        exercise_dates.sort_unstable();
        exercise_dates.dedup();

        let defaults = &crate::monte_carlo::registry::embedded_defaults()?.rust.lsmc;
        checked_path_values(num_paths, num_steps, false)?;
        Ok(Self {
            num_paths,
            seed: defaults.seed,
            exercise_dates,
            use_parallel: defaults.use_parallel,
            antithetic: defaults.antithetic,
        })
    }

    /// American convenience schedule: exercise at every simulated step
    /// `0..=num_steps`, including valuation and the terminal date.
    ///
    /// This is the exercise grid used by the host-binding `LsmcPricer` GBM
    /// helpers; both hosts delegate here rather than building the index vector
    /// themselves.
    ///
    /// # Arguments
    ///
    /// * `num_paths` - Independent path estimators; must be at least two.
    /// * `num_steps` - Time-grid steps between `0` and expiry; the returned
    ///   dates are `0..=num_steps`.
    ///
    /// # Errors
    ///
    /// Returns an error for the path, step, or retained-storage limits documented
    /// by [`Self::new`], before allocating the exercise schedule.
    pub fn every_step(num_paths: usize, num_steps: usize) -> finstack_quant_core::Result<Self> {
        checked_path_values(num_paths, num_steps, false)?;
        Self::new(num_paths, (0..=num_steps).collect(), num_steps)
    }

    /// Set random seed.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Enable or disable parallel path generation.
    ///
    /// Path generation is the dominant cost for large `num_paths`; when
    /// `enabled` is `true` the pricer uses a rayon par-iter and each path
    /// derives its own RNG via [`crate::monte_carlo::traits::RandomStream::split`] keyed
    /// on the path index, which keeps results bit-identical to the serial
    /// run.
    #[must_use]
    pub fn with_parallel(mut self, enabled: bool) -> Self {
        self.use_parallel = enabled;
        self
    }

    /// Enable or disable antithetic path pairing (`Z` and `-Z`).
    ///
    /// When enabled, each configured path is paired with its sign-flipped
    /// counterpart from the same Gaussian draws. The price estimator averages
    /// each pair, so [`Estimate::num_paths`](crate::monte_carlo::estimate::Estimate::num_paths) stays `num_paths` while
    /// [`Estimate::num_simulated_paths`](crate::monte_carlo::estimate::Estimate::num_simulated_paths) is `2 * num_paths`.
    ///
    /// # Arguments
    ///
    /// * `enabled` - `true` pairs every path with its sign-flipped counterpart
    ///   from the same Gaussian draws; `false` leaves the path count unchanged.
    #[must_use]
    pub fn with_antithetic(mut self, enabled: bool) -> Self {
        self.antithetic = enabled;
        self
    }
}
