# finstack-quant-models benchmarks

The package is `finstack-quant-models`. Its [manifest](../Cargo.toml) declares five
Criterion targets with `harness = false` and disables automatic benchmark discovery.
All numerical modules, including Monte Carlo, are available by default. There is no
models `mc` feature; the optional/default feature is `json-schema`.

| Target | Cases | Measured work |
| --- | ---: | --- |
| `mc_hot_paths` | 13 | Full serial pricing, least squares, direct rough-Heston stepping and supported Sobol/bridge prices |
| `mc_scaling` | 38 | Separate path, step, asset, regression and LMM tenor axes plus a complete model caplet |
| `fourier_pricing` | 7 | COS strips and repeated independent scalar Heston calls |
| `migration_matrix` | 4 | Recorded paths and empirical transition matrices |
| `liquidity` | 2 | Roll spread and Amihud estimators |
| Total | 64 | Checked selected-case workloads |

## Validity and timing boundaries

Fixture construction and numerical preflight run inside the selected Criterion
closure, before `b.iter`. A filter excludes unrelated fixtures and their preflight.
Each case checks meaningful finite content and an analytic, independently derived,
or explicitly labeled convergence reference. Valid-case errors remain fatal in the
timer. Complete results are consumed: estimates, regression coefficients, evolved
states, Volterra increments, migration histories/matrices and scalar prices.

`support/validity.rs` checks currencies, confidence intervals, estimator/physical
counts, deterministic repeats and known regression coefficients. Monte Carlo
preflight uses a fixed six-standard-error envelope plus an explicit discretization
or fitting allowance. These are benchmark smoke checks; they do not replace or relax
existing tighter model/product regression, convergence and martingale tests.

The full-price cases retain engine preparation, allocations and result destruction.
EuropeanPricer also retains consumed `TimeGrid` construction inside the timer.
Direct scheme cases exclude grid/kernel construction and RNG generation; they retain
state/work reset, stepping and result consumption. The LMM cases retain the initial
state-vector clone. These synthetic paths use fixed innovations and do not estimate a
price. The HW case is a call on the short rate with a scalar discount multiplier,
not a pathwise-discounted caplet or bond option.

All path-pricing cases use seed 42 and serial execution. LSMC explicitly enables
antithetic pairing; each configured estimator uses two physical paths. Other generic
engine cases explicitly disable antithetic, capture and auto-stopping and retain
adaptive deterministic chunking. EuropeanPricer exposes seed/parallel settings only;
its observed independent/physical counts and absent capture diagnostics are checked.
Migration recreates seed 42 for recorded paths and seed 7 for empirical matrices.
Volterra normals are generated from a fresh seed 42 per selected size, outside timing,
so filtering/registration order does not change the fixture. No seed applies to fixed
direct innovations, least squares, Fourier or liquidity.

## Cases and work units

`mc_hot_paths` registers:

| Group / IDs | Workload | Throughput unit |
| --- | --- | --- |
| `european_pricer/paths/10000` | GBM/ExactGbm call, 10,000 physical paths x252 steps | Physical paths |
| `lsmc_pricer/estimators/5000_physical_paths_10000` | 5,000 estimators/10,000 physical paths, monthly Bermudan put on indices 1..=12, quadratic basis | Physical paths |
| `lsq_regression/observations/500` | 500x3 full-rank SVD, expected beta [1,7,3] | Rows |
| `heston_qe_pricer/paths/5000` | 5,000 physical Heston/QE paths x252 steps | Physical paths |
| `rough_heston_step/steps/{100,252}` | Full [spot,variance] synthetic path, z=[.5,-.3,.2] | Triangular history terms |
| `hw1f_pricer/paths/20000` | 20,000 physical exact-HW paths x252, short-rate strike .03 | Physical paths |

`mc_scaling` registers:

