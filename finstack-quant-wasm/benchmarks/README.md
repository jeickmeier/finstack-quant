# WASM benchmarks

Node.js micro-benchmarks for the raw wasm-bindgen exports. Timings include
argument marshalling, the Rust computation inside WebAssembly, and result
conversion back to JavaScript. They are end-to-end raw-binding timings, not an
isolated measurement of boundary overhead or public-facade performance.

`bench.mjs` is the entry point. Timing uses `performance.now()`; the checked
core/model controls use fixed batches to keep tiny calls above clock overhead.

## Layout

| Path              | Role                                                                                                             |
| ----------------- | ---------------------------------------------------------------------------------------------------------------- |
| `bench.mjs`       | ESM entry point; invoke as `node benchmarks/bench.mjs`. Contains the finance fixtures and checked operations.    |
| `ccm-measure.mjs` | Private fixed core/model row identities, selector parsing, batched observation and deterministic handle cleanup. |

Nothing here is published. `scripts` and `benchmarks` are absent from the
`files` list in `package.json`, so this directory ships to nobody.

## Requires the Node build

`bench.mjs` imports `../pkg-node/finstack_quant_wasm.js` **directly** — the raw
wasm-bindgen output, not the `index.js` facade — and exits 1 with a build hint if
either `pkg-node/finstack_quant_wasm.js` or `pkg-node/finstack_quant_wasm_bg.wasm`
is missing. Every symbol in the file is therefore a flat bindgen name
(`w.bsPrice`, `w.csaSpecUsdRegulatory`), not a namespaced facade name
(`models.bsPrice`). That bypasses the facade layer; the benchmark does not exercise the wrappers
`exports/*.js` installs. Compare with a matched Rust workload before attributing
any timing difference to the binding boundary.

`wasm-pack` builds release by default, so the profile is correct as long as the
build was not run with `--dev`.

```bash
npm --prefix finstack-quant-wasm run build:node   # wasm-pack build --target nodejs --out-dir pkg-node
npm --prefix finstack-quant-wasm run bench        # node benchmarks/bench.mjs
node finstack-quant-wasm/benchmarks/bench.mjs --valuation-only # required pricing controls
node finstack-quant-wasm/benchmarks/bench.mjs --core-models-only # 22 required core/model controls
node finstack-quant-wasm/benchmarks/bench.mjs --help
```

`mise run wasm-pkg` also produces `pkg-node/`, with `--release` passed explicitly.

## What it measures

Rows are grouped by a `domain` string, printed as the first column.

| Domain                      | Covers                                                                                                                                                                                                                                                                                                                    |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `core`                      | 16 rows: `Currency`, `Money` add/sub, `DayCount.yearFraction`, `DiscountCurve.df`, `ForwardCurve`, `FxMatrix`, `Tenor.parse`, `Rate`/`Bps`, `choleskyDecomposition`/`choleskySolve`, `mean`/`variance`/`quantile`/`kahanSum`, `normCdf`, `availableCalendars`, `adjust`                                                   |
| `analytics`                 | `Performance` built from both prices and returns: `sharpe`, `volatility`, `sortino`, `meanReturn`, `downsideDeviation`, VaR/ES/parametric VaR, moments, `geometricMean`, `cumulativeReturns`, `excessReturns`, drawdown series and details, rolling sharpe/vol, `trackingError`, `informationRatio`, `rSquared`, `calmar` |
| `correlation`               | `correlationBounds`, `CorrelatedBernoulli.jointProbabilities`, Gaussian copula `conditionalDefaultProb`, `RecoverySpec.constant().conditionalRecovery`                                                                                                                                                                    |
| `models`                    | Two `bsPrice` closed-form call/put rows. There are no European, Asian or American option Monte Carlo rows.                                                                                                                                                                                                                |
| `margin`                    | `csaSpecUsdRegulatory`, `csaSpecEurRegulatory`, `csaSpecValidate`, `VmCalculator.calculate`                                                                                                                                                                                                                               |
| `statements`                | `validateFinancialModelJson`, `modelNodeIds`                                                                                                                                                                                                                                                                              |
| `statements_analytics`      | `runSensitivity`, `backtestForecast`, `runVariance`, `evaluateScenarioSet`, `generateTornadoEntries`, `evaluateMonteCarlo`, `goalSeek`, `dependencyTree`, `explainFormula`                                                                                                                                                |
| `portfolio`                 | `parsePortfolioSpecJson`, `Portfolio.fromSpec(...).toJson()`, strict standard-risk `valuePortfolio`, `aggregateFullCashflows` on two deposits                                                                                                                                                                             |
| `portfolio_materialization` | `Portfolio.validateMaterialization` and `Portfolio.fromMaterialization` (cold-unique, cold-dedup, warm-dedup) — the only gated block                                                                                                                                                                                      |
| `valuations`                | `validateInstrumentJson` for one deposit, `listStandardMetrics`, deposit `priceInstrument` PV / explicit `dv01`, successful `valuationResultToJson`                                                                                                                                                                       |
| `scenarios`                 | `listBuiltinTemplates`, `buildFromTemplate`, `listTemplateComponents`, `buildTemplateComponent`, `parseScenarioSpec`, `validateScenarioSpec`, `composeScenarios`, `applyScenario`                                                                                                                                         |

