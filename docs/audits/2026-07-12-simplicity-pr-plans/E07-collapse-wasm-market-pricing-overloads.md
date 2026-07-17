# Consolidation Plan: E07 — Collapse WASM market pricing overloads

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12
**User priorities:** complete all five clusters through PR-sized, independently green slices
**Plan date:** 2026-07-12
**Status:** planned
**Suggested branch:** `codex/simplify-e07-collapse-wasm-market-pricing-overloads`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Collapse WASM market pricing overloads

**Tier:** 3 (binding public surface)
**Estimated net LOC:** −60 to −120
**Addresses:** F28
**Depends on:** B01 and E04

**Files/filesets:**
- `finstack-quant-wasm/src/api/valuations/pricing.rs`
- `finstack-quant-wasm/exports/valuations.js`
- `finstack-quant-wasm/index.d.ts`
- `finstack-quant-wasm/tests/wasm_valuations.rs`
- `finstack-quant-py/parity_contract.toml`

**Scope:** Keep typed parsed-`Market` pricing/metrics as the wasm-bindgen API; move JSON-string-to-handle adaptation into the JS facade and remove duplicate Rust exports.

**Non-goals:** Do not change market JSON parsing, pricing lifecycle, result serialization, or generic instrument JSON format.

**Invariants touched:** Validation, as-of resolution, scenario exactly once, result metadata, serde.

## Implementation

1. Lock string-market and parsed-market equivalence for price and metrics.
2. Select typed `Market` methods as the low-level canonical surface.
3. Move string convenience into facade-only adapters under the same public names.
4. Delete duplicate wasm exports/declarations/parity pins and update tests.

## Tests to add or update

- WASM price/metrics tests with both facade input shapes; lifecycle validation tests from B01.

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

**Bindings/parity/serde impact:** WASM/JS/TS/parity.

**Parallel and merge safety:** Requires B01/E04; serialize with any pricing.rs or valuations export changes.

**Rollback:** Atomic binding/facade/parity revert.

## Done when

- One Rust pricing implementation per operation; string adaptation exists only in JS.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.
