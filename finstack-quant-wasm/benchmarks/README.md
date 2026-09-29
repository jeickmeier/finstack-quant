# WASM benchmarks

Node.js micro-benchmarks for the wasm-bindgen exports. Everything here measures
the _boundary_ — argument marshalling, wasm call, result conversion back to
JavaScript — layered on top of the Rust work, so a number from this directory is
only interpretable next to the corresponding Criterion number under
`finstack-quant/*/benches/`.

One file, one entry point. There is no framework: timing is `performance.now()`
around a single measured call per iteration.

## Layout

| Path        | Role                                                                                                                                                                                                                                                |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bench.mjs` | The entire harness. ESM script carrying a `#!/usr/bin/env node` shebang, though the file is not mode `+x` — invoke it as `node benchmarks/bench.mjs`. Roughly 90 rows across 11 domain groups, including the gated portfolio-materialization block. |

Nothing here is published. `scripts` and `benchmarks` are absent from the
`files` list in `package.json`, so this directory ships to nobody.

## Requires the Node build

`bench.mjs` imports `../pkg-node/finstack_quant_wasm.js` **directly** — the raw
wasm-bindgen output, not the `index.js` facade — and exits 1 with a build hint if
either `pkg-node/finstack_quant_wasm.js` or `pkg-node/finstack_quant_wasm_bg.wasm`
is missing. Every symbol in the file is therefore a flat bindgen name
(`w.blackScholesCall`, `w.csaUsdRegulatoryJson`), not a namespaced facade name
(`models.monteCarlo.blackScholesCall`). That deliberately bypasses the facade layer so
the numbers are boundary cost only; it also means the benchmark does not exercise
the wrappers `exports/*.js` installs.

`wasm-pack` builds release by default, so the profile is correct as long as the
build was not run with `--dev`.

```bash
npm --prefix finstack-quant-wasm run build:node   # wasm-pack build --target nodejs --out-dir pkg-node
npm --prefix finstack-quant-wasm run bench        # node benchmarks/bench.mjs
node finstack-quant-wasm/benchmarks/bench.mjs --help
```

`mise run wasm-pkg` also produces `pkg-node/`, with `--release` passed explicitly.

## What it measures

Rows are grouped by a `domain` string, printed as the first column.

| Domain                      | Covers                                                                                                                                                                                                                                                                                                                    |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `core`                      | `Currency`, `Money` add/sub, `DayCount.yearFraction`, `DiscountCurve.df`, `ForwardCurve`, `FxMatrix`, `Tenor`, `Rate`/`Bps`/`Percentage`, `choleskyDecomposition`/`choleskySolve`, `mean`/`variance`/`quantile`/`kahanSum`, `normCdf`, `countConsecutive`, `availableCalendars`, `adjust`                                 |
| `analytics`                 | `Performance` built from both prices and returns: `sharpe`, `volatility`, `sortino`, `meanReturn`, `downsideDeviation`, VaR/ES/parametric VaR, moments, `geometricMean`, `cumulativeReturns`, `excessReturns`, drawdown series and details, rolling sharpe/vol, `trackingError`, `informationRatio`, `rSquared`, `calmar` |
| `correlation`               | `correlationBounds`, `jointProbabilities`, Gaussian copula `conditionalDefaultProb`, `RecoverySpec.constant().conditionalRecovery`                                                                                                                                                                                        |
| `models.monteCarlo`         | `blackScholesCall`/`Put` closed forms, and European/Asian/American MC at **50,000 paths** per call (3–5 iterations each)                                                                                                                                                                                                  |
| `margin`                    | `csaUsdRegulatoryJson`, `csaEurRegulatoryJson`, `validateCsaJson`, `calculateVm`                                                                                                                                                                                                                                          |
| `statements`                | `validateFinancialModelJson`, `modelNodeIds`                                                                                                                                                                                                                                                                              |
| `statements_analytics`      | `runSensitivity`, `backtestForecast`, `runVariance`, `evaluateScenarioSet`, `generateTornadoEntries`, `evaluateMonteCarlo`, `goalSeek`, `traceDependencies`, `explainFormula`                                                                                                                                             |
| `portfolio`                 | `parsePortfolioSpecJson`, `buildPortfolioFromSpecJson`, `portfolioResultTotalValue`, `portfolioResultGetMetric`, `valuePortfolio`, `aggregateFullCashflows`                                                                                                                                                               |
| `portfolio_materialization` | `Portfolio.validateMaterialization` and `Portfolio.fromMaterialization` (cold-unique, cold-dedup, warm-dedup) — the only gated block                                                                                                                                                                                      |
| `valuations`                | `validateInstrumentJson`, `listStandardMetrics`, `priceInstrument` with and without an explicit metric list, `validateValuationResultJson`                                                                                                                                                                                |
| `scenarios`                 | `listBuiltinTemplates`, `buildFromTemplate`, `listTemplateComponents`, `buildTemplateComponent`, `parseScenarioSpec`, `validateScenarioSpec`, `buildScenarioSpec`, `composeScenarios`, `applyScenario`, `applyScenarioToMarket`                                                                                           |

Uncovered domains: `attribution`, `cashflows`, `covenants`, `factor_model`,
`features`.

JSON fixtures are inline string constants at the top of the file — a minimal
financial model, portfolio spec, portfolio result, deposit instrument, scenario
spec, market context (schema v2), two statement results, and a goal-seek and a
Monte Carlo model. They are hand-written, not generated; if a wire schema
changes, the affected rows start reporting as skipped rather than failing.

## Skips are not failures

Three helpers govern row behaviour:

- `bench(domain, name, iterations, fn, setup, cleanup)` — the plain path.
  `setup` runs untimed before each iteration, `cleanup` untimed after.
- `benchTry(...)` — runs `fn` once as an untimed probe; if it throws, the row
  becomes a skip and the suite continues.
- `skipBench(domain, name, reason)` — emits a `[bench skip]` warning and pushes a
  zero row that prints as em dashes.

So a broken fixture or a renamed export degrades to a skipped row, not a non-zero
exit. Read the `[bench skip]` warnings; a table with no failures is not evidence
that everything ran.

Handles created inside timed loops (`new w.Currency(...)`, `new w.ForwardCurve(...)`,
and similar) are not freed explicitly — only the materialization rows call
`free()` deterministically, in their `cleanup`. Keep that in mind before adding a
row that allocates a large handle at high iteration counts.

## The materialization block

`runMaterializationBenchmarks()` is the only part of this file tied to a
checked-in baseline, and the only part with its own CLI flag:

```bash
node benchmarks/bench.mjs --materialization-only    # skip every other row
mise run wasm-bench-materialization                 # fixtures + build:node + the above
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

`mise run materialization-benchmark-record` is what actually consumes the raw
output — it runs the Rust, Python and WASM measurements, applies both regression
gates, and writes `benchmarks/materialization/materialization-benchmark-results.json`.
This script itself applies no gate and always exits 0 on a completed run.

## Output

One table to stdout: domain, benchmark name, iteration count, best, average, p95,
and ops/sec. Nothing is written to disk unless `FQ_MATERIALIZATION_RAW_OUTPUT` is
set. Nothing is committed. Benchmarks do not gate PR CI.

## Adding a row

- Keep every fixture and handle construction outside the timed callable; use the
  `setup`/`cleanup` parameters of `bench` when per-iteration state is needed.
- Pick an iteration count that keeps the row under roughly a second. The existing
  spread runs from 3 (50k-path American MC) to 20,000 (`normCdf`).
- Use `benchTry` for anything whose fixture may drift with a schema change, and
  `bench` for the stable primitives.
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
