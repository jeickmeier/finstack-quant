---
trigger: model_decision
description: Component registry (finstack-quant-ui) and its docs-site gallery
globs: finstack-quant-ui/**,docs-site/src/components/registry-gallery/**
---

# Component Registry Standards (finstack-quant-ui)

Read `finstack-quant-ui/README.md`, section "Layout and extension", before you
add or move an item: directory layout, import alias table, add-an-item
checklist with the gate for each step, generator order, and each gate's
prerequisites and duration.

- Rust owns logic. Components present supplied values and native results, with
  no financial rules, unit tables or arithmetic (README "Validation boundary").
  `labelFor` in `src/format/label.mjs` is the shared label function for schema
  field, metric and registry names; enum and group values may render as raw
  words. Add no curated label maps for those names. A composing editor may
  present the choices its own controls offer, as `instrument-form` (desk-quote
  and term-sheet tables), the statements workbench (forecast and rollup method
  lists) and `valuation-details` (raw-detail labels) do.
- Never edit the stock shadcn files under `finstack-quant-ui/components/ui/`;
  compose them. Domain components never import `@base-ui/*` (`check-registry`).
- Colours, fonts, spacing and density come only from the theme tokens in
  `registry/theme/finstack-theme/tokens.json`; `check-tokens` rejects colour
  and font literals.
- Core items know no domain: a composing editor injects domain presentation
  (for example `SchemaForm`'s `disclosure`, `sheet` and `promotedField`).
- Item classification comes from data: the worker role and runtime packages
  from the registry, the install fixture from an existing
  `tests/install/fixture/<item>.tsx`, and the wrapper probe from a
  `presentation` prop in the gallery example. Never add item-name sets to
  scripts, harnesses or the gallery.
- Every Vitest timeout or hang allowance goes through `budget()` from
  `tests/shared/budget.ts`, never a bare millisecond value; a fixed wait that
  outlasts a component timer stays a literal tied to that timer.
- Generated files (`src/generated/`, registry `categories`/`meta`,
  `registry/theme/finstack-theme/theme.css`, `registry/theme/registry.json`,
  the gallery `inventory.json`, `data.json` and `imports.ts`) are regenerated
  with `npm --prefix finstack-quant-ui run generate` and never edited by hand.
  Never run `gen-registry` or `gen-theme` alone: they drop the `categories`
  and `meta` that only `gen-metadata` restores.
- When a change moves a native fixture, run `mise run ui-fixtures` and commit
  the regenerated fixtures in the same commit, with the reason in the message.
- Iterate with `mise run ui-test -- <file>` and finish with `mise run ui-check`.
  Run `mise run ui-e2e` for a visual change, `mise run ui-install` for a change
  to an install closure, and `mise run ui-harness` after changing figure
  export, `FinstackChart` interactions or `useLinkedSelection`, a `registry:ui`
  primitive, the heatmap preset or the measures-grid layout; all three are
  local, not CI.
