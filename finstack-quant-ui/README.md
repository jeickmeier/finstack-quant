# Finstack Quant UI contracts

Component registry contracts: offline JSON Schema bundles, generated wire
interfaces, lossless adapters, source provenance and a lazy instrument catalogue.
This is a standalone Node 24 package with installable controlled primitives.

```sh
mise run wasm-pkg
mise run ui-sync
mise run ui-gen
mise run ui-check
```

`ui-sync` installs the lockfile with lifecycle scripts disabled. The linked WASM
package is built explicitly by `wasm-pkg`; dependency installation does not launch
Rust or Python builds. `ui-check` checks generated drift, handwritten formatting,
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

## Ownership and generation

`scripts/gen.mjs` reads the canonical crate schema indexes by URI. Ref Parser
resolves offline with both network and filesystem fallback disabled. Only
reachable definitions enter each bundle; stable URI-derived keys distinguish
same-named definitions, retain recursive references and preserve reference
annotations. Metadata uses JSON Schema pointers plus original canonical source
pointers and refs, including non-`type` discriminator constants. Defaults,
descriptions and supplied `x-` annotations remain in the committed bundles.
Numeric type or a decimal reference never assigns a financial unit.

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

## Validation boundary

These validators establish JSON structure, not financial validity. Rust/WASM
still owns schedules, market lookups, cross-field rules, pricing and calibration.
No financial rules, unit tables or arithmetic are implemented here.

Generated interfaces describe wire shapes and cannot express every runtime
constraint (bounds, patterns or exclusive unions). They are not WASM host types.
Use each instrument loader's `codec.parse(text)` for JSON input and
`codec.stringify(value, rustCanonicalizer)` for export. The validator returns UI
state with schema `int64`/`uint64` fields as bigint; generated wire interfaces
still describe serialized JSON. Decimal-string money is unchanged. Rounded
integers and non-JSON host objects fail instead of silently losing information.

`finstack-quant-ui/host` reuses published facade types. `adaptValuation` preserves
actual structured results, and `exportValuation(result,
valuations.validateValuationResultJson)` emits integer tokens validated by Rust.
Only Monte Carlo details have a typed view. The other four variants retain raw
host values, including bigint counts found in live structured-credit results.
Cashflow viewers must keep the original Rust JSON string; the live FX-swap
fixture returns mixed-currency rows despite stale facade documentation.

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
the docs dependencies with `npm --prefix docs-site ci --ignore-scripts`, install
Chromium with `npm --prefix finstack-quant-ui exec -- playwright install chromium`,
and run `npm --prefix finstack-quant-ui run test:browser`. It checks root and
`/finstack-quant` deployments and removes its temporary app afterward.
`REGISTRY_SMOKE_DIR` selects a directory for JSON evidence.

The fixture runs independently of the docs notebook publication suite. See the
[PR-003 feasibility record](evidence/pr-003.md) for the actual docs export checks,
configuration, browser version, and validation boundaries. The browser smoke test
is separate from the fast `ui-check`; PR-004 owns optimized footprint acceptance.

## Browser footprint gate

PR-004 measured all 78 instrument validators and the optimized `release-size`
WASM package. The corpus completed and the optimized raw artifact is 21,534,873
bytes against the user-authorized revised 25,000,000-byte limit. Browser
feasibility passes; the original 10 MB failure is retained in the evidence.
Compression does not waive this raw-byte gate. See the
[PR-004 evidence and reproduction commands](evidence/pr-004.md).

`test:footprint` requires `REGISTRY_WASM_PACKAGE` to select the optimized web
package and matching generated glue. The size script reports raw/optimized/gzip/
Brotli bytes and returns a failing exit code when the raw optimized limit is exceeded.

## Registry distribution

`mise run ui-build` uses shadcn 4.21.0 and writes installable JSON to `public/r`.
`finstack-base` pins Base UI 1.8.0, React 19.2.8 and Tailwind 4.3.3. The registry
is Base UI only; the accepted `new-york` config does not select upstream Radix
components. Add our namespaced items, not upstream primitive names.

Every generated contract has one owner and installs under `lib/finstack` with
relative imports. `instrument-catalogue` depends on all instrument modules for
installation, while its runtime imports remain lazy. `contract-bond` can install
independently with just its codec closure. `finstack-host` declares the published
WASM package; this checkout validates against the matching local facade.

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
Tenant CSS loads after the theme and overrides the same variables.

The single token source includes the Fontsource package versions, selected faces
and SIL OFL-1.1 licences. `ui-gen` emits CSS and registry metadata; `ui-check`
checks drift, literal colours/fonts and contrast, including the tenant example.
`npm run test:theme:browser` builds and statically serves an installed theme
fixture, checking actual font loading and the theme/density switches.

