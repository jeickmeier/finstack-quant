# Consolidation Plan B15: Migrate commodity, equity, exotics, and FX override providers

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F17 provider half-migration, wave 1
- **Risk tier:** Tier 4 — broad serde-sensitive runtime-storage migration
- **Estimated net LOC:** -150 to +250
- **Dependencies:** B11, B14
- **Branch:** `codex/simplicity-b15-focused-overrides-wave-one`
- **Commit subject:** `refactor(valuations): migrate focused overrides wave one`
- **Parallel / merge safety:** Domain-disjoint from B16 and may be developed in parallel. Conflicts with B11 in the same files, so dependency wave one lands first. Both override waves must precede B17.

## Scope

Replace full-bag `PricingOverrides` storage and hooks in every commodity, equity, exotics, and FX provider with only the focused override values that the instrument consumes. Preserve the existing wire shape through B14's Rust adapter and remove provider-level full-bag hooks in the same atomic wave.

### Exact fileset

At the B15 parent commit, record and migrate the complete output of:

```sh
rtk rg -l 'PricingOverrides|pricing_overrides(_mut)?' \
  finstack-quant/valuations/src/instruments/commodity \
  finstack-quant/valuations/src/instruments/equity \
  finstack-quant/valuations/src/instruments/exotics \
  finstack-quant/valuations/src/instruments/fx \
  finstack-quant/valuations/src/instruments/common_impl/traits/option_greeks.rs
```

The captured list is the PR's exact fileset. This domain wave is an explicit exception to the 1–5-file target and must merge as one green, complete migration. Internal domain commits may not be merged independently.

### Non-goals

- No rates, fixed-income, or credit-derivatives provider changes.
- No override-field rename, policy change, or builder deletion.
- No dependency-provider changes.

## Invariants

- Old JSON fixtures produce the same focused runtime values.
- Each instrument stores only override categories it actually consumes.
- Model, quote, metric, and scenario behavior remains unchanged.
- Scenario adjustments remain exactly-once.

## Implementation steps

1. Freeze the fileset in the PR description.
2. Migrate commodity fields/accessors and roundtrip fixtures.
3. Migrate equity/exotics fields/accessors, including option macro expansion.
4. Migrate FX fields/accessors and preserve pair/model behavior.
5. Remove full-bag provider methods from every selected file.
6. Re-run the search and resolve every selected production match before merge.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations commodity --lib
rtk cargo test -p finstack-quant-valuations equity --lib
rtk cargo test -p finstack-quant-valuations exotics --lib
rtk cargo test -p finstack-quant-valuations fx --lib
rtk mise run rust-check-schemas
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

The external JSON contract must remain stable. Binding constructors/builders should expose focused Rust-owned types rather than rebuild a catch-all bag. Update stubs, exports, parity contract, and runtime tests together if any bound signature changes.

## Rollback

Revert the complete wave before B17. Do not partially revert a merged domain because mixed full-bag/focused storage obscures ownership.

## Done criteria

- Every selected provider stores and exposes only focused overrides.
- No provider-level full-bag hook remains in these domains.
- Legacy JSON fixtures and pricing regressions pass.
- Full verification is green at the atomic PR head.

## Targeted re-audit acceptance

The fileset search returns no production `PricingOverrides` field or `pricing_overrides(_mut)` provider method in commodity, equity, exotics, or FX; references are limited to the centralized wire adapter if any.