| Group / IDs | Fixed dimensions | Throughput unit |
| --- | --- | --- |
| `scaling_european_paths/{2500,10000,40000}` | 52 steps | Physical paths |
| `scaling_european_steps/{52,126,252}` | 2,000 physical paths | Physical path-steps |
| `scaling_heston_paths/{1000,4000,8000}` | 52 steps | Physical paths |
| `scaling_lsmc_estimators/{1000_estimators_2000_physical_paths,2500_estimators_5000_physical_paths,5000_estimators_10000_physical_paths}` | 12 monthly exercise dates, quadratic basis | Physical paths |
| `scaling_lsq_observations/{100,500,2000}` | Three basis columns; expected beta [1,2,3] | Rows |
| `scaling_rough_heston_steps/{50,100,200}` | One synthetic H=.1 path | Triangular history terms |
| `scaling_lmm_forwards_fixed_2y_8_steps/{10,20,40}` | Two years, eight steps, two factors | Forwards |
| `scaling_lmm_steps_fixed_2y_20_forwards/{8,16,32}` | Two years, 20 forwards, two factors | Steps |
| `scaling_multi_gbm_assets/{2,5,10}` | 2,000 physical paths x52, call on first marginal, correlation .3 | Assets |
| `scaling_volterra_steps/{64,128,256}` | RL Volterra H=.1, T=1, two pregenerated normals per step | Triangular kernel terms |
| `scaling_exact_gbm_steps/{52,126,252,504}` | One synthetic exact-GBM path, z=.5 | Steps |

The LMM forward axis now holds horizon/steps fixed; the previous case varied both.
At eight steps its alive-forward-step counts are 68/148/308 for 10/20/40 forwards;
the step axis has counts 148/296/592. Two drift evaluations with two factors imply
1,024/5,184/23,104 pair-factor terms on the forward axis and
5,184/10,368/20,736 on the step axis. These counts are derived from the validated
fixture grid and current pairwise algorithm, not measured profiler operation totals.
Forwards/steps throughput is not a claim of linear complexity. Preflight checks all
states, frozen F0, nonterminal homogeneous-state drift and the analytic driftless
terminal-forward path.

The remaining targets register:

- `cos_strip/{16,64,256}`: Black-Scholes CF, 128 COS terms, L=10, linear strikes
  70..130. Every price is checked against Black-Scholes within 1e-6.
- `heston_fourier_scalar_batch/{16,64,256}`: N independent checked scalar calls;
  each includes CF/grid construction and adaptive refinement. Every scalar price
  and the sum are consumed without a new timed output-vector allocation. The
  internal shared-strip API/cache already exists and is not measured here.
- `migration/simulate_1000_physical_paths/{5,10}`: 1,000 recorded physical paths
  from state zero over five years. Throughput is physical paths.
- `migration/empirical_matrix_500_per_state_2500_physical_paths/5` and
  `migration/empirical_matrix_500_per_state_5000_physical_paths/10`: 500 paths per
  initial state, including the absorbing state. Throughput includes all 2,500/5,000
  physical paths; absorbing paths do no random work.
- `models_liquidity_estimators/{roll_effective_spread_16384,amihud_illiquidity_16384}`:
  16,384 observations, alternating +/- .01 and positive volumes. Throughput is
  observations. Expected Roll spread is .02; Amihud is checked against a direct sum.

## Pre-C0 B3 additions

These bounded additions establish workloads and independent numerical contracts;
they do not optimize production code or establish a timing improvement.

- `sobol_bridge_price/{arithmetic_uniform8_points4096,arithmetic_uniform32_points4096,arithmetic_uniform40_points4096,arithmetic_uniform32_points16384,arithmetic_irregular17_points4096,arithmetic_uniform32_points4096_no_bridge}`
  prices arithmetic Asian fixtures through the supported single-factor Sobol
  path. Complete TimeGrid/run preparation remains timed. Runs are serial with
  dependent Sobol points, 16 replicates, no antithetic sampling or capture; use the
  separate convergence panel rather than IID standard errors for these points.
