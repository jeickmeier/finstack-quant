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
3. Run `mise run gen-write` from the repository root [`ui-check`: generator
   drift]. It runs the UI generators and rewrites the repository manifest of
   generated files, which CI `gen-check` verifies (`--check-manifest`).
4. For a visual item, add
   `docs-site/src/components/registry-gallery/examples/<item>.tsx`;
   `scripts/gen-gallery.mjs` generates the gallery's example map
   (`components.tsx`) from the registry's visual items [`ui-check`:
   `gen-gallery`]. An example that reads a native fixture names it in the
   hand-maintained `sources` map of `scripts/gen-gallery.mjs`, which writes the
   gallery `data.json`, and reads a native section through its one typed
   accessor in `fixtures.ts` there [local `ui-e2e`].
5. Cover its behaviour in Vitest under `tests/<crate>/` [review only].
6. Rebuild the gallery export with `mise run ui-docs-build` (`ui-e2e` serves
   that static export, so `ui-docs-install` alone tests a stale gallery), then
   record its two screenshots (light compact and dark comfortable; axe runs
   in all four theme and density modes) with
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
   each excluded key with one reason from the closed set `listedReasons` in
   `scripts/coverage.mjs`, which defines each: `data-constructor` (builds a
   plain value from fields or a label, or is a handle class whose other
   members only read back what it was built from), `schema-form` (an enum variant or
   preset a schema form builds), `setter` (a copy with fields set), `getter`
   (reads one stored field or entry), `validator` (a shape check returning
   nothing or canonical JSON) and `performance-duplicate` (an `analytics`
   series function with the same kernel as the `Performance` method).
   `coverage --check` rejects any other reason, and picking an excluded key
   fails until its entry is deleted. `schema.*` keys, `*Json` wire twins and
   builders are excluded by rule, and a dropped slice deletes its `planned`
   keys [`ui-check`: `coverage --check`].

CI runs only `ui-check` (the Test UI job); the other gates are local.

`npm run generate` runs `scripts/run-generators.mjs`: `gen`,
`encoding-report`, `catalogue-docs`, `gen-registry`. The gallery artifacts
come from `gen-gallery`, the last step of the native fixture run in
`mise run gen-write`.
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