## Display formatting

`finstack-format` installs `lib/finstack/format/{format,columns,transport}.ts`.
Scalar formatting is independent of WASM initialization. Import `transport`
explicitly for canonical import/export; it reuses the existing adapters.

`formatMoney` preserves the amount and uses returned per-currency rounding
stamps for display. Without an explicit scale it preserves all supplied digits.
`formatRate` requires a source-backed wire/display convention; absent metadata
returns raw text and `unit: null`. Decimal conversions use exact decimal text;
source strings remain the caller's state. Grouping currently uses en-US.
`isoToEpoch` and `epochToIso` call the native core date functions in an initialized
host (inside the worker for browser consumers). Column presets target Table
9.2.4 and add formatting/alignment only.

## Controlled primitives

The 18 public primitives install under `components/finstack/primitives`, with a
shared `field-frame`. Each declares its theme and actual dependency closure.
Cross-category imports use the consumer's standard `@/lib/finstack` alias;
configure `@/*` to the application source root. The CLI handles `src` layouts.

Inputs receive controlled values, labels, help, constraints and options. Calendar,
model, metric and ID choices come from the caller. Decimal money stays text;
number-backed rate and knot inputs preserve incomplete/invalid text for editing.
Only explicit source-backed rate presentation changes display units. Domain
validation remains in Rust. Date calendars only present dates and any supplied
business-day highlights.

`FinstackTable` uses Table 9 with no optional features and caller-owned stable row
IDs; `KnotTable` reuses it. `JsonViewer` displays/copies the original supplied
string and never parses it. `npm run test:primitives:browser` builds an isolated
consumer from actual registry JSON, checks keyboard/focus/clipboard behavior and
10,000-option virtualization, and runs axe in both themes and densities.

## Worker and query boundary

Install `@finstack/use-price-instrument` and/or `@finstack/use-validate-instrument`.
They include `use-finstack` and the worker closure. Hooks install at project-root
`hooks/<item>/`; the worker/service/contracts install at project-root `workers/`
(the `~/` registry target intentionally stays outside an optional `src` folder).
The worker uses the consumer's standard `@/lib/finstack` alias for the installed
codec. The root registry owns these worker files because included indexes cannot
traverse to sibling source directories.

Use `FinstackQueryProvider` around a standalone feature, or `FinstackProvider`
inside the application's existing `QueryClientProvider`. `useFinstack` exposes
`starting`, `ready`, `error` and `reset`. Nothing initializes during SSR. One
provider shares one worker; cleanup rejects pending calls and terminates it.
Use the matching built `finstack-quant-wasm` package, linked locally until the
release packaging slice. An optional `wasmUrl` selects a matching web artifact;
reset or changing that URL creates a fresh worker and query session.

`usePriceInstrument` requires the complete immutable request (`instrumentJson`,
`marketJson`, `asOf`, `model`, `metrics`, `pricingOptions`, `marketHistory`). JSON
strings remain unchanged, including wide integer tokens. Results are structured
clone values; export through the worker's `exportResult` method for native
canonical JSON. `useCashflows` (the individual `use-cashflows` item) returns original native JSON
text. `CashflowViewer` composes that hook with exact copy/print presentation;
unsupported export preserves caller pricing/model state and shows the native error.
`useModels`, `useMetrics`, and `useCalendars` return native option registries.
`useValidateInstrument` takes canonical text and a caller-owned revision; its
250ms debounce never exposes a different revision's validation error or result.

Run `UV_NO_SYNC=1 mise run wasm-pkg` for scoped package builds; disabling uv
synchronization prevents the final marker-file command from rebuilding Python.
The production browser harness now exercises the installed provider and worker,
not a separate probe implementation. See [PR-009 evidence](evidence/pr-009.md).

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
instead of silently dropping text. Same-origin font stylesheets allow the export
to embed loaded webfonts; desktop editors may require the font installed locally.
PNG scale changes pixel density, not data/layout coordinates. For example, a
900×600 layout at scale 2 gives 1800×1200 pixels, suitable for 6×4 inches at 300 PPI.
The original inputs are unchanged. SVG includes all persistent prose, sources,
axes, legends and marks, without HTML overlays or rasterized marks.

`npm run test:figures:browser` builds the installed example, exports narrow/wide
numeric/date/category fixtures, checks text clipping/overlap, vector elements,
font/style portability, exact raster dimensions, accessibility and no WASM loads.
See [PR-010 evidence](evidence/pr-010.md); desktop-editor acceptance stays open
until its planned release slice.

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
coordinates. See the installable `LinkedFigureExample` and [PR-011 evidence](evidence/pr-011.md).
The hook imports only React; chart-only consumers load neither tables nor WASM.

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
`layout="basic"` hides defaulted fields under More; `layout="full"` exposes them.
`fields={{ allow, deny }}` uses canonical paths such as
`instrument.spec.notional.amount` and preserves hidden values. Schema errors appear
under fields where mapped, with remaining native errors in the summary.

