# Consolidation Plan B03: Delete disconnected common option parameter DTOs

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F1
- **Risk tier:** Tier 3 — Rust public API removal
- **Estimated net LOC:** -220 to -300
- **Dependencies:** None
- **Branch:** `codex/simplicity-b03-delete-option-dtos`
- **Commit subject:** `refactor(valuations): remove disconnected option DTOs`
- **Parallel / merge safety:** Safe beside B01, B02, B04, B06–B08. Conflicts with B05 in `market.rs` and `instruments/mod.rs`; land B03 before B05.

## Scope

Delete the disconnected common `EquityOptionParams`, `FxOptionParams`, and `CapFloorParams` DTOs and their re-exports. Keep the actual instrument-domain parameter types as the only runtime definitions. Replace the sole integration-test construction of the common FX DTO with the canonical FX instrument inputs.

### Exact files

- `finstack-quant/valuations/src/instruments/common_impl/parameters/market.rs`
- `finstack-quant/valuations/src/instruments/common_impl/parameters/mod.rs`
- `finstack-quant/valuations/src/instruments/mod.rs`
- `finstack-quant/valuations/tests/instruments/fx_option/test_instrument.rs`

### Non-goals

- No redesign of canonical equity-option, FX-option, or cap/floor parameters.
- No field renames or pricing changes in the real instrument types.
- No compatibility alias that preserves a second DTO surface.

## Invariants

- Instrument construction continues through domain-owned parameter types.
- JSON instrument schemas and binding constructors are unchanged.
- The deleted types have no production deserialization or pricing role.

## Implementation steps

1. Replace the integration-test use of common `FxOptionParams` with the canonical FX type or direct builder.
2. Delete all three common DTO definitions and their impl blocks.
3. Remove module and root re-exports.
4. Remove imports and tests that existed only for the disconnected DTOs.
5. Confirm no compatibility alias or duplicate replacement is introduced.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations --test instruments fx_option
rtk cargo check -p finstack-quant-valuations
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

No intended binding or serde change because these DTOs are disconnected from the binding and instrument JSON surfaces. Treat any parity-contract or stub diff as evidence of hidden use and stop the PR until reconciled.

## Rollback

Revert the PR. There is no persisted-data migration because the deleted DTOs are not canonical wire types.

## Done criteria

- The three common DTO names and re-exports are absent.
- Canonical domain types remain unchanged.
- No new alias or wrapper recreates the deleted surface.
- Full verification is green.

## Targeted re-audit acceptance

```sh
rtk rg -n 'struct (EquityOptionParams|FxOptionParams|CapFloorParams)|\b(EquityOptionParams|FxOptionParams|CapFloorParams)\b' finstack-quant/valuations
```

The command returns no common DTO definitions or uses; domain-specific canonical names may be explicitly reviewed if similarly named.
