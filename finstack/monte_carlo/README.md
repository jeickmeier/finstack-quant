# finstack-monte-carlo

`finstack-monte-carlo` is the Monte Carlo simulation crate in the `finstack`
workspace. It provides reusable pricing infrastructure for quantitative finance:
random streams, stochastic processes, discretization schemes, payoffs, pricing
engines, captured-path diagnostics, variance reduction, and Monte Carlo result
types with explicit currencies.

Composable traits: `RandomStream`, `StochasticProcess`, `Discretization`, `Payoff`,
and `McEngine`. Typical entry points: `prelude`, `engine::McEngine`, or
`pricer::european::EuropeanPricer` for GBM Europeans only.

## What This Crate Covers

Available functionality:

- Generic Monte Carlo pricing engine with serial and parallel execution
- Deterministic splittable pseudo-random generation via `rng::philox::PhiloxRng`
- Sobol / quasi-random generators and Brownian-bridge utilities
- GBM, Brownian, multi-GBM, OU, Heston, CIR, Hull-White, Vasicek,
  jump-diffusion, Bates, Schwartz-Smith, rough-volatility, and LMM-style
  process support
- Exact, Euler, Milstein, QE, jump-Euler, and model-specific discretizations
- Vanilla and path-dependent payoffs such as European, Asian, basket, barrier,
  and lookback options
- European, path-dependent, and Longstaff-Schwartz LSMC pricers, including
  a two-pass unbiased variant (`LsmcPricer::price_unbiased`) that mitigates
  the in-sample upward bias of single-pass LSMC by training the regression
  on one path set and replaying the frozen exercise policy on a second,
  independent path set
- Monte Carlo Greeks via pathwise, likelihood-ratio, and finite-difference
  estimators, including paired common-random-number variants
  (`finite_diff_delta_crn` / `finite_diff_gamma_crn`) report paired
  standard errors from a common-random-number bump loop (serial only)
- Online summary statistics and currency-aware pricing results
- Optional captured-path datasets for diagnostics and visualization
- Variance-reduction tools: antithetic pairing (engine-native) and control
  variates with closed-form Black-Scholes are always available

## Feature Flags

- `vr-experimental` (off by default): compiles the experimental
  `importance_sampling` and `moment_matching` variance-reduction modules.
  These estimators are not yet wired into the engine or pricers and are
  intended for offline development and evaluation rather than production
  pricing. Production paths use control variates and antithetic pairing,
  both of which are always available.

Rayon-backed parallel simulation is compiled by default; there is no opt-out
flag for it currently.

## Core Workflow

At a high level, pricing works like this:

1. Build an `McEngine` with a time grid and runtime options.
2. Choose an RNG, usually `PhiloxRng` for deterministic replay and parallel safety.
3. Select a `StochasticProcess` and a compatible `Discretization`.
4. Provide the initial process state as a raw `&[f64]`.
5. Choose or implement a `Payoff`.
6. Call `price()` for aggregate results or `price_with_capture()` for aggregate
   results plus captured paths.

The engine owns the simulation loop, Welford-style online statistics, optional
parallel chunking, optional early stopping, and optional path capture.

## Quick Start

### Generic Engine Example

This is the canonical way to use the crate when you want explicit control over
the engine, RNG, process, discretization, and payoff.

```rust,no_run
use finstack_core::currency::Currency;
use finstack_monte_carlo::prelude::*;

let engine = McEngine::builder()
    .num_paths(50_000)
    .seed(7)
    .uniform_grid(1.0, 252)
    .parallel(true)
    .build()
    .expect("valid Monte Carlo configuration");

let rng = PhiloxRng::new(7);
let process = GbmProcess::with_params(0.03, 0.01, 0.20)
    .expect("valid GBM parameters");
let disc = ExactGbm::new();
let payoff = EuropeanCall::new(100.0, 1.0, 252);
let discount_factor = (-0.03_f64).exp();

let result = engine
    .price(
        &rng,
        &process,
        &disc,
        &[100.0],
        &payoff,
        Currency::USD,
        discount_factor,
    )
    .expect("pricing should succeed");

println!(
    "price={} stderr={} ci=({}, {}) n={}",
    result.mean,
    result.stderr,
    result.ci_95.0,
    result.ci_95.1,
    result.num_paths
);
```

