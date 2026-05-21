## Math Module (core)

The `math` module in `finstack-core` provides deterministic numerical building
blocks used across curves, cashflows, valuations, scenarios, and portfolio
analytics. It includes interpolation, solvers, integration, statistics,
random-number utilities, and finance-oriented helpers.

- **Root finding and optimization**: 1D and multi‑dimensional solvers for pricing and calibration
- **Integration and interpolation**: Quadrature rules and curve interpolation for term structures
- **Distributions and random numbers**: Probability functions and RNG traits for Monte Carlo
- **Probability utilities**: Joint probabilities and bounds for correlated Bernoulli variables
- **Linear algebra and statistics**: Correlation, Cholesky, and time‑series statistics
- **Time grids**: Year‑fraction grids for simulation time stepping
- **Special functions and numerically stable summation**: Normal distribution, error function, and compensated summation utilities

`finstack_core::math` re-exports the common entry points from `mod.rs`:

- **Root finding**: `Solver`, `NewtonSolver`, `BrentSolver`, `BracketHint`
- **Multi-dimensional optimization**: `LevenbergMarquardtSolver`, `AnalyticalDerivatives`
- **Integration**: `GaussHermiteQuadrature`, `GaussLaguerreQuadrature`, `gauss_legendre_integrate`, `adaptive_simpson`, `trapezoidal_rule`, `simpson_rule`
- **Interpolation**: `LinearDf`, `LogLinearDf`, `MonotoneConvex`, `CubicHermite`, `InterpFn`, `ExtrapolationPolicy`
- **Compounding**: `Compounding` (rate convention conversions)
- **Linear algebra**: `cholesky_decomposition`, `apply_correlation`, `CholeskyError`
- **Random numbers**: `RandomNumberGenerator`, `Pcg64Rng`, `SobolRng`, `box_muller_transform`
- **Distributions**: `binomial_distribution`, `binomial_probability`, `sample_beta`, and related helpers
- **Probability**: `joint_probabilities`, `correlation_bounds`, `CorrelatedBernoulli`
- **Special functions**: `erf`, `norm_cdf`, `norm_pdf`, `standard_normal_inv_cdf`
- **Statistics**: `mean`, `variance`, `covariance`, `correlation`, `OnlineStats`, `OnlineCovariance`, `required_samples`
- **Summation**: `kahan_sum`, `neumaier_sum`, `NeumaierAccumulator`
- **Time grids**: `TimeGrid`, `TimeGridError`

Additional types (`BrownianBridge`, `pca_ordering`, volatility models, etc.) live
in submodules — import them explicitly when needed.

---

## Module Structure

- **`mod.rs`**
  - Public entrypoint for the math module.
  - Re‑exports high‑level numerical primitives (solvers, integrators, interpolation wrappers, stats, etc.).
  - Documents the primary entry points for root finding and basic statistics.
- **`solver.rs`**
  - 1D root‑finding interfaces and implementations:
    - `trait Solver`: generic trait for solving `f(x) = 0` from an initial guess.
    - `NewtonSolver`: Newton–Raphson with scale‑adaptive finite‑difference derivatives and optional analytic derivative via `solve_with_derivative`.
    - `BrentSolver`: robust bracketing method combining bisection, secant, and inverse quadratic interpolation.
  - Typical use cases:
    - Implied volatility and option Greeks.
    - Yield‑to‑maturity and spread solving.
    - IRR/XIRR and general scalar root‑finding in pricing routines.
- **`solver_multi.rs`**
  - Multi‑dimensional optimization and calibration:
    - `trait MultiSolver`: interface for minimizing scalar objectives and solving systems via least‑squares.
    - `trait AnalyticalDerivatives`: optional analytic gradient/Jacobian support for calibration.
    - `LevenbergMarquardtSolver`: damped least‑squares algorithm for non‑linear least‑squares problems.
  - Used by calibration and curve/surface fitting routines (e.g., SABR, Heston, multi‑curve bootstrapping).
- **`integration.rs`**
  - Deterministic quadrature and integration utilities:
    - `GaussHermiteQuadrature` with pre‑computed nodes/weights and `integrate` helpers for expectations under the standard normal.
    - Gauss–Legendre routines (`gauss_legendre_integrate`, `gauss_legendre_integrate_adaptive`, `gauss_legendre_integrate_composite`) for finite‑interval integrals.
    - Classic rules (`trapezoidal_rule`, `simpson_rule`, `adaptive_simpson`) for general one‑dimensional integration.
  - Typical use cases:
    - Fourier/integral‑based option pricing (Heston, characteristic‑function models).
    - Risk‑neutral expectations and probability integrals.
