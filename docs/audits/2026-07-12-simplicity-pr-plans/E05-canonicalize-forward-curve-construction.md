# Consolidation Plan: E05 — Give ForwardCurve one host construction path

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12  
**User priorities:** complete all five clusters through PR-sized, independently green slices  
**Plan date:** 2026-07-12  
**Status:** planned  
**Suggested branch:** `codex/simplify-e05-canonicalize-forward-curve-construction`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Give ForwardCurve one host construction path

**Tier:** 3 (binding public surface)  
**Estimated net LOC:** −40 to −90  
**Addresses:** F28  
**Depends on:** D10 recommended

**Files/filesets:**
- `finstack-quant-py/src/bindings/core/market_data/curves/forward.rs`
- `finstack-quant-wasm/src/api/core/market_data.rs`
- `finstack-quant-wasm/exports/core.js`
- `finstack-quant-wasm/index.d.ts`
- `finstack-quant-py/parity_contract.toml`

**Scope:** Keep one keyword/options-shaped host constructor that delegates to the Rust builder; remove Python `from_knots` and the duplicate WASM positional/`fromOptions` pair.

**Non-goals:** Do not change Rust curve validation, interpolation, projection grid, or fixed-tenor semantics.

**Invariants touched:** Interpolation, projection tenor/grid, serde and parity.

## Implementation

1. Lock constructor equivalence and error behavior in both hosts.
2. Select the final Python keyword and WASM options-object shapes.
3. Delete alternate constructors and facade adapters that add names rather than normalization.
4. Update stubs, declarations, exports, parity, and examples.

## Tests to add or update

- Python/WASM ForwardCurve construction and projection parity tests.

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

**Bindings/parity/serde impact:** Python + WASM + JS/TS + parity.

**Parallel and merge safety:** Avoid D10 and C09 because `market_data.rs`/context construction may overlap.

**Rollback:** Atomic binding-surface revert.

## Done when

- Exactly one documented host constructor per language, both mapping to the same Rust builder.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.
