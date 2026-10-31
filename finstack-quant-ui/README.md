# Finstack Quant UI contracts

Component registry contracts: offline JSON Schema bundles, generated wire
interfaces, lossless adapters, source provenance and a lazy instrument catalogue.
This is a standalone Node 24 package with installable controlled primitives.

```sh
mise run wasm-pkg
mise run ui-sync
npm --prefix finstack-quant-ui run generate
mise run ui-fmt
mise run ui-typecheck
mise run ui-test
mise run ui-check
```

`ui-sync` installs the lockfile with lifecycle scripts disabled. The linked WASM
package is built explicitly by `wasm-pkg`; dependency installation does not launch
Rust or Python builds. Use `ui-fmt`, `ui-typecheck`, and `ui-test` while iterating;
`ui-check` is the combined gate for generated drift, handwritten formatting,
TypeScript and the package's Vitest tests. It does not run the library suites.

```ts
import { instruments } from "finstack-quant-ui/instruments";

const bond = instruments.find((entry) => entry.type === "bond")!;
const { validator, example, schema, metadata } = await bond.loader();
const result = validator.safeParse(example);
```

Each loader imports its schema, metadata and example only when selected. ESM
caches the module and its validator. The catalogue has no static imports of Zod,
schemas, metadata or examples. Individual wire types are exported by the loaded
module, and live under `src/generated/types/` for type-only imports.

## Layout and extension

| Path                                                 | Holds                                                                                                                                                                                                                                                                                           |
| ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `registry/<category>/`                               | Item sources. `core`, `valuations`, `calibration`, `covenants`, `scenarios`, `attribution`, `analytics`, `portfolio`, `margin`, `models` and `statements` follow the crates, beside `base`, `theme` and `shared`. Each `registry.json` lists files, install targets and `registryDependencies`. |
| `components/ui/`                                     | Stock shadcn source, hash-pinned in `components/shadcn.json`. Never edited.                                                                                                                                                                                                                     |
| `src/`                                               | Codec, host, views and format; `src/format/label.mjs` holds `labelFor`, the shared label function, also used by `registry-content`; `src/generated/` is generator output.                                                                                                                       |
| `scripts/`                                           | Generators, the fixture runner, checks and install gates.                                                                                                                                                                                                                                       |
| `tests/<crate>/`                                     | Vitest suites and native fixture generators.                                                                                                                                                                                                                                                    |
| `tests/shared/browser/`, `tests/valuations/browser/` | Standalone consumer harnesses run by `ui-harness`.                                                                                                                                                                                                                                              |
| `tests/e2e/`, `tests/install/`, `tests/publishing/`  | The `ui-e2e`, `ui-install` and `ui-publish-check` gates.                                                                                                                                                                                                                                        |
| `tests/browser/`                                     | Worker smoke, footprint gate, the harness runner and shared consumer helpers.                                                                                                                                                                                                                   |

Sources import through the paths an installed consumer uses. `tsconfig.json`
maps them back here; `vitest.config.mts` copies that table into prefix aliases
in order, so a specific key must precede its prefix.

| Import                    | Source                                                                                                                                                                                                          |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `@/lib/finstack/form`     | `registry/shared/lib/finstack-form/form.tsx` (before the next row)                                                                                                                                              |
| `@/lib/finstack/*`        | `src/*`                                                                                                                                                                                                         |
| `@/hooks/<crate>/*`       | `registry/<crate>/hooks/*` for `shared`, `core`, `valuations`, `calibration`, `covenants`, `scenarios`, `attribution`, `analytics`, `portfolio`, `margin`, `models` and `statements`; a new crate adds its line |
| `@/workers/*`             | `registry/shared/file/*`                                                                                                                                                                                        |
| `@/components/finstack/*` | `registry/*`                                                                                                                                                                                                    |
| `@/components/ui/*`       | `components/ui/*`                                                                                                                                                                                               |

A new crate category also adds its directory name to the `include` list in
`scripts/registry-content.mjs`, then runs
`npm --prefix finstack-quant-ui run generate`; without it, the build,
`check-registry`, the gallery and the install matrix never see the crate's items.

To add an item (the gate that catches each omission is in brackets):

1. Write it under `registry/<category>/`, composing stock controls from
   `@/components/ui/*` (never `@base-ui/*`), with colours and fonts from theme
   tokens only [`ui-check`: `check-registry`, `check-tokens`].
2. Declare it in `registry/<category>/registry.json` so every import resolves
   inside its `registryDependencies` closure; visual items must reach
   `finstack-theme`, and a closure that reaches a stock shadcn component must
   contain `finstack-base` [`ui-check`: `check-registry`].
3. Run `npm --prefix finstack-quant-ui run generate` [`ui-check`: generator
   drift]. When that adds or removes a file under `src/generated/`, rewrite
   the repository manifest from the repository root with
   `uv run --no-sync python scripts/generation_digest.py --write-manifest`
   [CI `gen-check` (`--check-manifest`); `ui-check` does not catch it].
4. For a visual item, add
   `docs-site/src/components/registry-gallery/examples/<item>.tsx` and its key
   in `primitives.tsx` or `components.tsx` there [`ui-check`: `gen-gallery`].
   An example that reads a native fixture names it in the hand-maintained
   `sources` map of `scripts/gen-gallery.mjs`, which writes the gallery
   `data.json` [local `ui-e2e`].
5. Cover its behaviour in Vitest under `tests/<crate>/` [review only].
6. Rebuild the gallery export with `mise run ui-docs-build` (`ui-e2e` serves
   that static export, so `ui-docs-install` alone tests a stale gallery), then
   record its four screenshots with
   `mise run ui-e2e -- --update-snapshots -g "<item>"` and inspect them [local
   `ui-e2e`].
7. Check it installs in isolation with
   `mise run ui-build && REGISTRY_INSTALL_ITEMS=<item> node finstack-quant-ui/scripts/install-matrix.mjs`
   while iterating; `mise run ui-install` is the full local gate. Its worker
   role comes from the registry, and an install that needs extra files uses
   `tests/install/fixture/<item>.tsx` [local `ui-install`].
8. When the item changes what an entry in the `displays` array or the
   `deferred` map of `scripts/provenance.mjs` asserts (route, convention,
   deferral), update that entry and rerun `generate`, which rewrites
   `src/contract-provenance.json` [review only; `ui-check` catches an
   unregenerated file].
9. When a worker slice calls a new facade function or class, pick it as a
   leaf in its `<Crate>Native` (never a whole sub-namespace) and run
   `npm --prefix finstack-quant-ui run coverage`, which moves the key from
   `planned` to `covered` in `facade-coverage.json`. That file also lists
   each excluded key with its reason (`data-constructor` or `schema-form`);
   `schema.*` keys, `*Json` wire twins and builders are excluded by rule, and
   a dropped slice deletes its `planned` keys [`ui-check`: `coverage --check`].

CI runs only `ui-check` (the Test UI job); the other gates are local.