- **`interp/`**
  - Interpolation framework for yield curves and term structures:
    - `generic`: generic `Interpolator` container wrapping different interpolation strategies.
    - `strategies`: concrete interpolation algorithms.
    - `traits`: core `InterpolationStrategy` and `InterpFn` traits.
    - `types`: configuration types such as `InterpStyle`, `ExtrapolationPolicy`, and numerical constants.
    - `utils`: shared validation and search helpers.
    - `wrappers`: public user‑facing wrapper types:
      - `LinearDf`: linear interpolation on discount factors (simple baseline, may create arbitrage).
      - `LogLinearDf`: log‑linear DF interpolation (constant forwards, positive DF).
      - `MonotoneConvex`: Hagan–West monotone convex scheme (no‑arbitrage, positive forwards).
      - `CubicHermite`: PCHIP‑style shape‑preserving cubic for smooth curves when data is monotone.
  - Reused by curve builders in `market_data::term_structures` and pricing logic in `valuations`.
- **`distributions.rs`**
  - Probability distributions and sampling helpers:
    - Binomial:
      - `binomial_probability`: PMF `P(X = k)` with log‑space computation and Stirling approximation for large `n`.
      - `binomial_distribution`: full `{P(X=0..n)}` vector, normalized defensively.
      - `log_binomial_coefficient`, `log_factorial`: numerically stable combinatorics.
    - Beta:
      - `sample_beta`: Beta(α, β) sampling using a `RandomNumberGenerator`.
  - Use cases:
    - Credit portfolio loss distributions, default counting models.
    - Recovery‑rate and correlation priors in Bayesian/Monte Carlo frameworks.
- **`random.rs`**
  - Random number generation:
    - `RandomNumberGenerator`: trait (`uniform`, `normal`, `bernoulli`) for pluggable RNGs.
    - `Pcg64Rng`: deterministic PCG64 generator with seed and stream accessors.
    - `box_muller_transform`: Box–Muller transform producing two independent `N(0,1)` samples from uniform inputs.
  - Usage:
    - Use `Pcg64Rng::new(seed)` for deterministic, reproducible simulations.
    - Use `Pcg64Rng::new_with_stream(seed, stream)` for parallel Monte Carlo with independent streams.
- **`linalg.rs`**
  - Small linear‑algebra utilities for covariance and correlation matrices:
    - `cholesky_decomposition`: factorization `Σ = L Lᵀ` with robust error reporting via `CholeskyError`.
    - `apply_correlation`: transforms independent `N(0,1)` shocks into correlated shocks via Cholesky factor.
    - Helpers for building and validating correlation matrices.
  - Primary use cases:
    - Monte Carlo simulation of correlated asset paths.
    - Portfolio risk and factor models.
    - Copula‑based credit and structured products.
- **`stats.rs`**
  - Time‑series and cross‑sectional statistics:
    - `mean`, `variance`, `mean_var`: basic statistics with Kahan summation and Welford’s algorithm.
    - `covariance`, `correlation`: Chan/Welford style numerically stable covariance/correlation.
    - `moment_match`: mean/variance matching for variance reduction.
    - Online estimators:
      - `OnlineStats`, `OnlineCovariance` with confidence intervals and merge support.
      - `required_samples` for confidence‑targeted sample sizing.
    - Realized variance utilities:
      - `RealizedVarMethod` with variants like `CloseToClose`, `Parkinson`, `GarmanKlass`, `RogersSatchell`, `YangZhang`.
      - `log_returns`, `realized_variance`, `realized_variance_ohlc`.
  - Used for volatility estimation, correlation matrices, and variance‑based risk statistics.
- **`summation.rs`**
  - Numerically stable summation algorithms:
    - `kahan_sum`: compensated summation with error tracking.
    - `neumaier_sum`: improved compensated summation for mixed‑sign data.
    - `pairwise_sum`: divide‑and‑conquer summation for better numerical behavior.
    - `stable_sum`: determinism‑aware “default” sum using Neumaier summation.
  - Underpins higher‑level statistics and financial sums where order‑sensitivity of naive summation would be problematic.
