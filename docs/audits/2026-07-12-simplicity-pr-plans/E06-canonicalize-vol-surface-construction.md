# Consolidation Plan: E06 — Give VolSurface one canonical construction model

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12  
**User priorities:** complete all five clusters through PR-sized, independently green slices  
**Plan date:** 2026-07-12  
**Status:** planned  
**Suggested branch:** `codex/simplify-e06-canonicalize-vol-surface-construction`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Give VolSurface one canonical construction model

**Tier:** 3/4 (public and numerical boundary)  
**Estimated net LOC:** −80 to −160  
**Addresses:** F28  
**Depends on:** C09 and D09

**Files/filesets:**
- `finstack-quant/core/src/market_data/surfaces/vol_surface.rs`
- `finstack-quant/core/src/market_data/surfaces/vol_cube.rs`
- `finstack-quant-py/src/bindings/core/market_data/curves/surfaces.rs`
- `finstack-quant-wasm/src/api/core/market_data.rs`
- `finstack-quant-py/parity_contract.toml`

**Scope:** Choose one checked row/grid input model as canonical, keep alternate shapes as private adapters only where internal cube code needs them, and expose one host options/input shape.

**Non-goals:** Do not change interpolation, arbitrage validation, grid flattening order, or checked/clamped lookup semantics.

**Invariants touched:** Interpolation, grid order, arbitrage checks, numerical outputs, parity.

## Implementation

1. Characterize rows/grid/builder equivalence and ragged/duplicate input errors.
2. Choose the canonical Rust constructor and route private internal adapters through it.
3. Collapse Python/WASM constructors onto one input model.
4. Delete redundant public constructors and update parity/declarations.

## Tests to add or update

- Core surface row/grid property tests; Python/WASM construction and lookup parity; arbitrage tests.

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

**Bindings/parity/serde impact:** Both hosts and parity touched.

**Parallel and merge safety:** Requires C09/D09; conflicts with FX-delta and arbitrage surface work.

**Rollback:** Atomic core+binding+parity revert.

## Done when

- One public validation/construction kernel and one host construction shape remain.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.
