# finstack-quant-wasm tests

Four independent test layers guard the WASM bindings, because the published
surface is assembled from four different artifacts: Rust bindings, the
wasm-pack output in `pkg/` and `pkg-node/`, the hand-written JS facade
(`../index.js` + `../exports/*.js`), and the hand-written TypeScript
declarations (`../index.d.ts`). A bug can live in the seam between any two of
them, and only one layer sees each seam.

## Layout

```
tests/
  wasm.rs                one integration target linking the suites below
  wasm_*.rs              wasm-bindgen-test modules (run on wasm32 under Node)
  dts_contract.rs        host test: index.d.ts matches the facade surface
  return_shapes.rs       host test: declared return shapes, mirror of the Python file
  boundary_signatures.rs host test: exports take host values as JsValue
  facade/*.test.mjs      Node tests against the built package via the JS facade
  typescript/            tsc compile checks of index.d.ts and the generated types
  scripts/               tests for the JSDoc/TypeScript doc tooling in ../scripts/
```

## Layer 1 — `wasm_*.rs` (wasm-bindgen-test, wasm32)

Twenty-one suites, one per binding domain, compiled as modules of `tests/wasm.rs`.
Each is gated with `#![cfg(target_arch = "wasm32")]` and written with
`#[wasm_bindgen_test]`. They
call the Rust binding types directly (`finstack_quant_wasm::api::…`) and inspect
the returned `JsValue` with `js_sys`, which is the only place the `JsValue`
contract can be exercised at all. Native `#[test]` functions cannot build a
`JsValue`, so a binding test that has to pass one (every exported function
takes its host arguments as `JsValue`) lives here; native unit tests next to
the bindings exercise the typed Rust helpers instead.

| Suite                             | Surface                                                                      |
| --------------------------------- | ---------------------------------------------------------------------------- |
| `wasm_analytics.rs`               | the `Performance` panel facade — the whole analytics surface                 |
| `wasm_attribution.rs`             | `attributePnl` / `attributePnlJson` and the schema gate                      |
| `wasm_cashflows.rs`               | `api::cashflows` build/validate/flows/accrual                                |
| `wasm_core_market_data.rs`        | market-data and date bindings                                                |
| `wasm_core_primitives.rs`         | `Currency`, `DayCount`/`DayCountContext`, `Money` and epoch-day helpers      |
| `wasm_credit_factor_hierarchy.rs` | `CreditFactorModel` round-trip and calibrate → decompose                     |
| `wasm_features.rs`                | panel feature transforms                                                     |
| `wasm_fixed_income.rs`            | the typed `Bond` / `TermLoan` classes                                        |
| `wasm_implied_vol.rs`             | implied-volatility adapters                                                  |
| `wasm_margin.rs`                  | `calculate_vm`                                                               |
| `wasm_math.rs`                    | linear algebra, statistics, summation wrappers                               |
| `wasm_metric_keys.rs`             | canonical valuation metric-key validation                                    |
| `wasm_models_analytic.rs`         | closed-form and COS option kernels                                           |
| `wasm_models_correlation.rs`      | correlation-matrix validation and repair                                     |
| `wasm_models_credit.rs`           | Merton, recovery, endogenous hazard and toggle-exercise models               |
| `wasm_models_liquidity.rs`        | liquidity estimators                                                         |
| `wasm_portfolio.rs`               | every portfolio computation result, asserted to be a plain structured object |
| `wasm_scenarios.rs`               | template listing and `apply_scenario` / `apply_scenario_to_market`           |
| `wasm_statements.rs`              | node enumeration, evaluator, validator, DSL                                  |
| `wasm_statements_analytics.rs`    | `goal_seek`, `backtest_forecast`, `pl_summary_report_text`                   |
| `wasm_valuations.rs`              | `list_standard_metrics`, `price_instrument` and the FX classes               |

Two suites — `wasm_analytics.rs` and `wasm_credit_factor_hierarchy.rs` —
`include_str!` a fixture straight out of the corresponding Rust crate's test
data (`wasm_analytics.rs` reads
`finstack-quant/analytics/tests/data/api_invariants_data.json`) so the WASM
numbers are compared against the same inputs the native tests use.

Run:

```bash
npm --prefix finstack-quant-wasm run test     # wasm-pack test --node
```

## Layer 2 — host tests (`dts_contract.rs`, `return_shapes.rs`, `boundary_signatures.rs`)