Uncovered domains: `attribution`, `cashflows`, `covenants`, `factor_model`,
`features`.

JSON fixtures are inline string constants at the top of the file. The pricing
portfolio holds two USD deposits with different notionals and maturities. These
are deposit-only controls, not representative mixed-asset portfolio timings.

## Checked core and model controls

`--core-models-only` selects exactly 22 required rows: 16 core, four
correlation, and two Black-Scholes controls. It returns before valuation,
materialization and other domain setup, never invokes Cargo, and does not need
materialization sample settings. Selectors are mutually exclusive; unknown
flags are fatal. Direct cashflows host coverage is absent.

Each row checks an untimed preflight and every warmup/measured result. Checks
include complete array dimensions and Cholesky reconstruction, scalar reference
values, supplied discount pillars, currency tags, Bernoulli marginals and
probability totals. The fixed numerical bounds and required row IDs are in
`ccm-measure.mjs`. Errors are fatal and never appear as skipped success rows.
The normal-CDF constants were computed independently using the error-function
Taylor expansion before runtime evidence. These checks complement owning
financial tests; they do not replace them.

The protocol defaults to ten measured batches and one warmup batch per row.
`FQ_CCM_JS_SAMPLES` is a positive integer; `FQ_CCM_JS_WARMUP_SAMPLES` is a
nonnegative integer. Criterion's `FQ_BENCH_*` variables have no effect. Batch
sizes are fixed before outcomes at each old row's iteration count divided by
ten, preserving its total measured calls under the default protocol. Both raw
batch milliseconds and normalized milliseconds per call are retained. Each
batch preallocates its output buffer before timing; the loop and complete-result
assignment are inside timing. Validation, full snapshots and disposal are
outside timing. At most one batch of outputs is retained. Each acquired handle,
including a partial failed construction or failed output check, is explicitly
disposed; copied JavaScript typed arrays require no disposal.

The tenor row now uses the existing raw `Tenor.parse('3M')` API. The old
string-constructor row was invalid and is not a timing baseline. Raw FX uses
`FxMatrix.rate` with an explicit `FxConversionPolicy` handle. Gaussian and
constant-recovery labels now correctly describe queries with construction
outside timing. These repairs and batched boundaries require fresh baselines.

Set `FQ_CCM_RAW_OUTPUT` to a fresh path to persist all required IDs, complete
normalized outputs, raw samples, medians, parameters/fixture hashes, boundaries,
process clocks, Node/platform identity and exact loaded JS/WASM artifact hashes.
Writes are create-only: an existing result path is rejected. Optional
`FQ_CCM_SOURCE_IDENTITY` and `FQ_CCM_BUILD_RECEIPT` paths add the root runner's
JSON payloads and SHA-256 identities. Artifact hashes establish identity; a
successful source-matched release build receipt supplies profile evidence.
The harness does not infer release status from file names, speed or mtime.

For each campaign, run three independent processes with separate output paths
against the same verified release artifact. For example, from the repository root:

```sh
RAYON_NUM_THREADS=8 FQ_CCM_JS_SAMPLES=10 FQ_CCM_JS_WARMUP_SAMPLES=1 FQ_CCM_RAW_OUTPUT=target/core-cashflow-model-benchmarks/execution/B2/node-r1.json node finstack-quant-wasm/benchmarks/bench.mjs --core-models-only
```

Repeat with `node-r2.json` and `node-r3.json`. Reuse release artifacts only while
their source/runtime inputs and hashes match the build receipt. Report process
variation separately from within-process samples. These are raw-bindgen
workloads with different inputs/boundaries from Python/native rows; subtracting
their times does not measure binding overhead. Batch output retention also
precludes treating whole-harness RSS as a selected-operation memory result.

Pure helper behavior is checked without loading WASM by:

```sh
node --test finstack-quant-wasm/tests/scripts/ccm_benchmarks.test.mjs
```

## Required pricing controls

The nine `portfolio` / `valuations` rows run through `benchRequired`, which checks
one successful output before timing. Initialization, pricing and validation
errors abort the run with a nonzero exit. Preflight requires the expected position
IDs/counts, finite USD PVs, explicit finite `dv01` where requested, complete risk,
no `risk_error` or `degraded_positions`, and nonempty cashflows without issues.
The result JSON row canonicalizes a successfully priced valuation result.
Exceptions during measured calls are also fatal.

`--valuation-only` executes exactly these required rows without constructing
unrelated fixtures or running materialization generation. It is mutually
exclusive with `--materialization-only`. All rows retain their normal iteration
counts in this selection mode.