`mise run gen-write` (after building the WASM packages) runs the forty native
fixture generators under
`tests/**/generate.mjs` in dependency order (results → scenario application,
horizon return, scenario templates, attribution, cashflows → details; loan yields; workout ladders; quote sets;
portfolio → portfolio margin → portfolio workbench → primitive exposures; portfolio returns; Brinson; curves → market-browser → curve-query; cubes, calibration,
scenario-table; statements → covenants, normalizations, capital structures, relative value; WACC rates; statement tables; analytics → correlation; models credit, risk, options, liability management and recovery;
margin), then
refreshes the gallery copies with `scripts/gen-gallery.mjs`. It needs
`finstack-quant-wasm/pkg-node`, built by `mise run wasm-build-node` (dev) or
`mise run wasm-pkg` (release). Generators load that package, read repository
files and build source records through `scripts/fixture-inputs.mjs`, and write
through `emit` in `scripts/fixture-output.mjs`. The check mode
(`run-fixtures.mjs --check`) first verifies that every source a committed fixture records still has its
recorded SHA-256, then that the fixture matches a fresh regeneration, ignoring
formatting and key order, and names the generator to rerun;
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
`StatementExport` (`@finstack/statement-export`), fetched with
`useStatementResultTable` (`@finstack/use-statement-result-table`), draws one
`statements.statementResultToTableWide` or `statementResultToTableLong` result
through `TableEnvelope`, with a wide or long switch, and Download CSV writes
exactly those cells: RFC 4180 quoting, native column names as the header and each
number as `String(value)`. A node with no value is `NaN` in a live table and null
in JSON; both show as Undefined and write an empty field, never zero. In the
workbench it sits below the statement grids in a disclosure that calls Rust when
first opened, over the result the grids show, reported or annual. The native
fixture is `tests/statements/export/cases.json`.
`NormalizationEditor` (`@finstack/normalization-editor`), fetched with
`useNormalize` (`@finstack/use-normalize`) and validated with
`useNormalizationConfigValidator`
(`@finstack/use-normalization-config-validator`), answers what adjusted EBITDA
is. It edits a `NormalizationConfig` with a schema form over the generated
`normalization_config` contract (node fields as id-comboboxes) and shows
`statements.normalize` as returned: a bridge for a chosen period from the
reported base through each adjustment to the final value, each step drawn from
the previous step's `adjusted_value` (the base for the first) to its own, and a
table of every adjustment's capped amount by period, with the raw amount beside
a capped one. Rust returns every position, so nothing is summed. In the
workbench it sits in Metrics & ratios, in a disclosure that calls Rust when
first opened, over the reported result. The native fixture is
`tests/statements/normalization/cases.json`.
`CapitalStructureReport` (`@finstack/capital-structure-report`), fetched with
`useCapitalStructureSummary` (`@finstack/use-capital-structure-summary`),
answers how much debt sits ahead of each tranche and what leverage and coverage
look like through it. It edits a `CapitalStructureSummaryConfig` with a schema
form over the generated `capital_structure_summary_config` contract (the period
over the model's periods, the EBITDA and cash nodes as id-comboboxes; Rust
validates it on the call) and shows `statements_analytics.capitalStructureSummary`
as returned: total debt, cash, net debt, LTM EBITDA with the LTM periods Rust
found of those it requires, total and net leverage and total coverage; the debt
stack in Rust's rank-then-declaration order with cumulative debt, leverage and
interest coverage through each payment class; and the maturity wall by year. A
ratio Rust returns as null (a short LTM window, non-positive EBITDA or debt), or
a live NaN, shows as Undefined; negative net debt shows as returned. In the
workbench it sits in Metrics & ratios, in a disclosure that calls Rust when
first opened; a model with no capital structure shows Rust's error. The native
fixture is `tests/statements/capital-structure/cases.json`, a quarterly model
with two first-lien bonds and a second-lien bond.
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
`WaccCalculator` (`@finstack/wacc-calculator`), fetched with `useWacc`
(`@finstack/use-wacc`), takes five decimals, the two capital weights separate,
and shows the one `statements_analytics.wacc` rate as returned. Weights that do
not sum to one, a tax rate outside [0, 1] or an empty field show the native
error; nothing is blended or checked in JavaScript. In the Valuation tab its
Use as WACC button sets the form's WACC field, through the form's optional
`wacc` and `onWaccChange` props so the other fields keep what was typed. It
passes the returned number on the user's click only, never after an edit since
the rate was calculated, and valuing stays the form's own action. The native
fixture is `tests/statements/wacc/cases.json`.
`useEvaluateLbo` (`use-evaluate-lbo`) passes the model and `LboConfig` JSON
to `statements_analytics.evaluateLbo`. The Valuation tab's `LboReport` edits
the config with a schema form over the generated `lbo_config` contract (string
`*_node` fields as id-comboboxes; checked structurally and validated by Rust on
evaluation, as there is no standalone `LboConfig` parse) and shows the
returned sources and uses, entry and exit values, MOIC and the LBO check
report. Returns are MOIC only: IRR is not part of the native result.
`useRunSensitivity` (`use-run-sensitivity`) passes the Rust
`SensitivityConfig` to `statements_analytics.runSensitivity` unchanged
(`StatementSensitivity` gives each parameter its percentage moves as the
`ParameterSpec` `pct_range`, the base value being the evaluated cell, a missing
or non-finite one sent as NaN) and returns its `generateTornadoEntries` at the
target metric and period, which Rust returns as null for a `full_grid` run it
cannot rank.
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

## Component conventions

What each item calls natively, which units, nulls and errors it shows as
returned, and which fixture pins it is written once, in
`scripts/provenance.mjs`. `npm run generate` compiles it into
`src/contract-provenance.json` and each item's registry `docs`, which the
gallery shows above every preview and the
[registry docs](../docs-site/content/docs/registry/components.mdx) list. Read
and change a convention there; an item's fixture generator sits beside its
tests (see [Native fixtures](#native-fixtures)).

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

Run `mise run ui-e2e` after the build. The suite opens each item in focus mode,
which shows the whole item uncropped, and verifies light-compact and
dark-comfortable screenshots, serious/critical axe findings in all four theme
and density modes, nonvisual imports,
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
at the paths in the report. `REGISTRY_INSTALL_CONCURRENCY` accepts 1–8 independent
consumers (default 1; `ui-install` uses 6). Each consumer is a copy-on-write clone
of a scaffold that `npm install` and `shadcn init` built once. A hook or library
with no WASM, worker or lazy-catalogue role is type-checked only, without a bundle
or browser load. A passing item is cached in `test-results/install-cache.json`
keyed on its closure files, harness, installer, toolchain and WASM package;
`REGISTRY_INSTALL_CACHE=0` reruns everything. `REGISTRY_INSTALL_SHARD=i/n` runs
every n-th item so CI can split the catalogue; its report is a subset. A focused diagnostic run can invoke
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