`npm run generate` runs `scripts/run-generators.mjs`: `gen`,
`encoding-report`, `catalogue-docs`, `gen-registry`, `gen-gallery`.
`gen-registry` is the sole writer of final registry manifests, the default
stylesheet and the compiled WASM version. It collects authored manifests and
proposed root/theme content in memory, validates the complete dependency graph,
and derives metadata before writing each artifact once. `--check` computes the
same final artifacts and compares their exact contents without writing.
`registry-content.mjs` and `themeArtifacts` are content builders; the brand CLI
remains `gen-theme.mjs --brand <brand.json> --out <theme.css>`.
Native fixtures follow [Native fixtures](#native-fixtures).

| Gate                             | Needs                                                  | Measured                                                               |
| -------------------------------- | ------------------------------------------------------ | ---------------------------------------------------------------------- |
| `ui-test`, `ui-check`            | `finstack-quant-wasm/pkg` and `pkg-node`               | `ui-check` 143 s, of which Vitest 76 s                                 |
| Test UI (CI)                     | Builds both dev packages, then runs `ui-check`         | 1094 s: packages 448 s, `ui-check` 583 s                               |
| `wasm-pkg`                       | Rust toolchain and `wasm-pack`                         | 278 s                                                                  |
| `ui-e2e`                         | The export from `mise run ui-docs-build`               |                                                                        |
| `ui-install`, `ui-publish-check` | A fresh `mise run wasm-pkg`                            | `ui-install` 2995 s (173-item matrix, docs install, composition smoke) |
| `ui-harness`                     | Nothing prebuilt; each harness builds its own registry | 16 s (five harnesses)                                                  |

Local times are wall clock on an Apple M5 Max (18 cores, 128 GB, macOS),
measured one gate at a time on 2026-10-28. Test UI is CI run 40609354672
(2026-10-28) on the hosted `ubuntu-24.04` runner.

`mise run wasm-build` and `wasm-build-node` build the development packages and
`wasm-pkg` the release ones. Each `ui-*` task that needs a WASM package checks
for it before any WASM-dependent work and names the missing build;
`ui-docs-install`, `ui-docs-build`, `ui-install` and `ui-publish-check` run the
`ui-build` registry build, which needs no WASM, before that check.

## Ownership and generation

`scripts/gen.mjs` reads the canonical crate schema indexes by URI. Ref Parser
resolves offline with both network and filesystem fallback disabled. Only
reachable definitions enter each bundle; stable URI-derived keys distinguish
same-named definitions, retain recursive references and preserve reference
annotations. Metadata uses JSON Schema pointers plus original canonical source
pointers and refs, including non-`type` discriminator constants. Defaults,
descriptions and supplied `x-` annotations remain in the committed bundles.
Numeric type or a decimal reference never assigns a financial unit.

Wide instrument roots (schemas with more than 80 local definitions) reference
one shared definition document instead of copying those definitions into every
bundle. Their instrument modules link that document back into a closed schema
before validation. Narrow roots stay self-contained.

The fixture manifest is rediscovered from the instrument example directory and
calibration bootstrap directory. Every instrument root requires one example.
Calibration envelopes validate as inputs; an optional returned `final_market`
is recorded separately against the market-state schema. Fixture filenames and
counts are not hard-coded. All generated files are committed; `--check` compares
contents and detects missing and stale files without writing them.

The [Zod converter](https://zod.dev/json-schema) is experimental, so its exact
version is pinned. The small converter projection removes validation-time
default insertion, preserves `$ref` sibling assertions, and lowers the canonical
closed object union only when required, distinct string discriminators prove
branches disjoint. Other `unevaluatedProperties` shapes fail generation. Tuple
schemas are projected to the TypeScript generator's supported tuple syntax.
Original bundles remain unchanged by these projections. Ajv 2020-12 is a
test-only oracle for the supported construct and boundary corpus.

## Native fixtures

`mise run ui-fixtures` runs the twenty-four native fixture generators under
`tests/**/generate.mjs` in dependency order (results → scenario application,
horizon return, scenario templates, attribution, cashflows → details; loan yields;
portfolio → portfolio margin; curves → market-browser → curve-query; cubes, calibration,
scenario-table; statements → covenants; analytics; models credit, risk, options and liability management;
margin), then
refreshes the gallery copies with `scripts/gen-gallery.mjs`. It needs
`finstack-quant-wasm/pkg-node`, built by `mise run wasm-build-node` (dev) or
`mise run wasm-pkg` (release). `mise run ui-fixtures --check` verifies that every
committed fixture matches a fresh regeneration, ignoring formatting and key
order, and names the generator to rerun;
`ui-check` runs that check before vitest. Fixtures record only the SHA-256 of the
JSON inputs they were built from; no binding source or WASM binary is pinned. A
Rust pricing change that moves a fixture is regenerated in the same commit, with
the reason in the commit message.

## Validation boundary

These validators establish JSON structure, not financial validity. Rust/WASM
still owns schedules, market lookups, cross-field rules, pricing and calibration.
No financial rules, unit tables or arithmetic are implemented here.
Schema field, metric and registry names render through `labelFor`; components
add no curated label maps for those names. A composing editor presents the
choices its own controls offer: instrument-form's desk-quote and term-sheet
tables, the statements workbench's forecast and rollup method lists and the
valuation-details raw-detail labels.
`check-no-arithmetic` enforces this in `ui-check`: arithmetic in `registry/` or
`src/` needs a `// finstack-arith: <reason>` comment or a reasoned glob in
`scripts/no-arithmetic-allow.json`, and financial logic moves to Rust.
`check-worker-compose` flags an `if`, `?:`, `??`, loop or `.map`-style iteration in
`registry/shared/file/services/**` that reads a native result: a decision moves
to Rust and pure composition takes `// finstack-compose: <reason>`. A value traces
from the service's `native` parameter by symbol; request arguments, handle-passing
chains, plain maps over user-supplied points and `x ?? null` are not flagged. The
test for any line of JS: would Python get a different number or decision without
it?

Generated interfaces describe wire shapes and cannot express every runtime
constraint (bounds, patterns or exclusive unions). They are not WASM host types.
Use each instrument loader's `codec.parse(text)` for JSON input and
`codec.stringify(value, rustCanonicalizer)` for export. The validator returns UI
state with schema `int64`/`uint64` fields as bigint; generated wire interfaces
still describe serialized JSON. Decimal-string money is unchanged. Rounded
integers and non-JSON host objects fail instead of silently losing information.

`finstack-quant-ui/host` reuses published facade types. `adaptValuation` preserves
actual structured results, and `exportValuation(result,
valuations.valuationResultToJson)` emits integer tokens validated by Rust.
All five detail variants use Rust-derived WASM host declarations and validation
metadata. Monte Carlo has a dedicated view; the other four retain raw views.
Path counts are plain numbers (Rust `wire::count`); only schema `int64`/`uint64`
fields, such as seeds, arrive as bigint. Cashflows arrive as the facade's
envelope object, validated against the published Rust `instrument_cashflow`
schema, including mixed-currency FX-swap rows. Its JSON view is `serializeHost`
text of that object; the facade returns no cashflow text.

`src/contract-provenance.json` records canonical schema pointers, exact facade
signatures and inputs, conventions, raw routes and deferred gaps. It includes
cube/surface constructors and the Rust authority for scenario-price conventions.
`getCurveView` projects unchanged stored knots for seven canonical variants;
base-correlation and parametric variants expose schema-declared fields. It
expects validated market state and does not rebuild or interpolate curves.
`getValueView` retains raw values and source help when unit metadata is absent.
All generated artifacts and provenance participate in the repository digest,
manifest and `gen-check`; `ui-check` verifies their drift and manifest coverage.

The scoped tests price all five result-detail variants using the actual facade,
transport an actual derived seed above 2^53, validate a u64::MAX transport-boundary
mutation through Rust (not a claim of pricing with that seed), structured-clone
the cashflow envelope object, and preserve the published Float64Array return.
These are Node structured-clone tests. PR-003 owns the production browser-worker
proof; static export and installation remain later gates. Full `gen-check` and
Rust schema gates are separate library-wide validation and are not implied by a
passing `ui-check`.

## Production browser worker smoke test

PR-003 retains an isolated Next production-export fixture using the docs site's
shared configuration and local dependencies. After `mise run wasm-pkg`, install
the docs dependencies with `npm --prefix docs-site ci --ignore-scripts`, make
sure Playwright's headless Chromium is present (`mise install` provisions it),
and run `node finstack-quant-ui/tests/browser/run.mjs`. It checks root and
`/finstack-quant` deployments and removes its temporary app afterward.
`REGISTRY_SMOKE_DIR` selects a directory for JSON evidence.

The fixture runs independently of the docs notebook publication suite. The browser
smoke test is separate from the fast `ui-check`; the footprint gate below owns
optimized-size acceptance.

## Browser footprint gate

The footprint gate measures all 78 instrument validators and the optimized
`release-size` WASM package. The optimized raw artifact is 39,345,664 bytes
against the 40,000,000-byte limit. Compression does not waive this raw-byte gate.

`mise run wasm-pkg` owns the optimized web build and records raw/optimized/gzip/
Brotli measurements without a UI-specific limit. `npm --prefix finstack-quant-ui run size` checks the
selected artifact hash and enforces the 40,000,000-byte UI budget.

`REGISTRY_MEASURE_FOOTPRINT=1 node finstack-quant-ui/tests/browser/run.mjs` requires `REGISTRY_WASM_PACKAGE` to select the optimized web
package and matching generated glue, and runs the size gate before browser
measurements. `REGISTRY_WASM_PACKAGE` also selects the package for `npm run size`; it
defaults to the repository web package.

## Registry distribution

`mise run ui-build` uses shadcn 4.21.0 and writes installable JSON to `public/r`.
`finstack-base` pins Base UI 1.8.0, React 19.2.8 and Tailwind 4.3.3. Financial
components compose the official shadcn Base UI components using `base-nova`.
Their `registryDependencies` install upstream controls under `components/ui`;
our namespaced items own financial presentation, state and block composition.
Upstream component source is unmodified. Use supported component variants and
application CSS variables; do not override control internals, sizing or indicators.
The docs and offline fixtures use verified CLI output whose source URLs and
SHA-256 hashes are recorded in `components/shadcn.json`. `ui-check` rejects
modified stock source and direct Base UI imports from domain components.

Every generated contract has one owner and installs under `lib/finstack` with
relative imports. `instrument-catalogue` depends on all instrument modules for
installation, while its runtime imports remain lazy. `contract-bond` can install
independently with just its codec closure. `finstack-host` declares the
workspace WASM package at its current version.

The root registry owns existing `src/` contracts. The pinned CLI forbids parent
traversal from included indexes, so category indexes own only sources beneath
their directories. This follows the upstream [registry composition rules](https://ui.shadcn.com/docs/registry/registry-json)
without duplicating generated source. No visual placeholders are published.

`ui-check` verifies generated entries, dependency closure, target ownership and
imports, and type-checks files extracted from a fresh CLI build in a temporary
consumer with its own `@/*` application alias (no workspace aliases). Package dependencies are supplied from the locked
installation; a complete network install matrix is a later slice.

## Shared theme

Install `@finstack/finstack-theme`, then import `styles/finstack/theme.css` after
Tailwind. Set `data-theme="light"` or `"dark"` and `data-density="compact"` or
`"comfortable"` on the application root. Compact is the default. UI, table and
chart wrappers share these properties; numeric text uses `finstack-numeric`.

The single token source includes the Fontsource package versions, selected faces
and SIL OFL-1.1 licences. `npm run generate` emits CSS and registry metadata; `ui-check`
checks drift, literal colours/fonts and contrast, including the brand example
and the tenant example.

### Branding

Every colour, font family and chart palette the registry renders is a CSS
variable declared from `registry/theme/finstack-theme/tokens.json`, and so are
the type scale, spacing, radius and density that the Tailwind utilities and the
component rules use. Components use the semantic utilities mapped to those
variables; `check-tokens` rejects literal colours and fonts in `src/` and
`registry/`, and the theme test proves every colour literal in the generated
stylesheet other than `transparent` is a token value. A few fixed label sizes
remain in component rules, and the stock controls take their shadows from
Tailwind's translucent black. The native chart tooltip, crosshair and focus
overlays read the same variables through the chart library's hooks. The
registry ships no logo or wordmark (`PricingWorkbench` renders one only
through its `brand` prop), so a brand is the token set. Two paths apply one:

- **Brand file, for a complete rebrand.** Write a JSON file with the sections of
  the token file (`theme`, `light`, `dark`, `density`, `fonts`, plus an
  optional `name` label) listing only the keys you change;
  `registry/theme/finstack-theme/brand.example.json` is a worked example. After
  `mise run ui-sync`, run `mise run ui-brand -- <brand.json> <out.css>` from any
  directory in this repository (paths are relative to where you run it), or
  `node finstack-quant-ui/scripts/gen-theme.mjs --brand <brand.json> --out <out.css>`
  from the repository root. The generator merges the file over the defaults,
  rejects unknown sections, unknown keys and palette keys set in only one mode,
  runs the same contrast rules as the default theme (`checkContrast` and
  `checkStockContrast` in `scripts/check-tokens.mjs`), and writes a complete
  stylesheet to install in the application in place of
  `styles/finstack/theme.css`; Tailwind utilities, component rules, chart
  palettes, tooltips and printed reports all follow. The brand file is the
  durable source: re-adding `finstack-theme` overwrites the installed
  stylesheet, so regenerate it after every re-add. A `fonts` list replaces the
  Fontsource imports: each entry needs `package`, `version`, `family`,
  `faces` and `license`, the application must install every listed
  `package@version` (the generator prints the list), and a non-empty list must
  still supply each family that `theme.font-sans` and `theme.font-mono` name.
  An empty list means the application loads its own fonts and sets both theme
  fonts.
- **Runtime overrides, for tweaks.** A stylesheet loaded after `theme.css` may
  redeclare any variable on `:root, [data-theme="light"]` and
  `[data-theme="dark"]` (`tests/shared/fixtures/tenant.css`). Declare light
  values on `[data-theme="light"]`, not only `:root`, because the printed
  report region switches to the light theme. Runtime overrides are not
  contrast-checked; prefer a brand file when more than a few keys change.

Popups and chart tooltips render in a portal, so they follow a section-scoped
`data-theme` only through the `finstack-surface` item, which the registry
primitives stamp on their portaled content; a theme set on the application
root needs nothing more. The theme item ships no `cssVars`, so the shadcn CLI
writes no token block into the application stylesheet that could override a
brand. Known limit: inside a dark application, Tailwind `dark:` variants from
the dark ancestor still apply within the printed report region after it
switches to `data-theme="light"`; the palette variables are re-declared there,
so colours stay the light tokens.

## Display formatting

`finstack-format` installs `lib/finstack/format/{format,columns}.ts` and
`label.mjs` with its `label.d.mts` declaration.
Scalar formatting is independent of WASM initialization.

`formatMoney` preserves the amount and applies returned per-currency rounding
stamps through the native `core.Money` constructor supplied by the caller
(inside the worker for browser consumers). Without an explicit scale it
preserves all supplied digits; `formatRawMoney` is the same exact display
without any stamp.
`formatRate` requires a source-backed wire/display convention; absent metadata
returns raw text and `unit: null`. Decimal conversions use exact decimal text;
source strings remain the caller's state. Grouping currently uses en-US.
`isoToEpoch` and `epochToIso` call the native core date functions in an initialized
host (inside the worker for browser consumers). Column presets target Table
9.2.4 and add formatting/alignment only.

`labelFor` (`label.mjs`, also used by `registry-content` for item titles) turns a
Rust wire or metric name into a sentence-case label with one set of upright
market acronyms; it is typography only and adds no financial names.
`compareCoordinates` sorts coordinate labels by the group and value Rust publishes
on `MetricMetadata.coordinates` (a tenor in years or a decimal strike), then by
the label text; nothing parses a label for its maturity.

## Controlled primitives

The controlled input and value primitives install under
`components/finstack/<crate>/primitives` (`core`, `valuations`, or `shared`).
Shared chrome such as `field-frame` stays in `components/finstack/shared/primitives`.
`FinstackTable` installs at `components/finstack/shared/table` and `FinstackChart`
at `components/finstack/shared/chart`. Each item declares its theme and actual
dependency closure.
Cross-category imports use the consumer's standard `@/lib/finstack` alias;
configure `@/*` to the application source root. The CLI handles `src` layouts.

Inputs receive controlled values, labels, help, constraints and options. Calendar,
model, metric and ID choices come from the caller. Decimal money stays text;
number-backed rate and knot inputs preserve incomplete/invalid text for editing.
Only explicit source-backed rate presentation changes display units. Domain
validation remains in Rust. Date calendars only present dates and any supplied
business-day highlights.

`FinstackTable` uses Table 9 with no optional features and caller-owned stable row
IDs; `KnotTable` reuses it. `JsonViewer` defaults to a readable formatted view
using pinned `lossless-json`, preserving number tokens without converting them
to JavaScript numbers. Its Original toggle shows the supplied bytes; Copy and
Download always use those original bytes. Print uses the compact original text
with wrapping, independently of the screen toggle. Invalid JSON, including duplicate keys,
stays in Original view with a formatting notice. `mise run ui-harness` (`tests/shared/browser/primitives.mjs`) builds an isolated
consumer from actual registry JSON, checks keyboard/focus/clipboard behavior and
10,000-option virtualization, and runs axe in both themes and densities.

## Worker and query boundary

Install `@finstack/use-price-instrument` and/or `@finstack/use-instrument-validator`.
They include `use-finstack` and the worker closure. Hooks install at
`hooks/<crate>/<item>/` and the worker/service/contracts at `workers/`, both under
the consumer's source directory (`src/` when the application has one).
The worker uses the consumer's standard `@/lib/finstack` alias for the installed
codec. The shared registry index owns these worker files.
The contract (`workers/finstack-contract.ts`) groups the API by crate
(`CoreApi`, `ValuationsApi`, `CashflowsApi`, `CovenantsApi`, `CalibrationApi`,
`ModelsApi`, `StatementsApi`), and
each method's comment names the facade call it wraps. The service
(`workers/finstack-service.ts`) is a composer over `workers/services/<crate>.ts`,
which share `workers/services/context.ts`. A new crate adds its interface to the
contract and to the `WorkerApi` intersection there, one slice in
`workers/services/<crate>.ts`, its `<Crate>Native` in the `createService`
parameter type and one spread in `finstack-service.ts`, the import and pass of
its facade namespace in `workers/finstack.worker.ts`, and a `files` entry in
the `finstack-worker` item of `registry/shared/registry.json`. The method-set
test in `tests/shared/worker.test.mjs` lists every method exactly, so the new
methods join it there; it is the tripwire that fails when a spread is missing.
The Node test worker (`tests/shared/worker/node.mjs`) passes the whole facade
and needs no edit.
Only `finstack.worker.ts` imports the facade as a value; every facade member a
service calls is a leaf of its `<Crate>Native` Pick, which
`scripts/coverage.mjs --check` holds equal to `covered` in
`facade-coverage.json`.

Use `FinstackQueryProvider` around a standalone feature, or `FinstackProvider`
inside the application's existing `QueryClientProvider`. `useFinstack` exposes
`starting`, `ready`, `error` and `reset`. Nothing initializes during SSR. One
provider shares one worker; cleanup rejects pending calls and terminates it.
Use the matching built `finstack-quant-wasm` package, linked locally until the
release packaging slice. An optional `wasmUrl` selects a matching web artifact;
reset or changing that URL creates a fresh worker and query session.

`usePriceInstrument` requires the complete immutable request (`instrumentJson`,
`marketJson`, `asOf`, `model`, `metrics`, `metricPricingOverrides`, `marketHistory`). JSON
strings remain unchanged, including wide integer tokens. Results are structured
clone values; export through the worker's `exportResult` method for native
canonical JSON. `useCashflows` (the individual `use-cashflows` item) returns the native cashflow
envelope object. `CashflowViewer` accepts that object as `cashflows` plus optional loading/error props and needs no provider. It presents native rows in a table, with `serializeHost` JSON copy/download in a secondary **Envelope JSON** disclosure;
unsupported export preserves caller pricing/model state and shows the native error. The workbench owns one cashflow query for both display and its print snapshot.
**Principal before** and **Principal after** show the envelope's `beginning_balance` and `ending_balance` as supplied, in the row currency. Rust fills them per row for bonds, term loans and revolvers (a coupon shows the principal it accrued on, a PIK, amortization, sweep, draw or repayment row moves it, and an issue funding row starts from zero) and per period for agency MBS (rows of one period share the pool balances); nothing is accumulated in JavaScript, and the columns hide only when no row carries a balance.
`useCashflowLadder` (the individual `use-cashflow-ladder` item, beside
`use-cashflows`) takes that envelope object and calls
`valuations.instruments.calendarYearLadderByCurrency`: one ladder per row
currency in first-appearance order, each over `native_pv`, the row-currency PV
before FX conversion and any scenario price shock. `CashflowViewer`'s optional `ladder` (and `ladderError`) props add a
Schedule / Annual ladder toggle with one table per currency headed Native PV;
nothing is summed or converted across currencies.
`useDiffStatementScenarios` (`use-diff-statement-scenarios`) sends the scenario
set, the `serializeHost` text of its evaluated results, baseline/comparison
pairs, metric ids and periods; the worker calls
`statements_analytics.scenarioDiff` once per pair, in pair order, and a metric
missing at a period fails the request with the native error.
`useTraceStatementScenario` (`use-trace-statement-scenario`) returns
`scenarioSetTrace`'s lineage, root first; its `useTraceStatementScenarios`
runs one query per scenario. The workbench scenario panel defaults each
scenario's baseline to the entry before it in that lineage and shows Rust's
`abs_var` as delta columns and the shown pair's `VarianceReport` in
`VarianceTable`; capital-structure rows have no native diff and show none.
`useCreditAssessment` (`use-credit-assessment`) and `usePlSummaryReport`
(`use-pl-summary-report`) send the `serializeHost` text of an evaluated
result; the worker calls `statements_analytics.creditAssessment` at the
requested period and `plSummaryReport` over the line items and periods in
request order. The workbench's Metrics & ratios tab asks for the active
period's assessment and a P&L summary of the income-statement lines over every
reported period, and shows both in `CreditAssessmentReport`: the returned
values, the two ratios charted over Rust's series with the periods Rust leaves
without a value listed instead of plotted, and the summary through
`TableEnvelope`.
`useEvaluateDcf` (`use-evaluate-dcf`) and `useDcfSensitivity`
(`use-dcf-sensitivity`) pass the model, WACC, `TerminalValueSpec` and the
optional UFCF node, net-debt override and `DcfOptions` JSON to
`statements_analytics.evaluateDcf` (plus the market and as-of date, which
Rust requires together) and `dcfSensitivity` (which takes no market)
positionally, leaving every unset optional argument unset. The workbench's
Valuation tab composes `DcfAssumptionsForm` (the terminal-value union is the
one hand-written wire shape; the options are a schema form over the generated
`dcf_options` contract, checked structurally and validated by Rust when the
model is valued, because the facade has no standalone `DcfOptions` parse) and
`DcfReport`, which shows the valuation, bridge fields, per-period present
values, terminal value and the native tornado through `TornadoChart` as
returned.
`useEvaluateLbo` (`use-evaluate-lbo`) passes the model and `LboConfig` JSON
to `statements_analytics.evaluateLbo`. The Valuation tab's `LboReport` edits
the config with a schema form over the generated `lbo_config` contract (string
`*_node` fields as id-comboboxes; checked structurally and validated by Rust on
evaluation, as there is no standalone `LboConfig` parse) and shows the
returned sources and uses, entry and exit values, MOIC and the LBO check
report. Returns are MOIC only: IRR is not part of the native result.
`useRunSensitivity` (`use-run-sensitivity`) builds each parameter with
`statements_analytics.parameterSpecWithPercentages` (the base value is the
evaluated cell, a missing or non-finite one sent as NaN), runs
`runSensitivity` and returns its `generateTornadoEntries` at the target metric
and period, which Rust returns as null for a `full_grid` run it cannot rank.
`useGoalSeek` (`use-goal-seek`) passes the target, driver, update flag and an
optional bracket to `goalSeek`, leaving an unset bracket unset. Below the
Scenarios tab's scenario set, `StatementSensitivity` shows the native tornado
and the target metric in every returned scenario, and `GoalSeekForm` solves
only on Solve; Apply to model is a second, explicit action that commits the
returned model through the workbench (no undo), blocked once the model has
changed since the solve.
`useRunVariance` (`use-run-variance`), `useVarianceBridge`
(`use-variance-bridge`) and `useBacktestForecast` (`use-backtest-forecast`)
pass two host-serialized results to `statements_analytics.runVariance` (with
a `VarianceConfig`) and `varianceBridge` (target, period, drivers, labels),
and two aligned series to `backtestForecast`. When a roll forward is accepted
the workbench keeps the pre-roll model and result; the Forecast tab's "Since
last roll forward" section then compares them over the periods the roll
reported (forecast before, actual after), offering only nodes present in both
results: `VarianceTable` for the variance, `VarianceBridgeChart` (baseline,
one signed bar from zero per driver contribution in Rust's order, Rust's
unexplained residual and the comparison; no cumulative positioning, since the
contributions are raw driver deltas) and `ForecastBacktest` for the pre-roll
forecast against the reported values.
`useEvaluateMonteCarlo` (`use-evaluate-monte-carlo`) passes the model and
`MonteCarloConfig` JSON to `statements.Evaluator#evaluateMonteCarlo` (the
schema form keeps the `uint64` seed an exact integer token, so a seed above
2^53 reaches Rust unchanged), and `useMonteCarloBreachProbability`
(`use-monte-carlo-breach-probability`) passes host-serialized results, a
metric and a threshold to `statements.monteCarloBreachProbability`, returning
null when Rust reports none (results without path data). The workbench's
Monte Carlo tab composes `StatementMonteCarlo`: the config form (starting
empty), a fan of bands between the returned percentiles paired from the
outside in (the unpaired middle one a line), the percentile table, the breach
readout and the warnings, all as returned.
`useModels`, `useMetrics`, and `useCalendars` return native option registries.
`useMetrics(instrumentType)` returns every metric Rust registers for that type
(`metricsForInstrument`), by native group, with those outside a group under
"Instrument Specific"; `null` disables it. `PricingParamsForm` reports selected metrics the type does not offer and
never drops them.
`useInstrumentValidator` accepts canonical text and an optional abort signal;
`SchemaForm` owns the 250ms debounce and prevents superseded responses from
replacing the current validation state.

Run `UV_NO_SYNC=1 mise run wasm-pkg` for scoped package builds; disabling uv
synchronization prevents the final marker-file command from rebuilding Python.
The production browser harness now exercises the installed provider and worker,
not a separate probe implementation.

## Publication figures

`@finstack/finstack-chart` installs the shared SVG figure; `@finstack/figure-example`
installs a standalone controlled-data example with SVG/PNG download actions.
Supply a native `defineChart` definition, an accessible label, and optional title,
subtitle, caption, sources and `figureAnnotations` at explicit figure fractions.
Use native text, dot, arrow, rule and rectangle marks for data-coordinate
annotations. Native scale/axis props carry supplied unit labels, tick formatting,
thinning and explicit label rotation. No units or financial values are inferred.

A `FigureHandle` ref exposes `exportSvg` and `exportPng` with explicit width/height,
optional raster scale, theme and background. Exports use the same composition at
that size after fonts load. Dimensions too small for the supplied prose reject
instead of silently dropping text. On screen, a container too small for the prose
(a panel animating open, for example) draws the plot without it instead of
throwing. Same-origin font stylesheets allow the export
to embed loaded webfonts; desktop editors may require the font installed locally.
PNG scale changes pixel density, not data/layout coordinates. For example, a
900×600 layout at scale 2 gives 1800×1200 pixels, suitable for 6×4 inches at 300 PPI.
The original inputs are unchanged. SVG includes all persistent prose, sources,
axes, legends and marks, without HTML overlays or rasterized marks.

`mise run ui-harness` (`tests/shared/browser/figures.mjs`) builds the installed example, exports narrow/wide
numeric/date/category fixtures, checks text clipping/overlap, vector elements,
font/style portability, exact raster dimensions, accessibility and no WASM loads.
Pages 14.5 places the complete SVG at 15.24 × 10.16 cm with IBM Plex Sans
400/500/600 and Mono 400/500 installed. Keep it as a placed SVG: Pages
“Break Apart” discards SVG text and axes. The exported SVG retains editable
text and vectors for SVG-aware editing; Pages shape conversion is unsupported.

## Linked chart interactions

`FinstackChart` forwards typed `onFocusChange`, `onFocusGroupChange`, `onSelect`
and `renderTooltipBody` to the native chart adapter. The tooltip context includes
`defaultBody`, original typed points, `pinned` and `dismiss`; its controls retain
normal keyboard behavior, and Escape uses native dismissal.

`useLinkedSelection({ selectedKey, onSelectedKeyChange })` shares an accepted
string key between charts and ordinary controls. Omit `selectedKey` for local
ownership and optionally provide `defaultSelectedKey`. Call `select(key)` or
`clear()`; equal accepted keys are no-ops. The controlled parent can reject a
proposal. Data removal does not silently change the accepted key.

Derive `chartSelection(binding, datum => datum.id)` on render and assign it to
`definition.selection`. Use native `whenSelected(mark, selection)` for decorative
highlights without extra hit targets. `onSelect` only reports activation; writing
accepted state again there would duplicate the native selection proposal. Shared
cursors use a parent-owned native `createChartCursor`, with explicitly compatible
coordinates. See the installable `LinkedFigureExample`.
The hook imports only React; chart-only consumers load neither tables nor WASM.
`mise run ui-harness` (`tests/shared/browser/interactions.mjs`) builds the
installed `figure-example` with two `LinkedFigureExample` parents and checks
controlled accept, reject and clear, focus preservation, the shared cursor,
keyboard-pinned tooltip actions and that figure export omits the transient
tooltip, with no TanStack Table or WASM module bundled and no `.wasm` request.

## Bond schema form

Install `schema-form`, `contract-bond` and `use-instrument-validator`. Inside the
existing `FinstackProvider`, call `useInstrumentValidator()` and pass its result
as `validate` to `SchemaForm`. Supply the lazily imported generated bond module
as `module`; `onSubmit(json)` receives native canonical JSON. The form uses the
module's example initially, or supplied `defaultValues`. Remount when switching
instruments so active edits are never replaced by new defaults.

Structural validation runs on changes; native validation is debounced and ignores
superseded responses. Numeric edit text is converted at validation/output boundaries;
exact decimal text and full-width integers retain their canonical representations.
`layout="basic"` collapses defaulted or empty values under Less common terms;
`layout="full"` exposes them. `disclosure`, `sheet` and `promotedField` let a
composing editor group domain terms, choose the term-sheet root and promote one
input; `instrument-form` supplies the instrument term sheet (convention groups
and the desk quote). A bare `SchemaForm` can pass
`sheet={{ path: "instrument.spec", label: "Instrument terms" }}`.
`fields={{ allow, deny }}` uses canonical paths such as
`instrument.spec.notional.amount` and preserves hidden values. Schema errors appear
under fields where mapped, with remaining native errors in the summary.

The shared field kit is available through `useAppForm`. Complete cross-instrument
rendering and field-error mapping remain separate planned slices.

## Supplied valuation results

`ValuationSummary` displays a supplied result's value, instrument ID, as-of date
and calculation stamps, with full metadata in a disclosure. `compact` controls
presentation; `compareTo` retains an independent result with its own currency,
date and metadata. `density` accepts the shared compact/comfortable modes. Missing
metadata does not create rounding or FX defaults.

`MeasuresGrid` composes that context with the unlinked table. Labels are the
native metric id in sentence case with recognised acronyms upper-cased (`ytm`
reads YTM); prose
names would need a Rust metadata field. Native metric metadata groups qualified
buckets by identifier with tenors in maturity order; keys without metadata remain
opaque. The Exact keys control reveals fully qualified keys. Values display six
significant digits by default;
Exact values and the value title retain the supplied precision. The nonzero view
hides zero rows (both sides must be supplied and zero in a comparison); Show all
reveals every row. Missing values remain distinct from zero. Units appear only when native
metadata or `units`/`comparisonUnits` supplies a source-backed unit for the complete
key. The grid does not calculate differences, totals or currency conversions. The
retained native bond result fixture is `src/fixtures/results/bond.json`.

## Returned structured-credit scenarios

With the `@finstack` registry namespace configured, install the standalone component:

```sh
npx shadcn@4.21.0 add @finstack/scenario-heatmap
```

```tsx
import { ScenarioHeatmapPanel } from "@/components/finstack/valuations/components/scenario-heatmap/scenario-heatmap";

// table is the unchanged result of trancheScenarioTable.
// Choose an explicit display extent containing every supplied price and par 100.
<ScenarioHeatmapPanel table={table} priceDomain={[80, 140]} />;
```

`ScenarioHeatmap` accepts controlled `severity` and `onSeverityChange`, optional
shared `link`, publication text, annotations, tooltip/callback props and a
`figureRef` for the existing complete SVG/PNG export API. CPR, CDR and severity
remain their returned decimal values. Color is the returned clean settlement
price as a percentage of **current tranche balance**, with 100 as par. The Rust
`ScenarioCell.price` contract owns this wording; the facade's original-balance
comment remains an upstream documentation exclusion. No rebasing, normalization
or WAL/writedown overlay is performed. Neither the component nor its panel
requires a worker or workbench block. Native fixtures, including a factor-adjusted
tranche, live in `tests/valuations/scenario-table/cases.json`.

## Returned tranche analytics

`TrancheAnalytics` (`@finstack/tranche-analytics`) presents one tranche's native
`trancheMetrics`, `trancheBreakevenCdr`, `trancheDiscountMargin` and `trancheOas`
results, each passed as `{value, error, loading}` so every tile shows its own
native error. `TrancheMetrics` fields appear in Rust declaration order with the
unit each Rust field documents (bp, years, decimal, percent of current balance,
the returned currency); requested registry metrics stay raw. Break-even CDR,
discount margin and OAS are returned decimals. The clean price is a controlled
exact-decimal input, percent of current balance, with no default: the
discount margin waits for one, and OAS runs only through `onSolveOas`.
`use-tranche-analytics` holds the four query option builders and hooks
(`TrancheRequest` plus an optional metrics price and metric ids, or a required
`marketPricePct` for OAS and discount margin); the worker loads the deal once
per call and frees it. OAS takes an optional complete `OasConfig` JSON
(`configJson`) and otherwise uses Rust's default; with `onOasConfigChange` the
component shows an OAS configuration disclosure over the generated
`contract-oas-config`, seeded from its Rust example (`OasConfig::default()`,
since the type carries no per-field schema defaults), and hands the applied
JSON to the host. The workbench
mounts it above the scenario prices for the configured `scenario.trancheId`,
valuing `scenario.trancheMetrics` registry metric ids on the note. Native
fixtures (fixed-rate `CLONOTES-A` and the floating `SENIOR` note of the CLO
mezzanine regression golden) live in
`tests/valuations/scenario-table/tranche-analytics.json`.

The complete workbench is installed with `npx shadcn@4.21.0 add @finstack/pricing-workbench`.
Embed `PricingWorkbench` inside `FinstackQueryProvider`. Supply an explicit
a required `defaultRequest` (the caller owns example/default data) and optionally `defaultCalibrationJson` (a complete canonical
envelope). The Calibrate tab can also import an envelope, shows native static
diagnostics even for semantically invalid plans, and runs the solver only on
submission. **Use calibrated market** validates the returned market and updates
the shared market editor and pricing request. Editing the envelope disables that
handoff until the current envelope has been solved.

Evaluated market views take explicit domain inputs through public options:

```tsx
<PricingWorkbench
  defaultRequest={request}
  defaultCalibrationJson={envelopeJson}
  surfaceOptions={() => ({ colorDomain: [0, 1] })}
  cubeOptions={() => ({
    initialStrike: 0.05,
    initialConvention: "normal",
    colorDomains: { normal: [0, 0.1], black_lognormal: [0, 2] },
    slice: {
      initialStrikes: [0.03, 0.04, 0.05, 0.06, 0.07],
      initialRate: 0.04,
      repairIterations: 10,
    },
  })}
  fxOptions={() => ({
    coordinates: [{ expiry: 1, strike: 1.1, forward: 1.12 }],
    initialSpot: 1.1,
    initialDomesticRate: 0.04,
    initialForeignRate: 0.03,
    colorDomain: [0, 1],
  })}
  scenario={{ trancheId, gridJson, priceDomain: [80, 140] }}
/>
```

Those numbers are illustrative caller inputs, not financial defaults. Use extents,
strike, forward and coordinates appropriate to the supplied objects. The FX
strike grid (`models.volatility.materializeFxDeltaSurface`) takes a spot in
domestic per unit of foreign and continuously compounded decimal rates; the FX
delta state carries no currency pair, so the workbench cannot look either up and
leaves the fields empty unless `fxOptions` seeds them. The cube `slice` option
mounts `vol-cube-slice`: a fixed-tenor or fixed-expiry slice that Rust
materializes (`models.volatility.materializeCube{Tenor,Expiry}Slice[Normal]`) at
the given absolute strikes, plus the selected node's native SABR smile. The
smile's arbitrage check takes an explicit continuously compounded decimal rate
and the repaired line appears only when `repairIterations` is supplied; neither
is defaulted, and without `slice` no slice is shown. Without
options, the market browser retains exact stored data. Scenario prices are fetched
only when the detail panel is opened for a completed structured-credit request.
The panel edits the native `ScenarioGrid` (`contract-scenario-grid`) seeded from
`scenario.gridJson`: the supplied grid runs as given, an edited grid runs only on
**Run scenarios**, and Rust validates the axis values and the cell cap.
`calibrationChartOptions` forwards existing figure annotations, export refs and
interaction callbacks; the workbench owns the shared report/chart selection.

## Horizon return and P&L attribution

`useHorizonReturn` (`@finstack/use-horizon-return`) calls
`scenarios.computeHorizonReturn` on an instrument, its market and valuation
date and a canonical `ScenarioSpec` (for example one edited in
`market-scenario-builder`), then `horizonResultExplainText`. Omitted `method`,
`calendarId` and the config stay unset, so Rust applies its default
attribution method and a weekends-only roll calendar; an unknown calendar or a
failing attribution returns the native error (`metrics_based` on a hazard
curve without a calibration recipe drops the quote-space credit metrics with a
note, and its credit P&L flows to the residual). The display-only
`HorizonReturnReport` shows Rust's summary (total and annualized return and
factor contributions as decimal fractions; an undefined value, such as the
annualized return without a time roll, reads "not defined", never zero), the
horizon, the endpoint values, the explain text, the attribution and, in a
disclosure, the scenario report. `PnlAttributionReport` (the `attribution`
category) shows any returned `PnlAttribution`:
total, mark-to-market and endpoint values, every factor bucket, a signed
factor chart, the waterfall drawn only from Rust's `waterfall_steps`, the
residual and the notes. Its required `buckets` prop takes Rust's
`pnlAttributionPctOfTotalByBucket`, which names the buckets, their amounts and
shares. Its optional `explain` and `residualWithinTolerance` props take
`pnlAttributionExplainText` and `pnlAttributionResidualWithinTolerance`;
without the residual check no verdict is shown. In `pricing-workbench` the
horizon return sits in the Market scenario results tab under the applied
scenario, with method and roll-calendar choices.

`useAttributePnlMany` (`@finstack/use-attribute-pnl-many`) calls
`attribution.attributePnlMany` with one `AttributionInputs` built by splicing
the two canonical market texts (through the worker's market cache) and the
dates into JSON text, never parsing them, over the instrument envelopes in
order (`attributePnl` takes an `AttributionSpec`, not an envelope). Each
result comes back with `pnlAttributionExplainText`,
`pnlAttributionResidualWithinTolerance` (the run's own tolerances) and
`pnlAttributionPctOfTotalByBucket`, ready for
`PnlAttributionReport`. Rust requires the method, so the request always
carries one; every other input stays unset. `useDefaultWaterfallOrder` returns
`defaultWaterfallOrder`, the factor order a `{waterfall: order}` method uses.
The workbench's P&L explain results tab pins the last priced request's market
and date as T0, takes T1 from the last priced request or the applied market
scenario, and attributes the last priced instrument between them with a
chosen method (none is preselected); a changed instrument is repriced at T0
too, which the tab says. On the workbench bond market (no calibration recipe
for its hazard curve) `metrics_based` attributes without credit sensitivities:
Rust's note says so and the credit P&L flows to the residual.
Native fixtures live in `tests/scenarios/horizon/` and `tests/attribution/`.

## Returned portfolio valuation

`useValuePortfolio` (`@finstack/use-value-portfolio`) values a host-supplied
`PortfolioSpec` over a market: `portfolio.valuePortfolio` over the worker's
cached `Portfolio` and `MarketContext` handles (each a four-entry LRU keyed on
canonical JSON), then `aggregateMetrics` over the same market. It returns
`{positionIds, entityIds, valuation, metrics}`; the ids come from the
`Portfolio` handle in spec and registration order, because integer-like
`position_values` and `by_entity` keys enumerate numerically in JavaScript.
Omitted `metrics` and `strictRisk` are left to Rust (PV plus `dv01`; fail
closed). A metric on the menu that an instrument cannot compute is listed on
that position's `inapplicable_metrics` instead of thrown, unlike
`priceInstrument`. `usePortfolioMetricsSeries` decodes one base metric with
`portfolioMetricsSeries`, and `usePortfolioSpecValidator` validates spec text
(`parsePortfolioSpecJson`, then `Portfolio.fromSpec` through the cache).

The display-only `PortfolioSummary` (total, the stamped `fx_collapse_policy`
as text, value by entity, degraded positions), `PositionsTable` (rows in spec
order; the selected row's `valuation_result` through `ValuationSummary` and
`MeasuresGrid`) and `PortfolioMetrics` (the bucketed series chart, aggregated
totals by entity, raw metrics by position, unaggregated, skipped and degraded
lists) take the returned report as props. The series chart keeps the order
Rust returns, which is lexical (`10y`, `15y`, `1y`, …); tenor ordering is an
open owner decision, so no sort is applied. Native fixtures (the gallery bond
and swap book over the pricing market, and the host-parity deposit book) live
in `tests/portfolio/`.

`useScenarioPnlBatch` (`@finstack/use-scenario-pnl-batch`) calls
`portfolio.scenarioPnlBatch` over the same cached handles with the
`ScenarioSpec` texts joined into one JSON array (never parsed in TypeScript):
Rust values the base once, then each scenario, and returns one
`ScenarioPnlBatchItem` per scenario in input order with the book's
`positionIds`. A batch fails whole with the earliest failing scenario's error.
`useApplyScenarioAndRevalue` returns `applyScenarioAndRevalue`'s stressed
valuation and report with the book's id orders, for `PortfolioSummary` and
`PositionsTable`. The display-only `ScenarioPnlTable` shows positions (spec
order) by scenarios (batch order) as returned, a total row from each
scenario's returned `pnl.total`, a bar chart of those totals and each
scenario's report; nothing is summed. Its fixture
(`tests/portfolio/scenario-pnl.json`, the host-parity book under the parity
scenarios) matches the batch the Py/WASM parity tests pin.

`usePortfolioMargin` (`@finstack/use-portfolio-margin`) calls
`portfolio.portfolioMargin` over the same cached handles on the requested date.
Positions whose instruments carry a `margin_spec` form netting sets; the rest
count toward `positions_without_margin`. An omitted `currentImCollateral`
stays unset (none held); collateral under a CSA the book does not have, or
conflicting margin terms within one netting set, return the native validation
error. The display-only `PortfolioMarginReport` shows the returned
`PortfolioMarginResult`: the totals as exact base-currency money, the two
counts as returned (`total_positions` counts positions inside netting sets),
netting sets in returned order with Rust's `is_approximate` flag beside an
always-shown Indicative badge (SIMM sets use Rust's default SIMM version;
cleared sets a standalone proxy), IM collateral by CSA in the netting sets'
CSA order, degraded positions, and the selected set's `im_breakdown` and its
SIMM sensitivities through `SimmSensitivityTable`. Rust serializes a zero
variation margin as `-0`, and the report shows it as returned. Nothing is
summed, netted or converted. The fixture (`tests/portfolio/margin/`: two swaps
under the Rust USD regulatory CSA and a regulatory CSA-B, plus an unmargined
deposit, over the pricing market) also pins collateral held and an unknown CSA.

## Returned rating migration

`useMigrationMatrix` (`@finstack/use-migration-matrix`) builds a
`models.credit.migration.TransitionMatrix` from labelled rows with
`TransitionMatrix.fromLabeled` (rows and columns share one label order; the
last label is the absorbing default state) and returns its rows, default
probabilities and horizon with the labels and default state of its
`RatingScale`. `useMigrationProjection` estimates the generator with
`GeneratorMatrix.fromTransitionMatrix` (an omitted `roundTripTol` stays unset,
so Rust's default applies) and calls `models.credit.migration.project` once
per requested horizon. Every handle is freed, including the fresh
`RatingScale` each `scale` read returns. The display-only
`TransitionMatrixHeatmap` draws the matrix and the generator as heatmaps and
the projected default probabilities per origin state, or shows them as
tables; it stamps the generator's `roundTripError` and `regularizationL1` and
shows a projection error inline beside the matrix. Probabilities are coloured
over `[0, 1]`; the generator's colour domain is the extent of its intensities,
centred on zero, because its diagonal is negative. The native fixture (an
illustrative eight-state one-year matrix) lives in `tests/models/credit/`.

## Surface static arbitrage

`useCheckSurface` (`@finstack/use-check-surface`) runs
`core.VolSurface.fromJson` and `models.volatility.checkSurface` on a canonical
stored surface with every check enabled (butterfly, calendar spread, local-vol
density) and host-supplied forwards in the surface's strike units, one to
broadcast or one per expiry; a tolerance is sent only when supplied, so Rust's
default applies otherwise. The handle is freed. `SurfaceArbitrageBadge` shows
the result as Passed, the violation count, Not checked (the native error; Rust
checks only unshifted Black lognormal quotes on a strike axis) or Forwards not
supplied (no request is made), with the returned counts by type and severity
and every violation in a popover. Forwards are never inferred: `PricingWorkbench`
mounts the badge beside a stored surface's chart only when `surfaceOptions`
returns a `forwards` key, for example
`surfaceOptions={() => ({ colorDomain: [0, 1], forwards: [175] })}`.

## Merton structural credit

`useMertonModelValidator` (`@finstack/use-merton-model-validator`) returns
canonical `MertonModel` JSON (`models.credit.MertonModel.fromJson`, `toJson`).
`useMertonReport` makes one native call, `models.credit.mertonReport`, with the
model as canonical JSON or an equity source Rust calibrates to the KMV default
point. It returns the default probabilities, distances to default, implied and
debt spreads per requested horizon, the CDS par spread and implied equity at the
requested maturity, and the default PD master-scale grades. A measure Rust
rejects for the model (the debt spread of a first-passage barrier, a CDS recovery
that contradicts a CreditGrades mean recovery) carries its message beside the
rest of the report. `MertonCalculator` composes `SchemaForm` over the generated
`merton_model` contract (Rust publishes no example, so the form starts from the
caller's request) or an equity-calibration form with the horizon and CDS
inputs, and shows the returned report. The native fixture (the facade example
firm and a KMV calibration) is `tests/models/credit/merton.json`.

## Curve query

`useCurveQuery` (`@finstack/use-curve-query`) reads one stored object of a market
at supplied points over the cached market handle: `MarketContext.getDiscount`
(`df`, `zeroRate`, `forward`), `getForward` (`rate`), `getHazard` (`sp`,
`hazardRate`) or `getSurface` (`models.volatility.getSurfaceVol`). `CurveQuery`
is a separate item, so `market-context-browser` still needs no worker; a host
mounts it through the browser's `renderObject` slot with `curveQueryTarget(entry)`,
which names the queryable target of a browser entry and is null for the rest, and
the slot replaces the stored chart, so a host that wants both renders
`StoredCurveView` beside it.

Curve points are years from the base date or ISO dates. Dates pass through as
entered and Rust converts them with the curve day count; JavaScript never turns
a date into a year fraction. `DiscountCurve.forward` and `getSurfaceVol` take
year fractions only, so forward intervals and surface expiries are entered in
years and labelled so. A point Rust rejects keeps its native error in its own
cell (a malformed date, a reversed interval, a coordinate outside the surface
grid); an id the market does not hold is the whole error. The zero rate uses
the Rust default compounding, continuous. The native fixture, which cuts the
market-browser snapshot down and adds a stored surface, is
`tests/core/curve-query/cases.json`.

## Loan yield card

`useMetricValues` (`@finstack/use-metric-values`) prices each supplied metric id
on its own `valuations.instruments.priceInstrumentWithMarket` call over the cached
market handle. It exists because one failing id fails a whole native request: a
request for `ytm`, `discount_margin` and `all_in_rate` on a fixed loan returns only
the discount-margin error, so a card asking for them together would go blank. Each
id keeps its value or its native error; a malformed instrument or market is the
whole error.

`LoanYieldCard` is the view over it, for term loans: `ytm`, `ytw`, `ytc`, `yt2y`,
`yt3y`, `yt4y`, `discount_margin`, `all_in_rate`, `oid_eir_rate`,
`oid_eir_amortization` and `embedded_option_value`. `applicableLoanYieldMetrics`
keeps those `metricsForInstrument` lists for the type, and all eleven are listed
for `term_loan`. An id Rust rejects shows its error in its row:
`embedded_option_value` on a callable loan needs the `tree` model, and
`discount_margin` needs a floating loan and, when callable, a
`quoted_clean_price_pct`. Yields solve to that quote when the instrument carries
one, otherwise to the model price, which differs by model. Unit and group are what
`metricMetadata` declares. It declares no unit for the ids outside a metric group
(`ytc`, `yt2y`, `yt3y`, `yt4y`, `all_in_rate`, `oid_eir_rate`,
`oid_eir_amortization`), so the card says "not declared" and never rescales.
`oid_eir_amortization` shows Rust's total; the per-date keys Rust also returns are
not shown. The revolver's own card is a later item; on a revolver this card shows
the five ids Rust lists. The native fixture is
`tests/valuations/loan-yield/cases.json`.

## Option calculator

`useOptionCalculator` (`@finstack/use-option-calculator`) calls one closed-form
function per request: `models.bsPrice`, `black76Price` or `bachelierPrice` of a
volatility, or `models.bsImpliedVol`, `black76ImpliedVol` or
`models.volatility.impliedVolBachelier` of a price, then `bsGreeks`,
`black76Greeks` or `bachelierGreeks` at that volatility. Named terms pass in the
facade's positional order, which differs by function
(`tests/models/options/option-calculator.test.tsx` pins every order against
native output). A price Rust cannot invert (below intrinsic value, say) is the
whole error; Greeks Rust rejects (zero volatility under Black-Scholes) keep their
message beside the price. `bsGreeksIsValid` is not wired: `bsGreeks` already
rejects a non-finite Greek and its closed form cannot return a negative gamma or
vega, so the check could not fail on a returned result.
`OptionCalculator` shows the conventions the facade documents: Black-76 prices
are discounted and Bachelier prices undiscounted, Black-Scholes vega and rhos are
per 1% move and theta per day, and forward-Greek vega is per unit of volatility.
The native fixture is `tests/models/options/cases.json`.

## Liability management card

`useExchangeOffer` and `useLme` (`@finstack/use-liability-management`) call
`models.credit.liabilityManagement.analyzeExchangeOffer` and `analyzeLme`, and
`useTenderRecommendationHurdle` reads `tenderRecommendationHurdle`. Each is one
native call; `tests/models/liability-management/liability-management-card.test.tsx`
pins the positional order against the fields Rust echoes.
`LiabilityManagementCard` has a form and a table per analysis. The recommendation
is Rust's `tender_recommended`: the card never compares `tender_total` with the
hurdle, which it shows beside the badge as supplied, as a multiple of the hold-out
PV. Amounts use one unit the caller chooses and Rust converts no currency;
percentages are fractions (0.60 is 60 cents, and 60 is rejected), and the price
input means a price, an extension fee or a transferred-asset fraction by structure.
An empty amount is sent as NaN for Rust to reject and an empty EBITDA stays unset,
which leaves the leverage block out. Leverage covers the target instrument's debt
only. The native fixture is `tests/models/liability-management/cases.json`.

## Credit scorecard

`useCreditScore` (`@finstack/use-credit-score`) calls one
`models.credit.scoring` function (`altmanZScore`, `altmanZPrime`,
`altmanZDoublePrime`, `altmanEmScore`, `ohlsonOScore` or `zmijewskiScore`)
with the request's named ratios in the facade's positional order (one explicit
call per model, conversion only; `tests/models/credit/scorecard.test.tsx` pins
each order against `index.d.ts`), then
`models.credit.pd.MasterScale.defaultCalibration().mapScore` of the result. A result Rust cannot grade (the Altman models return no implied PD)
keeps that native error beside the score; the scale handle is freed.
`CreditScorecard` takes its fields, their order and their help from the
generated input contracts (`altman_z_score_input`, `altman_z_prime_input`,
`altman_z_double_prime_input` for Z'' and EM, `ohlson_o_score_input`,
`zmijewski_input`); Rust publishes no example for them and the facade has no
native parse of the input structs, so there is no validator and the form starts
from the caller's ratios. The returned score, zone and implied PD are shown as
Rust returned them. The native fixture (the inputs of the Rust unit tests, one
per model) is `tests/models/credit/scorecard.json`.

## Position risk report

`usePositionRisk` (`@finstack/use-position-risk`) decomposes position VaR and
ES with `models.factor.risk.parametricVarDecomposition` (weights and a
position covariance) or `historicalVarDecomposition` (position-major scenario
P&L). With a budget it calls `evaluateRiskBudgetFromDecomposition` over the
decomposition and the target shares; for a historical source it also calls
`buildStressAttribution` of the same P&L. Omitted confidence, incremental flag
and utilization threshold are left to Rust; the budget and stress attribution
keep their own native errors beside the decomposition. `PortfolioRiskReport`
shows the result as returned: losses negative, the Euler residual stamp (none
for a historical decomposition), component-VaR bars, the VaR and ES
contribution tables, the budget's `has_breach` and per-position utilization
(Rust returns no per-position breach flag, so none is drawn) and the tail
contributions in Rust's largest-driver-first order. The native fixture (the
three-sleeve inputs of the `portfolio_risk_decomposition` notebook, parametric
and historical) is `tests/models/risk/cases.json`.

`useFactorRisk` (`@finstack/use-factor-risk`) chains two worker calls:
`portfolio.computeFactorSensitivities` of a host-supplied book over the cached
`Portfolio` and `MarketContext` handles (`PortfolioApi`), then
`models.factor.risk.decomposeFactors` of the returned matrix's JSON under the
supplied factor covariance (`ModelsApi`). Omitted bump config and risk measure
are left to Rust. Passed to `PortfolioRiskReport` as `factors`, the result is
the Factor contributions section: total and residual risk, risk by factor and
by position and factor, and the sensitivity matrix in the book's base
currency. The fixture is the host-parity book at t0 under its one-factor
`usd_rates` model (`factor` in `tests/models/risk/cases.json`); a richer factor
set needs a Rust-owned example.

## Indicative SIMM initial margin

`useSimmSensitivitiesValidator` (`@finstack/use-simm-sensitivities-validator`)
returns canonical `SimmSensitivitiesJson` text (`margin.SimmSensitivities.fromJson`,
`validate`, `toJson`). `useSimmIm` builds the set and a
`margin.SimmCalculator` (omitted `version` and `mporDays` stay unset, so Rust's
defaults apply) and calls `calculateFromSensitivities` in the set's own base
currency; both handles are freed per call. Rust accepts only USD-based sets,
because SIMM concentration thresholds are in USD; another base currency
returns the native error. The display-only `SimmSensitivityTable` lists a set
by risk class as supplied, and `SimmReport` shows the returned amount, an
always-shown Indicative badge beside the returned `approximation` flag, the
component breakdown and the `simm_detail` drill-down; nothing is aggregated in
TypeScript. `SimmCalculator` composes `SchemaForm` over the generated
`simm_sensitivities_json` contract, a `DateInput`, the table and the report,
with validation and calculation as caller callbacks. The native fixture (the
schema example and a single USD 5Y IR delta of 1000, whose IM is 60000) is
`src/fixtures/margin/simm.json`.

## CSA and variation margin

`useCsaPreset` (`@finstack/use-csa-preset`) returns a loader of the Rust
regulatory CSAs (`margin.csaSpecUsdRegulatory`, `csaSpecEurRegulatory`, or
`csaSpecRegulatory` for a currency, id and collateral curve), and
`useCsaValidator` returns canonical `CsaSpec` text (`margin.csaSpecValidate`,
then the `csa` of a `margin.VmCalculator` built from it). `useVm` builds a
`VmCalculator` per call and runs `calculate` in the CSA's own base currency,
returning the `VmResult` with Rust's `vmResultRequiresCall` decision and
`vmResultNetMargin` (post minus collect; positive when the desk posts).
`useMarginCalls` and `useMarginCallDates` run `generateMarginCalls` and
`marginCallDates` on a calculator bound to the CSA. Every calculator is freed
per call; a cleared amount is sent as `NaN` for Rust to reject. `CsaEditor`
composes `SchemaForm` over the generated `csa_spec` contract (the Rust example
by default) with an optional preset bar whose regulatory inputs start empty;
the display-only `MarginCallTable` presents the returned result, decision, net
margin, schedule and call dates. No threshold, MTA, rounding or call decision
is computed in TypeScript. The native fixture (USD regulatory CSA on
2024-06-17) is `src/fixtures/margin/vm.json`.

## Covenant engine and compliance

`useCovenantTemplate` (`@finstack/use-covenant-template`) returns a loader of
the Rust covenant packages (`covenants.covLite`, `lboStandard`, `realEstate`,
`projectFinance`) for typed thresholds, and `useCovenantEngineValidator`
returns canonical `CovenantEngine` text (`covenants.validateCovenantEngineJson`).
`useEvaluateCovenants` builds a `covenants.CovenantEngine` per call and runs
`statements_analytics.evaluateCovenants` over an evaluated statement result:
Rust tests every result period on its last day with the covenants applicable
on that date (a window replaces the top-level specs on its dates), so the UI
builds no rows; a metric an applicable covenant reads that the results lack is
Rust's `not_found` error, a non-finite one its `validation` error. The engine
is freed per call. `CovenantEngineEditor` composes `SchemaForm` over the
generated `covenant_engine` contract (Rust publishes no example) with a
template bar whose thresholds start empty (a blank one is sent as `NaN`),
`IdCombobox` metric ids over the model's node ids through `renderField`, and a
table of each spec's metric id with whether the model has that node. The
statements workbench's Covenants tab (`covenants-panel.tsx`) composes the
editor with `CovenantCompliance` and tests `defaultCovenantEngineJson` on open.

`useForecastBreaches` runs `statements_analytics.forecastBreaches` on the same
engine, model and result with an optional `CovenantForecastConfig` (omitted is
Rust's deterministic maintenance-scope default; the config crosses as JSON
text, so a `uint64` `random_seed` stays exact). Every result period is a test
date and, without `reference_date`, the horizon starts the day before the first
period. `CovenantBreachForecast` composes `SchemaForm` over the generated
`covenant_forecast_config` contract (structure only; Rust validates it when it
forecasts) with a table of the returned `FutureBreach` rows, and sits below the
compliance grid in the Covenants tab. Covenant-lite packages are incurrence
tests, so the maintenance default forecasts nothing for them. The native
fixture (`src/fixtures/covenants/analyst-breach-forecast.json`) forecasts a
tightened analyst engine (`analyst-covenants-tight.json`) deterministically
and under the seeded stochastic `analyst-forecast-config.json`.

## Served gallery checks

The public namespace is
`https://jeickmeier.github.io/finstack-quant/r/{name}.json`. See the
[installation and MCP guide](../docs-site/content/docs/registry/distribution.mdx)
for `components.json`, pinned CLI discovery and required local WASM installation.
Every item includes generated registry/WASM versions and canonical schema IDs;
visual categories are Primitive Components, Individual Components and Blocks.
`scripts/gen-registry.mjs` derives these fields from package metadata and the
existing schema/provenance inventory, and `ui-check` rejects drift.

CI runs `mise run ui-check` in the `Test UI` job on every pull request. The
gallery export, screenshot suite, installation matrix, publishing check and
standalone harnesses are local gates: `mise run ui-docs-build`,
`mise run ui-e2e`, `mise run ui-install`, `mise run ui-publish-check` and
`mise run ui-harness`. No workflow deploys the public namespace URL in
`components.json` today; `scripts/verify-public.mjs <base-url>` is the local
check of a served registry, and local-host success does not close the
public-site gate.

`mise run ui-docs-build` installs all registry items into the docs consumer and
exports `/registry-gallery/` with the production Next build. The generated
inventory and import harness follow the registry catalogue; `mise run ui-check`
rejects stale copies. Fixtures preserve their source-file hashes.
The gallery wraps an item in `FinstackQueryProvider` exactly when `workerRoles`
in `scripts/check-registry.mjs` classifies it `host`: its dependency closure
contains `use-finstack` and none of its own files renders a
`FinstackQueryProvider` or `FinstackProvider` element imported, under any local
name, from `use-finstack` (`self`, as `pricing-forms-example` does). The generated `inventory.json` carries that
role. The schema-form examples add validator hooks outside those items'
closures, so `native-forms.tsx` mounts its own provider.

For interactive review, run `mise run ui-docs-install`, then
`npm --prefix docs-site run dev` to start the docs-site Next.js server. Open
[http://localhost:3000/registry-gallery/](http://localhost:3000/registry-gallery/)
once it is ready. If the catalogue is already installed and unchanged, `npm run dev`
in this package starts the same server.

To exercise a running development server, use
`REGISTRY_GALLERY_URL=http://localhost:3000 mise run ui-e2e -- workbench-controls.spec.ts radio-alignment.spec.ts`.
These checks cover opaque select/calendar popups, value changes, radio labels,
keyboard selection and visible selected indicators. The docs stylesheet explicitly
registers the gitignored installed component directories as Tailwind sources.

Run `mise run ui-e2e` after the build. The suite verifies light/dark and
compact/comfortable screenshots, serious/critical axe findings, nonvisual imports,
publication layouts, accepted table/chart selection, pinned tooltip actions and
bond-form keyboard submission. It serves the static export without a dev server.
Playwright defaults to **no snapshot updates**. For an intentional visual change,
run `mise run ui-e2e -- --update-snapshots`, inspect the changed PNGs, record the
reason in the pull request, and then rerun `mise run ui-e2e` without that flag.
Baselines use pinned Chromium 145, local Fontsource faces, UTC and en-US at 1×.
Only the volatile calculation timestamp is masked. Baselines are recorded on
macOS with the pinned Chromium and do not certify rendering on other operating
systems.

The shared docs gallery proves composition. Independent installation and desktop
publishing have separate gates and evidence.

## Independent installation gate

Registry items pin `finstack-quant-wasm` to the version in
`finstack-quant-wasm/package.json`; the generators read it from there, so
`npm --prefix finstack-quant-ui run generate` (part of `mise run gen-write`)
writes that pin into every registry item and a version bump
without it fails `mise run ui-check` and `gen-check`. The package is not
published to npm: the local install tasks serve that exact dependency as a
`file:` path to the workspace package.

After a fresh `mise run wasm-pkg`, run `mise run ui-install`. It builds the registry,
initializes a separate Next 16/Base UI application outside the repository for
**every catalogue item**, and asks the pinned CLI to add exactly that item. Each
consumer gets its declared dependency closure, a production build, a type check
and a served Chromium render/import check. Required unpublished WASM packages
are copied from an absolute local path before registry resolution.

Only after all independent installs pass does the gate run a separate composed
workbench and table/chart selection consumer at root and deployment base paths.
It exercises the real worker, edited inputs, wide integers, structured failures,
font/worker/WASM assets, native-result agreement and native cashflow envelopes.
Standalone chart and financial-wrapper checks use their public callbacks, tooltip
props and shared SVG/PNG handles. `CashflowViewer` validates the supplied envelope
object against the Rust schema and offers its `serializeHost` JSON for copy and
download, without computing financial values.

Set `REGISTRY_INSTALL_EVIDENCE` and `REGISTRY_SMOKE_DIR` to retain matrix and
composition reports, logs, screenshots and exports. Failed applications are kept
at the paths in the report. `REGISTRY_INSTALL_CONCURRENCY` accepts 1–4 independent
consumers (default 1). A focused diagnostic run can invoke
`node finstack-quant-ui/scripts/install-matrix.mjs` with
`REGISTRY_INSTALL_ITEMS=finstack-chart,cashflow-viewer`; its report is explicitly a
subset and cannot satisfy `ui-install`. The faster workbench check,
`node finstack-quant-ui/tests/browser/install-smoke.mjs` after
`mise run ui-docs-install`, remains available. `mise run ui-install` is a local
gate run on demand against fresh native packages, not a CI job.

The matrix reads the same roles: it mounts `FinstackQueryProvider` only for
`host` items and fails any item that is neither `host` nor `self` and still
loads WASM. An item with `tests/install/fixture/<name>.tsx` installs that
fixture (the four schema forms use a caller `validate` stub); any other visual
item installs its gallery example, and an example declaring a `presentation`
prop also gets the wrapper probe. An `@/` import in a fixture or example that
resolves outside the item's closure is copied under `app/` at its installed
target; the copy's own imports are not followed. Every registry-declared package
a consumer loads at runtime must be declared inside the item's closure, apart
from the scaffold packages (`next`, `react`, `react-dom`, `@base-ui/react`).
`check-registry` requires `finstack-base`, which declares the stock controls'
packages (`cn`, `class-variance-authority`, `lucide-react`), in every closure
that reaches a stock shadcn component. For an item whose closure reaches
`instrument-catalogue`, the page's first static load (the chunks the exported
HTML names; for a non-visual item, everything it loads) must not contain the
instrument, schema or example module of a type the closure reaches only through
the catalogue.

## Printing completed valuations

Use **Print report** after the workbench reports **Priced**. It captures that
completed request and result, waits for the native cashflow query and fonts,
and opens the browser print dialog. Choose A4 or Letter; shared rules use 12 mm
margins, a light theme, wrapping JSON and repeating table headers. Instrument,
market, valuation, measures and the cashflow table reuse the existing components.
Closing the dialog restores the prior tabs and disclosure state. Unsupported
cashflow requests retain the native error. The cashflow table preserves row-currency
amounts separately from reporting-currency PV and shows the supplied total without
recalculating it. Envelope JSON (the host serialization of the returned object)
remains available in a collapsed disclosure for copy/download; printed reports
show the table instead of the raw export.

`mise run ui-publish-check` builds a fresh installed workbench consumer, generates
four PDFs, verifies the envelope JSON view and download, checks offline SVG/PNG reopening
and inspects PDF text/fonts/vectors/bounds with pinned pdfplumber. Evidence is
written under `finstack-quant-ui/test-results/publishing` by default; set
`REGISTRY_INSTALL_EVIDENCE` to retain it elsewhere. Visual page and desktop-editor
inspection remain explicit release checks; no library tests run in this task.