- `scaling_lmm_forwards_fixed_2y_8_steps/80` extends the fixed-work tenor axis.
  `scaling_lmm_controls_fixed_2y_8_steps/{factors3_forwards40,loading_knots_forwards20}`
  adds two controls. The loading knots are exactly .75/1.25; the three-factor
  innovation is [.4,-.2,.1]. One-factor LMM is an expected validation failure,
  since the current supported dimension is two or three factors.
- `lmm_caplet_terminal_measure/forwards40_estimators2048_steps8` includes model/grid
  validation, RNG/scratch setup, 2,048 ordinary physical paths, payoff/reduction
  and output construction. It fixes nonterminal F2 at T2=2 and pays at T3=3;
  the terminal-measure estimand uses the bond ratio product over forwards3..39
  and initial terminal discount from the same model. Its displaced-Black smoke
  bound is six sampling standard errors plus USD5 for the approximate scheme;
  the frozen paired original-drift numerical panel uses much tighter bounds.
- `heston_fourier_scalar_batch/1` times one scalar public call at explicit ATM
  strike100. The existing16/64/256 repeated-call boundaries remain unchanged.
- Two additive full public-pricer LMM consumer rows live in the existing
  valuations `mc_pricing` target, under
  `mc_lmm_consumer/{single_exercise_20f_2048_estimators,two_exercise_20f_2048_estimators}`.
  They preserve all previously accepted valuation rows. Each uses matching
  single-curve semiannual schedules, 20 forwards, 2,048 estimators/4,096 physical
  paths, and includes schedule/parameter/grid construction inside pricing.

Test-only original pairwise LMM drift/step and separate Heston probability-loop
oracles live under `tests/performance`; no public diagnostic API is introduced.
Ignored numerical exporters use distinct `CCM_MODEL_NUMERICS_OUTPUT`,
`CCM_HESTON_NUMERICS_OUTPUT` and `CCM_LMM_CONSUMER_NUMERICS_OUTPUT` file paths. They
record seed/grid/reference/output fingerprints and numerical errors separately
from benchmark timing. Preserve the existing exact LMM default-PV fingerprint,
Heston tail/corruption/convergence failures and tighter model/product tests.

## Running checks and measurements

Use the scoped mise task; for example:

```sh
mise run rust-bench-crate -- finstack-quant-models mc_hot_paths --test
mise run rust-bench-crate -- finstack-quant-models mc_scaling --test --filter '^scaling_lmm_'
mise run rust-bench-crate -- finstack-quant-models fourier_pricing --save-baseline c0-r1
mise run rust-bench-crate -- finstack-quant-models fourier_pricing --baseline c0-r1
```

`--test` is Criterion correctness/smoke mode and records no timing baseline. The task
supports `--filter`, `--test`, `--save-baseline` and `--baseline`; baseline flags are
mutually exclusive and cannot accompany `--test`. It does not forward arbitrary
feature/no-run flags. Run every declared target for complete coverage. Benchmark
sampling is configured by `FQ_BENCH_SAMPLE_SIZE`, `FQ_BENCH_WARM_UP_TIME`,
`FQ_BENCH_MEASUREMENT_TIME` and `FQ_BENCH_NRESAMPLES`. Archive raw results before
Criterion overwrites them, and keep source/toolchain/thread policy matched.

`mise run rust-test` and the ordinary `mise run all` gate do not execute Criterion
targets. Rust lint tasks omit benches. Explicit checked benchmark execution is
therefore required in addition to appropriate owner tests. No performance gain is
established by a passing smoke run, and repaired workload boundaries need a fresh
baseline. See the [performance plan](../../../.plans/CORE-CASHFLOW-MODEL-PERFORMANCE-PLAN.md)
and [review](../../../benchmarks/CORE_CASHFLOW_MODEL_PERFORMANCE_REVIEW.md).
