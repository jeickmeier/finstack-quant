# Design polish pass (2026-09-20)

Presentation-only changes to the shipped registry components, driven by a
multi-lens design review of the served gallery (48 visual items, 224 captured
states across light/dark, compact/comfortable, publication variants and the
workbench at 1920/1440/1280/1024/900/390). The review produced 314
deduplicated findings; this pass closes the shared and highest-severity ones
and records the rest as follow-ups. No stock shadcn source under
`components/ui` changed; no financial calculation, unit inference or rounding
was added; every value keeps its exact digits.

## Theme and tokens

- Type scale from the design language is now real: `text-xs` 11 px, `sm`
  12.5 px, `base` 14 px, `lg` 16 px, `xl` 20 px, `2xl` 26 px with line
  heights, emitted through `@theme` so Tailwind utilities and chart text
  measurement use the same sizes.
- `--radius` 0.625 rem → 0.375 rem (controls 6 px, `sm` 2 px, `md` 4 px);
  pill-shaped inputs and tabs are gone.
- Density now reaches controls: `--control-height` joins `--row-height`
  (28/36 px). Tables, term-sheet rows and the workbench bar consume both.
- Light sequential ramp replaced (the first three bins were indistinguishable
  from the page); dark ramp is a dark-surface ramp instead of the light copy;
  `diverging-neutral` is a distinct mid grey so par cells and the legend
  midpoint no longer vanish; light `--border` and `--accent` separated from
  the page; a `--hairline` token backs table and term-sheet rules.
- All `.finstack-*` workbench and market rules use spacing/type tokens instead
  of pixel literals.

## Shared idioms (theme CSS)

- Term sheet: text-at-rest inputs and selects (border and fill appear on
  hover/focus; invalid keeps the destructive border), content-sized inputs,
  muted regular-weight term column, hairline row rules, `--row-height` rows,
  horizontal radio groups, circled help glyph, changed-term dot from the form
  kit's `isDirty`, balanced two-column sections that never break inside a row
  or after a legend.
- One disclosure style, one pressed-toggle style, one top-left table caption
  style, and `.finstack-na` for missing data.
- Workbench: instrument id in the bar, underline tabs with `kbd` numerals
  (1/2 and 3/4/5), 28 px bar controls, state chip coloured by state
  (priced / pending / failed), stale results dimmed with a "Stale · reprice to
  update" badge whenever the completed request differs from the current
  candidate, "Edits pending validation" instead of "Waiting for valid inputs"
  after a priced result, status line with as-of, model, numeric mode,
  rounding, version and key hints, sticky bar in the stacked layout, bar
  labels hidden below 1180 px container width so the bar stays one row at 1024.
- Market rail: ghost rows with the 2 px primary bar and accent fill on the
  selected entry.

## Components

- `FinstackTable`: `--row-height` rows, sticky muted headers, visible focus
  ring, caption on top.
- `MeasuresGrid`: "Value" header, inline bucket bars.
- `MeasureValue`: inherits size, groups the integer digits of plain decimals
  for display (title keeps the raw text), quieter unit note.
- `MoneyValue` (prominent): smaller muted currency code.
- `StampBadge`: supplied `null` policy fields read "none"; absent keys stay
  "Unavailable"; compact chips carry short labels (mode / rounding / fx / v).
- `JsonViewer`: bounded card surface, no mid-token breaks, visible pressed
  toggle.
- `CashflowViewer`: unsupported-export error in body size rather than a
  full-width alert.
- `DecimalInput`: typed grouping separators are stripped so the displayed
  grouped text is accepted; suffix never wraps.
- `MoneyInput` / `TenorInput`: amount takes the row, unit hugs its content.
- Form kit: `SubmitButton` accepts `variant`; the workbench passes `outline`
  so "Price" is the only primary action. Instrument-form alerts use the error
  colour. Labels keep market acronyms upright (MC, FX, OIS, CDS, …).
- Heatmap: in-cell labels use a compact display form when the raw text is
  longer than seven characters; the tooltip keeps the exact value.
- Gallery harness: demo `<output>` counters are visually hidden (still
  accessible to tests).

## Validation

- `mise run ui-check` (generation drift, formatting, tokens + contrast incl.
  the tenant fixture, 150 registry closures, TypeScript, Vitest): 575 tests /
  43 files passed.
- `mise run ui-docs-build`: registry-only production export passed (27.6 s).
- Gallery e2e: baselines refreshed with `--update-snapshots` after visual
  review (195 PNGs), then `playwright test` without updates: 210/210 passed.
  The docs-host selected-control contrast test was already failing at the
  branch head because the cashflow JSON viewer moved behind the "Original
  JSON" disclosure; the test now opens that disclosure first.
- Headless captures of 50 review states (workbench light/dark/comfortable,
  market, calibrate, diagnostics, 1920/1440/1280/1024/900/390 and 18
  components in both themes): no horizontal overflow at any desktop width;
  the bar stays on one row at 1024 px.
- The pinned Playwright Chromium (headless shell 1208) had to be installed
  locally; earlier full-red e2e runs were that missing binary, not the code.

No Rust, Python, notebook or library suites ran; this is a UI-only slice.

## Follow-ups (verified findings not closed here)

- Calibration fit chart: per-panel figure height, repeated caption/sources,
  curve-order quote sorting, comparable y-domains.
- Scenario heatmap: square cells, legend par marker, CPR/CDR axis wording.
- Chart primitive: niced/padded domains so end marks are not clipped, grid
  colour from `--grid-opacity`, single-series legends.
- Market browser: instrument-referenced curve as the default selection,
  grouped curves by variant, search placeholder/empty state.
- Schema form: required/optional indicators, id-named array items, decimal
  recognition for money amounts, combobox popup width.
- Calibration panel: results in the results region, actions above the fold.
