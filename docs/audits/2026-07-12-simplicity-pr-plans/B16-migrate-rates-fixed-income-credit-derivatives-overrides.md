# Consolidation Plan B16: Migrate rates, fixed-income, and credit-derivatives override providers

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F17 provider half-migration, wave 2
- **Risk tier:** Tier 4 — broad serde-sensitive runtime-storage migration
- **Estimated net LOC:** -180 to +250
- **Dependencies:** B02, B07, B12, B14
- **Branch:** `codex/simplicity-b16-focused-overrides-wave-two`
- **Commit subject:** `refactor(valuations): migrate focused overrides wave two`
- **Parallel / merge safety:** Domain-disjoint from B15 and may be developed in parallel. Conflicts with B12 and B18, so dependency wave two lands first and swaption normalization lands later. Both override waves must precede B17.

## Scope

Replace full-bag `PricingOverrides` storage and hooks in every rates, fixed-income, and credit-derivatives provider with only the focused values the instrument consumes. Preserve legacy wire input through B14's Rust adapter and remove provider-level full-bag hooks in the same atomic wave.

### Exact fileset

At the B16 parent commit, record and migrate the complete output of:

```sh
rtk rg -l 'PricingOverrides|pricing_overrides(_mut)?' \
  finstack-quant/valuations/src/instruments/rates \
  finstack-quant/valuations/src/instruments/fixed_income \
  finstack-quant/valuations/src/instruments/credit_derivatives
```

The captured list is the PR's exact fileset. This domain wave is an explicit exception to the 1–5-file target and must merge as one green, complete migration; internal vertical commits may not be merged separately.

### Non-goals

- No commodity, equity, exotics, or FX provider changes.
- No override-field rename, pricing-policy change, or builder deletion.
- No dependency-provider changes.

## Invariants

- Old JSON fixtures produce identical focused runtime values.
- Rate model configuration, fixed-income quote/model inputs, credit assumptions, metric bumps, and scenarios retain their behavior.
- Each instrument stores only the override categories it consumes.
- Scenario and price shocks remain exactly-once.

## Implementation steps

1. Freeze the fileset in the PR description.
2. Migrate rates providers by complete instrument vertical.
3. Migrate fixed-income providers, preserving quote/model semantics.
4. Migrate credit-derivatives providers, preserving hazard/recovery/model behavior.
5. Remove full-bag provider methods from every selected file.
6. Re-run the search and resolve every selected production match before merge.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations rates --lib
rtk cargo test -p finstack-quant-valuations fixed_income --lib
rtk cargo test -p finstack-quant-valuations credit_derivatives --lib
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

The external JSON contract must remain stable. Binding surfaces should follow focused canonical Rust types; update Python/WASM exports, stubs, parity contract, and runtime tests in the same vertical if a bound signature changes.

## Rollback

Revert the complete wave before B17. Do not partially revert a merged vertical migration.

## Done criteria

- Every selected provider stores and exposes only focused overrides.
- No provider-level full-bag hook remains in these domains.
- Legacy fixtures and pricing regressions pass.
- Full verification is green at the atomic PR head.

## Targeted re-audit acceptance

The fileset search returns no production `PricingOverrides` field or `pricing_overrides(_mut)` provider method in rates, fixed-income, or credit-derivatives; references are limited to the centralized wire adapter if any.
