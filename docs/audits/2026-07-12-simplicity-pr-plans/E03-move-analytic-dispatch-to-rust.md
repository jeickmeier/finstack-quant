# Consolidation Plan: E03 — Move analytic option dispatch and validation into Rust

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12  
**User priorities:** complete all five clusters through PR-sized, independently green slices  
**Plan date:** 2026-07-12  
**Status:** planned  
**Suggested branch:** `codex/simplify-e03-move-analytic-dispatch-to-rust`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Move analytic option dispatch and validation into Rust

**Tier:** 3/4 (binding logic and numerical boundary)  
**Estimated net LOC:** −120 to −220  
**Addresses:** F26  
**Depends on:** B08 and C07

**Files/filesets:**
- `finstack-quant/valuations/src/models/closed_form/api.rs`
- `finstack-quant/valuations/src/models/closed_form/mod.rs`
- `finstack-quant-py/src/bindings/valuations/analytic.rs`
- `finstack-quant-wasm/src/api/valuations/analytic.rs`
- `finstack-quant-py/parity_contract.toml`

**Scope:** Add checked Rust-canonical dispatch for barrier direction/knock, Asian averaging, lookback strike style, call/put, quanto, and theta-day validation; bindings only convert host values and map errors.

**Non-goals:** Do not change formulas, default labels, Greek units, theta denominator defaults, or accepted canonical strings.

**Invariants touched:** Option formula selection, defaults, errors, Greek scaling, numerical outputs.

## Implementation

1. Define small Rust enums/request types reusing B08 barrier and existing option types.
2. Move every binding `match` and non-trivial validation into checked Rust functions.
3. Reduce Python/WASM wrappers to convert → call → map/serialize.
4. Add cross-host success/error/default parity tests.
5. Update parity contract and stubs/declarations if request shapes become public.

## Tests to add or update

- Rust checked-facade tests; Python/WASM analytic behavioral parity; existing option goldens.

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

**Parallel and merge safety:** Requires B08/C07 merged; serialize with C06-C07 and other analytic binding work.

**Rollback:** Atomic Rust+binding+parity revert.

## Done when

- No arithmetic or financial dispatch `match` remains in either analytic binding.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.
