# finstack-quant-core benchmarks

Criterion suites for `finstack-quant-core`. The benchmark sources are the ground
truth; this file says what each suite measures and how to run it. It carries no
latency, allocation, or "target met" numbers — those belong in current Criterion
output, not in a document that cannot be re-run.

The crate sets `autobenches = false`, so a new file under `benches/` does nothing
until it is added as a `[[bench]]` target in `Cargo.toml`. That is deliberate: it
keeps benchmark runtime an explicit decision.

## Suites

Targets are registered in `Cargo.toml`. Each is `harness = false` (Criterion owns `main`).

| Target | Measures |
|--------|----------|
| `calendar_lookup` | Calendar lookup and business-day evaluation. |
| `brownian_bridge` | Brownian bridge path construction. |
| `sobol` | Seeded Sobol sequence generation. |
| `correlation_apply` | Applying correlation factors to vectors. |
| `fx_matrix` | FX quote lookup, caching, and triangulation. |
| `daycount_operations` | Year fractions across day-count conventions; the `ActActIsma` and `Bus252` paths that need a `DayCountContext`; batch date-period calculation. |
| `interpolation` | Linear, log-linear, cubic Hermite, monotone convex, and piecewise-quadratic-forward interpolation; per-strategy comparison; extrapolation. |
| `curve_operations` | `DiscountCurve` `df`/`zero`/`forward` lookups, `ForwardCurve` rates, `HazardCurve` survival, interpolation-style comparison, and curve construction. |
| `rolling` | `CompiledExpr::eval` for `RollingMean`, `RollingStd`, and `RollingMedian` over a 500-row column with a 10-row window. This exercises the expression engine, not a standalone rolling API. |
| `solver_operations` | Newton with finite differences vs an analytic derivative; Brent; IRR and XIRR (including day-count variants); `LevenbergMarquardtSolver` on a global-fit problem. |
| `rate_conversions` | Simple / periodic / continuous compounding conversions, round trips, a fixed 100-rate batch conversion, market-convention scenarios, and negative rates. |
| `cashflow_operations` | Curve-based `npv` with `Money` flows on flat and shaped curves, scalar `npv_amounts`, `Discountable` trait dispatch, and bond/swap flow profiles. Each group runs one fixed flow count; no group sweeps size. |
| `schedule_generation` | `ScheduleBuilder` across frequencies, stub conventions, tenors, and EOM handling; IMM and CDS-IMM generation; business-day adjustment; schedule iteration. |
| `expr_eval` | Steady-state `CompiledExpr::eval` cost over a multi-node DAG with many rows — the pooled-arena, node-id-indexed path that statements hits once per period. |
| `context_bump` | `MarketContext::bump` cost as a function of context size — the finite-difference greeks hot path, using copy-on-write contexts for factor shocks. |

`benches/support/bench_utils.rs` holds `bench_iter` and `bench_with_criterion`,
pulled in via `#[path = "support/bench_utils.rs"] mod bench_utils;`. It is a
helper module, not a bench target.

## Running

```bash
# All core suites
cargo bench --package finstack-quant-core

# One suite
cargo bench --package finstack-quant-core --bench curve_operations

# Compile bench targets without measuring (fast check during refactors)
cargo bench --package finstack-quant-core --no-run

# Save and compare Criterion baselines
cargo bench --package finstack-quant-core -- --save-baseline before
cargo bench --package finstack-quant-core -- --baseline before
```

Workspace tasks run every crate's benches, not just core's, with reduced
Criterion timing:

```bash
mise run rust-bench            # quick pass; override via FQ_BENCH_* env vars
mise run rust-bench-baseline   # saves the baseline named "main"
mise run rust-bench-compare    # fails above a 10% median regression
```

`mise run rust-fmt` and `mise run rust-lint` skip Criterion targets. Compile
them with `mise run rust-bench` or `cargo bench`.

## Reading results

Criterion writes to `target/criterion/`:

- terminal summaries with confidence intervals,
- HTML reports at `target/criterion/*/report/index.html` (suppressed when
  `--noplot` is passed, as the `mise` tasks do),
- raw measurements under each benchmark directory.

## Evidence standard

Make performance claims from current output, never from this file.

1. Compile touched targets with `--no-run` while refactoring.
2. Run the relevant suites on the branch.
3. Save a baseline before a change you expect to matter.
4. Compare against that baseline afterwards.
5. Only then write a number into a release note or a README.

Results vary with hardware, toolchain, thermal state, and background load.
`std::hint::black_box` is used to limit optimizer distortion. When you add a
suite, add a row to the table above describing what it measures — not a number.