Portfolio and market handles used for pricing are constructed before timing and
freed after their rows. The `Portfolio.fromSpec + toJson` row includes construction
and serialization in its timer; its handle is freed after each measured call,
including the untimed preflight. Pricing results are plain JavaScript objects.

Domains outside the checked core/model, pricing and materialization groups
retain their existing optional-probe behavior:

- `bench(domain, name, iterations, fn, setup, cleanup)` measures each call;
  `setup` and `cleanup` run outside timing.
- `benchTry(...)` probes once and records a skip if it throws.
- `skipBench(...)` emits a warning and prints the row as em dashes.

A skipped optional row is not evidence of successful execution. Handle cleanup
in the remaining optional domain rows has not been normalized.

## The materialization block

`runMaterializationBenchmarks()` is the only part of this file tied to a
checked-in baseline, and can be selected independently:

```bash
node benchmarks/bench.mjs --materialization-only  # skip every other row
scripts/materialization_bench.sh wasm              # fixtures + build:node + the above (from the repo root)
```

It shells out to
`cargo run --release -p finstack-quant-portfolio --example materialization_fixtures`
first, so cargo must be on `PATH`, and reads two deterministic fixtures from
`target/materialization-benchmarks/`:

- `materialization-a-5000-unique.json` — 5,000 positions over 5,000 unique
  instrument artifacts (cold).
- `materialization-b-5000-50.json` — 5,000 positions over 50 artifacts (measured
  cold and warm; the cache-hit case).

Neither fixture is checked in. The generator is shared with the Rust Criterion
bench, so Rust, Python and Node measure identical bytes.

Environment variables, read the same way by the Python and Rust harnesses:

| Variable                         | Effect                                                                                                                                                                       |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `FQ_MATERIALIZATION_P95_SAMPLES` | Iteration count for every materialization row. Default 100. Must be a finite integer at or above the minimum, or the script throws at startup.                               |
| `FQ_MATERIALIZATION_SMOKE=1`     | Lowers that minimum from 100 to 1. Test-only override; a record produced under it is invalid.                                                                                |
| `FQ_MATERIALIZATION_RAW_OUTPUT`  | Path for a raw JSON fragment: per-sample timings, phase counters, fixture SHA-256s, and the timing-boundary description. Relative paths resolve against the repository root. |

The timing boundary is documented in the emitted JSON and is deliberately narrow:
the fixture string and an explicitly-sized `InstrumentArtifactCache` exist before
the clock starts; the elapsed span covers the binding call and the JavaScript
result conversion; handle cleanup happens after the stop.

`scripts/materialization_bench.sh record` is what actually consumes the raw
output — it runs the Rust, Python and WASM measurements, applies both regression
gates, and writes `benchmarks/materialization/materialization-benchmark-results.json`.
This script itself applies no latency regression gate. Required output checks
and initialization/pricing failures still cause a nonzero exit.

## Output

One table to stdout: domain, benchmark name, iteration count, best, average,
median, p95, and ops/sec. Core/model times are normalized per operation, and
their iteration column counts measured operations across all batches. Nothing
is written unless the relevant `FQ_CCM_RAW_OUTPUT` or
`FQ_MATERIALIZATION_RAW_OUTPUT` path is set. Benchmarks do not gate PR CI.

## Adding a row

- Keep every fixture and handle construction outside the timed callable; use the
  `setup`/`cleanup` parameters of `bench` when per-iteration state is needed.
- Freeze an appropriate operation count and batch size before measuring either
  revision. Do not change financial workloads or choose a favorable protocol
  after seeing a result.
- Use `benchRequired` with a meaningful output check for valuation and portfolio
  controls. Schema drift must fail these required cases. `benchTry` is reserved
  for the existing optional controls in other domains.
- Reference flat bindgen names off `w`, matching the rest of the file. Do not
  import the facade here.
- eslint runs over this file under `mise run wasm-lint`; the `no-console`
  exemption for `scripts/**` does not apply, which is why the file carries an
  explicit `/* eslint-disable no-console */`. Prettier covers `.mjs`.

## Related

- [`../README.md`](../README.md) — the WASM package overview
- [`../tests/README.md`](../tests/README.md) — the four test layers
- [`../scripts/README.md`](../scripts/README.md) — the documentation gates
- [`../../benchmarks/README.md`](../../benchmarks/README.md) — the checked-in
  performance records; only the materialization path is tracked there
- [`../../benchmarks/MATERIALIZATION_BENCHMARKS.md`](../../benchmarks/MATERIALIZATION_BENCHMARKS.md)
  — fixture definitions, timing boundaries, gates, hardware provenance
- [`../../finstack-quant-py/benchmarks/README.md`](../../finstack-quant-py/benchmarks/README.md)
  — the Python sibling
- [`../../finstack-quant/portfolio/benches/README.md`](../../finstack-quant/portfolio/benches/README.md)
  — the Rust side of the same materialization measurement