Ordinary `#[test]` functions compiled for the host. They read
[`../index.d.ts`](../index.d.ts) and the binding sources as text and assert
properties of the _declaration_, which no runtime test can reach: a `.d.ts` that
lies about a type is invisible to JS at runtime and fatal at compile time for a
TypeScript consumer.

- **`dts_contract.rs`** (33 tests) pins the declared signatures per namespace —
  argument lists, full-word parameter names, `Float64Array` returns on the
  numeric fast paths, structured (not string) valuation results, the
  `WasmOwned` / `[Symbol.dispose]` handle contract, and that the package
  documents the hand-written facade rather than the raw wasm-bindgen types. It
  also reads [`../benchmarks/bench.mjs`](../benchmarks/bench.mjs) so the
  benchmark script cannot drift from the declared API.
- **`return_shapes.rs`** (6 tests) pins _shapes_: no binding bypasses the
  JSON-compatible serializer, only `*Json`-suffixed exports return strings,
  computation results are structured rather than strings, prose-returning
  exports are named `*Text`, numeric vector exports declare `Float64Array`, and
  the facade does no JSON parsing of its own.

- **`boundary_signatures.rs`** scans every `#[wasm_bindgen]` export under
  `src/api` and rejects `&str`, `String`, `bool`, integer and `Vec<String>`
  parameters: their generated glue traps the instance or coerces silently
  (`ToInt32`, truthiness). Exports take `JsValue` and convert through
  `src/utils/input.rs`.

`return_shapes.rs` is the deliberate mirror of
[`../../finstack-quant-py/tests/parity/test_return_shapes.py`](../../finstack-quant-py/tests/parity/test_return_shapes.py)
— same entries, same order — so a cross-language divergence reads as a
one-screen diff. Keep them edited together.

Run:

```bash
# `mise run rust-test` selects both host targets for this crate:
# dts_contract, and the consolidated `wasm` binary, whose only host-runnable
# modules are return_shapes and boundary_signatures (the wasm_* modules are
# wasm32-gated).
mise run rust-test-integration -- finstack-quant-wasm dts_contract
mise run rust-test-filter -- finstack-quant-wasm return_shapes --integration wasm
```

## Layer 3 — `facade/` (Node, built package)

Node test-runner suites that import the **public** entrypoint
[`../index.js`](../index.js) and therefore catch the failure mode the Rust tests
structurally cannot: a `js_name` rename or a missing key in `../exports/*.js`
silently exporting `undefined`. `plain_object_returns.test.mjs` is the one
exception — it imports the generated Node module directly, because the bug it
hunts lives below the facade (see below).

| File                                                       | Asserts                                                                                                                                                                              |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `core_namespace.test.mjs`                                  | `Currency`, `Money` (including the lossless `amountDecimal` getter), `FxDeltaVolSurface`, `FxMatrix`, `FxRateResult` getters                                                         |
| `cashflows.test.mjs`                                       | every exported key is a live function, plus an end-to-end build/validate/flows/accrual round trip                                                                                    |
| `covenants.test.mjs`                                       | the covenants namespace                                                                                                                                                              |
| `statements.test.mjs`                                      | statements / statements-analytics results come back as structured objects, matching the Python twins                                                                                 |
| `portfolio.test.mjs`, `portfolio_materialization.test.mjs` | portfolio runtime contract and the materialization API                                                                                                                               |
| `valuations_instruments.test.mjs`                          | typed `Bond` / `TermLoan`                                                                                                                                                            |
| `margin_xva.test.mjs`                                      | `computeBilateralXva` through `core.HazardCurve`, against a fixture byte-equivalent to the Rust integration test                                                                     |
| `return_floor.test.mjs`                                    | `moic` / `moic_to_worst` / `xirr` / `xirr_to_worst` through the JSON-native Bond path                                                                                                |
| `analytics_period_stats.test.mjs`                          | non-finite ratio round-tripping in `Performance.periodStats`                                                                                                                         |
| `plain_object_returns.test.mjs`                            | map-returning functions produce plain objects, not ES2015 `Map`s                                                                                                                     |
| `boundary_input.test.mjs`                                  | wrong argument types throw `TypeError` (`kind: "invalid_type"`) without trapping or leaking; JSON inputs accept objects with unknown-field checks                                    |
| `core_members.test.mjs`                                    | S20 handle members and JSON round-trips (Rate/Bps/Percentage, DayCount, Tenor, curves, surfaces, FxRateResult, realized variance); goldens shared with `test_core_members_parity.py` |
| `core_rust_owned.test.mjs`                                 | core currency/money/date/curve behaviour owned by Rust; asserts the same values and messages as `finstack-quant-py/tests/test_core_rust_owned.py`                                    |
| `contract_types.test.mjs`                                  | runtime calibration, materialization and valuation outputs type-check against the schema-generated TypeScript types under NodeNext                                                   |