The shared field kit is available through `useAppForm`. Bond-subset evidence is
recorded in [PR-012](evidence/pr-012.md); complete cross-instrument rendering and
field-error mapping remain separate planned slices.

## Supplied valuation results

`ValuationSummary` displays a supplied result's value, instrument ID, as-of date
and calculation stamps. `compact` controls presentation; `compareTo` retains an
independent result with its own currency and metadata. `density` accepts the shared
compact/comfortable modes. Missing metadata does not create rounding or FX defaults.

`MeasuresGrid` composes that context with the unlinked table. Pass `groups` from
`listStandardMetricsGrouped`; fully qualified keys are retained. Measures are raw,
with “Unit unavailable” unless `units` or `comparisonUnits` supplies a label and
source for that complete key. Neither component calculates differences, totals,
ratios or currency conversions. See [PR-013 evidence](evidence/pr-013.md) and the
retained native bond result fixture.

## Returned structured-credit scenarios

With the `@finstack` registry namespace configured, install the standalone component:

```sh
npx shadcn@4.21.0 add @finstack/scenario-heatmap
```

```tsx
import { ScenarioHeatmapExample } from "@/components/finstack/components/scenario-heatmap/example";

// table is the unchanged result of structuredCreditTrancheScenarioTable.
// Choose an explicit display extent containing every supplied price and par 100.
<ScenarioHeatmapExample table={table} priceDomain={[80, 140]} />;
```

`ScenarioHeatmap` accepts controlled `severity` and `onSeverityChange`, optional
shared `link`, publication text, annotations, tooltip/callback props and a
`figureRef` for the existing complete SVG/PNG export API. CPR, CDR and severity
remain their returned decimal values. Color is the returned clean settlement
price as a percentage of **current tranche balance**, with 100 as par. The Rust
`ScenarioCell.price` contract owns this wording; the facade's original-balance
comment remains an upstream documentation exclusion. No rebasing, normalization
or WAL/writedown overlay is performed. Neither the component nor its example
requires a worker or workbench block. Native fixture provenance, including a
factor-adjusted tranche, is in [PR-034 evidence](evidence/pr-034.md).

The complete workbench is installed with `npx shadcn@4.21.0 add @finstack/pricing-workbench`.
Embed `PricingWorkbench` inside `FinstackQueryProvider`. Supply an explicit
`defaultRequest` and optionally `defaultCalibrationJson` (a complete canonical
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

`mise run ui-docs-build` installs all registry items into the docs consumer and
exports `/registry-gallery/` with the production Next build. The generated
inventory and import harness follow the registry catalogue; `mise run ui-check`
rejects stale copies. Fixtures preserve their source-file hashes.

Run `mise run ui-e2e` after the build. The suite verifies light/dark and
compact/comfortable screenshots, serious/critical axe findings, nonvisual imports,
publication layouts, accepted table/chart selection, pinned tooltip actions and
bond-form keyboard submission. It serves the static export without a dev server.
Playwright defaults to **no snapshot updates**. For an intentional visual change,
run `mise run ui-e2e -- --update-snapshots`, inspect the changed PNGs, record the
reason in PR evidence, and then rerun `mise run ui-e2e` without that flag.
Baselines use pinned Chromium 145, local Fontsource faces, UTC and en-US at 1×.
Only the volatile calculation timestamp is masked. The CI gallery job uses macOS;
these baselines do not certify rendering on other operating systems.

The shared docs gallery proves composition. Independent installation and desktop
publishing have separate gates and evidence.

## Independent installation gate

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
download; it does not parse or reserialize the native export.

Set `REGISTRY_INSTALL_EVIDENCE` and `REGISTRY_SMOKE_DIR` to retain matrix and
composition reports, logs, screenshots and exports. Failed applications are kept
at the paths in the report. `REGISTRY_INSTALL_CONCURRENCY` accepts 1–4 independent
consumers (default 1). A focused diagnostic run can invoke
`node finstack-quant-ui/scripts/install-matrix.mjs` with
`REGISTRY_INSTALL_ITEMS=finstack-chart,cashflow-viewer`; its report is explicitly a
subset and cannot satisfy `ui-install`. The faster `ui-install-smoke` workbench
check remains available. The dedicated CI `ui-install` job consumes the gallery
job's fresh native packages and runs the complete gate.
