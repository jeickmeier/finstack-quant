# Component registry design correction

Historical evidence for commit `0fa56f72a`. The subsequent
[stock shadcn composition correction](shadcn-composition.md) supersedes its
bespoke control styling and records the current verification.

The implementation drifted from the agreed component-registry mockups. This
correction changes the shared registry components and their docs host, rather
than adding a separate demo renderer.

## Reviewed scope

- Restore the desktop toolbar and 52/48 input/result composition, internal
  scrolling, large persistent valuation summary and tabbed result views.
- Use the existing IBM Plex fonts, restrained teal palette, fine decorative
  rules and compact editable term sheets. Separate decorative borders from
  meaningful control boundaries in the contrast check.
- Replace boxed schema nesting and always-visible help/actions with shared
  inline fields and disclosures. Keep supplied values and validation errors
  accessible; a stable keyed field list preserves focus through default-value
  transitions. Canonical schema order remains the source of field ordering.
- Put market navigation beside the selected stored view, with compact charts,
  linked node tables and an active mobile category selector.
- Format JSON losslessly for reading. Original mode, copy, download and compact
  print retain the supplied text. Wide numbers, decimal tokens, duplicate-key
  failures and exact native output are covered by focused regressions.
- Keep docs prose styles out of registry components. Rename the docs-owned
  accent variable so it cannot override the registry's paired color tokens.

Independent visual evaluation passed on the third review of rendered bond,
equity-option and structured-credit workbenches, market/surface views, docs,
light/dark modes and desktop/tablet/mobile layouts. The remaining docs contrast
finding was fixed and verified by a computed-color browser regression.

Source review additionally caught and fixed field remounting during typing and
print-source expansion. Financial calculations, canonical wire contracts and
native worker ownership are unchanged. V1 cashflows remain native JSON;
unsupported native cashflow exports retain their error and successful valuation.
This does not claim the mockups' deferred typed cashflow or analytics features.

## Validation

- Working-tree registry gate: generation, formatting, contrast, all 150 dependency
  closures and TypeScript passed; 564 tests across 43 files passed. This working
  tree also contains pending PR-039 metadata checks, which are excluded from the
  visual correction commit.
- All 204 gallery, accessibility, keyboard and layout checks passed while
  accepting intentional screenshots after rendered review. The final run
  without screenshot updates passed all 204 checks in 228.73 seconds.
- Installed schema form: actual worker validation, canonical output, retained
  edits, decimal errors, invalid-submit focus and accessibility passed. The
  focus assertion waits for asynchronous submit completion.
- Installed workbench: edited pricing matches the native facade, results persist
  through tab changes, controls retain edits and stored-market views pass.
- Publication checks passed after removing the hidden market-rail grid column
  from report layout and preparing figures at a stable physical width. Actual
  SVG transform scales stay at 1x. All 14 PDF pages were visually inspected:
  bond A4/Letter two pages each, mixed-currency A4/Letter five pages each; no
  clipping, blank pages or oversized chart text. Exact native data, embedded
  fonts, vector output and page bounds pass. See `design-publishing.json`.
- Isolated visual-only generation, dependency closure, formatting and TypeScript
  checks pass; all **562 tests / 42 files** pass in 87.90 seconds. This snapshot
  excludes pending PR-039 metadata and workflow changes. A native-worker form
  assertion initially exceeded Testing Library's one-second default; its explicit
  debounced-validation wait is now bounded at three seconds. Native-value
  assertions and per-test deadlines remain intact.

No full Rust/Python/library, notebook or broad documentation suites ran. Pending
registry metadata/distribution work is deliberately excluded from this change.

## Rendered evidence

[Desktop](design-desktop.png) · [Mobile](design-mobile.png) · [Market view](design-market.png).