### Pricing With Captured Paths

Use `price_with_capture()` when you want the aggregate estimate plus a captured
subset of paths for plotting, debugging, cashflow analysis, or downstream data
inspection.

```rust,no_run
use finstack_core::currency::Currency;
use finstack_monte_carlo::prelude::*;

let engine = McEngine::builder()
    .num_paths(10_000)
    .seed(11)
    .uniform_grid(1.0, 12)
    .path_capture(PathCaptureConfig::sample(200, 17).with_payoffs())
    .parallel(false)
    .build()
    .expect("valid Monte Carlo configuration");

let rng = PhiloxRng::new(11);
let process = GbmProcess::with_params(0.03, 0.01, 0.20)
    .expect("valid GBM parameters");
let disc = ExactGbm::new();
let payoff = EuropeanCall::new(100.0, 1.0, 12);
let discount_factor = (-0.03_f64).exp();
let process_params = ProcessParams::new("GBM").with_factors(vec!["spot".to_string()]);

let result = engine
    .price_with_capture(
        &rng,
        &process,
        &disc,
        &[100.0],
        &payoff,
        Currency::USD,
        discount_factor,
        process_params,
    )
    .expect("pricing with capture should succeed");

println!("estimate={}", result.estimate);
println!("captured_paths={}", result.num_captured_paths());

if let Some(paths) = result.paths() {
    println!("sampling={}", paths.sampling_method);
    println!("state_keys={:?}", paths.state_var_keys());
}
```

### Unbiased American Pricing (Two-Pass LSMC)

Single-pass Longstaff-Schwartz LSMC fits the regression and prices on the
same path set, which biases the resulting price upward. The crate provides a
two-pass workflow that mitigates this bias by training the policy on one
path set and replaying it on a second, independent set:

```rust,no_run
use finstack_core::currency::Currency;
use finstack_monte_carlo::prelude::*;

let cfg = LsmcConfig::new(50_000, vec![25, 50, 75, 100], 100)
    .expect("valid config")
    .with_seed(42);
let pricer = LsmcPricer::new(cfg);
let process = GbmProcess::with_params(0.05, 0.0, 0.3).unwrap();
let put = AmericanPut::new(100.0).unwrap();
let basis = PolynomialBasis::new(2);

let unbiased = pricer
    .price_unbiased(
        &process, 100.0, 1.0, 100, &put, &basis,
        Currency::USD, 0.05,
        /* pricing_seed = */ 4243, // must differ from the configured seed
    )
    .expect("two-pass pricing should succeed");

assert!(unbiased.mean.amount() > 0.0);
```

For finer control, use [`LsmcPricer::fit_exercise_policy`] and
[`LsmcPricer::price_with_policy`] to fit and price separately and reuse the
captured [`ExercisePolicy`] across multiple pricing scenarios.

### Greeks With Common Random Numbers

Finite-difference Greeks come in two flavours:

- `finite_diff_delta` / `finite_diff_gamma` use an independence-bound stderr.
- `finite_diff_delta_crn` / `finite_diff_gamma_crn` use per-path paired bumps
  and report paired stderr (serial only).

```rust,no_run
use finstack_core::currency::Currency;
use finstack_monte_carlo::prelude::*;

let engine = McEngine::builder()
    .num_paths(20_000)
    .seed(42)
    .uniform_grid(1.0, 50)
    .parallel(false)
    .build()
    .expect("valid config");
let rng = PhiloxRng::new(42);
let gbm = GbmProcess::with_params(0.05, 0.0, 0.2).unwrap();
let disc = ExactGbm::new();
let call = EuropeanCall::new(100.0, 1.0, 50);

let (delta, paired_stderr) = finite_diff_delta_crn(
    &engine, &rng, &gbm, &disc, 100.0, &call, Currency::USD,
    /* discount_factor = */ (-0.05_f64).exp(),
    /* relative bump = */ 0.01,
)
.expect("CRN delta should succeed");
```

### Compact GBM-Only Entry Point

If you only need a European-style payoff under GBM dynamics, use
`pricer::european::EuropeanPricer` instead of wiring the generic engine yourself.
It constructs a time grid, `PhiloxRng`, `ExactGbm`, and `McEngine` internally.

## Public Surface

### Engine and runtime configuration

