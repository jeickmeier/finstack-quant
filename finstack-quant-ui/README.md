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

| Path                                                 | Holds                                                                                                                                                                                                                           |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `registry/<category>/`                               | Item sources. `core`, `valuations`, `calibration`, `covenants`, `models` and `statements` follow the crates, beside `base`, `theme` and `shared`. Each `registry.json` lists files, install targets and `registryDependencies`. |
| `components/ui/`                                     | Stock shadcn source, hash-pinned in `components/shadcn.json`. Never edited.                                                                                                                                                     |
| `src/`                                               | Codec, host, views and format; `src/format/label.mjs` holds `labelFor`, the shared label function, also used by `gen-registry`; `src/generated/` is generator output.                                                           |
| `scripts/`                                           | Generators, the fixture runner, checks and install gates.                                                                                                                                                                       |
| `tests/<crate>/`                                     | Vitest suites and native fixture generators.                                                                                                                                                                                    |
| `tests/shared/browser/`, `tests/valuations/browser/` | Standalone consumer harnesses run by `ui-harness`.                                                                                                                                                                              |
| `tests/e2e/`, `tests/install/`, `tests/publishing/`  | The `ui-e2e`, `ui-install` and `ui-publish-check` gates.                                                                                                                                                                        |
| `tests/browser/`                                     | Worker smoke, footprint gate, the harness runner and shared consumer helpers.                                                                                                                                                   |

Sources import through the paths an installed consumer uses. `tsconfig.json`
maps them back here; `vitest.config.mts` copies that table into prefix aliases
in order, so a specific key must precede its prefix.

| Import                    | Source                                                                                                                             |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `@/lib/finstack/form`     | `registry/shared/lib/finstack-form/form.tsx` (before the next row)                                                                 |
| `@/lib/finstack/*`        | `src/*`                                                                                                                            |
| `@/hooks/<crate>/*`       | `registry/<crate>/hooks/*` for `shared`, `core`, `valuations`, `calibration`, `models` and `statements`; a new crate adds its line |
| `@/workers/*`             | `registry/shared/file/*`                                                                                                           |
| `@/components/finstack/*` | `registry/*`                                                                                                                       |
| `@/components/ui/*`       | `components/ui/*`                                                                                                                  |

A new crate category also adds its directory name to the `include` list in
`scripts/gen-registry.mjs`, then runs
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
   drift].
4. For a visual item, add
   `docs-site/src/components/registry-gallery/examples/<item>.tsx` and its key
   in `primitives.tsx` or `components.tsx` there [`ui-check`: `gen-gallery`].
5. Cover its behaviour in Vitest under `tests/<crate>/` [review only].
6. Record its four screenshots with
   `mise run ui-e2e -- --update-snapshots -g "<item>"` and inspect them [local
   `ui-e2e`].
7. Check it installs in isolation with
   `mise run ui-build && REGISTRY_INSTALL_ITEMS=<item> node finstack-quant-ui/scripts/install-matrix.mjs`
   while iterating; `mise run ui-install` is the full local gate. Its worker
   role comes from the registry, and an install that needs extra files uses
   `tests/install/fixture/<item>.tsx` [local `ui-install`].

CI runs only `ui-check` (the Test UI job); the other gates are local.

`npm run generate` runs `scripts/run-generators.mjs`: `gen`, `gen-registry`,
`gen-theme`, `encoding-report`, `catalogue-docs`, `gen-metadata`,
`gen-gallery`. `gen-registry` and `gen-theme` rewrite `registry.json` and
`registry/theme/registry.json` without `categories` and `meta`, which only
`gen-metadata` restores; running either alone leaves metadata drift that
`ui-check` rejects. Native fixtures follow [Native fixtures](#native-fixtures).

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

`mise run ui-fixtures` runs the nine native fixture generators under
`tests/**/generate.mjs` in dependency order (results → cashflows → details;
curves → market-browser; cubes, calibration, scenario-table, statements), then
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
fields, such as seeds, arrive as bigint. Cashflow structure comes from the
published Rust `instrument_cashflow` schema. Its presentation adapter preserves
numeric tokens and the original JSON, including mixed-currency FX-swap rows.

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
mutation through Rust (not a claim of pricing with that seed), compare original
cashflow text byte for byte, and preserve the published Float64Array return.
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

`labelFor` (`label.mjs`, also used by `gen-registry` for item titles) turns a
Rust wire or metric name into a sentence-case label with one set of upright
market acronyms; it is typography only and adds no financial names.
`compareTenorLabels` puts whole-label `Nm`/`Ny` tenors first in maturity order on
Rust's year basis (a month is 1/12 year). Other labels compare run by run:
numbers by value, an embedded tenor run such as vega's `SURF::0.25y` in years,
and text by en-US collation; ties fall back to the full label.

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
(`CoreApi`, `ValuationsApi`, `CalibrationApi`, `ModelsApi`, `StatementsApi`), and
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
canonical JSON. `useCashflows` (the individual `use-cashflows` item) returns original native JSON
text. `CashflowViewer` accepts that text plus optional loading/error props and needs no provider. It presents native rows in a table, with exact JSON copy/download in a secondary disclosure;
unsupported export preserves caller pricing/model state and shows the native error. The workbench owns one cashflow query for both display and its print snapshot.
`useModels`, `useMetrics`, and `useCalendars` return native option registries.
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
  })}
  fxOptions={() => ({
    coordinates: [{ expiry: 1, strike: 1.1, forward: 1.12 }],
    colorDomain: [0, 1],
  })}
  scenario={{ trancheId, gridJson, priceDomain: [80, 140] }}
/>
```

Those numbers are illustrative caller inputs, not financial defaults. Use extents,
strike, forward and coordinates appropriate to the supplied objects. Without
options, the market browser retains exact stored data. Scenario prices are fetched
only when the detail panel is opened for a completed structured-credit request.
`calibrationChartOptions` forwards existing figure annotations, export refs and
interaction callbacks; the workbench owns the shared report/chart selection.

## Served gallery checks

The public namespace is
`https://jeickmeier.github.io/finstack-quant/r/{name}.json`. See the
[installation and MCP guide](../docs-site/content/docs/registry/distribution.mdx)
for `components.json`, pinned CLI discovery and required local WASM installation.
Every item includes generated registry/WASM versions and canonical schema IDs;
visual categories are Primitive Components, Individual Components and Blocks.
`scripts/gen-metadata.mjs` derives these fields from package metadata and the
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
font/worker/WASM assets, native-result agreement and exact cashflow output.
Standalone chart and financial-wrapper checks use their public callbacks, tooltip
props and shared SVG/PNG handles. `CashflowViewer` offers an exact-text JSON
download; its presentation adapter parses numeric tokens losslessly, without reserializing exports or computing financial values.

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
recalculating it. Original JSON remains available in a collapsed disclosure for
exact copy/download; printed reports show the table instead of the raw export.

`mise run ui-publish-check` builds a fresh installed workbench consumer, generates
four PDFs, verifies native download identity, checks offline SVG/PNG reopening
and inspects PDF text/fonts/vectors/bounds with pinned pdfplumber. Evidence is
written under `finstack-quant-ui/test-results/publishing` by default; set
`REGISTRY_INSTALL_EVIDENCE` to retain it elsewhere. Visual page and desktop-editor
inspection remain explicit release checks; no library tests run in this task.
