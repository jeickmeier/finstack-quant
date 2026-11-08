# finstack-quant-core benchmarks

Criterion suites for `finstack-quant-core`. The benchmark sources are the ground
truth; this file says what each suite measures and how to run it. It carries no
latency, allocation, or "target met" numbers — those belong in current Criterion
output, not in a document that cannot be re-run.

The crate sets `autobenches = false`, so a new file under `benches/` does nothing
until it is added as a `[[bench]]` target in `Cargo.toml`. That is deliberate: it
keeps benchmark runtime an explicit decision.

## Suites

The 14 targets register 192 cases in source. Each is `harness = false`
(Criterion owns `main`). Registration counts do not establish successful
execution or a timing baseline.

| Target                | Measures                                                                                                                                                                                                                                     |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `calendar_lookup`     | Warm year-cache holiday and business-day classification over the same 2025 dates; calendar rule counts are descriptive, not a rule-scan axis.                                                                                                |
| `brownian_bridge`     | Allocation-excluded whole-path construction on uniform grids of 1/8/16/32/40/64/256 steps and an irregular 17-step grid, including per-call validation and coefficient arithmetic.                                                           |
| `sobol`               | Advancing-state seeded Sobol uniform/normal generation; 1,024 points per call, with scalar count proportional to dimension.                                                                                                                  |
| `correlation_apply`   | Allocation-excluded triangular and pivoted-dense covariance-factor application; factorization is untimed.                                                                                                                                    |
| `fx_matrix`           | Separate direct, reciprocal, pinned and warm-cache FX paths, plus an explicitly cold cross-currency workflow.                                                                                                                                |
| `daycount_operations` | Year fractions across day-count conventions; the `ActActIsma` and `Bus252` paths that need a `DayCountContext`; batch date-period calculation.                                                                                               |
| `interpolation`       | Linear, log-linear, cubic Hermite, monotone convex, and piecewise-quadratic-forward interpolation; per-strategy comparison; extrapolation.                                                                                                   |
| `curve_operations`    | `DiscountCurve` `df`/`zero`/`forward` lookups, `ForwardCurve` rates, `HazardCurve` survival, interpolation-style comparison, and curve construction.                                                                                         |
| `solver_operations`   | Brent, IRR/XIRR, a checked full-rank nonlinear LM fit and a separately named degenerate termination control.                                                                                                                                 |
| `rate_conversions`    | Simple / periodic / continuous compounding arithmetic, round trips, fixed 100-rate allocated batches, market-quote aliases and negative rates.                                                                                               |
| `cashflow_operations` | Curve-based `npv` with `Money` flows on a flat curve and on a shaped curve. Each group runs one fixed flow count; no group sweeps size.                                                                                                      |
| `schedule_generation` | Complete schedule construction across frequencies, genuine irregular stubs, tenors and EOM; IMM/CDS-IMM, business-day adjustment, clone-plus-date-visitation and clone-plus-collection.                                                      |
| `expr_eval`           | Compile-excluded arithmetic-chain evaluation over prepared multirow columns, including per-call arena/offset allocation and root-result copy; this is separate from the statements evaluator.                                                |
| `context_bump`        | Fresh +1bp curve bumps at 8/64/256 curves; Money multipliers at 8/64/256/1024 unrelated curves; fixed surface, series and typed-volatility controls at 256 curves. Includes map copy-on-write, target transformation and result destruction. |

Every selected case runs a checked, untimed preflight before its measurement
loop. Fixture construction and preflight are inside Criterion's selected-case
closure, so a case filter excludes unrelated fixtures. Valid-case errors fail
the process; meaningful outputs are opaque to the optimizer. Numerical,
dimension, currency, date and branch checks are tailored to each operation.

The existing batch cases include result allocation/destruction. Curve-building
cases include cloning the prepared knot input. Setup excluded from a kernel
case is not an end-to-end performance claim. Sobol intentionally advances the
same RNG through timed calls; preflight uses separate RNG state.

The 1024-curve mixed context is a large-context stress case. Its target stores
stay fixed as unrelated curves grow. Brownian bridge kernels at 64/256 steps
are core-math stress cases; the production single-factor Sobol pricing route
supports at most 40 dimensions. Setup-inclusive QMC pricing rows live in the
models suite and include grid and run preparation.

## Running

```bash
# One suite
mise run rust-bench-crate -- finstack-quant-core curve_operations

# Checked smoke execution without recording Criterion timings
mise run rust-bench-crate -- finstack-quant-core curve_operations --test

# Select only one case (the regex matches the complete Criterion case name)
mise run rust-bench-crate -- finstack-quant-core context_bump --filter '^context_bump/64_curves$'

# Save and compare Criterion baselines
mise run rust-bench-crate -- finstack-quant-core curve_operations --save-baseline before
mise run rust-bench-crate -- finstack-quant-core curve_operations --baseline before
```

Workspace tasks run every crate's benches, not just core's, with reduced
Criterion timing:

```bash
mise run rust-bench
```

Both tasks accept `FQ_BENCH_SAMPLE_SIZE`, `FQ_BENCH_WARM_UP_TIME`,
`FQ_BENCH_MEASUREMENT_TIME` and `FQ_BENCH_NRESAMPLES`. The scoped task supports
`--filter`, `--test`, `--save-baseline` and `--baseline`; it does not forward
arbitrary Cargo options. Smoke mode cannot be combined with baseline options.
Named Criterion comparisons do not apply an automatic acceptance threshold.

Ordinary Rust lint/test gates omit Criterion execution. Run the scoped smoke
task for every changed target. The models crate has no `mc` feature; related
model consumer checks use its declared targets and supported features.

## Reading results

Criterion writes under `CRITERION_HOME` when set, otherwise under
`target/criterion/`:

- terminal summaries with confidence intervals,
- HTML reports at `target/criterion/*/report/index.html` (suppressed when
  `--noplot` is passed, as the `mise` tasks do),
- raw measurements under each benchmark directory.

## Evidence standard

Make performance claims from current output, never from this file.

1. Run checked smoke execution of every touched target.
2. Run the relevant suites on source-pinned, fully built revisions.
3. Save a baseline before a change you expect to matter.
4. Compare against that baseline afterwards.
5. Only then write a number into a release note or a README.

Results vary with hardware, toolchain, thermal state, and background load.
`std::hint::black_box` is used to limit optimizer distortion. When you add a
suite, add a row to the table above describing what it measures — not a number.
Archive raw observations and source/binary identity before reruns overwrite
Criterion's directories. Keep within-process intervals separate from variation
between independent processes.
