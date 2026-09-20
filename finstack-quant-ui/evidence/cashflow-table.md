# Cashflow table

CashflowViewer now presents native cashflow rows in a stock shadcn Table. The
workbench tab is named Cashflows. Original JSON is collapsed for exact
copy/download and omitted from print.

## Source contract and review

The presentation adapter follows `InstrumentCashflowEnvelope` and `CashflowRow`
in `finstack-quant/valuations/src/instruments/common_impl/cashflow_export.rs`.
Existing native fixtures pin that source and compare current exports byte for
byte. The worker continues to return the original native JSON string; this is
not a new typed WASM API.

- Preserve native order, payment dates, kinds and signed amounts, including
  settled rows. Amounts use each row's currency; PV and total PV use the envelope
  reporting currency. Rust performs FX conversion and supplies the total and
  reconciliation flag. The UI does not sum, price or convert cashflows.
- Parse numeric tokens with the existing lossless-json dependency and format
  monetary digits with the existing formatter. No Number conversion, inferred
  currency rounding scale or borrowed valuation rounding stamp. This preserves
  serialized digits, not precision already lost in native f64 calculations.
- Optional diagnostics remain absent rather than zero; malformed exports show
  an explicit table error while retaining original source access.
- Stock shadcn sources are unchanged. Component registry dependencies and
  provenance now describe the table adapter instead of the obsolete raw-only
  presentation restriction.

## Validation

- 32 focused cashflow/workbench/provenance tests passed. After print composition
  changes, all 11 cashflow tests passed again, including exact wide numeric
  tokens, mixed currencies, zero/missing values, empty schedules, native errors,
  exact source copy and optional diagnostic footer alignment.
- Focused cashflow browser checks passed for bond, FX swap and cross-currency
  swap at both densities, native error behavior and accessibility. No axe
  violations. The final publishing check additionally exercised final print DOM.
- The gallery audit exposed an unfocusable scroll area. The stock Table now
  accepts keyboard focus through its public props. ArrowRight scrolls the actual
  container at 390px, with zero axe violations at 390×844 and 1440×1700 in the
  focused browser check.
- All eight affected gallery snapshots passed after visual review and baseline
  refresh, covering cashflow-viewer and pricing-workbench in both themes and
  densities. The final registry-only production export passed in 24.09 seconds.
- Typecheck, theme/token checks, all 150 registry import closures / stock-source
  integrity checks, generated provenance/metadata/gallery checks and scoped
  formatting passed.
- Independent visual review: SCREEN PASS across desktop light/dark, tablet and
  mobile, source disclosure and horizontal scrolling. The evaluator inspected
  fresh parent-produced screenshots because its browser connection was
  unavailable. At 375px, document width remained 375px while the table scrolled
  within its 341px container.
- Fresh one-item CLI install and production consumer build passed. All four
  A4/Letter PDF reports passed raw instrument/market JSON, fonts, vector graphics,
  page bounds, measures and complete cashflow-word checks. All 4 bond and 44
  cross-currency rows were retained. Bond reports are 3 pages and cross-currency
  reports 7 pages in both paper sizes; sampled cashflow pages and final totals
  were visually reviewed.
- Print uses four columns with stacked pricing factors to keep numeric tokens
  whole on portrait paper. The retained seven-column broken PDF fails the new
  word check (160 missing whole words), while the revised reports pass.

Desktop/mobile screenshots and browser/PDF reports accompany this note.
No full-library, native rebuild, Python, notebook or broad documentation suite
ran. GitHub Pages remains restricted to master.