- **`special_functions.rs`**
  - Special functions commonly used in finance:
    - `erf`: error function.
    - `norm_cdf`: standard normal CDF Φ.
    - `norm_pdf`: standard normal PDF φ.
    - `standard_normal_inv_cdf`: inverse standard normal CDF Φ⁻¹.
  - Thin wrappers around the `statrs` crate to provide accurate, deterministic implementations with good tail behavior.
- **`probability.rs`**
  - Joint probability helpers for correlated Bernoulli variables:
    - `joint_probabilities` with Fréchet‑Hoeffding clamping.
    - `correlation_bounds` for feasible correlation ranges.
    - `CorrelatedBernoulli` distribution helper.
- **`time_grid.rs`**
  - Year‑fraction time grids for Monte Carlo and lattice time stepping:
    - `TimeGrid::uniform`, `TimeGrid::from_times`, validation and accessors.
    - Date/time mapping helpers: `map_date_to_step`, `map_dates_to_steps`, `map_exercise_dates_to_steps`.

---

## Core Concepts and Types

### Solvers and Optimization

- **`Solver`** (1D root‑finding):
  - `fn solve<F>(&self, f: F, initial_guess: f64) -> Result<f64>`
  - Implemented by `NewtonSolver` (with finite‑difference derivative) and `BrentSolver` (robust bracketed).
- **`NewtonSolver`**:
  - Adaptive finite‑difference derivative, configurable tolerance/iteration/step limits.
  - `solve_with_derivative` lets callers supply an analytic derivative.
- **`LevenbergMarquardtSolver`**:
  - `minimize` for scalar objectives with optional box constraints.
  - `solve_system_with_dim_stats` for residual-based systems.
  - `solve_system_with_jacobian_stats` when an analytic Jacobian is available.
  - `AnalyticalDerivatives` supplies exact gradients/Jacobians when available.

These solvers are used by IRR/XIRR, implied-volatility, and calibration code.

### Integration and Interpolation

- **Integration**:
  - Use `GaussHermiteQuadrature::new(order)?` (supported orders: 5, 7, 10, 15, 20) and `.integrate` for expectations under a standard normal.
  - Use `gauss_legendre_integrate*`, `trapezoidal_rule`, `simpson_rule`, or `adaptive_simpson` for scalar integrals on finite intervals.
- **Interpolation**:
  - Use `LinearDf`, `LogLinearDf`, `MonotoneConvex`, or `CubicHermite` as high‑level interpolation wrappers.
  - `InterpFn` and `InterpolationStrategy` allow generic interpolation strategies to be swapped without changing calling code.
  - `ExtrapolationPolicy` controls behavior outside the knot range (e.g., flat, clamp, error).

### Random Numbers, Distributions, and Linear Algebra

- **Random numbers**:
  - `RandomNumberGenerator` defines the RNG surface used by simulation helpers.
  - `Pcg64Rng` wraps PCG64 and provides deterministic seed/stream construction.
  - `box_muller_transform` is the canonical helper for turning uniforms into standard normals.
- **Distributions**:
  - Binomial and Beta implementations are tailored for financial use (credit portfolios, recovery modeling).
  - Log‑space computations are used wherever necessary to maintain numerical stability.
- **Linear algebra and correlation**:
  - `cholesky_decomposition` is the main entry point for decomposing correlation/covariance matrices.
  - `apply_correlation` turns independent shocks into correlated ones using the Cholesky factor.

### Statistics and Summation

- **Statistics**:
  - `mean`, `variance`, `covariance`, `correlation` avoid catastrophic cancellation via Kahan/Welford/Chan algorithms.
  - Realized variance methods support both simple price series and OHLC data.
- **Summation**:
  - `kahan_sum` and `neumaier_sum` provide compensated summation.
  - `NeumaierAccumulator` supports incremental aggregation.

---

## Usage Examples

### 1. 1D Root Finding (Implied Rate or Vol)

```rust
use finstack_core::math::solver::{NewtonSolver, Solver};

// Solve x^2 - 2 = 0 using Newton-Raphson
let solver = NewtonSolver::new().with_tolerance(1e-10);
let f = |x: f64| x * x - 2.0;
let root = solver.solve(f, 1.0)?;
assert!((root - 2.0_f64.sqrt()).abs() < 1e-10);
# Ok::<(), finstack_core::Error>(())
```

When an analytic derivative is available, use `solve_with_derivative`:

