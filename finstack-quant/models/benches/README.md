# finstack-quant-models benchmarks

Criterion benchmarks for the highest-iteration Monte Carlo paths: path generation
through the engine, LSMC backward induction, and the least-squares solve inside it. The
crate sets `autobenches = false` in [`../Cargo.toml`](../Cargo.toml), so a new file here
is inert until it is added as a `[[bench]]` target. One target is registered:
`mc_hot_paths` (`harness = false`).

The package name is `finstack-quant-models`; Monte Carlo benchmarks exercise the
crate's nested `monte_carlo` module.

## Groups

| Group | Id | Measures |
|-------|-----|----------|
| `european_pricer` | `paths/10000` | `EuropeanPricer::price` over `GbmProcess`, 252 steps, `with_seed(42)`, `with_parallel(false)`. The pricer selects `ExactGbm` internally, so no discretization is passed at the callsite |
| `lsmc_pricer` | `paths/5000` | `LsmcPricer::price` for an `AmericanPut` with 12 monthly exercise dates and a degree-2 `PolynomialBasis` — full backward induction |
| `lsq_regression` | `observations/500` | `solve_least_squares` alone: the SVD solve LSMC performs once per exercise date, on a deterministic 500×3 design |
| `heston_qe_pricer` | `paths/5000` | `McEngine::price` with `HestonProcess` + `QeHeston`, exercising `populate_path_state`, the QE variance step, and the per-step `dt`-constant transcendentals |
| `rough_heston_step` | `steps/100`, `steps/252` | `RoughHestonHybrid::step` driven directly in a per-path loop, isolating the O(n²)-per-path Volterra discretization from engine overhead |
| `hw1f_pricer` | `paths/20000` | `McEngine::price` with `HullWhite1FProcess` + `ExactHullWhite1F`, where per-step cost is dominated by the transcendentals `prepare` hoists |

`rough_heston_step` is the only group with a real size sweep, and it is there for a
reason: the hybrid scheme is quadratic in step count, so 100 vs 252 is the check that
the convolution has not become worse than quadratic. The other groups pin a single size.

Every group that simulates paths runs serially on a fixed seed: `european_pricer` sets
`with_seed(42).with_parallel(false)`; `heston_qe_pricer` and `hw1f_pricer` pass
`PhiloxRng::new(42)` and build the engine with `parallel(false)`; `lsmc_pricer` sets
`with_seed(42)` and inherits `use_parallel: false` from the embedded defaults in
`../data/defaults/pricer_defaults.v1.json` rather than setting it at the callsite.
`lsq_regression` and `rough_heston_step` draw no random numbers at all — they feed a
deterministic design matrix and a fixed `z = [0.5, -0.3]` — so neither seed nor
parallelism applies to them. The convention is deliberate: these measure per-path
serial cost, not Rayon scaling, and identical inputs across runs make a measured delta
a code delta.

## Run

```bash
cargo bench -p finstack-quant-models --bench mc_hot_paths
cargo bench -p finstack-quant-models --bench mc_hot_paths -- --quick
cargo bench -p finstack-quant-models -- rough_heston_step    # filter by group name
cargo bench -p finstack-quant-models -- --save-baseline before
cargo bench -p finstack-quant-models -- --baseline before
```

Benchmarks are measurement tasks, not gates: they are not run by `mise run rust-test`
(nextest), not by `mise run all-test`, and not by PR CI. `mise run rust-lint` and
`mise run rust-fix` do not compile Criterion targets. Workspace-wide measurement goes through `mise run rust-bench` (reduced
sampling, tunable via `FQ_BENCH_SAMPLE_SIZE`, `FQ_BENCH_WARM_UP_TIME`,
`FQ_BENCH_MEASUREMENT_TIME`, `FQ_BENCH_NRESAMPLES`), with
`mise run rust-bench-baseline` and `mise run rust-bench-compare` (fails above a 10%
median regression).

The `#[ignore]`d rough-volatility convergence tests are a separate concern from this
directory; they run under `mise run rust-test-slow`. Model-owned integration
tests live under `tests/`, while unit tests run with the crate library tests.

Criterion writes to `target/criterion/<group>/<id>/report/index.html`; the
`mise run rust-bench*` tasks pass `--noplot`.

## Conventions when adding a case

- Pin the seed and disable parallelism. An unseeded or Rayon-parallel benchmark measures
  the machine, not the code.
- Construct the process, discretization, payoff, and `McEngine` outside `b.iter`; only
  `price` (or `step`) belongs inside.
- To isolate a discretization scheme from engine overhead, follow `rough_heston_step`:
  drive `Discretization::step` directly over a pre-sized work buffer, resetting state at
  the top of each iteration.
- The file sets `#![allow(clippy::unwrap_used)]` and `#![allow(clippy::expect_used)]` at
  file scope. These are defensive, not load-bearing: those lints are denied by an inner
  attribute in [`../src/lib.rs`](../src/lib.rs), which covers the library crate only,
  and they are absent from `[workspace.lints.clippy]`. A bench target inherits neither,
  so `mise run rust-lint` is green over `.expect()` in fixtures either way.
- Register the function in the `criterion_group!` list at the bottom of
  `mc_hot_paths.rs`, or it never runs.

## See also

- [`../README.md`](../README.md) — crate overview, process/discretization/payoff traits,
  and the verification commands
- [`../src/README.md`](../src/README.md) — module-tree orientation for the code these
  benchmarks drive
- [`../../valuations/benches/README.md`](../../valuations/benches/README.md) —
  `mc_pricing`, `mc_exotics_pricing`, and `merton_mc_pricing`, the instrument-level
  benchmarks layered on this engine
- [`../../core/benches/README.md`](../../core/benches/README.md) — core numerics
  benchmarks. The core modules this crate actually calls (`math::random`,
  `math::fractional`, `math::special_functions`, `math::linalg`) have no dedicated core
  bench target, so their cost shows up in the groups above rather than there.