**These need a build.** All of them except `plain_object_returns.test.mjs` load
the web target from `pkg/finstack_quant_wasm_bg.wasm` (Node has no fetchable
URL, so the bytes are read and passed to `init`); `plain_object_returns.test.mjs`
imports the Node target from `pkg-node/`. Each throws a build-me message if its
artifact is missing.

```bash
npm --prefix finstack-quant-wasm run build         # -> pkg/      (web)
npm --prefix finstack-quant-wasm run build:node    # -> pkg-node/ (node)
npm --prefix finstack-quant-wasm run test:facade   # node --test tests/facade/**/*.test.mjs
```

`pkg/` and `pkg-node/` are gitignored build output. Never edit anything inside
them, and never treat a file in there as source.

### Why `plain_object_returns.test.mjs` exists

`serde_wasm_bindgen::to_value` serializes Rust maps as ES2015 `Map`s. Property
reads on a `Map` yield `undefined` and `JSON.stringify` drops the contents — a
silent, total data loss that no type check catches. Bindings must route through
`crate::utils::to_js_value` instead; `mise run wasm-check-serializer` greps for
the ban, `return_shapes.rs` asserts it, and this file proves it at runtime.

## Layer 4 — `typescript/` and `scripts/`

- **`typescript/`** type-checks two small consumer programs against
  `../index.d.ts` with `tsc -p` and `noEmit`, under two different `lib`
  settings: `es2020.ts` / `tsconfig.es2020.json` (ES2020 + DOM) exercises the
  ordinary typed surface, and `esnext-disposable.ts` /
  `tsconfig.esnext-disposable.json` (ES2022 + `ESNext.Disposable`) exercises the
  `WasmOwned` / `[Symbol.dispose]()` handle contract. Both run under
  `strict: true`. `nodenext.ts` / `tsconfig.nodenext.json` is the consumer view:
  it imports `finstack-quant-wasm` and `finstack-quant-wasm/types` by package
  name under `module`/`moduleResolution` `NodeNext` with `skipLibCheck: false`,
  compiles every schema-generated module under `../types/generated/`, and pins
  the serde wire shapes with `@ts-expect-error` negatives (a type that degrades
  to `any` fails the gate). `facade/contract_types.test.mjs` extends this to
  runtime values: it type-checks real `calibrate`, `dryRun`, canonical-envelope,
  materialization and valuation outputs against the generated types.
- **`scripts/typescript_docs.test.mjs`** tests the documentation tooling in
  [`../scripts/`](../scripts) — `sync-facade-jsdoc.mjs`,
  `complete-facade-jsdoc.mjs`, `check-typescript-docs.mjs` — in both `--write`
  and `--check` modes, against the committed fixtures in
  `scripts/fixtures/typescript-docs/` (`raw.d.ts`, `facade.expected.d.ts`,
  `facade.stale.d.ts`, `checker.valid.d.ts`, `checker.legacy.d.ts`). Fixtures
  are copied to a temp directory first, so the checks never mutate the
  repository.

Both run under `npm run docs:check`, not under `npm run test`:

```bash
npm --prefix finstack-quant-wasm run test:dts          # tsc, both tsconfigs
npm --prefix finstack-quant-wasm run test:docs-tools   # doc tooling
npm --prefix finstack-quant-wasm run docs:check        # both, plus the doc checkers
```

## Running everything

```bash
mise run wasm-test    # wasm-bindgen tests, then web + node builds, then facade tests
mise run wasm-doc     # doc checkers, tsc declaration checks, doc-tooling tests
mise run rust-test    # workspace nextest, including this crate's dts_contract
mise run wasm-lint    # prettier + eslint + the to_js_value serializer check
```

`mise run all-lint` includes `wasm-lint` and `wasm-doc`; CI runs `wasm-test`
and `rust-test` as separate jobs.

## See also

- [`../README.md`](../README.md) — the WASM package overview
- [`../index.d.ts`](../index.d.ts) — the published TypeScript contract these
  tests police
- [`../../.agents/rules/wasm/code-standards.md`](../../.agents/rules/wasm/code-standards.md)
- [`../../.agents/rules/wasm/javascript-usage-standards.md`](../../.agents/rules/wasm/javascript-usage-standards.md)
- [`../../finstack-quant-py/tests/parity/`](../../finstack-quant-py/tests/parity) —
  the Python side of the parity contract
