# Stock shadcn composition correction

This supersedes the bespoke control styling described in `design-alignment.md`.
Registry fields and blocks now compose the unchanged official shadcn Base UI
`base-nova` components. Sixteen CLI-generated source files are recorded with
upstream provenance and SHA-256 hashes in `components/shadcn.json`; the registry
check rejects edits to those files and raw Base UI imports in domain components.
The application keeps its financial layouts, IBM Plex fonts and paired theme
colors using the documented shadcn theme variables and CSS foundation.

## Radio defect

The previous hand-styled radio placed its checked dot three CSS pixels above the
outer circle's center. Stock `RadioGroupItem` centers the dot with the upstream
absolute-positioned indicator. Browser regressions check every enabled choice
at desktop/mobile widths in light/dark themes, plus arrow-key selection. Measured
horizontal and vertical offsets are zero. No stock component CSS was changed.

## Additional review fixes

- Stock selects, calendars, popovers, comboboxes, tables, tabs, buttons and fields
  replace bespoke controls. Domain callbacks retain canonical string values.
- The virtualized identifier list uses the stock list's own scroll viewport;
  keyboard navigation reaches the last of 10,000 suggestions and free text stays
  editable. Icon triggers have explicit accessible names.
- Workbench controls wrap without overlapping Price; financial group headings
  precede their stock tables. Desktop and mobile layouts were visually reviewed.
- Read-only tables accept keyboard focus inside the stock scroll container.
  ArrowRight scrolls wide supplied data; no second overflow wrapper is needed.
  The initial full-gallery accessibility failure on cube tables is fixed by this
  composition change, without editing the stock Table.
- The application's light foreground supports stock inactive tabs at their
  upstream 60% opacity (4.70:1 contrast on muted backgrounds). Token validation
  checks that composed color, and browser tests verify both light and dark tabs.
- Chart adapters resolve CSS lengths to pixels before native figure layout.
  Parsing standard `rem` font tokens directly had made chart text nearly invisible
  in exports. The fix belongs to chart presentation and leaves stock typography
  unchanged; browser regressions cover rem/em/px/calc and invalid values.

## Verification

Validation is limited to the component registry and its actual installed consumers.
The isolated correction excludes pending PR-039 distribution metadata/workflows.
Generation, formatting, theme contrast, all 150 dependency closures and TypeScript
pass; all 563 tests across 42 files pass. A focused chart browser check verifies
browser-resolved dimensions and exported text sizes after the presentation fix.
After final table and theme fixes, 15 table/figure tests and 14 theme tests pass,
including the new inactive-tab contrast regression; isolated generation, types,
formatting and token checks were refreshed for those changes.
Installed primitive checks cover exact decimals, keyboard input, calendars, JSON
copy and accessibility in both themes/densities. The stock sources match the
actual CLI-installed docs consumer byte for byte.

Final gallery and export results are recorded in `shadcn-verification.json`.
No full Rust/Python/library, notebook or broad documentation suites ran.

[Desktop](shadcn-desktop.png) · [Mobile](shadcn-mobile.png) · [Radio](shadcn-radio.png).