- `engine::McEngine` is the main entry point.
- `engine::McEngineBuilder` provides ergonomic defaults and validation.
- `engine::McEngineConfig` stores the explicit runtime configuration.
- `engine::PathCaptureConfig` and `engine::PathCaptureMode` configure captured
  paths for diagnostics.

### Traits and simulation contracts

- `traits::RandomStream` abstracts RNG implementations and stream splitting.
- `traits::StochasticProcess` defines drift, diffusion, dimensionality, and how
  a raw state vector maps into named path-state fields.
- `traits::Discretization` advances a process by one time step.
- `traits::Payoff` consumes path events and returns an undiscounted `Money`.
- `traits::PathState` exposes named state variables, per-step uniform draws, and
  optional typed cashflows.

### Processes

All process modules are compiled by default:

- `process::gbm`
- `process::brownian`
- `process::gbm_dividends`
- `process::multi_ou`
- `process::correlation`
- `process::metadata`
- `process::heston`
- `process::cir`
- `process::ou`
- `process::jump_diffusion`
- `process::bates`
- `process::schwartz_smith`
- `process::lmm`
- `process::rough_bergomi`
- `process::rough_heston`
- `process::cheyette_rough`

### Discretization schemes

All discretization modules are compiled by default:

- `discretization::exact`
- `discretization::exact_gbm_dividends`
- `discretization::euler`
- `discretization::milstein`
- `discretization::qe_heston`
- `discretization::qe_cir`
- `discretization::jump_euler`
- `discretization::exact_hw1f`
- `discretization::schwartz_smith`
- `discretization::lmm_predictor_corrector`
- `discretization::rough_bergomi`
- `discretization::rough_heston`
- `discretization::cheyette_rough`

### Payoffs and pricers

All payoff and pricer modules are compiled by default:

- `payoff::vanilla`
- `payoff::asian`
- `payoff::barrier`
- `payoff::basket`
- `payoff::lookback`
- `pricer::european`
- `pricer::path_dependent`
- `pricer::lsmc`
- `pricer::basis`

### Results and diagnostics

- `results::MoneyEstimate` wraps discounted Monte Carlo estimates in a currency.
- `results::MonteCarloResult` extends the estimate with optional captured paths.
- `paths::{PathDataset, SimulatedPath, PathPoint, ProcessParams}` exposes the
  captured-path data model for diagnostics, cashflow inspection, and visualization.
- `estimate` and `online_stats` contain the raw numeric statistics types used by
  pricing APIs.

### Other supporting modules

- `rng` for pseudo-random and quasi-random generation
- `variance_reduction` for control-variate helpers (always available);
  `importance_sampling` and `moment_matching` are gated behind the
  `vr-experimental` feature. Antithetic pairing is engine-native and
  configured via `McEngineConfig::antithetic`
- `greeks` for pathwise, LRM, and finite-difference estimators
- `barriers` for Brownian-bridge hit checks and continuity corrections
- `time_grid` for simulation time grids expressed in year fractions
- `seed` for deterministic seed helpers behind `mc`

## Conventions and Units

Unless a specific process or payoff module says otherwise:

- Rates, dividend yields, and volatilities are quoted in decimals, not basis points.
- Time values and time-grid coordinates are year fractions.
- `initial_state` must have length `process.dim()`.
- `Payoff::value()` returns an undiscounted `Money` amount.
- `McEngine::price()` and `price_with_capture()` expect a caller-supplied
  discount factor for the payoff horizon, typically `exp(-rT)` under flat
  continuous compounding.
- Captured `payoff_value` snapshots are stored in the payoff's native amount
  units and are not separately discounted inside each `PathPoint`.
- Captured cashflows use `positive = inflow`, `negative = outflow`.
- Captured percentiles, minima, maxima, and medians are computed from the
  captured subset, not necessarily the full path population.

## Reproducibility and Parallelism

The engine is designed to be reproducible across runs:

- Each Monte Carlo path is mapped to a deterministic substream using
  `rng.split(path_id)`.
- In parallel mode, chunk statistics are reduced deterministically after
  simulation.
- Captured paths are sorted by `path_id` before being returned, so the dataset
  ordering is stable across serial and parallel runs.

Parallel execution has a few important constraints:

- Parallel pricing requires an RNG whose `supports_splitting()` returns `true`.
  `PhiloxRng` is the intended default for this case.
