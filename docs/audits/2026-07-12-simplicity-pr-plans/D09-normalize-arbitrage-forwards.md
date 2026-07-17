# Consolidation Plan: D09 — Normalize arbitrage forwards to one representation

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12
**User priorities:** complete all five clusters through PR-sized, independently green slices
**Plan date:** 2026-07-12
**Status:** planned
**Suggested branch:** `codex/simplify-d09-normalize-arbitrage-forwards`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Normalize arbitrage forwards to one representation

**Tier:** 4 (serde/parity-sensitive)
**Estimated net LOC:** −40 to −90
**Addresses:** F23
**Depends on:** D03 recommended

**Files/filesets:**
- `finstack-quant/core/src/market_data/arbitrage/mod.rs`
- `finstack-quant-py/src/bindings/core/market_data/arbitrage.rs`
- `finstack-quant-py/finstack_quant/core/market_data.pyi`
- `finstack-quant-py/parity_contract.toml`
- `finstack-quant/core/tests/market_data/arbitrage/**`

**Scope:** Represent forwards once as a checked scalar-or-per-expiry `ForwardPrices` input normalized at construction; remove scalar/vector runtime precedence branches.

**Non-goals:** Do not change arbitrage equations, local-vol density requirements, or tolerance defaults.

**Invariants touched:** Per-expiry forwards, local-vol density inputs, serde and parity.

## Implementation

1. Lock scalar broadcast, per-expiry, mismatch, and conflicting-input behavior.
2. Add one checked normalization type/function in core.
3. Make surface/grid adapters and Python delegate to it.
4. Delete `forward` plus `forward_prices` dual state and update serde compatibility.

## Tests to add or update

- Core grid/surface arbitrage tests; Python scalar/vector/error parity tests.

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

**Bindings/parity/serde impact:** Python and parity touched; verify whether WASM exposes the config before implementation.

**Parallel and merge safety:** Serialize with D03 and E06 because arbitrage uses `VolSurface` construction.

**Rollback:** Revert core, binding, stub, and parity changes together.

## Done when

- One normalized forward representation and one validation policy remain.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.
