# Component registry changelog

## 0.2.0 — 2026-09-20

Release preparation for the v1 source registry. Public availability remains
pending merge and deployment from `master`; this is not an npm release.

### Included

- 150 installable items: shared theme and primitives, generated instrument and
  market contracts, worker/query hooks, individual financial forms and views,
  examples, and the composed pricing workbench.
- Exact JSON/decimal/integer transport, native validation and pricing through the
  existing WASM facade, and raw presentation where a published contract is absent.
- Shared table/chart selection, editable SVG and PNG figure export, and A4/Letter
  reports composed from the workbench's completed valuation.
- Generated registry/WASM versions and canonical schema IDs, dependency checks,
  independent installation verification, and CLI/MCP distribution checks.

### Breaking item changes

- UI primitives now compose unmodified shadcn Base UI `base-nova` controls. The
  prior custom control markup and styles are removed, including the offset radio
  indicator. Consumer selectors targeting those internals must be removed.
- The theme uses the stock shadcn tokens and domain layout rules. Apply app-specific
  layout in compositions and use supported theme variables; do not patch the
  installed stock controls.
- Re-add the items you use and their dependency closure together. There is no
  in-place update, migration layer or compatibility guarantee for copied sources.
  Save local edits before an overwrite and reconcile them in your compositions.
  Follow [the upgrade guide](../docs-site/content/docs/registry/upgrading.mdx).

### Scope and acceptance

This registry presents existing native contracts; it adds no financial model,
calculation or host API. Typed cashflow rows, inferred financial units, full-state
curve evaluation, off-grid plain volatility-surface evaluation, absent aggregates,
uncontracted result-detail types and quote-space calibration fits remain excluded.
Scenario-price documentation drift remains an upstream issue.

The complete final-source installation matrix and public CLI/MCP acceptance
remain open until merge and deployment from `master`. Earlier installation
results predate the final stock-control correction and are not a fresh complete
matrix.
