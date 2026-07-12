# Consolidation Plan: E01 — Collapse WASM scalar and typed-array statistic exports

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12  
**User priorities:** complete all five clusters through PR-sized, independently green slices  
**Plan date:** 2026-07-12  
**Status:** planned  
**Suggested branch:** `codex/simplify-e01-collapse-wasm-stat-array-exports`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Collapse WASM scalar and typed-array statistic exports

**Tier:** 3 (binding public surface)  
**Estimated net LOC:** −80 to −140  
**Addresses:** F7  
**Depends on:** None

**Files/filesets:**
- `finstack-quant-wasm/src/api/core/math.rs`
- `finstack-quant-wasm/exports/core.js`
- `finstack-quant-wasm/index.d.ts`
- `finstack-quant-wasm/tests/**/math*`
- `finstack-quant-py/parity_contract.toml`

**Scope:** Expose one canonical name for mean, variance, population variance, correlation, covariance, quantile, Kahan/Neumaier sums, and consecutive counts. Keep typed-array Rust kernels; normalize `number[] | Float64Array` in the JS facade.

**Non-goals:** Do not change formulas, NaN policy, length mismatch behavior, or floating-point operation order.

**Invariants touched:** Floating-point identity and JS/TS export parity.

## Implementation

1. Add equivalence tests for number arrays and typed arrays under the canonical names.
2. Rename/retain one wasm-bindgen kernel per calculation.
3. Move host-shape normalization into the facade.
4. Delete `*Array` twin exports/declarations/parity pins and stale tests.

## Tests to add or update

- WASM math unit/facade/type-declaration tests and exact-result parity cases.

## Verify

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
rtk mise run python-build -- --release
rtk mise run python-lint
rtk mise run python-test
rtk mise run wasm-build
rtk mise run wasm-lint
rtk mise run wasm-test
rtk uv run pytest finstack-quant-py/tests/parity -x
```

**Bindings/parity/serde impact:** WASM, JS facade, TypeScript, and parity contract touched; Python behavior unchanged.

**Parallel and merge safety:** Can run with most Rust-only plans; serialize with E02 and any parity-contract PR at merge.

**Rollback:** Atomic revert of Rust binding, facade, d.ts, and parity.

## Done when

- One public calculation name per statistic; no `*Array` export twins.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.
