# Consolidation Plan: E02 — Collapse nested and flat WASM matrix APIs

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12  
**User priorities:** complete all five clusters through PR-sized, independently green slices  
**Plan date:** 2026-07-12  
**Status:** planned  
**Suggested branch:** `codex/simplify-e02-collapse-wasm-matrix-exports`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Collapse nested and flat WASM matrix APIs

**Tier:** 3 (binding public surface)  
**Estimated net LOC:** −50 to −100  
**Addresses:** F7  
**Depends on:** E01 recommended

**Files/filesets:**
- `finstack-quant-wasm/src/api/core/math.rs`
- `finstack-quant-wasm/exports/core.js`
- `finstack-quant-wasm/index.d.ts`
- `finstack-quant-wasm/tests/**/math*`
- `finstack-quant-py/parity_contract.toml`

**Scope:** Keep one flat row-major Rust primitive for Cholesky decomposition/solve/correlation validation; adapt nested matrices in the facade under the canonical unsuffixed names.

**Non-goals:** Do not change positive-definite checks, row-major order, dimensions, or numerical tolerances.

**Invariants touched:** Matrix layout, error policy, numerical outputs, parity.

## Implementation

1. Lock nested/flat equivalence and malformed-dimension errors.
2. Select flat wasm-bindgen kernels and facade normalization.
3. Remove `Flat`-suffixed public twins and redundant serde conversion.
4. Update declarations, exports, parity pins, and examples.

## Tests to add or update

- WASM matrix unit/facade/type tests; numerical equivalence to core.

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

**Bindings/parity/serde impact:** WASM + JS/TS + parity.

**Parallel and merge safety:** Implement after or beside E01 on a separate branch, but merge serially because files overlap.

**Rollback:** Atomic binding-surface revert.

## Done when

- One Cholesky/validation name per capability; exactly one Rust calculation kernel.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.
