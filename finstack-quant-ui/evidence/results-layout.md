# Results layout alignment

The results hierarchy now follows the agreed workbench mockup: a prominent
present value with a secondary currency label, inline date/model/instrument
context, compact policy badges and grouped measure headers. Workbench result
headers align, the detail view uses the stock Tabs line variant, and the summary
body scrolls independently on desktop. Stock shadcn sources remain unchanged.

The first pricing row moved from y=678.48 to y=605.42 at 1280×720, with the same
results region dimensions. At 390px, long keys wrap while raw numeric values
remain readable. Desktop and mobile captures are retained alongside this note.

Exact decimal formatting, complete metric keys, raw values and independent
comparison context are preserved. The optional model labels come from caller
context; the workbench uses the completed request. No units, financial totals,
typed cashflows or comparison calculations are inferred.

## Focused review and validation

- Source review: no financial calculations, dependency changes or stock shadcn
  edits. Missing metadata remains available in full disclosure and print.
- Independent visual review: screen PASS. The initial review used live desktop,
  tablet and mobile inspection; the second review checked fresh final desktop
  and mobile captures plus all 12 updated gallery baselines. The second review
  used supplied captures because its browser connection was unavailable.
- Results/workbench tests: 23 passed, including exact amounts larger than 2^53,
  independent comparison models and retained completed pricing context.
- Focused layout/radio browser checks: 11 passed.
- Affected gallery snapshots: 12 passed after reviewing updated baselines,
  covering valuation summary, measures grid and workbench in both themes and
  densities.
- Registry-only production export and fresh one-item CLI install, TypeScript
  check and production consumer build passed.
- All four A4/Letter publishing reports passed exact raw JSON, font, vector,
  page-bound and whole-number checks; the results and cashflow pages were
  visually reviewed. Bond reports are 2 A4 / 3 Letter pages; mixed-currency
  reports remain 5 pages each. See `results-pdf-check.json`.
- A forced key-column width found during review squeezed printed values into
  one-character lines. Removing that width restored natural column sizing.
  The new PDF regression check rejects the retained broken report and passes
  the corrected reports. Empty measure sets remain valid for the mixed-currency
  fixture; the bond fixture must contain measures.
- Scoped formatting and diff whitespace checks passed.
- Theme generation and all 150 registry import closures / stock-source integrity
  checks passed.

No full-library, Rust, Python, notebook or broad documentation suite ran.
GitHub Pages remains restricted to master. Native artifacts were reused.