```rust
use finstack_core::math::solver::NewtonSolver;

let solver = NewtonSolver::new();
let f = |x: f64| x * x - 2.0;
let f_prime = |x: f64| 2.0 * x;

let root = solver.solve_with_derivative(f, f_prime, 1.0)?;
assert!((root - 2.0_f64.sqrt()).abs() < 1e-10);
# Ok::<(), finstack_core::Error>(())
```

### 2. Multi‑Dimensional Calibration with Levenberg–Marquardt

```rust
use finstack_core::math::solver_multi::{LevenbergMarquardtSolver, MultiSolver};

// Minimize (x-2)^2 + (y-3)^2
let solver = LevenbergMarquardtSolver::new().with_tolerance(1e-8);

let objective = |params: &[f64]| -> f64 {
    (params[0] - 2.0).powi(2) + (params[1] - 3.0).powi(2)
};

let initial = vec![0.0, 0.0];
let result = solver.minimize(objective, &initial, None)?;
assert!((result[0] - 2.0).abs() < 1e-6);
assert!((result[1] - 3.0).abs() < 1e-6);
# Ok::<(), finstack_core::Error>(())
```

### 3. Gauss–Hermite Integration under a Normal Distribution

```rust
use finstack_core::math::integration::GaussHermiteQuadrature;

// Integrate x^2 under standard normal: E[X^2] = 1
let quad = GaussHermiteQuadrature::new(7)?;
let integral = quad.integrate(|x| x * x);
assert!((integral - 1.0).abs() < 1e-3);
# Ok::<(), finstack_core::Error>(())
```

### 4. Interpolating a Discount Curve

```rust
use finstack_core::math::interp::{LogLinearDf, ExtrapolationPolicy};

// Simple log-linear DF curve
let times = vec![0.0, 1.0, 2.0, 3.0];
let dfs = vec![1.0, 0.98, 0.95, 0.90];

let interp = LogLinearDf::new(times, dfs, ExtrapolationPolicy::Flat)?;

let df_18m = interp.interpolate(1.5)?;
assert!(df_18m < 1.0 && df_18m > 0.0);
# Ok::<(), finstack_core::Error>(())
```

### 5. Correlated Shocks via Cholesky

```rust
use finstack_core::math::linalg::{cholesky_decomposition, apply_correlation};

// 2x2 correlation matrix
let corr = vec![1.0, 0.5, 0.5, 1.0];
let chol = cholesky_decomposition(&corr, 2)?;

let z = vec![1.0, 0.0];      // independent N(0,1)
let mut z_corr = vec![0.0; 2];
apply_correlation(&chol, &z, &mut z_corr);
// z_corr now has correlation ~0.5
# Ok::<(), finstack_core::Error>(())
```

### 6. Random Numbers and Beta Sampling

```rust
use finstack_core::math::random::{Pcg64Rng, RandomNumberGenerator};
use finstack_core::math::distributions::sample_beta;

let mut rng = Pcg64Rng::new(42);
let u = rng.uniform();
assert!((0.0..1.0).contains(&u));

// Sample a recovery rate from Beta(4, 2)
let recovery = sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 4.0, 2.0);
assert!(recovery >= 0.0 && recovery <= 1.0);
```

### 7. Basic Statistics and Realized Volatility

```rust
use finstack_core::math::{mean, variance, covariance, correlation};

let xs = [1.0, 2.0, 3.0, 4.0];
let ys = [2.0, 4.0, 6.0, 8.0];

assert!((mean(&xs) - 2.5).abs() < 1e-12);
assert!((correlation(&xs, &ys) - 1.0).abs() < 1e-12);
```

For realized variance:

```rust
use finstack_core::math::stats::{realized_variance, RealizedVarMethod};

let prices = vec![100.0, 101.0, 99.0, 102.0];
let rv = realized_variance(&prices, RealizedVarMethod::CloseToClose, 252.0);
assert!(rv >= 0.0);
```

`RealizedVarMethod::CloseToClose` uses the mean squared log-return estimator
rather than sample variance, and the Yang-Zhang implementation includes
overnight, open-to-close, and Rogers-Satchell components in the standard
weighting formula.

---

## Extending

Add solvers to `solver.rs` or `solver_multi.rs`, interpolation strategies under
`interp/`, and distribution helpers to `distributions.rs`. Public APIs return
`crate::Result<T>`. Prefer numerically stable algorithms (Kahan/Neumaier,
Welford/Chan) and include unit tests with known analytic values.
