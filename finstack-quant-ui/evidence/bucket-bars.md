# Results bucket bars

MeasuresGrid composes small signed horizontal bars beneath the unchanged raw
numbers in qualified bucketed DV01 and CS01 rows. Negative values extend left
of a centered zero axis; positive values extend right. Zero retains the axis
without a fill. Stock shadcn components are unchanged.

The native three-segment key contract determines eligible rows. Identifiers
and bucket labels remain opaque, including native delimiter escapes and
duplicate-pillar suffixes. Scalar totals and parallel sensitivities are not
graphed. Each risk family, exact identifier, supplied unit/source and valuation
column has an independent scale. The UI normalizes display widths only; it
does not calculate sensitivities, aggregate values, infer units or convert FX.
Missing/nonfinite values remain ungraphed. Decorative bars are aria-hidden;
the table retains full keys and exact numeric text.

## Focused validation

- Nine results tests pass, including native fixture parity, exact values,
  signed/zero buckets, malformed/scalar exclusions, independent scales,
  escaped identifiers, unit/source separation and numeric extremes.
- Fresh installed results consumer passes production build, browser sign and
  width assertions, four light/dark-density accessibility checks and mobile
  accessibility at 390px. No WASM requests or new chart dependencies.
- Mobile review found an automatic grid minimum expanding the page. Setting
  the group section to min-width zero preserves stock table scrolling: document
  and viewport are both 390px, with 358px table containers holding wider content.
- Independent source and visual review passed after the containment fix.
  The reviewer inspected fresh supplied screenshots; no cross-provider review
  is claimed. Parent inspected native desktop/mobile and dark gallery views.
- All eight affected results/workbench gallery snapshots passed in both themes
  and densities after baseline review. Typecheck, token/stock integrity,
  150 registry closures, metadata/gallery generation and formatting passed.
- Registry-only production gallery export passed. Fresh one-item workbench
  installation and production build passed. All four A4/Letter reports passed
  exact text, numeric words, cashflow cells, fonts, vector and page-bound checks.
  Bond reports retain three pages; cross-currency reports retain seven.
  The bond results print page was visually checked with bars and full numbers.

The desktop image includes explicitly synthetic comparison buckets to exercise
both signs and CS01. The native bond fixture is unchanged (one nonzero DV01
bucket and ten zero buckets). The mobile image shows the native result.
Browser and PDF verification reports accompany this note.

No full-library, Rust, Python, notebook or broad documentation suite ran.
GitHub Pages remains restricted to master.