- Sobol / quasi-random generators do not support stream splitting, so they must
  be used in serial mode.
- Rayon-backed parallel execution is compiled in by default, but you can still
  request serial execution per run.
- A `chunk_size` of `1000` means "use the engine's adaptive chunking heuristic".

## Runtime Constraints and Unsupported Combinations

The engine validates several combinations at runtime and returns an error instead
of silently proceeding:

- `num_paths` must be greater than zero.
- `num_paths` must not exceed `MAX_NUM_PATHS` (`10_000_000`).
- `chunk_size` must be greater than zero.
- `discount_factor` must be finite and non-negative.
- `target_ci_half_width` must be finite and positive if supplied.
- `target_ci_half_width` is currently supported only in serial mode.
- Path capture cannot be combined with `antithetic = true`.
- Sampled path capture requires `1 <= count <= num_paths`.

For sampled capture, the sample size is deterministic but approximate:
`PathCaptureMode::Sample { count, seed }` uses deterministic Bernoulli sampling,
so the realized number of retained paths is generally close to `count` rather
than guaranteed to equal it exactly.

## Results and Captured Diagnostics

`MoneyEstimate` contains:

- `mean`
- `stderr`
- `ci_95`
- `num_paths`
- optional `std_dev`
- optional `median`, `percentile_25`, `percentile_75`, `min`, and `max`

`MonteCarloResult` adds an optional `PathDataset`. A captured dataset can
contain:

- the full captured subset of `SimulatedPath` values
- the total number of simulated paths
- the sampling method used to retain paths
- process metadata describing how to interpret raw state vectors

Each `SimulatedPath` can contain:

- `PathPoint` entries for every captured time step
- per-step raw state vectors
- typed cashflows emitted by the payoff
- optional per-step payoff snapshots
- final discounted path value
- optional IRR inferred from captured cashflows when available

If you supply `ProcessParams::with_factors(...)`, downstream consumers can call
`PathDataset::state_var_keys()` to recover stable names for each state-vector
position.

## Extending the Crate

The crate is intended to be extended via traits rather than by modifying the
engine loop directly.

### Implement a new process

Implement `StochasticProcess` when you want a new model family. In particular:

- `dim()` defines the raw state-vector length
- `num_factors()` defines the number of independent shocks
- `drift()` and `diffusion()` define the SDE
- `populate_path_state()` maps raw state entries into semantic keys such as
  `spot`, `variance`, or `short_rate`

That last step is important because payoffs should read named values from
`PathState` instead of assuming a raw state layout.

### Implement a new discretization

Implement `Discretization<P>` when you want a new stepping scheme. Prefer exact
schemes when an analytical transition is available, and otherwise document the
stability and positivity assumptions of the approximation.

### Implement a new payoff

Implement `Payoff` when you want a custom contract or event-driven path logic:

- use `on_path_start()` for per-path random setup
- use `on_event()` to consume each path state
- use `state.add_cashflow()` or `state.add_typed_cashflow()` when the payoff
  should emit diagnostic cashflows
- use `value()` to return the final undiscounted payoff amount
- use `reset()` to clear all per-path state before the next simulation

## Build, Test, and Document

From the workspace root:

```bash
cargo test -p finstack-monte-carlo
cargo test -p finstack-monte-carlo -- --ignored
cargo doc -p finstack-monte-carlo --no-deps
```

If you are consuming the crate from another workspace member, prefer importing
through `finstack_monte_carlo::prelude::*` in examples and prototypes, and then
switch to explicit module paths where you want a narrower public surface.

## References

This crate implements standard Monte Carlo and quantitative-finance techniques.
Canonical references live in [`docs/REFERENCES.md`](../../docs/REFERENCES.md).
Useful anchors include:

- [`#glasserman-2004-monte-carlo`](../../docs/REFERENCES.md#glasserman-2004-monte-carlo)
- [`#welford-1962`](../../docs/REFERENCES.md#welford-1962)
- [`#heston-1993`](../../docs/REFERENCES.md#heston-1993)
- [`#hull-options-futures`](../../docs/REFERENCES.md#hull-options-futures)

Process- and scheme-specific assumptions live in module docs under `src/`.
See [`src/README.md`](src/README.md) for the directory map.
