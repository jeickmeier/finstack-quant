# Consolidation Plan: E04 — Remove the WASM-only FX JSON shell classes

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12  
**User priorities:** complete all five clusters through PR-sized, independently green slices  
**Plan date:** 2026-07-12  
**Status:** planned  
**Suggested branch:** `codex/simplify-e04-remove-wasm-fx-json-classes`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Remove the WASM-only FX JSON shell classes

**Tier:** 3 (binding public surface)  
**Estimated net LOC:** −250 to −450  
**Addresses:** F26  
**Depends on:** E03 recommended

**Files/filesets:**
- `finstack-quant-wasm/src/api/valuations/fx.rs`
- `finstack-quant-wasm/src/api/valuations/mod.rs`
- `finstack-quant-wasm/exports/valuations.js`
- `finstack-quant-wasm/index.d.ts`
- `finstack-quant-wasm/tests/wasm_valuations.rs`
- `finstack-quant-py/parity_contract.toml`

**Scope:** Delete the ten JSON-retaining FX instrument wrapper classes and route users through the canonical generic instrument JSON pricing/metric functions, matching Python's JSON-first surface.

**Non-goals:** Do not remove FX instruments, change their JSON schemas, or alter pricing/metric behavior.

**Invariants touched:** Instrument serde, pricing outputs, metric keys, error mapping.

## Implementation

1. Add facade tests proving each FX instrument JSON prices through the generic API.
2. Remove class macros, registrations, exports, declarations, and class-specific tests.
3. Update examples to use the generic valuation surface.
4. Remove WASM-only member pins and document the intentional common surface.

## Tests to add or update

- WASM generic pricing tests covering forwards, options, barriers, digitals, variance swaps, and spots.

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

**Bindings/parity/serde impact:** WASM/JS/TS/parity; Python remains the reference shape.

**Parallel and merge safety:** Serialize with E07 and any valuations export change.

**Rollback:** Revert all facade/declaration/parity changes together.

## Done when

- No WASM valuation class merely stores JSON; one generic host pricing path remains.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.
